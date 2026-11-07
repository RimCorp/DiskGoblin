//! Dependency-free localhost HTTP bridge for the visual UI.
use crate::args::Args;
use diskgoblin_core::{
    scan_directory_with_progress, CancelToken, ProgressSink, ScanOptions, ScanProgress, SizeMetric,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    io::{self, BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    path::{Component, Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

#[derive(Deserialize, Serialize)]
struct ScanOptionsDto {
    root_path: PathBuf,
    size_metric: SizeMetric,
    one_file_system: bool,
    max_depth: Option<usize>,
    top_n: usize,
    min_size_bytes: u64,
    threads: usize,
}
impl From<ScanOptionsDto> for ScanOptions {
    fn from(dto: ScanOptionsDto) -> Self {
        Self {
            root_path: dto.root_path,
            size_metric: dto.size_metric,
            one_file_system: dto.one_file_system,
            max_depth: dto.max_depth,
            top_n: dto.top_n,
            min_size_bytes: dto.min_size_bytes,
            threads: dto.threads,
        }
    }
}
#[derive(Default)]
struct ActiveScan {
    generation: u64,
    stopping: bool,
    token: CancelToken,
    progress: Option<ScanProgress>,
}
struct Sink {
    state: Arc<Mutex<ActiveScan>>,
    generation: u64,
}
impl ProgressSink for Sink {
    fn on_progress(&self, progress: ScanProgress) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.generation == self.generation {
            state.progress = Some(progress);
        }
    }
}

pub fn run_ui_server(args: &Args, cancel: CancelToken) -> anyhow::Result<()> {
    let listener = match TcpListener::bind(("127.0.0.1", args.ui_port)) {
        Err(e) if args.ui_port == 7878 && e.kind() == io::ErrorKind::AddrInUse => {
            TcpListener::bind(("127.0.0.1", 0))?
        }
        result => result?,
    };
    let url = format!("http://127.0.0.1:{}", listener.local_addr()?.port());
    eprintln!("DiskGoblin UI listening on {url}");
    if !args.no_open {
        let mut command = if cfg!(target_os = "macos") {
            Command::new("open")
        } else if cfg!(target_os = "windows") {
            let mut c = Command::new("cmd");
            c.args(["/C", "start", ""]);
            c
        } else {
            Command::new("xdg-open")
        };
        let _ = command.arg(url).spawn();
    }
    let config = json!({"root_path": args.path, "size_metric": args.scan_options().size_metric, "one_file_system": args.one_file_system, "max_depth": null, "top_n": args.top, "min_size_bytes": args.min_size, "threads": args.threads, "auto_scan": true});
    serve(listener, cancel, config, resolve_dist())?;
    Ok(())
}
fn resolve_dist() -> PathBuf {
    let candidates = [
        std::env::var_os("DISK_GOBLIN_UI_DIST").map(PathBuf::from),
        Some(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/diskgoblin-desktop/dist")),
        Some(PathBuf::from("apps/diskgoblin-desktop/dist")),
    ];
    candidates
        .into_iter()
        .flatten()
        .find(|p| p.join("index.html").is_file())
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/diskgoblin-desktop/dist")
        })
}
fn serve(
    listener: TcpListener,
    cancel: CancelToken,
    config: serde_json::Value,
    dist: PathBuf,
) -> io::Result<()> {
    listener.set_nonblocking(true)?;
    let state = Arc::new(Mutex::new(ActiveScan::default()));
    let mut workers = Vec::new();
    while !cancel.is_cancelled() {
        match listener.accept() {
            Ok((stream, _)) => {
                let (state, config, dist) = (state.clone(), config.clone(), dist.clone());
                workers.push(thread::spawn(move || {
                    let _ = handle(stream, state, &config, &dist);
                }));
                let mut index = 0;
                while index < workers.len() {
                    if workers[index].is_finished() {
                        let _ = workers.swap_remove(index).join();
                    } else {
                        index += 1;
                    }
                }
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(10))
            }
            Err(e) => return Err(e),
        }
    }
    state
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .token
        .cancel();
    for worker in workers {
        let _ = worker.join();
    }
    Ok(())
}
fn respond(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &[u8],
) -> io::Result<()> {
    write!(stream, "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\nX-Content-Type-Options: nosniff\r\n\r\n", body.len())?;
    stream.write_all(body)
}
fn handle(
    mut stream: TcpStream,
    state: Arc<Mutex<ActiveScan>>,
    config: &serde_json::Value,
    dist: &Path,
) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or("").to_owned();
    let path = parts
        .next()
        .unwrap_or("")
        .split('?')
        .next()
        .unwrap_or("")
        .to_owned();
    let mut length = 0usize;
    let mut header_bytes = 0;
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            return Ok(());
        }
        header_bytes += line.len();
        if header_bytes > 16384 {
            return respond(
                &mut stream,
                "400 Bad Request",
                "application/json",
                br#"{"error":"headers too large"}"#,
            );
        }
        if line == "\r\n" {
            break;
        }
        if let Some((key, value)) = line.split_once(':') {
            if key.eq_ignore_ascii_case("host")
                && !matches!(
                    value.trim().split(':').next(),
                    Some("127.0.0.1" | "localhost")
                )
            {
                return respond(&mut stream, "403 Forbidden", "application/json", b"{}");
            }
            if key.eq_ignore_ascii_case("content-length") {
                length = value.trim().parse().unwrap_or(usize::MAX);
            }
            // Only same-origin JSON requests may invoke filesystem operations.
            if key.eq_ignore_ascii_case("sec-fetch-site") && value.trim() == "cross-site" {
                return respond(&mut stream, "403 Forbidden", "application/json", b"{}");
            }
        }
    }
    if length > 1024 * 1024 {
        return respond(
            &mut stream,
            "400 Bad Request",
            "application/json",
            br#"{"error":"body too large"}"#,
        );
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    let mut status = "200 OK";
    let value = match (method.as_str(), path.as_str()) {
        ("GET", "/api/config") => config.clone(),
        ("GET", "/api/progress") => json!(state.lock().unwrap_or_else(|e| e.into_inner()).progress),
        ("POST", "/api/cancel") => {
            state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .token
                .cancel();
            json!({"cancelled": true})
        }
        ("POST", "/api/pick-directory") => {
            let path = if cfg!(target_os = "macos") {
                Command::new("osascript").args(["-e", "POSIX path of (choose folder with prompt \"Select folder to scan with DiskGoblin\")"]).output().ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
            } else {
                None
            };
            json!({"path": path})
        }
        ("POST", "/api/scan") => match serde_json::from_slice::<ScanOptionsDto>(&body) {
            Ok(dto) => {
                let (token, generation) = {
                    let mut active = state.lock().unwrap_or_else(|e| e.into_inner());
                    active.token.cancel();
                    active.token = CancelToken::new();
                    if active.stopping {
                        active.token.cancel();
                    }
                    active.progress = None;
                    active.generation = active.generation.wrapping_add(1);
                    (active.token.clone(), active.generation)
                };
                match scan_directory_with_progress(&dto.into(), &token, &Sink { state, generation })
                {
                    Ok(report) => json!(report),
                    Err(e) => {
                        status = "400 Bad Request";
                        json!({"error": e.to_string()})
                    }
                }
            }
            Err(e) => {
                status = "400 Bad Request";
                json!({"error": e.to_string()})
            }
        },
        ("GET", _) if !path.starts_with("/api/") => return serve_static(&mut stream, &path, dist),
        _ => {
            status = "404 Not Found";
            json!({"error": "not found"})
        }
    };
    respond(
        &mut stream,
        status,
        "application/json",
        &serde_json::to_vec(&value)?,
    )
}
fn serve_static(stream: &mut TcpStream, request: &str, dist: &Path) -> io::Result<()> {
    let relative = Path::new(request.trim_start_matches('/'));
    if relative
        .components()
        .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
    {
        return respond(stream, "403 Forbidden", "text/plain", b"Forbidden");
    }
    let file = dist.join(if request == "/" {
        Path::new("index.html")
    } else {
        relative
    });
    // Canonical containment also rejects symlinks escaping the asset directory.
    if let (Ok(base), Ok(target)) = (dist.canonicalize(), file.canonicalize()) {
        if !target.starts_with(base) {
            return respond(stream, "403 Forbidden", "text/plain", b"Forbidden");
        }
    }
    match std::fs::read(&file) {
        Ok(body) => {
            let mime = match file.extension().and_then(|e| e.to_str()) {
                Some("html") => "text/html; charset=utf-8",
                Some("js") => "application/javascript; charset=utf-8",
                Some("css") => "text/css; charset=utf-8",
                Some("svg") => "image/svg+xml",
                Some("json") => "application/json",
                _ => "application/octet-stream",
            };
            respond(stream, "200 OK", mime, &body)
        }
        Err(_) if request == "/" => respond(
            stream,
            "200 OK",
            "text/html; charset=utf-8",
            FALLBACK.as_bytes(),
        ),
        Err(_) => respond(stream, "404 Not Found", "text/plain", b"Not found"),
    }
}
const FALLBACK: &str = r#"<!doctype html><html lang="en"><meta charset="utf-8"><title>DiskGoblin</title><style>body{font:16px system-ui;max-width:60rem;margin:3rem auto;padding:1rem}pre{white-space:pre-wrap}</style><h1>DiskGoblin</h1><p>Read-only disk usage analysis. Build the desktop frontend for the interactive treemap.</p><button id="scan">Scan</button><pre id="result">Loading configuration…</pre><script>let config;const result=document.querySelector('#result');fetch('/api/config').then(r=>r.json()).then(c=>{config=c;result.textContent=c.root_path;scan()}).catch(e=>result.textContent=e.message);async function scan(){if(!config)return;result.textContent='Scanning…';try{const r=await fetch('/api/scan',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(config)});result.textContent=JSON.stringify(await r.json(),null,2)}catch(e){result.textContent=e.message}}document.querySelector('#scan').onclick=scan;</script></html>"#;

#[cfg(test)]
mod tests {
    use super::*;
    use diskgoblin_core::test_fixtures::FixtureBuilder;

    fn request(
        address: std::net::SocketAddr,
        method: &str,
        path: &str,
        body: &str,
    ) -> (String, String) {
        let mut stream = TcpStream::connect(address).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        write!(stream, "{method} {path} HTTP/1.1\r\nHost: {address}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len()).unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        let (headers, body) = response.split_once("\r\n\r\n").unwrap();
        (headers.to_owned(), body.to_owned())
    }

    #[test]
    fn bridge_endpoints_and_clean_shutdown() {
        let fixture = FixtureBuilder::new()
            .add_file("data.bin", 4096)
            .build()
            .unwrap();
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let cancel = CancelToken::new();
        let shutdown = cancel.clone();
        let options = json!({"root_path": fixture.path(), "size_metric": "apparent", "one_file_system": false, "max_depth": null, "top_n": 20, "min_size_bytes": 0, "threads": 1});
        let config = options.clone();
        let absent_dist = fixture.path().join("missing-dist");
        let worker = thread::spawn(move || serve(listener, shutdown, config, absent_dist).unwrap());
        let (headers, body) = request(address, "GET", "/", "");
        assert!(headers.contains("200 OK"));
        assert!(
            body.contains("DiskGoblin")
                && body.contains("/api/config")
                && body.contains("/api/scan")
        );
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(
                &request(address, "GET", "/api/config", "").1
            )
            .unwrap(),
            options
        );
        assert_eq!(request(address, "GET", "/api/progress", "").1, "null");
        let (headers, body) = request(address, "POST", "/api/scan", &options.to_string());
        assert!(headers.contains("200 OK"));
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&body).unwrap()["schema_version"],
            1
        );
        let progress: serde_json::Value =
            serde_json::from_str(&request(address, "GET", "/api/progress", "").1).unwrap();
        assert!(progress.is_object());
        assert_eq!(
            request(address, "POST", "/api/cancel", "").1,
            "{\"cancelled\":true}"
        );
        assert!(request(address, "POST", "/api/scan", "bad json")
            .0
            .contains("400 Bad Request"));
        assert!(request(address, "GET", "/../Cargo.toml", "")
            .0
            .contains("403 Forbidden"));
        cancel.cancel();
        worker.join().unwrap();
    }

    #[test]
    fn stale_scan_progress_cannot_overwrite_current_scan() {
        let state = Arc::new(Mutex::new(ActiveScan {
            generation: 2,
            ..ActiveScan::default()
        }));
        let stale = Sink {
            state: state.clone(),
            generation: 1,
        };
        stale.on_progress(ScanProgress::default());
        assert!(state.lock().unwrap().progress.is_none());
        let current = Sink {
            state: state.clone(),
            generation: 2,
        };
        current.on_progress(ScanProgress::default());
        assert!(state.lock().unwrap().progress.is_some());
    }

    #[test]
    fn serves_assets_and_handles_concurrent_connections() {
        let assets = tempfile::tempdir().unwrap();
        std::fs::write(
            assets.path().join("index.html"),
            "<h1>DiskGoblin assets</h1>",
        )
        .unwrap();
        std::fs::write(assets.path().join("app.js"), "console.log('DiskGoblin')").unwrap();
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let cancel = CancelToken::new();
        let shutdown = cancel.clone();
        let dist = assets.path().to_owned();
        let worker = thread::spawn(move || serve(listener, shutdown, json!({}), dist).unwrap());
        // An unfinished request must not prevent progress and cancellation requests.
        let mut pending = TcpStream::connect(address).unwrap();
        pending
            .write_all(b"POST /api/scan HTTP/1.1\r\nContent-Length: 2\r\n\r\n{")
            .unwrap();
        assert_eq!(request(address, "GET", "/api/progress", "").1, "null");
        assert_eq!(
            request(address, "POST", "/api/cancel", "").1,
            "{\"cancelled\":true}"
        );
        let (headers, body) = request(address, "GET", "/app.js", "");
        assert!(headers.contains("application/javascript; charset=utf-8"));
        assert!(body.contains("DiskGoblin"));
        assert_eq!(
            request(address, "GET", "/", "").1,
            "<h1>DiskGoblin assets</h1>"
        );
        assert!(request(address, "GET", "/missing", "")
            .0
            .contains("404 Not Found"));
        drop(pending);
        cancel.cancel();
        worker.join().unwrap();
    }
}

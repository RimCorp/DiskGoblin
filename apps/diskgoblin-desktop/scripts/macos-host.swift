import Cocoa
import WebKit

final class AppDelegate: NSObject, NSApplicationDelegate, NSWindowDelegate, WKNavigationDelegate {
    var window: NSWindow!
    var webView: WKWebView!
    var engine: Process?
    var startupTimer: Timer?
    var snapshotTimer: Timer?
    var snapshotStarted = false
    var stderrBuffer = Data()
    let environment = ProcessInfo.processInfo.environment

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.appearance = NSAppearance(named: .darkAqua)
        let dimensions = (environment["DISK_GOBLIN_WINDOW_SIZE"] ?? "1440x900").split(separator: "x").compactMap { Double($0) }
        let size = dimensions.count == 2 ? NSSize(width: max(960, dimensions[0]), height: max(620, dimensions[1])) : NSSize(width: 1440, height: 900)
        window = NSWindow(contentRect: NSRect(origin: .zero, size: size), styleMask: [.titled, .closable, .miniaturizable, .resizable], backing: .buffered, defer: false)
        window.title = "DiskGoblin"
        window.contentMinSize = NSSize(width: 960, height: 620)
        window.backgroundColor = NSColor(srgbRed: 24/255, green: 28/255, blue: 27/255, alpha: 1)
        window.delegate = self
        webView = WKWebView(frame: NSRect(origin: .zero, size: size))
        webView.navigationDelegate = self
        webView.setValue(false, forKey: "drawsBackground")
        window.contentView = webView
        window.center(); window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        startEngine()
    }
    func startEngine() {
        let bundle = Bundle.main.bundleURL
        let process = Process()
        process.executableURL = bundle.appendingPathComponent("Contents/MacOS/diskgoblin-engine")
        let scanPath = CommandLine.arguments.dropFirst().first ?? FileManager.default.homeDirectoryForCurrentUser.path
        process.arguments = ["--ui", "--ui-port", "0", "--no-open", scanPath]
        var env = environment
        env["DISK_GOBLIN_UI_DIST"] = bundle.appendingPathComponent("Contents/Resources/dist").path
        process.environment = env
        let pipe = Pipe()
        process.standardError = pipe
        process.standardOutput = FileHandle.nullDevice
        engine = process
        pipe.fileHandleForReading.readabilityHandler = { [weak self] handle in
            let data = handle.availableData
            if data.isEmpty { handle.readabilityHandler = nil; return }
            DispatchQueue.main.async {
                guard let self = self else { return }
                self.stderrBuffer.append(data)
                let text = String(decoding: self.stderrBuffer, as: UTF8.self)
                if let range = text.range(of: #"http://127\.0\.0\.1:[0-9]+(?=\s)"#, options: .regularExpression), let url = URL(string: String(text[range])) {
                    handle.readabilityHandler = nil
                    self.startupTimer?.invalidate()
                    self.webView.load(URLRequest(url: url))
                }
            }
        }
        do { try process.run() }
        catch { fail("Unable to start scanner: \(error)"); return }
        startupTimer = Timer.scheduledTimer(withTimeInterval: 30, repeats: false) { [weak self] _ in self?.fail("Scanner startup timed out") }
    }
    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        guard environment["DISK_GOBLIN_SCREENSHOT_PATH"] != nil else { return }
        var attempts = 0
        snapshotTimer = Timer.scheduledTimer(withTimeInterval: 0.25, repeats: true) { [weak self] timer in
            guard let self = self, !self.snapshotStarted else { return }
            attempts += 1
            if attempts > 240 { timer.invalidate(); self.fail("Timed out waiting for treemap"); return }
            self.webView.evaluateJavaScript("document.querySelector('[aria-label=\"Disk usage treemap\"] rect') !== null") { result, _ in
                if result as? Bool == true && !self.snapshotStarted {
                    self.snapshotStarted = true; timer.invalidate()
                    DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) { self.capture() }
                }
            }
        }
    }
    func capture() {
        webView.takeSnapshot(with: nil) { image, error in
            guard let image = image, let tiff = image.tiffRepresentation,
                  let bitmap = NSBitmapImageRep(data: tiff), let png = bitmap.representation(using: .png, properties: [:]),
                  let path = self.environment["DISK_GOBLIN_SCREENSHOT_PATH"] else { self.fail("Snapshot failed: \(String(describing: error))"); return }
            do { try png.write(to: URL(fileURLWithPath: path)); NSApp.terminate(nil) }
            catch { self.fail("Cannot write screenshot: \(error)") }
        }
    }
    func fail(_ message: String) {
        FileHandle.standardError.write(Data((message + "\n").utf8))
        stopEngine()
        exit(1)
    }
    func stopEngine() {
        startupTimer?.invalidate(); snapshotTimer?.invalidate()
        if let engine = engine, engine.isRunning { engine.terminate(); engine.waitUntilExit() }
    }
    func windowWillClose(_ notification: Notification) { NSApp.terminate(nil) }
    func applicationWillTerminate(_ notification: Notification) { stopEngine() }
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { true }
}
let app = NSApplication.shared
let delegate = AppDelegate()
app.setActivationPolicy(.regular)
app.delegate = delegate
app.run()

//! Deterministic synthetic filesystem fixtures for DiskGoblin tests.
//!
//! [`FixtureBuilder`] offers a small declarative API for constructing temporary
//! directory trees (nested directories, files of exact byte lengths, hard links,
//! symlinks, and unreadable directories) without ever touching host paths
//! outside a freshly created [`tempfile::TempDir`].
//!
//! All paths supplied to the builder must be relative and must contain only
//! normal path components. Validation happens in [`FixtureBuilder::build`];
//! violations are reported as [`std::io::ErrorKind::InvalidInput`] errors.
//!
//! The module is available to this crate's own tests and to workspace crates
//! that enable the `test-utils` feature.

use std::fs;
use std::io::{self, BufWriter, Write};
use std::path::{Component, Path, PathBuf};

use tempfile::TempDir;

/// Length of the deterministic byte pattern written into fixture files.
const FILE_PATTERN_LEN: usize = 256;

/// Returns whether the current process is running as the root user (`uid == 0`).
///
/// Permission-denied tests should skip mode-based assertions when this returns
/// `true` (for example inside a root Docker container), because the root user
/// bypasses filesystem permission checks.
#[cfg(unix)]
pub fn is_root_user() -> bool {
    use std::os::unix::fs::MetadataExt;

    // A freshly created file is owned by the process's effective UID.
    tempfile::tempfile()
        .and_then(|file| file.metadata())
        .map(|metadata| metadata.uid() == 0)
        .unwrap_or(false)
}

/// Declarative builder for deterministic temporary filesystem fixtures.
///
/// Entries are applied in the order they were added, so later entries may
/// depend on files or directories created by earlier ones (for example,
/// [`FixtureBuilder::add_hard_link`] requires its source to already exist).
///
/// ```
/// # #[cfg(feature = "test-utils")]
/// # fn main() -> std::io::Result<()> {
/// use diskgoblin_core::test_fixtures::FixtureBuilder;
///
/// let fixture = FixtureBuilder::new()
///     .add_dir("a/b")
///     .add_file("a/b/data.bin", 1024)
///     .build()?;
///
/// assert_eq!(std::fs::metadata(fixture.path().join("a/b/data.bin"))?.len(), 1024);
/// # Ok(())
/// # }
/// # #[cfg(not(feature = "test-utils"))]
/// # fn main() {}
/// ```
#[derive(Debug, Default)]
pub struct FixtureBuilder {
    entries: Vec<FixtureEntry>,
}

#[derive(Debug)]
enum FixtureEntry {
    Dir(PathBuf),
    File {
        rel_path: PathBuf,
        size_bytes: u64,
    },
    HardLink {
        existing_rel_path: PathBuf,
        new_link_rel_path: PathBuf,
    },
    #[cfg(unix)]
    Symlink {
        target: PathBuf,
        link_rel_path: PathBuf,
    },
    #[cfg(unix)]
    UnreadableDir(PathBuf),
}

impl FixtureBuilder {
    /// Creates an empty fixture builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a directory, creating any missing ancestors.
    pub fn add_dir<P: AsRef<Path>>(mut self, rel_path: P) -> Self {
        self.entries
            .push(FixtureEntry::Dir(rel_path.as_ref().to_path_buf()));
        self
    }

    /// Adds a file containing exactly `size_bytes` deterministic bytes.
    ///
    /// The byte at offset `i` is `(i % 256) as u8`, which keeps fixture content
    /// reproducible across runs and platforms. Missing parent directories are
    /// created automatically.
    pub fn add_file<P: AsRef<Path>>(mut self, rel_path: P, size_bytes: u64) -> Self {
        self.entries.push(FixtureEntry::File {
            rel_path: rel_path.as_ref().to_path_buf(),
            size_bytes,
        });
        self
    }

    /// Adds a hard link at `new_link_rel_path` pointing to `existing_rel_path`.
    ///
    /// `existing_rel_path` is resolved relative to the fixture root and must
    /// already exist when the fixture is built.
    pub fn add_hard_link<P: AsRef<Path>, Q: AsRef<Path>>(
        mut self,
        existing_rel_path: P,
        new_link_rel_path: Q,
    ) -> Self {
        self.entries.push(FixtureEntry::HardLink {
            existing_rel_path: existing_rel_path.as_ref().to_path_buf(),
            new_link_rel_path: new_link_rel_path.as_ref().to_path_buf(),
        });
        self
    }

    /// Adds a symbolic link at `link_rel_path` pointing to `target_path`.
    ///
    /// The target is stored verbatim and may be relative, absolute, dangling,
    /// or point back to an ancestor directory; only the link path itself is
    /// required to stay inside the fixture root.
    #[cfg(unix)]
    pub fn add_symlink<P: AsRef<Path>, Q: AsRef<Path>>(
        mut self,
        target_path: P,
        link_rel_path: Q,
    ) -> Self {
        self.entries.push(FixtureEntry::Symlink {
            target: target_path.as_ref().to_path_buf(),
            link_rel_path: link_rel_path.as_ref().to_path_buf(),
        });
        self
    }

    /// Adds a directory and marks it unreadable (`mode 0o000`) on Unix.
    ///
    /// The path is recorded so that [`TestFixture`]'s drop implementation can
    /// restore `mode 0o700` before the underlying [`TempDir`] is removed.
    /// If the directory already exists (for example because files were added
    /// inside it earlier), its permissions are restricted in place.
    #[cfg(unix)]
    pub fn add_unreadable_dir<P: AsRef<Path>>(mut self, rel_path: P) -> Self {
        self.entries
            .push(FixtureEntry::UnreadableDir(rel_path.as_ref().to_path_buf()));
        self
    }

    /// Materializes every recorded entry inside a new temporary directory.
    ///
    /// Returns [`std::io::ErrorKind::InvalidInput`] if any path is absolute,
    /// empty, or contains `..`, `.`, or root components. If creating an entry
    /// fails, the partially built fixture is dropped, which restores any
    /// restricted permissions before the temporary directory is removed.
    pub fn build(self) -> io::Result<TestFixture> {
        for entry in &self.entries {
            validate_entry(entry)?;
        }

        let temp_dir = tempfile::Builder::new()
            .prefix("diskgoblin-fixture-")
            .tempdir()?;
        let mut fixture = TestFixture {
            temp_dir,
            #[cfg(unix)]
            restricted_paths: Vec::new(),
        };

        for entry in self.entries {
            fixture.apply(entry)?;
        }

        Ok(fixture)
    }
}

/// A fully materialized synthetic fixture backed by a temporary directory.
///
/// Dropping the fixture restores permissions on any directories created with
/// [`FixtureBuilder::add_unreadable_dir`] and then removes the temporary
/// directory tree from disk.
#[derive(Debug)]
pub struct TestFixture {
    temp_dir: TempDir,
    #[cfg(unix)]
    restricted_paths: Vec<PathBuf>,
}

impl TestFixture {
    /// Returns the absolute path of the fixture root directory.
    pub fn path(&self) -> &Path {
        self.temp_dir.path()
    }

    fn resolve(&self, rel_path: &Path) -> PathBuf {
        self.temp_dir.path().join(rel_path)
    }

    fn apply(&mut self, entry: FixtureEntry) -> io::Result<()> {
        match entry {
            FixtureEntry::Dir(rel_path) => {
                fs::create_dir_all(self.resolve(&rel_path))?;
            }
            FixtureEntry::File {
                rel_path,
                size_bytes,
            } => {
                let path = self.resolve(&rel_path);
                create_parent_dirs(&path)?;
                write_deterministic_file(&path, size_bytes)?;
            }
            FixtureEntry::HardLink {
                existing_rel_path,
                new_link_rel_path,
            } => {
                let existing = self.resolve(&existing_rel_path);
                let new_link = self.resolve(&new_link_rel_path);
                create_parent_dirs(&new_link)?;
                fs::hard_link(existing, new_link)?;
            }
            #[cfg(unix)]
            FixtureEntry::Symlink {
                target,
                link_rel_path,
            } => {
                let link = self.resolve(&link_rel_path);
                create_parent_dirs(&link)?;
                std::os::unix::fs::symlink(target, link)?;
            }
            #[cfg(unix)]
            FixtureEntry::UnreadableDir(rel_path) => {
                use std::os::unix::fs::PermissionsExt;

                let path = self.resolve(&rel_path);
                fs::create_dir_all(&path)?;
                fs::set_permissions(&path, fs::Permissions::from_mode(0o000))?;
                self.restricted_paths.push(path);
            }
        }

        Ok(())
    }
}

impl Drop for TestFixture {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            // Restore traversability deepest-first so `TempDir::drop` can
            // recursively delete the whole tree, including restricted dirs.
            for path in self.restricted_paths.iter().rev() {
                let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o700));
            }
        }
    }
}

fn validate_entry(entry: &FixtureEntry) -> io::Result<()> {
    match entry {
        FixtureEntry::Dir(rel_path) => validate_relative_path(rel_path),
        FixtureEntry::File { rel_path, .. } => validate_relative_path(rel_path),
        FixtureEntry::HardLink {
            existing_rel_path,
            new_link_rel_path,
        } => {
            validate_relative_path(existing_rel_path)?;
            validate_relative_path(new_link_rel_path)
        }
        #[cfg(unix)]
        FixtureEntry::Symlink {
            target,
            link_rel_path,
        } => {
            if target.as_os_str().is_empty() {
                return Err(invalid_input("symlink target must not be empty"));
            }
            validate_relative_path(link_rel_path)
        }
        #[cfg(unix)]
        FixtureEntry::UnreadableDir(rel_path) => validate_relative_path(rel_path),
    }
}

/// Confines a path to the fixture root by requiring only normal components.
fn validate_relative_path(path: &Path) -> io::Result<()> {
    if path.as_os_str().is_empty() {
        return Err(invalid_input("fixture path must not be empty"));
    }

    if path.is_absolute() {
        return Err(invalid_input(format!(
            "fixture path must be relative to the fixture root: {}",
            path.display()
        )));
    }

    for component in path.components() {
        if !matches!(component, Component::Normal(_)) {
            return Err(invalid_input(format!(
                "fixture path must contain only normal components: {}",
                path.display()
            )));
        }
    }

    Ok(())
}

fn invalid_input(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

fn create_parent_dirs(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn write_deterministic_file(path: &Path, size_bytes: u64) -> io::Result<()> {
    let file = fs::File::create(path)?;
    let mut writer = BufWriter::new(file);
    let pattern: Vec<u8> = (0..FILE_PATTERN_LEN).map(|byte| byte as u8).collect();
    let mut remaining = size_bytes;

    while remaining > 0 {
        let chunk_len = remaining.min(FILE_PATTERN_LEN as u64) as usize;
        writer.write_all(&pattern[..chunk_len])?;
        remaining -= chunk_len as u64;
    }

    writer.flush()
}

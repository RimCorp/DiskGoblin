//! Portable metadata extraction. Callers should obtain metadata with `symlink_metadata`.
use std::fs::Metadata;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileId {
    pub device_id: u64,
    pub inode: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    File,
    Directory,
    Symlink,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawMetadata {
    pub kind: EntryKind,
    pub apparent_size: u64,
    pub allocated_size: u64,
    pub file_id: Option<FileId>,
    pub hard_link_count: u64,
}

pub fn extract_metadata(metadata: &Metadata) -> RawMetadata {
    let file_type = metadata.file_type();
    let kind = if file_type.is_symlink() {
        EntryKind::Symlink
    } else if file_type.is_file() {
        EntryKind::File
    } else if file_type.is_dir() {
        EntryKind::Directory
    } else {
        EntryKind::Other
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        RawMetadata {
            kind,
            apparent_size: metadata.len(),
            allocated_size: metadata.blocks().saturating_mul(512),
            file_id: Some(FileId {
                device_id: metadata.dev(),
                inode: metadata.ino(),
            }),
            hard_link_count: metadata.nlink(),
        }
    }
    #[cfg(not(unix))]
    {
        RawMetadata {
            kind,
            apparent_size: metadata.len(),
            allocated_size: metadata.len(),
            file_id: None,
            hard_link_count: 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, io};

    fn check_metadata(metadata: &Metadata, kind: EntryKind) -> RawMetadata {
        let raw = extract_metadata(metadata);
        assert_eq!(raw.kind, kind);
        assert_eq!(raw.apparent_size, metadata.len());
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            assert_eq!(raw.allocated_size, metadata.blocks().saturating_mul(512));
            assert_eq!(
                raw.file_id,
                Some(FileId {
                    device_id: metadata.dev(),
                    inode: metadata.ino()
                })
            );
            assert_eq!(raw.hard_link_count, metadata.nlink());
        }
        #[cfg(not(unix))]
        {
            assert_eq!(raw.allocated_size, metadata.len());
            assert_eq!(raw.file_id, None);
            assert_eq!(raw.hard_link_count, 1);
        }
        raw
    }

    #[test]
    fn regular_file_and_directory() -> io::Result<()> {
        let fixture = tempfile::tempdir()?;
        let path = fixture.path().join("file");
        fs::write(&path, b"hello")?;
        let raw = check_metadata(&fs::symlink_metadata(path)?, EntryKind::File);
        assert_eq!(raw.apparent_size, 5);
        check_metadata(&fs::symlink_metadata(fixture.path())?, EntryKind::Directory);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_classified_without_following_targets() -> io::Result<()> {
        use std::os::unix::fs::symlink;
        let fixture = tempfile::tempdir()?;
        for (name, target) in [("directory_link", "."), ("dangling_link", "missing")] {
            let path = fixture.path().join(name);
            symlink(target, &path)?;
            check_metadata(&fs::symlink_metadata(path)?, EntryKind::Symlink);
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn hard_links_share_identity_and_report_two_links() -> io::Result<()> {
        let fixture = tempfile::tempdir()?;
        let original = fixture.path().join("original");
        let link = fixture.path().join("link");
        fs::write(&original, b"shared")?;
        fs::hard_link(&original, &link)?;
        let first = check_metadata(&fs::symlink_metadata(original)?, EntryKind::File);
        let second = check_metadata(&fs::symlink_metadata(link)?, EntryKind::File);
        assert_eq!(first.file_id, second.file_id);
        assert!(first.file_id.is_some());
        assert_eq!(first.hard_link_count, 2);
        assert_eq!(second.hard_link_count, 2);
        assert_eq!(first, second);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn unix_socket_is_other() -> io::Result<()> {
        let fixture = tempfile::tempdir()?;
        let path = fixture.path().join("socket");
        let _listener = std::os::unix::net::UnixListener::bind(&path)?;
        check_metadata(&fs::symlink_metadata(path)?, EntryKind::Other);
        Ok(())
    }
}

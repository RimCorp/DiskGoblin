//! Fatal scan failures and recoverable entry error records.
use std::{io, path::PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum FatalScanError {
    #[error("root path does not exist: {path}")]
    RootNotFound { path: PathBuf },
    #[error("failed to read root metadata for {path}: {source}")]
    RootUnreadable {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("scan cancelled by user")]
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScanErrorKind {
    PermissionDenied,
    NotFound,
    IoError,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ScanErrorRecord {
    pub path: PathBuf,
    pub kind: ScanErrorKind,
    pub message: String,
}

impl ScanErrorRecord {
    pub fn from_io_error(path: PathBuf, err: io::Error) -> Self {
        let kind = match err.kind() {
            io::ErrorKind::PermissionDenied => ScanErrorKind::PermissionDenied,
            io::ErrorKind::NotFound => ScanErrorKind::NotFound,
            _ => ScanErrorKind::IoError,
        };
        Self {
            path,
            kind,
            message: err.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn classifies_io_errors_and_preserves_context() {
        for (input, expected) in [
            (
                io::ErrorKind::PermissionDenied,
                ScanErrorKind::PermissionDenied,
            ),
            (io::ErrorKind::NotFound, ScanErrorKind::NotFound),
            (io::ErrorKind::InvalidData, ScanErrorKind::IoError),
            (io::ErrorKind::Other, ScanErrorKind::IoError),
        ] {
            let path = PathBuf::from("entry");
            let record =
                ScanErrorRecord::from_io_error(path.clone(), io::Error::new(input, "detail"));
            assert_eq!(record.path, path);
            assert_eq!(record.kind, expected);
            assert_eq!(record.message, "detail");
        }
    }

    #[test]
    fn missing_fixture_entry_is_recoverable() -> io::Result<()> {
        let fixture = tempfile::tempdir()?;
        let path = fixture.path().join("missing");
        let err = std::fs::symlink_metadata(&path).unwrap_err();
        let record = ScanErrorRecord::from_io_error(path.clone(), err);
        assert_eq!(record.path, path);
        assert_eq!(record.kind, ScanErrorKind::NotFound);
        Ok(())
    }

    #[test]
    fn fatal_errors_display_context_and_expose_source() {
        let missing = FatalScanError::RootNotFound {
            path: PathBuf::from("missing"),
        };
        assert_eq!(missing.to_string(), "root path does not exist: missing");
        assert!(missing.source().is_none());
        let unreadable = FatalScanError::RootUnreadable {
            path: PathBuf::from("root"),
            source: io::Error::new(io::ErrorKind::PermissionDenied, "denied"),
        };
        assert_eq!(
            unreadable.to_string(),
            "failed to read root metadata for root: denied"
        );
        assert_eq!(unreadable.source().unwrap().to_string(), "denied");
        assert_eq!(
            FatalScanError::Cancelled.to_string(),
            "scan cancelled by user"
        );
    }
}

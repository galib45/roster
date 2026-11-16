//! Reading and writing project files.

use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

use crate::domain::{ProjectFile, SCHEMA_VERSION};

/// Reasons a project file could not be read or written.
#[derive(Debug)]
pub enum StoreError {
    Io(io::Error),
    Json(serde_json::Error),
    UnsupportedSchema(u32),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "file error: {err}"),
            Self::Json(err) => write!(f, "invalid project JSON: {err}"),
            Self::UnsupportedSchema(version) => write!(
                f,
                "unsupported project schema version {version} (this build supports {SCHEMA_VERSION})"
            ),
        }
    }
}

impl std::error::Error for StoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::Json(err) => Some(err),
            Self::UnsupportedSchema(_) => None,
        }
    }
}

impl From<io::Error> for StoreError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<serde_json::Error> for StoreError {
    fn from(err: serde_json::Error) -> Self {
        Self::Json(err)
    }
}

/// Destination for the temporary file written before the real save.
fn temp_path(path: &Path) -> std::path::PathBuf {
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| format!("{ext}.tmp"))
        .unwrap_or_else(|| "tmp".to_owned());

    path.with_extension(extension)
}

/// Write a project file.
///
/// The JSON is written to a sibling temporary file first so a failed write
/// cannot truncate an existing project. `fs::rename` does not replace an
/// existing file on Windows, so that case removes the destination and retries
/// once.
pub fn save_project(project: &ProjectFile, path: &Path) -> Result<(), StoreError> {
    let json = serde_json::to_string_pretty(project)?;
    let tmp = temp_path(path);

    fs::write(&tmp, json.as_bytes())?;

    match fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(err)
            if matches!(
                err.kind(),
                io::ErrorKind::AlreadyExists | io::ErrorKind::PermissionDenied
            ) =>
        {
            fs::remove_file(path)?;
            if let Err(retry_err) = fs::rename(&tmp, path) {
                let _ = fs::remove_file(&tmp);
                return Err(StoreError::Io(retry_err));
            }
            Ok(())
        }
        Err(err) => {
            let _ = fs::remove_file(&tmp);
            Err(StoreError::Io(err))
        }
    }
}

/// Read a project file, rejecting schemas this build does not understand.
pub fn load_project(path: &Path) -> Result<ProjectFile, StoreError> {
    let text = fs::read_to_string(path)?;
    let project: ProjectFile = serde_json::from_str(&text)?;

    if project.schema_version != SCHEMA_VERSION {
        return Err(StoreError::UnsupportedSchema(project.schema_version));
    }

    Ok(project)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn temp_path(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("roster-test-{tag}-{}.json", Uuid::new_v4()))
    }

    #[test]
    fn round_trips_a_project_file() {
        let path = temp_path("roundtrip");
        let mut project = ProjectFile::new();
        project.add_organization("City Hospital");

        save_project(&project, &path).expect("save should succeed");
        let loaded = load_project(&path).expect("load should succeed");

        assert_eq!(project, loaded);

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn rejects_unsupported_schema_version() {
        let path = temp_path("schema");
        fs::write(
            &path,
            r#"{
                "schema_version": 99,
                "app": "roster",
                "saved_at": "2026-11-16T00:00:00Z",
                "organizations": []
            }"#,
        )
        .unwrap();

        let error = load_project(&path).expect_err("schema 99 must be rejected");
        assert!(matches!(error, StoreError::UnsupportedSchema(99)));

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn rejects_malformed_json() {
        let path = temp_path("malformed");
        fs::write(&path, "{ not json").unwrap();

        let error = load_project(&path).expect_err("malformed JSON must be rejected");
        assert!(matches!(error, StoreError::Json(_)));

        let _ = fs::remove_file(&path);
    }
}

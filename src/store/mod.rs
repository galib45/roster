//! Reading and writing project files.

use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

use serde::Deserialize;

use crate::domain::{ProjectFile, SCHEMA_VERSION};

/// The only field needed to check file compatibility.
#[derive(Debug, Deserialize)]
struct SchemaHeader {
    schema_version: u32,
}

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

/// Write a project file at the current schema version.
pub fn save_project(project: &ProjectFile, path: &Path) -> Result<(), StoreError> {
    let json = serde_json::to_string_pretty(project)?;
    fs::write(path, json.as_bytes())?;
    Ok(())
}

/// Read a project file, rejecting schema versions this build does not understand.
pub fn load_project(path: &Path) -> Result<ProjectFile, StoreError> {
    let text = fs::read_to_string(path)?;
    let value: serde_json::Value = serde_json::from_str(&text)?;
    let header: SchemaHeader = serde_json::from_value(value.clone())?;

    if header.schema_version != SCHEMA_VERSION {
        return Err(StoreError::UnsupportedSchema(header.schema_version));
    }

    let project: ProjectFile = serde_json::from_value(value)?;
    Ok(project)
}

//! Reading and writing project files.

use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

use serde::Deserialize;

use crate::domain::migration::ProjectFileV1;
use crate::domain::{ProjectFile, SCHEMA_VERSION};

/// The only field needed to choose a migration path.
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

/// Destination for the temporary file written before the real save.
fn temp_path(path: &Path) -> std::path::PathBuf {
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| format!("{ext}.tmp"))
        .unwrap_or_else(|| "tmp".to_owned());

    path.with_extension(extension)
}

/// Write a project file at the current schema version.
///
/// The JSON is written to a sibling temporary file first so a failed write
/// cannot truncate an existing project. `fs::rename` does not replace an
/// existing file on Windows, so that case removes the destination and retries
/// once. A migrated project is always written back as the current version.
pub fn save_project(project: &ProjectFile, path: &Path) -> Result<(), StoreError> {
    let mut project = project.clone();
    project.schema_version = SCHEMA_VERSION;

    let json = serde_json::to_string_pretty(&project)?;
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

/// Read a project file, migrating supported older schemas and rejecting
/// versions this build does not understand.
pub fn load_project(path: &Path) -> Result<ProjectFile, StoreError> {
    let text = fs::read_to_string(path)?;
    let value: serde_json::Value = serde_json::from_str(&text)?;
    let header: SchemaHeader = serde_json::from_value(value.clone())?;

    match header.schema_version {
        1 => {
            let v1: ProjectFileV1 = serde_json::from_value(value)?;
            Ok(v1.into())
        }
        2 => {
            let project: ProjectFile = serde_json::from_value(value)?;
            Ok(project)
        }
        version => Err(StoreError::UnsupportedSchema(version)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveTime, TimeZone, Utc};
    use std::path::PathBuf;
    use uuid::Uuid;

    fn temp_path(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("roster-test-{tag}-{}.json", Uuid::new_v4()))
    }

    fn at(hour: u32, minute: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(hour, minute, 0).unwrap()
    }

    fn populated_project() -> ProjectFile {
        let mut project = ProjectFile::new();
        project
            .add_organization("City Hospital")
            .expect("organization should be added");
        let organization = &mut project.organizations[0];

        let doctors = organization
            .add_roster("Doctors")
            .expect("doctors roster")
            .id
            .clone();
        let nurses = organization
            .add_roster("Nurses")
            .expect("nurses roster")
            .id
            .clone();
        let medical_officer = organization
            .add_position("Medical Officer")
            .expect("medical officer position")
            .id
            .clone();
        let staff_nurse = organization
            .add_position("Senior Staff Nurse")
            .expect("nurse position")
            .id
            .clone();

        let amit = organization
            .add_employee("Dr. Amit", &doctors, &medical_officer)
            .expect("employee")
            .id
            .clone();
        organization
            .employee_mut(&amit)
            .expect("employee should exist")
            .add_skill("ACLS")
            .expect("skill");
        organization
            .add_employee("Dr. Khushi", &doctors, &medical_officer)
            .expect("employee");
        organization
            .add_employee("Nurse Rina", &nurses, &staff_nurse)
            .expect("employee");

        let opd = organization
            .add_shift(&doctors, "OPD", at(8, 0), at(14, 0))
            .expect("shift")
            .id
            .clone();
        let night = organization
            .add_shift(&doctors, "Emergency Night", at(15, 0), at(8, 0))
            .expect("shift")
            .id
            .clone();
        organization
            .require_position(&doctors, &opd, &medical_officer, 2)
            .expect("requirement");
        organization
            .require_position(&doctors, &night, &medical_officer, 1)
            .expect("requirement");

        project
    }

    #[test]
    fn round_trips_a_project_file() {
        let path = temp_path("roundtrip");
        let project = populated_project();
        project.organizations[0].validate().expect("valid project");

        save_project(&project, &path).expect("save should succeed");
        let loaded = load_project(&path).expect("load should succeed");

        assert_eq!(project, loaded);
        assert_eq!(loaded.organizations[0].validate(), Ok(()));

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn saves_the_current_schema_version_and_nested_entities() {
        let path = temp_path("shape");
        let project = populated_project();

        save_project(&project, &path).expect("save should succeed");
        let text = fs::read_to_string(&path).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();

        assert_eq!(value["schema_version"], SCHEMA_VERSION);
        let organization = &value["organizations"][0];
        assert!(organization["positions"].is_array());
        assert!(organization["employees"].is_array());
        assert!(organization["rosters"].is_array());
        assert!(organization["rosters"][0]["shifts"].is_array());
        assert!(organization["employees"][0].get("shifts").is_none());

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn migrates_schema_one_projects() {
        let path = temp_path("v1");
        fs::write(
            &path,
            r#"{
                "schema_version": 1,
                "app": "roster",
                "saved_at": "2026-11-16T00:00:00Z",
                "organizations": [
                    { "id": "org-1", "name": "City Hospital" }
                ]
            }"#,
        )
        .unwrap();

        let project = load_project(&path).expect("v1 should migrate");

        assert_eq!(project.schema_version, SCHEMA_VERSION);
        assert_eq!(project.app, "roster");
        assert_eq!(
            project.saved_at,
            Utc.with_ymd_and_hms(2026, 11, 16, 0, 0, 0).unwrap()
        );
        assert_eq!(project.organizations.len(), 1);
        assert_eq!(project.organizations[0].id, "org-1");
        assert_eq!(project.organizations[0].name, "City Hospital");
        assert!(project.organizations[0].positions.is_empty());
        assert!(project.organizations[0].employees.is_empty());
        assert!(project.organizations[0].rosters.is_empty());

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn migrated_projects_are_saved_as_schema_two() {
        let path = temp_path("v1-rewrite");
        fs::write(
            &path,
            r#"{
                "schema_version": 1,
                "app": "roster",
                "saved_at": "2026-11-16T00:00:00Z",
                "organizations": [{ "id": "org-1", "name": "City Hospital" }]
            }"#,
        )
        .unwrap();

        let project = load_project(&path).expect("v1 should migrate");
        save_project(&project, &path).expect("save should succeed");

        let text = fs::read_to_string(&path).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["schema_version"], 2);
        assert!(value["organizations"][0]["rosters"].is_array());

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn rejects_unsupported_schema_version() {
        let path = temp_path("schema");
        fs::write(
            &path,
            r#"{
                "schema_version": 3,
                "app": "roster",
                "saved_at": "2026-11-16T00:00:00Z",
                "organizations": []
            }"#,
        )
        .unwrap();

        let error = load_project(&path).expect_err("schema 3 must be rejected");
        assert!(matches!(error, StoreError::UnsupportedSchema(3)));

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

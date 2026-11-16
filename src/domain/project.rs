//! The project file: the unit of persistence for the application.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Current on-disk schema version.
///
/// Bump this whenever the serialized shape changes incompatibly, and add a
/// migration before accepting older files.
pub const SCHEMA_VERSION: u32 = 1;

/// An organization that owns one or more rosters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Organization {
    /// Stable identifier, independent of the display name.
    pub id: String,
    pub name: String,
}

impl Organization {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
        }
    }
}

/// Everything saved to a `.json` project file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectFile {
    pub schema_version: u32,
    pub app: String,
    pub saved_at: DateTime<Utc>,
    pub organizations: Vec<Organization>,
}

impl ProjectFile {
    /// A new, empty project file at the current schema version.
    pub fn new() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            app: "roster".to_owned(),
            saved_at: Utc::now(),
            organizations: Vec::new(),
        }
    }

    /// Add an organization by name.
    ///
    /// Surrounding whitespace is trimmed; a name that is empty after trimming
    /// is rejected and `None` is returned so the UI can surface it instead of
    /// silently storing a blank organization.
    pub fn add_organization(&mut self, name: &str) -> Option<&Organization> {
        let name = name.trim();
        if name.is_empty() {
            return None;
        }

        self.organizations.push(Organization::new(name));
        self.saved_at = Utc::now();
        self.organizations.last()
    }
}

impl Default for ProjectFile {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_blank_organization_names() {
        let mut project = ProjectFile::new();

        assert!(project.add_organization("   ").is_none());
        assert!(project.organizations.is_empty());
    }

    #[test]
    fn trims_organization_names_and_assigns_ids() {
        let mut project = ProjectFile::new();

        let org = project.add_organization("  City Hospital ").unwrap();
        assert_eq!(org.name, "City Hospital");
        assert!(!org.id.is_empty());
    }

    #[test]
    fn new_project_has_current_schema_version() {
        let project = ProjectFile::new();

        assert_eq!(project.schema_version, SCHEMA_VERSION);
        assert_eq!(project.app, "roster");
    }
}

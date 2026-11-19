//! The project file: the unit of persistence for the application.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub use crate::domain::organization::Organization;

/// Current on-disk schema version.
///
/// The version is recorded so a file from another schema fails with a clear
/// message instead of loading as garbage. Migrations are deferred until after
/// the MVP; bump this whenever the serialized shape changes.
pub const SCHEMA_VERSION: u32 = 3;

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

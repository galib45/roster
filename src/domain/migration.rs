//! Migrations from earlier project-file schemas.
//!
//! Version 1 stored organizations only. Version 2 adds organization-level
//! positions, employees, and rosters. The v1 struct below is the migration
//! contract: it must not change when v2 evolves.

use chrono::{DateTime, Utc};
use serde::Deserialize;

use super::{Organization, ProjectFile, SCHEMA_VERSION};

/// The project shape written by Phase 1 (`schema_version: 1`).
#[derive(Debug, Deserialize)]
pub struct ProjectFileV1 {
    pub schema_version: u32,
    pub app: String,
    pub saved_at: DateTime<Utc>,
    pub organizations: Vec<OrganizationV1>,
}

/// A Phase 1 organization, which had no children.
#[derive(Debug, Deserialize)]
pub struct OrganizationV1 {
    pub id: String,
    pub name: String,
}

impl From<ProjectFileV1> for ProjectFile {
    fn from(v1: ProjectFileV1) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            app: v1.app,
            saved_at: v1.saved_at,
            organizations: v1
                .organizations
                .into_iter()
                .map(|organization| Organization {
                    id: organization.id,
                    name: organization.name,
                    positions: Vec::new(),
                    employees: Vec::new(),
                    rosters: Vec::new(),
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn migrates_v1_organizations_to_empty_v2_collections() {
        let organization = OrganizationV1 {
            id: "org-1".to_owned(),
            name: "City Hospital".to_owned(),
        };
        let v1 = ProjectFileV1 {
            schema_version: 1,
            app: "roster".to_owned(),
            saved_at: Utc.with_ymd_and_hms(2026, 11, 16, 0, 0, 0).unwrap(),
            organizations: vec![organization],
        };

        let project: ProjectFile = v1.into();

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
    }
}

//! Sharing groups: staff who cover for each other.
//!
//! Fairness is measured against a comparison mean. Employees grouped together
//! (for example the registrars who swap nights) are compared with their group's
//! average workload instead of the whole roster's, so a group that carries
//! heavier duties is judged on its own distribution.
//!
//! A group belongs to exactly one roster and an employee may join at most one
//! group. That keeps the comparison mean unambiguous.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::DomainError;

/// A set of roster employees whose workloads are averaged for fairness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharingGroup {
    /// Stable identifier, independent of the display name.
    pub id: String,
    pub name: String,
    /// Member employee IDs in insertion order. Never empty.
    pub employee_ids: Vec<String>,
}

impl SharingGroup {
    /// Create a group from a name and member IDs.
    ///
    /// The name is trimmed and blank names are rejected. Duplicate member IDs
    /// are removed (the first occurrence wins) and an empty membership is
    /// rejected. Cross-entity checks (known employees, roster membership, one
    /// group per employee) live on [`Organization`](super::Organization).
    pub fn new(name: &str, employee_ids: &[&str]) -> Result<Self, DomainError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(DomainError::BlankName);
        }

        let mut members: Vec<String> = Vec::with_capacity(employee_ids.len());
        for employee_id in employee_ids {
            let employee_id = employee_id.trim();
            if employee_id.is_empty() {
                return Err(DomainError::BlankName);
            }
            if !members.iter().any(|existing| existing == employee_id) {
                members.push(employee_id.to_owned());
            }
        }

        if members.is_empty() {
            return Err(DomainError::EmptySharingGroup {
                name: name.to_owned(),
            });
        }

        Ok(Self {
            id: Uuid::new_v4().to_string(),
            name: name.to_owned(),
            employee_ids: members,
        })
    }

    /// True when the employee belongs to this group.
    pub fn contains(&self, employee_id: &str) -> bool {
        self.employee_ids.iter().any(|member| member == employee_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_names_and_assigns_ids() {
        let group = SharingGroup::new("  Registrars ", &["employee-1", "employee-2"]).unwrap();

        assert_eq!(group.name, "Registrars");
        assert!(!group.id.is_empty());
        assert_eq!(group.employee_ids, vec!["employee-1", "employee-2"]);
        assert!(group.contains("employee-1"));
        assert!(!group.contains("employee-3"));
    }

    #[test]
    fn removes_duplicate_members_keeping_first_order() {
        let group =
            SharingGroup::new("Registrars", &["employee-2", "employee-1", "employee-2"]).unwrap();

        assert_eq!(group.employee_ids, vec!["employee-2", "employee-1"]);
    }

    #[test]
    fn rejects_blank_names_and_blank_members() {
        assert_eq!(
            SharingGroup::new("   ", &["employee-1"]),
            Err(DomainError::BlankName)
        );
        assert_eq!(
            SharingGroup::new("Registrars", &["  "]),
            Err(DomainError::BlankName)
        );
    }

    #[test]
    fn rejects_empty_membership() {
        assert_eq!(
            SharingGroup::new("Registrars", &[]),
            Err(DomainError::EmptySharingGroup {
                name: "Registrars".to_owned()
            })
        );
    }
}

//! Employees: organization-owned staff who each hold one position.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{DomainError, RecurringAssignment, Unavailability};

/// A member of staff.
///
/// Employees live on the organization and hold exactly one organization-level
/// [`Position`](super::Position). Roster membership is not stored on the
/// employee: templates gather staff by including positions and excluding
/// individual employees.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Employee {
    /// Stable identifier, independent of the display name.
    pub id: String,
    pub name: String,
    /// The organization-level position this employee holds.
    pub position_id: String,
    /// Free-form skill labels (for example "ACLS").
    #[serde(default)]
    pub skills: Vec<String>,
    /// Periods when the employee is unavailable or on leave.
    #[serde(default)]
    pub unavailability: Vec<Unavailability>,
    /// Weekly fixed assignments (demand, not a restriction).
    #[serde(default)]
    pub recurring_assignments: Vec<RecurringAssignment>,
}

impl Employee {
    /// Create an employee from a display name and position reference.
    ///
    /// The position is validated by [`Organization`](super::Organization),
    /// which can see the organization's positions.
    pub fn new(name: &str, position_id: impl Into<String>) -> Result<Self, DomainError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(DomainError::BlankName);
        }

        Ok(Self {
            id: Uuid::new_v4().to_string(),
            name: name.to_owned(),
            position_id: position_id.into(),
            skills: Vec::new(),
            unavailability: Vec::new(),
            recurring_assignments: Vec::new(),
        })
    }

    /// Add a skill label.
    ///
    /// Labels are trimmed, blanks are rejected, and an existing label is
    /// ignored so callers can be idempotent.
    pub fn add_skill(&mut self, skill: &str) -> Result<(), DomainError> {
        let skill = skill.trim();
        if skill.is_empty() {
            return Err(DomainError::BlankName);
        }

        if self.skills.iter().any(|existing| existing == skill) {
            return Ok(());
        }

        self.skills.push(skill.to_owned());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_names_and_assigns_ids() {
        let employee = Employee::new("  Dr. Amit ", "position-1").unwrap();

        assert_eq!(employee.name, "Dr. Amit");
        assert!(!employee.id.is_empty());
        assert_eq!(employee.position_id, "position-1");
        assert!(employee.skills.is_empty());
        assert!(employee.unavailability.is_empty());
        assert!(employee.recurring_assignments.is_empty());
    }

    #[test]
    fn rejects_blank_names() {
        assert_eq!(
            Employee::new("   ", "position-1"),
            Err(DomainError::BlankName)
        );
    }

    #[test]
    fn trims_skills_and_ignores_duplicates() {
        let mut employee = Employee::new("Dr. Amit", "position-1").unwrap();

        employee.add_skill("  ACLS  ").unwrap();
        employee.add_skill("ACLS").unwrap();
        employee.add_skill("BLS").unwrap();

        assert_eq!(employee.skills, vec!["ACLS".to_owned(), "BLS".to_owned()]);
    }

    #[test]
    fn rejects_blank_skills() {
        let mut employee = Employee::new("Dr. Amit", "position-1").unwrap();

        assert_eq!(employee.add_skill("   "), Err(DomainError::BlankName));
        assert!(employee.skills.is_empty());
    }
}

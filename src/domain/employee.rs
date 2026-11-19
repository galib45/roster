//! Employees: organization-owned staff who each hold one position.

use chrono::Weekday;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{DomainError, RecurringAssignment, Severity, Unavailability, UnavailabilityScope};

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
            unavailability: Vec::new(),
            recurring_assignments: Vec::new(),
        })
    }

    /// Add an unavailability or leave rule.
    ///
    /// The scope, date range, and severity/penalty pairing are validated by
    /// [`Unavailability::new`]. Shift ids are stored as given; the employee has
    /// no visibility into the organization's templates.
    pub fn add_unavailability(
        &mut self,
        scope: UnavailabilityScope,
        shift_ids: &[&str],
        severity: Severity,
        penalty: u32,
    ) -> Result<&Unavailability, DomainError> {
        let mut shifts = Vec::with_capacity(shift_ids.len());
        for shift_id in shift_ids {
            if shift_id.trim().is_empty() {
                return Err(DomainError::BlankName);
            }
            shifts.push((*shift_id).to_owned());
        }

        let rule = Unavailability::new(scope, shifts, severity, penalty)?;
        self.unavailability.push(rule);
        Ok(self
            .unavailability
            .last()
            .expect("an unavailability rule was just pushed"))
    }

    /// Add a weekly recurring preference with the default penalty of 1.
    pub fn add_recurring_assignment(
        &mut self,
        weekday: Weekday,
        shift_id: &str,
    ) -> Result<&RecurringAssignment, DomainError> {
        self.add_recurring_assignment_with_penalty(weekday, shift_id, 1)
    }

    /// Add a weekly recurring preference with an explicit penalty.
    ///
    /// A penalty of 0 is rejected: recurring assignments stay soft and must
    /// cost something when unmet.
    pub fn add_recurring_assignment_with_penalty(
        &mut self,
        weekday: Weekday,
        shift_id: &str,
        penalty: u32,
    ) -> Result<&RecurringAssignment, DomainError> {
        let assignment = RecurringAssignment::with_penalty(weekday, shift_id, penalty)?;
        self.recurring_assignments.push(assignment);
        Ok(self
            .recurring_assignments
            .last()
            .expect("a recurring assignment was just pushed"))
    }
}

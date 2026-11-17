//! Rosters: staff groups that own the shift templates for the group.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{DomainError, ShiftTemplate};

/// A staff group such as "Doctors", "Nurses", or "Cleaners".
///
/// A roster owns its shifts and nothing else. Employees and positions live on
/// the organization and reference the roster by ID.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Roster {
    /// Stable identifier, independent of the display name.
    pub id: String,
    pub name: String,
    /// Shift templates owned by this roster.
    #[serde(default)]
    pub shifts: Vec<ShiftTemplate>,
}

impl Roster {
    /// Create a roster from a display name.
    ///
    /// The name is trimmed and blank names are rejected. Duplicate names are
    /// checked by [`Organization`](super::Organization), which can see sibling
    /// rosters.
    pub fn new(name: &str) -> Result<Self, DomainError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(DomainError::BlankName);
        }

        Ok(Self {
            id: Uuid::new_v4().to_string(),
            name: name.to_owned(),
            shifts: Vec::new(),
        })
    }

    /// Find a shift by ID.
    pub fn shift(&self, id: &str) -> Option<&ShiftTemplate> {
        self.shifts.iter().find(|shift| shift.id == id)
    }

    /// Find a shift by ID for mutation.
    ///
    /// Crate-visible so `Organization` remains the validated entry point.
    pub(crate) fn shift_mut(&mut self, id: &str) -> Option<&mut ShiftTemplate> {
        self.shifts.iter_mut().find(|shift| shift.id == id)
    }

    /// Append a shift. Crate-visible for the same reason as [`Self::shift_mut`].
    pub(crate) fn push_shift(&mut self, shift: ShiftTemplate) {
        self.shifts.push(shift);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveTime;

    fn at(hour: u32, minute: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(hour, minute, 0).unwrap()
    }

    #[test]
    fn trims_names_and_rejects_blanks() {
        let roster = Roster::new("  Doctors ").unwrap();
        assert_eq!(roster.name, "Doctors");
        assert!(!roster.id.is_empty());
        assert_eq!(Roster::new("   "), Err(DomainError::BlankName));
    }

    #[test]
    fn shifts_are_scoped_to_their_roster() {
        let mut doctors = Roster::new("Doctors").unwrap();
        let nurses = Roster::new("Nurses").unwrap();

        doctors.push_shift(ShiftTemplate::new("OPD", at(8, 0), at(14, 0)).unwrap());
        let shift_id = doctors.shifts[0].id.clone();

        assert!(doctors.shift(&shift_id).is_some());
        assert!(nurses.shift(&shift_id).is_none());
    }
}

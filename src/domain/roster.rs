//! Roster templates: reusable staffing definitions that own shifts and dated
//! rosters.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{DomainError, RestRule, Roster, SharingGroup, ShiftTemplate};

/// A reusable roster definition such as "Doctors", "Nurses", or "Cleaners".
///
/// A template selects the organization positions it covers and may exclude
/// individual employees. Membership is derived from the organization's
/// employees, so an employee never stores a roster id. The template owns its
/// shifts, rest rules, sharing groups, and dated rosters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RosterTemplate {
    /// Stable identifier, independent of the display name.
    pub id: String,
    pub name: String,
    /// Organization positions whose employees belong to this template.
    #[serde(default)]
    pub position_ids: Vec<String>,
    /// Employees who would otherwise be included but are excluded explicitly.
    #[serde(default)]
    pub excluded_employee_ids: Vec<String>,
    /// Shift templates owned by this template.
    #[serde(default)]
    pub shifts: Vec<ShiftTemplate>,
    /// Minimum-rest rules between pairs of this template's shifts.
    #[serde(default)]
    pub rest_rules: Vec<RestRule>,
    /// Staff who share a fairness comparison mean.
    #[serde(default)]
    pub sharing_groups: Vec<SharingGroup>,
    /// Dated rosters created from this template.
    #[serde(default)]
    pub rosters: Vec<Roster>,
}

impl RosterTemplate {
    /// Create a template from a display name.
    ///
    /// The name is trimmed and blank names are rejected. Duplicate names are
    /// checked by [`Organization`](super::Organization), which can see sibling
    /// templates.
    pub fn new(name: &str) -> Result<Self, DomainError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(DomainError::BlankName);
        }

        Ok(Self {
            id: Uuid::new_v4().to_string(),
            name: name.to_owned(),
            position_ids: Vec::new(),
            excluded_employee_ids: Vec::new(),
            shifts: Vec::new(),
            rest_rules: Vec::new(),
            sharing_groups: Vec::new(),
            rosters: Vec::new(),
        })
    }

    /// Whether this template covers the given position.
    pub fn includes_position(&self, position_id: &str) -> bool {
        self.position_ids
            .iter()
            .any(|candidate| candidate == position_id)
    }

    /// Whether the given employee is explicitly excluded.
    pub fn excludes_employee(&self, employee_id: &str) -> bool {
        self.excluded_employee_ids
            .iter()
            .any(|candidate| candidate == employee_id)
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

    /// Append a rest rule. Crate-visible so `Organization` stays the validated
    /// entry point.
    pub(crate) fn push_rest_rule(&mut self, rule: RestRule) {
        self.rest_rules.push(rule);
    }

    /// Find a rest rule by ID.
    pub fn rest_rule(&self, id: &str) -> Option<&RestRule> {
        self.rest_rules.iter().find(|rule| rule.id == id)
    }

    /// Find a sharing group by ID.
    pub fn sharing_group(&self, id: &str) -> Option<&SharingGroup> {
        self.sharing_groups.iter().find(|group| group.id == id)
    }

    /// Find the sharing group an employee belongs to.
    pub fn sharing_group_for_employee(&self, employee_id: &str) -> Option<&SharingGroup> {
        self.sharing_groups
            .iter()
            .find(|group| group.contains(employee_id))
    }

    /// Append a sharing group. Crate-visible so `Organization` stays the
    /// validated entry point.
    pub(crate) fn push_sharing_group(&mut self, group: SharingGroup) {
        self.sharing_groups.push(group);
    }

    /// Find a dated roster by ID.
    pub fn roster(&self, id: &str) -> Option<&Roster> {
        self.rosters.iter().find(|roster| roster.id == id)
    }

    /// Find a dated roster by ID for mutation.
    ///
    /// Crate-visible so `Organization` remains the validated entry point.
    pub(crate) fn roster_mut(&mut self, id: &str) -> Option<&mut Roster> {
        self.rosters.iter_mut().find(|roster| roster.id == id)
    }

    /// Append a dated roster. Crate-visible for the same reason.
    pub(crate) fn push_roster(&mut self, roster: Roster) {
        self.rosters.push(roster);
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
        let roster = RosterTemplate::new("  Doctors ").unwrap();
        assert_eq!(roster.name, "Doctors");
        assert!(!roster.id.is_empty());
        assert!(roster.position_ids.is_empty());
        assert!(roster.excluded_employee_ids.is_empty());
        assert!(roster.sharing_groups.is_empty());
        assert_eq!(RosterTemplate::new("   "), Err(DomainError::BlankName));
    }

    #[test]
    fn shifts_are_scoped_to_their_template() {
        let mut doctors = RosterTemplate::new("Doctors").unwrap();
        let nurses = RosterTemplate::new("Nurses").unwrap();

        doctors.push_shift(ShiftTemplate::new("OPD", at(8, 0), at(14, 0)).unwrap());
        let shift_id = doctors.shifts[0].id.clone();

        assert!(doctors.shift(&shift_id).is_some());
        assert!(nurses.shift(&shift_id).is_none());
    }

    #[test]
    fn tracks_positions_and_exclusions() {
        let mut doctors = RosterTemplate::new("Doctors").unwrap();
        doctors.position_ids.push("position-1".to_owned());
        doctors.excluded_employee_ids.push("employee-1".to_owned());

        assert!(doctors.includes_position("position-1"));
        assert!(!doctors.includes_position("position-2"));
        assert!(doctors.excludes_employee("employee-1"));
        assert!(!doctors.excludes_employee("employee-2"));
    }
}

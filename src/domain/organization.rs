//! Organizations: the root of the domain graph.
//!
//! An organization owns its positions, its employees (each assigned to exactly
//! one roster), and its rosters. Rosters own shift templates. Cross-entity
//! validation lives here because no child can see the whole graph.

use std::collections::HashSet;

use chrono::NaiveTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{DomainError, Employee, Position, Roster, ShiftTemplate, StaffingRequirement};

/// A hospital, clinic, or department that owns staff and rosters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Organization {
    /// Stable identifier, independent of the display name.
    pub id: String,
    pub name: String,
    /// Organization-level positions referenced by employees and shifts.
    #[serde(default)]
    pub positions: Vec<Position>,
    /// Organization-level staff, each assigned to one roster.
    #[serde(default)]
    pub employees: Vec<Employee>,
    /// Staff groups; each owns its shift templates.
    #[serde(default)]
    pub rosters: Vec<Roster>,
}

impl Organization {
    /// Create an empty organization.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            positions: Vec::new(),
            employees: Vec::new(),
            rosters: Vec::new(),
        }
    }

    /// Find a position by ID.
    pub fn position(&self, id: &str) -> Option<&Position> {
        self.positions.iter().find(|position| position.id == id)
    }

    /// Find an employee by ID.
    pub fn employee(&self, id: &str) -> Option<&Employee> {
        self.employees.iter().find(|employee| employee.id == id)
    }

    /// Find an employee by ID for mutation.
    pub fn employee_mut(&mut self, id: &str) -> Option<&mut Employee> {
        self.employees.iter_mut().find(|employee| employee.id == id)
    }

    /// Find a roster by ID.
    pub fn roster(&self, id: &str) -> Option<&Roster> {
        self.rosters.iter().find(|roster| roster.id == id)
    }

    /// Find a roster by ID for mutation.
    pub fn roster_mut(&mut self, id: &str) -> Option<&mut Roster> {
        self.rosters.iter_mut().find(|roster| roster.id == id)
    }

    /// Every employee assigned to the given roster.
    ///
    /// This is the only supported way to read a staff pool; an unknown roster
    /// yields an empty list rather than leaking other employees.
    pub fn employees_in_roster(&self, roster_id: &str) -> Vec<&Employee> {
        self.employees
            .iter()
            .filter(|employee| employee.roster_id == roster_id)
            .collect()
    }

    /// Add an organization-level position.
    pub fn add_position(&mut self, name: &str) -> Result<&Position, DomainError> {
        let name = name.trim();
        if self.positions.iter().any(|position| position.name == name) {
            return Err(DomainError::DuplicatePosition {
                name: name.to_owned(),
            });
        }

        self.positions.push(Position::new(name)?);
        Ok(self.positions.last().expect("a position was just pushed"))
    }

    /// Add a staff-group roster.
    pub fn add_roster(&mut self, name: &str) -> Result<&Roster, DomainError> {
        let name = name.trim();
        if self.rosters.iter().any(|roster| roster.name == name) {
            return Err(DomainError::DuplicateRoster {
                name: name.to_owned(),
            });
        }

        self.rosters.push(Roster::new(name)?);
        Ok(self.rosters.last().expect("a roster was just pushed"))
    }

    /// Add an employee assigned to a roster and holding an organization position.
    pub fn add_employee(
        &mut self,
        name: &str,
        roster_id: &str,
        position_id: &str,
    ) -> Result<&Employee, DomainError> {
        if self.roster(roster_id).is_none() {
            return Err(DomainError::UnknownRoster {
                id: roster_id.to_owned(),
            });
        }
        if self.position(position_id).is_none() {
            return Err(DomainError::UnknownPosition {
                id: position_id.to_owned(),
            });
        }

        self.employees
            .push(Employee::new(name, roster_id, position_id)?);
        Ok(self.employees.last().expect("an employee was just pushed"))
    }

    /// Add a shift to a roster.
    pub fn add_shift(
        &mut self,
        roster_id: &str,
        name: &str,
        start: NaiveTime,
        end: NaiveTime,
    ) -> Result<&mut ShiftTemplate, DomainError> {
        let name = name.trim();
        let roster = self
            .roster_mut(roster_id)
            .ok_or_else(|| DomainError::UnknownRoster {
                id: roster_id.to_owned(),
            })?;

        if roster.shifts.iter().any(|shift| shift.name == name) {
            return Err(DomainError::DuplicateShift {
                name: name.to_owned(),
            });
        }

        roster.push_shift(ShiftTemplate::new(name, start, end)?);
        Ok(roster.shifts.last_mut().expect("a shift was just pushed"))
    }

    /// Set (or raise/lower) the headcount a shift needs for a position.
    pub fn require_position(
        &mut self,
        roster_id: &str,
        shift_id: &str,
        position_id: &str,
        headcount: u16,
    ) -> Result<(), DomainError> {
        if self.position(position_id).is_none() {
            return Err(DomainError::UnknownPosition {
                id: position_id.to_owned(),
            });
        }
        if headcount == 0 {
            return Err(DomainError::EmptyHeadcount {
                shift_id: shift_id.to_owned(),
                position_id: position_id.to_owned(),
            });
        }

        let roster = self
            .roster_mut(roster_id)
            .ok_or_else(|| DomainError::UnknownRoster {
                id: roster_id.to_owned(),
            })?;
        let shift = roster
            .shift_mut(shift_id)
            .ok_or_else(|| DomainError::UnknownShift {
                id: shift_id.to_owned(),
            })?;

        if let Some(existing) = shift
            .required_positions
            .iter_mut()
            .find(|requirement| requirement.position_id == position_id)
        {
            existing.headcount = headcount;
        } else {
            shift.required_positions.push(StaffingRequirement {
                position_id: position_id.to_owned(),
                headcount,
            });
        }

        Ok(())
    }

    /// Verify IDs are unique and every stored reference resolves.
    ///
    /// This is mainly a guard for hand-edited project files; constructors
    /// already maintain these invariants.
    pub fn validate(&self) -> Result<(), DomainError> {
        let mut position_ids = HashSet::new();
        for position in &self.positions {
            if !position_ids.insert(position.id.as_str()) {
                return Err(DomainError::DuplicateId {
                    kind: "position",
                    id: position.id.clone(),
                });
            }
        }

        let mut employee_ids = HashSet::new();
        for employee in &self.employees {
            if !employee_ids.insert(employee.id.as_str()) {
                return Err(DomainError::DuplicateId {
                    kind: "employee",
                    id: employee.id.clone(),
                });
            }
        }

        let mut roster_ids = HashSet::new();
        for roster in &self.rosters {
            if !roster_ids.insert(roster.id.as_str()) {
                return Err(DomainError::DuplicateId {
                    kind: "roster",
                    id: roster.id.clone(),
                });
            }

            let mut shift_ids = HashSet::new();
            for shift in &roster.shifts {
                if !shift_ids.insert(shift.id.as_str()) {
                    return Err(DomainError::DuplicateId {
                        kind: "shift",
                        id: shift.id.clone(),
                    });
                }
            }
        }

        for employee in &self.employees {
            if self.roster(&employee.roster_id).is_none() {
                return Err(DomainError::DanglingReference {
                    kind: "employee roster",
                    id: employee.roster_id.clone(),
                });
            }
            if self.position(&employee.position_id).is_none() {
                return Err(DomainError::DanglingReference {
                    kind: "employee position",
                    id: employee.position_id.clone(),
                });
            }
        }

        for roster in &self.rosters {
            for shift in &roster.shifts {
                for requirement in &shift.required_positions {
                    if self.position(&requirement.position_id).is_none() {
                        return Err(DomainError::DanglingReference {
                            kind: "shift position",
                            id: requirement.position_id.clone(),
                        });
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(hour: u32, minute: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(hour, minute, 0).unwrap()
    }

    fn organization_with_rosters() -> (Organization, String, String) {
        let mut organization = Organization::new("City Hospital");
        let doctors = organization.add_roster("Doctors").unwrap().id.clone();
        let nurses = organization.add_roster("Nurses").unwrap().id.clone();
        (organization, doctors, nurses)
    }

    #[test]
    fn rejects_blank_and_duplicate_rosters_and_positions() {
        let (mut organization, _, _) = organization_with_rosters();

        assert_eq!(
            organization.add_roster("  Doctors "),
            Err(DomainError::DuplicateRoster {
                name: "Doctors".to_owned()
            })
        );
        assert_eq!(organization.add_roster("   "), Err(DomainError::BlankName));

        organization.add_position("Medical Officer").unwrap();
        assert_eq!(
            organization.add_position(" Medical Officer "),
            Err(DomainError::DuplicatePosition {
                name: "Medical Officer".to_owned()
            })
        );
        assert_eq!(
            organization.add_position("   "),
            Err(DomainError::BlankName)
        );
    }

    #[test]
    fn requires_known_roster_and_position_for_employees() {
        let (mut organization, doctors, _) = organization_with_rosters();
        let position = organization
            .add_position("Medical Officer")
            .unwrap()
            .id
            .clone();

        assert!(
            organization
                .add_employee("Dr. Amit", &doctors, &position)
                .is_ok()
        );

        assert!(matches!(
            organization.add_employee("Dr. Ghost", "missing-roster", &position),
            Err(DomainError::UnknownRoster { .. })
        ));
        assert!(matches!(
            organization.add_employee("Dr. Ghost", &doctors, "missing-position"),
            Err(DomainError::UnknownPosition { .. })
        ));
        assert!(matches!(
            organization.add_employee("   ", &doctors, &position),
            Err(DomainError::BlankName)
        ));
    }

    #[test]
    fn employees_stay_inside_their_roster() {
        let (mut organization, doctors, nurses) = organization_with_rosters();
        let position = organization
            .add_position("Medical Officer")
            .unwrap()
            .id
            .clone();
        let nurse_position = organization
            .add_position("Senior Staff Nurse")
            .unwrap()
            .id
            .clone();

        organization
            .add_employee("Dr. Amit", &doctors, &position)
            .unwrap();
        organization
            .add_employee("Nurse Rina", &nurses, &nurse_position)
            .unwrap();

        let doctors_staff: Vec<&str> = organization
            .employees_in_roster(&doctors)
            .iter()
            .map(|employee| employee.name.as_str())
            .collect();
        let nurses_staff: Vec<&str> = organization
            .employees_in_roster(&nurses)
            .iter()
            .map(|employee| employee.name.as_str())
            .collect();

        assert_eq!(doctors_staff, vec!["Dr. Amit"]);
        assert_eq!(nurses_staff, vec!["Nurse Rina"]);
        assert!(
            organization
                .employees_in_roster("missing-roster")
                .is_empty()
        );
    }

    #[test]
    fn adds_shifts_only_to_known_rosters_and_rejects_duplicate_names() {
        let (mut organization, doctors, _) = organization_with_rosters();

        organization
            .add_shift(&doctors, "OPD", at(8, 0), at(14, 0))
            .unwrap();

        assert!(matches!(
            organization.add_shift("missing-roster", "OPD", at(8, 0), at(14, 0)),
            Err(DomainError::UnknownRoster { .. })
        ));
        assert!(matches!(
            organization.add_shift(&doctors, "  OPD ", at(8, 0), at(14, 0)),
            Err(DomainError::DuplicateShift { .. })
        ));
    }

    #[test]
    fn staffing_requirements_upsert_and_validate() {
        let (mut organization, doctors, _) = organization_with_rosters();
        let medical_officer = organization
            .add_position("Medical Officer")
            .unwrap()
            .id
            .clone();
        let nurse = organization
            .add_position("Senior Staff Nurse")
            .unwrap()
            .id
            .clone();
        let shift = organization
            .add_shift(&doctors, "OPD", at(8, 0), at(14, 0))
            .unwrap()
            .id
            .clone();

        organization
            .require_position(&doctors, &shift, &medical_officer, 2)
            .unwrap();
        organization
            .require_position(&doctors, &shift, &nurse, 1)
            .unwrap();
        organization
            .require_position(&doctors, &shift, &medical_officer, 3)
            .unwrap();

        let requirements = &organization
            .roster(&doctors)
            .unwrap()
            .shift(&shift)
            .unwrap()
            .required_positions;
        assert_eq!(requirements.len(), 2);
        assert_eq!(
            requirements
                .iter()
                .find(|requirement| requirement.position_id == medical_officer)
                .unwrap()
                .headcount,
            3
        );

        assert!(matches!(
            organization.require_position(&doctors, &shift, &medical_officer, 0),
            Err(DomainError::EmptyHeadcount { .. })
        ));
        assert!(matches!(
            organization.require_position(&doctors, &shift, "missing-position", 1),
            Err(DomainError::UnknownPosition { .. })
        ));
        assert!(matches!(
            organization.require_position(&doctors, "missing-shift", &medical_officer, 1),
            Err(DomainError::UnknownShift { .. })
        ));
    }

    #[test]
    fn validates_a_consistent_organization() {
        let (mut organization, doctors, _) = organization_with_rosters();
        let position = organization
            .add_position("Medical Officer")
            .unwrap()
            .id
            .clone();
        organization
            .add_employee("Dr. Amit", &doctors, &position)
            .unwrap();
        let shift = organization
            .add_shift(&doctors, "OPD", at(8, 0), at(14, 0))
            .unwrap()
            .id
            .clone();
        organization
            .require_position(&doctors, &shift, &position, 1)
            .unwrap();

        assert_eq!(organization.validate(), Ok(()));
    }

    #[test]
    fn validate_catches_dangling_references_and_duplicate_ids() {
        let mut organization = Organization::new("City Hospital");
        organization.employees.push(Employee {
            id: "employee-1".to_owned(),
            name: "Dr. Ghost".to_owned(),
            roster_id: "missing-roster".to_owned(),
            position_id: "missing-position".to_owned(),
            skills: Vec::new(),
        });
        assert!(matches!(
            organization.validate(),
            Err(DomainError::DanglingReference {
                kind: "employee roster",
                ..
            })
        ));

        let mut organization = Organization::new("City Hospital");
        organization.positions.push(Position {
            id: "same".to_owned(),
            name: "One".to_owned(),
        });
        organization.positions.push(Position {
            id: "same".to_owned(),
            name: "Two".to_owned(),
        });
        assert!(matches!(
            organization.validate(),
            Err(DomainError::DuplicateId {
                kind: "position",
                ..
            })
        ));
    }
}

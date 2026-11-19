//! Roster templates: reusable staffing definitions that own shifts and dated
//! rosters.

use chrono::{NaiveDate, NaiveTime, Weekday};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{
    ContextAssignment, DomainError, Employee, RestRule, Roster, RosterMember, Severity,
    SharingGroup, ShiftTemplate, StaffingRequirement,
};

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

    /// Include a position so its employees join the template. Idempotent.
    pub fn include_position(&mut self, position_id: &str) {
        if !self.includes_position(position_id) {
            self.position_ids.push(position_id.to_owned());
        }
    }

    /// Remove a position from the template.
    ///
    /// A shift that still requires the position blocks the removal, so the
    /// template never stores a requirement for a position it does not include.
    pub fn remove_position(&mut self, position_id: &str) -> Result<(), DomainError> {
        if self.shifts.iter().any(|shift| {
            shift
                .required_positions
                .iter()
                .any(|requirement| requirement.position_id == position_id)
        }) {
            return Err(DomainError::DanglingReference {
                kind: "shift requirement position",
                id: position_id.to_owned(),
            });
        }

        self.position_ids
            .retain(|candidate| candidate != position_id);
        Ok(())
    }

    /// Exclude an employee from the template. Idempotent.
    ///
    /// The employee must hold a position the template includes.
    pub fn exclude_employee(&mut self, employee: &Employee) -> Result<(), DomainError> {
        if !self.includes_position(&employee.position_id) {
            return Err(DomainError::PositionNotInTemplate {
                position_id: employee.position_id.clone(),
                template_id: self.id.clone(),
            });
        }

        if !self.excludes_employee(&employee.id) {
            self.excluded_employee_ids.push(employee.id.clone());
        }
        Ok(())
    }

    /// Re-include an employee who was excluded from the template.
    pub fn include_employee(&mut self, employee_id: &str) {
        self.excluded_employee_ids
            .retain(|candidate| candidate != employee_id);
    }

    /// Add a shift with a unique name.
    pub fn add_shift(
        &mut self,
        name: &str,
        start: NaiveTime,
        end: NaiveTime,
    ) -> Result<&mut ShiftTemplate, DomainError> {
        let name = name.trim();
        if self.shifts.iter().any(|shift| shift.name == name) {
            return Err(DomainError::DuplicateShift {
                name: name.to_owned(),
            });
        }

        self.shifts.push(ShiftTemplate::new(name, start, end)?);
        Ok(self.shifts.last_mut().expect("a shift was just pushed"))
    }

    /// Set (or raise/lower) the headcount a shift needs for a position.
    pub fn require_position(
        &mut self,
        shift_id: &str,
        position_id: &str,
        headcount: u16,
    ) -> Result<(), DomainError> {
        if headcount == 0 {
            return Err(DomainError::EmptyHeadcount {
                shift_id: shift_id.to_owned(),
                position_id: position_id.to_owned(),
            });
        }
        if !self.includes_position(position_id) {
            return Err(DomainError::PositionNotInTemplate {
                position_id: position_id.to_owned(),
                template_id: self.id.clone(),
            });
        }

        let shift = self
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

    /// Change a shift's name and wall-clock times.
    pub fn update_shift(
        &mut self,
        shift_id: &str,
        name: &str,
        start: NaiveTime,
        end: NaiveTime,
    ) -> Result<(), DomainError> {
        let name = name.trim();
        if self.shift(shift_id).is_none() {
            return Err(DomainError::UnknownShift {
                id: shift_id.to_owned(),
            });
        }
        if name.is_empty() {
            return Err(DomainError::BlankName);
        }
        if start == end {
            return Err(DomainError::InvalidShiftTimes { start, end });
        }
        if self
            .shifts
            .iter()
            .any(|shift| shift.id != shift_id && shift.name == name)
        {
            return Err(DomainError::DuplicateShift {
                name: name.to_owned(),
            });
        }

        let shift = self.shift_mut(shift_id).expect("shift was resolved above");
        shift.name = name.to_owned();
        shift.start = start;
        shift.end = end;

        Ok(())
    }

    /// Replace a shift's staffing requirements exactly.
    ///
    /// Every position must be included by the template and have a non-zero
    /// headcount. Duplicate position ids collapse to their last value.
    pub fn set_shift_requirements(
        &mut self,
        shift_id: &str,
        requirements: &[(String, u16)],
    ) -> Result<(), DomainError> {
        if self.shift(shift_id).is_none() {
            return Err(DomainError::UnknownShift {
                id: shift_id.to_owned(),
            });
        }

        let mut merged: Vec<StaffingRequirement> = Vec::with_capacity(requirements.len());
        for (position_id, headcount) in requirements {
            if !self.includes_position(position_id) {
                return Err(DomainError::PositionNotInTemplate {
                    position_id: position_id.to_owned(),
                    template_id: self.id.clone(),
                });
            }
            if *headcount == 0 {
                return Err(DomainError::EmptyHeadcount {
                    shift_id: shift_id.to_owned(),
                    position_id: position_id.to_owned(),
                });
            }

            if let Some(existing) = merged
                .iter_mut()
                .find(|requirement| requirement.position_id == *position_id)
            {
                existing.headcount = *headcount;
            } else {
                merged.push(StaffingRequirement {
                    position_id: position_id.to_owned(),
                    headcount: *headcount,
                });
            }
        }

        let shift = self.shift_mut(shift_id).expect("shift was resolved above");
        shift.required_positions = merged;

        Ok(())
    }

    /// Set which weekdays a shift template occurs on.
    ///
    /// An empty slice resets the template to every day. Duplicates are removed
    /// and the result is sorted Monday-first so rosters and JSON stay stable.
    pub fn set_shift_weekdays(
        &mut self,
        shift_id: &str,
        weekdays: &[Weekday],
    ) -> Result<(), DomainError> {
        let shift = self
            .shift_mut(shift_id)
            .ok_or_else(|| DomainError::UnknownShift {
                id: shift_id.to_owned(),
            })?;

        let mut unique: Vec<Weekday> = Vec::with_capacity(weekdays.len());
        for weekday in weekdays {
            if !unique.contains(weekday) {
                unique.push(*weekday);
            }
        }
        unique.sort_by_key(|weekday| weekday.num_days_from_monday());
        shift.weekdays = unique;

        Ok(())
    }

    /// Find a shift by ID.
    pub fn shift(&self, id: &str) -> Option<&ShiftTemplate> {
        self.shifts.iter().find(|shift| shift.id == id)
    }

    /// Find a shift by ID for mutation.
    pub(crate) fn shift_mut(&mut self, id: &str) -> Option<&mut ShiftTemplate> {
        self.shifts.iter_mut().find(|shift| shift.id == id)
    }

    /// Add a minimum-rest rule between two shifts of this template.
    ///
    /// The shifts may be the same: a night shift followed by the next day's
    /// night shift is a valid rest rule.
    pub fn add_rest_rule(
        &mut self,
        preceding_shift_id: &str,
        following_shift_id: &str,
        min_rest_minutes: u32,
        severity: Severity,
        penalty: u32,
    ) -> Result<&RestRule, DomainError> {
        for shift_id in [preceding_shift_id, following_shift_id] {
            if shift_id.trim().is_empty() {
                return Err(DomainError::BlankName);
            }
            if self.shift(shift_id).is_none() {
                return Err(DomainError::ShiftOutsideTemplate {
                    shift_id: shift_id.to_owned(),
                    template_id: self.id.clone(),
                });
            }
        }

        let rule = RestRule::new(
            preceding_shift_id,
            following_shift_id,
            min_rest_minutes,
            severity,
            penalty,
        )?;
        self.rest_rules.push(rule);
        Ok(self.rest_rules.last().expect("a rest rule was just pushed"))
    }

    /// Find a rest rule by ID.
    pub fn rest_rule(&self, id: &str) -> Option<&RestRule> {
        self.rest_rules.iter().find(|rule| rule.id == id)
    }

    /// Add a sharing group.
    ///
    /// The name is trimmed and must be unique within the template. Duplicate
    /// member IDs are collapsed and empty membership is rejected.
    pub fn add_sharing_group(
        &mut self,
        name: &str,
        members: &[&str],
    ) -> Result<&SharingGroup, DomainError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(DomainError::BlankName);
        }
        if self.sharing_groups.iter().any(|group| group.name == name) {
            return Err(DomainError::DuplicateSharingGroup {
                name: name.to_owned(),
            });
        }

        for employee_id in members {
            let employee_id = employee_id.trim();
            if employee_id.is_empty() {
                return Err(DomainError::BlankName);
            }
            if let Some(existing) = self.sharing_group_for_employee(employee_id) {
                return Err(DomainError::EmployeeAlreadyGrouped {
                    employee_id: employee_id.to_owned(),
                    group_id: existing.id.clone(),
                });
            }
        }

        let group = SharingGroup::new(name, members)?;
        self.sharing_groups.push(group);
        Ok(self
            .sharing_groups
            .last()
            .expect("a sharing group was just pushed"))
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

    /// Create a dated roster for an inclusive date range.
    ///
    /// The roster snapshots the passed members and expands this template's
    /// weekday-recurring shifts into dated shifts with one slot per required
    /// seat. It starts in [`PlanStatus::Draft`](super::PlanStatus).
    pub fn create_roster(
        &mut self,
        start: NaiveDate,
        end: NaiveDate,
        context: Vec<ContextAssignment>,
        members: Vec<RosterMember>,
    ) -> Result<&Roster, DomainError> {
        if end < start {
            return Err(DomainError::InvalidDateRange { start, end });
        }

        for assignment in &context {
            if !members
                .iter()
                .any(|member| member.employee_id == assignment.employee_id)
            {
                return Err(DomainError::EmployeeNotInRoster {
                    employee_id: assignment.employee_id.clone(),
                    roster_id: self.id.clone(),
                });
            }
            if assignment.date >= start {
                return Err(DomainError::ContextDateOutOfRange {
                    date: assignment.date,
                    roster_start: start,
                });
            }
            if let Some(shift_id) = &assignment.template_id
                && self.shift(shift_id).is_none()
            {
                return Err(DomainError::ShiftOutsideTemplate {
                    shift_id: shift_id.clone(),
                    template_id: self.id.clone(),
                });
            }
        }

        let name = format!("{} — {start} to {end}", self.name);
        let roster = Roster::create(self, name, start, end, members, context)?;
        self.rosters.push(roster);
        Ok(self.rosters.last().expect("a roster was just pushed"))
    }

    /// Find a dated roster by ID.
    pub fn roster(&self, id: &str) -> Option<&Roster> {
        self.rosters.iter().find(|roster| roster.id == id)
    }

    /// Find a dated roster by ID for mutation.
    pub(crate) fn roster_mut(&mut self, id: &str) -> Option<&mut Roster> {
        self.rosters.iter_mut().find(|roster| roster.id == id)
    }
}

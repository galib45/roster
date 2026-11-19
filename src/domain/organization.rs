//! Organizations: the root of the domain graph.
//!
//! An organization owns its positions, its employees (each holding exactly one
//! position), and its roster templates. Each template selects the positions it
//! covers, may exclude individual staff, and owns shifts, rest rules, sharing
//! groups, and dated rosters. Cross-entity validation lives here because no
//! child can see the whole graph.

use std::collections::HashSet;

use chrono::{NaiveDate, NaiveTime, Utc, Weekday};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{
    ContextAssignment, DomainError, Employee, PlanStatus, Position, RecurringAssignment, RestRule,
    Roster, RosterMember, RosterTemplate, Severity, SharingGroup, ShiftTemplate,
    StaffingRequirement, Unavailability, UnavailabilityScope,
};

/// A hospital, clinic, or department that owns staff and roster templates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Organization {
    /// Stable identifier, independent of the display name.
    pub id: String,
    pub name: String,
    /// Organization-level positions referenced by employees and shifts.
    #[serde(default)]
    pub positions: Vec<Position>,
    /// Organization-level staff, each holding exactly one position.
    #[serde(default)]
    pub employees: Vec<Employee>,
    /// Reusable roster definitions, each owning shifts and dated rosters.
    #[serde(default)]
    pub roster_templates: Vec<RosterTemplate>,
}

impl Organization {
    /// Create an empty organization.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            positions: Vec::new(),
            employees: Vec::new(),
            roster_templates: Vec::new(),
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

    /// Find a roster template by ID.
    pub fn roster_template(&self, id: &str) -> Option<&RosterTemplate> {
        self.roster_templates
            .iter()
            .find(|template| template.id == id)
    }

    /// Find a roster template by ID for mutation.
    pub fn roster_template_mut(&mut self, id: &str) -> Option<&mut RosterTemplate> {
        self.roster_templates
            .iter_mut()
            .find(|template| template.id == id)
    }

    /// Find a shift template anywhere in the organization.
    ///
    /// Shift IDs are globally unique, so availability and recurring assignment
    /// rules can reference a shift from any template.
    pub fn shift_template(&self, shift_id: &str) -> Option<&ShiftTemplate> {
        self.roster_templates
            .iter()
            .find_map(|template| template.shift(shift_id))
    }

    /// The employees a template currently gathers.
    ///
    /// Membership is computed: every employee whose position the template
    /// includes, minus explicit exclusions. An unknown template yields an
    /// empty list.
    pub fn template_members(&self, template_id: &str) -> Vec<&Employee> {
        let Some(template) = self.roster_template(template_id) else {
            return Vec::new();
        };

        self.employees
            .iter()
            .filter(|employee| {
                template.includes_position(&employee.position_id)
                    && !template.excludes_employee(&employee.id)
            })
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

    /// Add a roster template.
    pub fn add_roster_template(&mut self, name: &str) -> Result<&RosterTemplate, DomainError> {
        let name = name.trim();
        if self
            .roster_templates
            .iter()
            .any(|template| template.name == name)
        {
            return Err(DomainError::DuplicateRosterTemplate {
                name: name.to_owned(),
            });
        }

        self.roster_templates.push(RosterTemplate::new(name)?);
        Ok(self
            .roster_templates
            .last()
            .expect("a roster template was just pushed"))
    }

    /// Include a position in a template, so its employees join automatically.
    ///
    /// Idempotent: including a position twice is accepted.
    pub fn include_position(
        &mut self,
        template_id: &str,
        position_id: &str,
    ) -> Result<(), DomainError> {
        if self.position(position_id).is_none() {
            return Err(DomainError::UnknownPosition {
                id: position_id.to_owned(),
            });
        }

        let template =
            self.roster_template_mut(template_id)
                .ok_or_else(|| DomainError::UnknownRosterTemplate {
                    id: template_id.to_owned(),
                })?;
        if !template.includes_position(position_id) {
            template.position_ids.push(position_id.to_owned());
        }

        Ok(())
    }

    /// Remove a position from a template.
    ///
    /// Staff holding that position stop being members. Exclusions for those
    /// staff are pruned, and a shift that still requires the position blocks
    /// the removal so the template never stores an invalid requirement.
    pub fn remove_position(
        &mut self,
        template_id: &str,
        position_id: &str,
    ) -> Result<(), DomainError> {
        let excluded_of_position: Vec<String> = {
            let template =
                self.roster_template(template_id)
                    .ok_or_else(|| DomainError::UnknownRosterTemplate {
                        id: template_id.to_owned(),
                    })?;
            if template.shifts.iter().any(|shift| {
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

            self.employees
                .iter()
                .filter(|employee| {
                    employee.position_id == position_id
                        && template.excludes_employee(&employee.id)
                })
                .map(|employee| employee.id.clone())
                .collect()
        };

        let template =
            self.roster_template_mut(template_id)
                .expect("template was resolved above");
        template
            .position_ids
            .retain(|candidate| candidate != position_id);
        template
            .excluded_employee_ids
            .retain(|employee_id| !excluded_of_position.contains(employee_id));

        Ok(())
    }

    /// Exclude an employee from a template.
    ///
    /// The employee must hold a position the template includes. Idempotent.
    pub fn exclude_employee(
        &mut self,
        template_id: &str,
        employee_id: &str,
    ) -> Result<(), DomainError> {
        let employee = self
            .employee(employee_id)
            .ok_or_else(|| DomainError::UnknownEmployee {
                id: employee_id.to_owned(),
            })?;
        let position_id = employee.position_id.clone();

        let template =
            self.roster_template(template_id)
                .ok_or_else(|| DomainError::UnknownRosterTemplate {
                    id: template_id.to_owned(),
                })?;
        if !template.includes_position(&position_id) {
            return Err(DomainError::PositionNotInTemplate {
                position_id,
                template_id: template_id.to_owned(),
            });
        }

        let template =
            self.roster_template_mut(template_id)
                .expect("template was resolved above");
        if !template.excludes_employee(employee_id) {
            template.excluded_employee_ids.push(employee_id.to_owned());
        }

        Ok(())
    }

    /// Re-include an employee who was excluded from a template.
    pub fn include_employee(
        &mut self,
        template_id: &str,
        employee_id: &str,
    ) -> Result<(), DomainError> {
        if self.employee(employee_id).is_none() {
            return Err(DomainError::UnknownEmployee {
                id: employee_id.to_owned(),
            });
        }

        let template =
            self.roster_template_mut(template_id)
                .ok_or_else(|| DomainError::UnknownRosterTemplate {
                    id: template_id.to_owned(),
                })?;
        template
            .excluded_employee_ids
            .retain(|candidate| candidate != employee_id);

        Ok(())
    }

    /// Add an employee holding an organization position.
    pub fn add_employee(
        &mut self,
        name: &str,
        position_id: &str,
    ) -> Result<&Employee, DomainError> {
        if self.position(position_id).is_none() {
            return Err(DomainError::UnknownPosition {
                id: position_id.to_owned(),
            });
        }

        self.employees.push(Employee::new(name, position_id)?);
        Ok(self.employees.last().expect("an employee was just pushed"))
    }

    /// Add a shift to a roster template.
    pub fn add_shift(
        &mut self,
        template_id: &str,
        name: &str,
        start: NaiveTime,
        end: NaiveTime,
    ) -> Result<&mut ShiftTemplate, DomainError> {
        let name = name.trim();
        let template =
            self.roster_template_mut(template_id)
                .ok_or_else(|| DomainError::UnknownRosterTemplate {
                    id: template_id.to_owned(),
                })?;

        if template.shifts.iter().any(|shift| shift.name == name) {
            return Err(DomainError::DuplicateShift {
                name: name.to_owned(),
            });
        }

        template.push_shift(ShiftTemplate::new(name, start, end)?);
        Ok(template.shifts.last_mut().expect("a shift was just pushed"))
    }

    /// Set (or raise/lower) the headcount a shift needs for a position.
    pub fn require_position(
        &mut self,
        template_id: &str,
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

        let template =
            self.roster_template_mut(template_id)
                .ok_or_else(|| DomainError::UnknownRosterTemplate {
                    id: template_id.to_owned(),
                })?;
        if !template.includes_position(position_id) {
            return Err(DomainError::PositionNotInTemplate {
                position_id: position_id.to_owned(),
                template_id: template_id.to_owned(),
            });
        }
        let shift = template
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
    ///
    /// The shift stays in its template. Blank names, equal start/end times, and
    /// names that collide with another shift in the same template are rejected.
    pub fn update_shift(
        &mut self,
        template_id: &str,
        shift_id: &str,
        name: &str,
        start: NaiveTime,
        end: NaiveTime,
    ) -> Result<(), DomainError> {
        let name = name.trim();
        let template =
            self.roster_template(template_id)
                .ok_or_else(|| DomainError::UnknownRosterTemplate {
                    id: template_id.to_owned(),
                })?;
        if template.shift(shift_id).is_none() {
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
        if template
            .shifts
            .iter()
            .any(|shift| shift.id != shift_id && shift.name == name)
        {
            return Err(DomainError::DuplicateShift {
                name: name.to_owned(),
            });
        }

        let shift = self
            .roster_template_mut(template_id)
            .expect("template was resolved above")
            .shift_mut(shift_id)
            .expect("shift was resolved above");
        shift.name = name.to_owned();
        shift.start = start;
        shift.end = end;

        Ok(())
    }

    /// Replace a shift's staffing requirements exactly.
    ///
    /// Every position must exist organization-wide, be included by the
    /// template, and have a non-zero headcount. Duplicate position ids collapse
    /// to their last value.
    pub fn set_shift_requirements(
        &mut self,
        template_id: &str,
        shift_id: &str,
        requirements: &[(String, u16)],
    ) -> Result<(), DomainError> {
        let template =
            self.roster_template(template_id)
                .ok_or_else(|| DomainError::UnknownRosterTemplate {
                    id: template_id.to_owned(),
                })?;
        if template.shift(shift_id).is_none() {
            return Err(DomainError::UnknownShift {
                id: shift_id.to_owned(),
            });
        }

        let mut merged: Vec<StaffingRequirement> = Vec::with_capacity(requirements.len());
        for (position_id, headcount) in requirements {
            if self.position(position_id).is_none() {
                return Err(DomainError::UnknownPosition {
                    id: position_id.to_owned(),
                });
            }
            if !template.includes_position(position_id) {
                return Err(DomainError::PositionNotInTemplate {
                    position_id: position_id.to_owned(),
                    template_id: template_id.to_owned(),
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

        let shift = self
            .roster_template_mut(template_id)
            .expect("template was resolved above")
            .shift_mut(shift_id)
            .expect("shift was resolved above");
        shift.required_positions = merged;

        Ok(())
    }

    /// Add an unavailability or leave rule to an employee.
    ///
    /// Every shift id must name a shift template anywhere in the organization;
    /// an empty list means every shift. Local validation (date range and
    /// severity/penalty pairing) happens in [`Unavailability::new`].
    pub fn add_unavailability(
        &mut self,
        employee_id: &str,
        scope: UnavailabilityScope,
        shifts: &[&str],
        severity: Severity,
        penalty: u32,
    ) -> Result<&Unavailability, DomainError> {
        if self.employee(employee_id).is_none() {
            return Err(DomainError::UnknownEmployee {
                id: employee_id.to_owned(),
            });
        }

        let mut shift_ids = Vec::with_capacity(shifts.len());
        for shift_id in shifts {
            if shift_id.trim().is_empty() {
                return Err(DomainError::BlankName);
            }
            if self.shift_template(shift_id).is_none() {
                return Err(DomainError::UnknownShift {
                    id: (*shift_id).to_owned(),
                });
            }
            shift_ids.push((*shift_id).to_owned());
        }

        let rule = Unavailability::new(scope, shift_ids, severity, penalty)?;
        let employee = self
            .employee_mut(employee_id)
            .expect("employee was resolved above");
        employee.unavailability.push(rule);

        Ok(employee
            .unavailability
            .last()
            .expect("an unavailability rule was just pushed"))
    }

    /// Add a weekly recurring preference to an employee.
    ///
    /// The shift must exist anywhere in the organization. Recurring assignments
    /// are **soft preferences**: an unmet one costs the default penalty of 1
    /// but never makes a roster infeasible. Use
    /// [`Self::add_recurring_assignment_with_penalty`] to tune the cost.
    pub fn add_recurring_assignment(
        &mut self,
        employee_id: &str,
        weekday: Weekday,
        shift_id: &str,
    ) -> Result<&RecurringAssignment, DomainError> {
        self.add_recurring_assignment_with_penalty(employee_id, weekday, shift_id, 1)
    }

    /// Add a weekly recurring preference with an explicit penalty.
    ///
    /// A penalty of 0 is rejected: recurring assignments stay soft and must cost
    /// something when unmet. Fairness may still outrank them.
    pub fn add_recurring_assignment_with_penalty(
        &mut self,
        employee_id: &str,
        weekday: Weekday,
        shift_id: &str,
        penalty: u32,
    ) -> Result<&RecurringAssignment, DomainError> {
        if self.employee(employee_id).is_none() {
            return Err(DomainError::UnknownEmployee {
                id: employee_id.to_owned(),
            });
        }
        if shift_id.trim().is_empty() {
            return Err(DomainError::BlankName);
        }
        if self.shift_template(shift_id).is_none() {
            return Err(DomainError::UnknownShift {
                id: shift_id.to_owned(),
            });
        }

        let assignment = RecurringAssignment::with_penalty(weekday, shift_id, penalty)?;
        let employee = self
            .employee_mut(employee_id)
            .expect("employee was resolved above");
        employee.recurring_assignments.push(assignment);

        Ok(employee
            .recurring_assignments
            .last()
            .expect("a recurring assignment was just pushed"))
    }

    /// Add a minimum-rest rule between two shifts of one template.
    ///
    /// The shifts may be the same template: a night shift followed by the next
    /// day's night shift is a valid rest rule.
    pub fn add_rest_rule(
        &mut self,
        template_id: &str,
        preceding_shift_id: &str,
        following_shift_id: &str,
        min_rest_minutes: u32,
        severity: Severity,
        penalty: u32,
    ) -> Result<&RestRule, DomainError> {
        let template =
            self.roster_template(template_id)
                .ok_or_else(|| DomainError::UnknownRosterTemplate {
                    id: template_id.to_owned(),
                })?;

        for shift_id in [preceding_shift_id, following_shift_id] {
            if shift_id.trim().is_empty() {
                return Err(DomainError::BlankName);
            }
            if template.shift(shift_id).is_none() {
                return Err(DomainError::ShiftOutsideTemplate {
                    shift_id: shift_id.to_owned(),
                    template_id: template_id.to_owned(),
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
        let template =
            self.roster_template_mut(template_id)
                .expect("template was resolved above");
        template.push_rest_rule(rule);

        Ok(template
            .rest_rules
            .last()
            .expect("a rest rule was just pushed"))
    }

    /// Add a sharing group to a template.
    ///
    /// The name is trimmed and must be unique within the template. Every member
    /// must be a current template member who does not already belong to another
    /// group. Duplicate member IDs are collapsed and empty membership is
    /// rejected.
    pub fn add_sharing_group(
        &mut self,
        template_id: &str,
        name: &str,
        members: &[&str],
    ) -> Result<&SharingGroup, DomainError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(DomainError::BlankName);
        }

        let template =
            self.roster_template(template_id)
                .ok_or_else(|| DomainError::UnknownRosterTemplate {
                    id: template_id.to_owned(),
                })?;
        if template.sharing_groups.iter().any(|group| group.name == name) {
            return Err(DomainError::DuplicateSharingGroup {
                name: name.to_owned(),
            });
        }

        for employee_id in members {
            let employee_id = employee_id.trim();
            if employee_id.is_empty() {
                return Err(DomainError::BlankName);
            }

            let employee =
                self.employee(employee_id)
                    .ok_or_else(|| DomainError::UnknownEmployee {
                        id: employee_id.to_owned(),
                    })?;
            if !template.includes_position(&employee.position_id)
                || template.excludes_employee(employee_id)
            {
                return Err(DomainError::EmployeeNotInTemplate {
                    employee_id: employee_id.to_owned(),
                    template_id: template_id.to_owned(),
                });
            }
            if let Some(existing) = template.sharing_group_for_employee(employee_id) {
                return Err(DomainError::EmployeeAlreadyGrouped {
                    employee_id: employee_id.to_owned(),
                    group_id: existing.id.clone(),
                });
            }
        }

        let group = SharingGroup::new(name, members)?;
        let template =
            self.roster_template_mut(template_id)
                .expect("template was resolved above");
        template.push_sharing_group(group);

        Ok(template
            .sharing_groups
            .last()
            .expect("a sharing group was just pushed"))
    }

    /// Set which weekdays a shift template occurs on.
    ///
    /// An empty slice resets the template to every day. Duplicates are removed
    /// and the result is sorted Monday-first so rosters and JSON stay stable.
    pub fn set_shift_weekdays(
        &mut self,
        template_id: &str,
        shift_id: &str,
        weekdays: &[Weekday],
    ) -> Result<(), DomainError> {
        let template =
            self.roster_template_mut(template_id)
                .ok_or_else(|| DomainError::UnknownRosterTemplate {
                    id: template_id.to_owned(),
                })?;
        let shift = template
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

    /// Create a dated roster from a template for an inclusive date range.
    ///
    /// The roster snapshots the template's current members (with their
    /// positions) and expands weekday-recurring shifts into dated shifts with
    /// one slot per required seat. It starts in [`PlanStatus::Draft`].
    ///
    /// Context employees must be snapshot members and their assignments must
    /// fall before the roster start. Every shift-requirement position must be
    /// included by the template and resolve organization-wide.
    pub fn create_roster(
        &mut self,
        template_id: &str,
        start: NaiveDate,
        end: NaiveDate,
        context: Vec<ContextAssignment>,
    ) -> Result<&Roster, DomainError> {
        let template =
            self.roster_template(template_id)
                .ok_or_else(|| DomainError::UnknownRosterTemplate {
                    id: template_id.to_owned(),
                })?;

        if end < start {
            return Err(DomainError::InvalidDateRange { start, end });
        }

        let members: Vec<RosterMember> = self
            .template_members(template_id)
            .into_iter()
            .map(|employee| RosterMember::new(&employee.id, &employee.position_id))
            .collect();

        for assignment in &context {
            if !members
                .iter()
                .any(|member| member.employee_id == assignment.employee_id)
            {
                return Err(DomainError::EmployeeNotInRoster {
                    employee_id: assignment.employee_id.clone(),
                    roster_id: template_id.to_owned(),
                });
            }
            if assignment.date >= start {
                return Err(DomainError::ContextDateOutOfRange {
                    date: assignment.date,
                    roster_start: start,
                });
            }
            if let Some(shift_id) = &assignment.template_id
                && template.shift(shift_id).is_none()
            {
                return Err(DomainError::ShiftOutsideTemplate {
                    shift_id: shift_id.clone(),
                    template_id: template_id.to_owned(),
                });
            }
        }

        for shift in &template.shifts {
            for requirement in &shift.required_positions {
                if self.position(&requirement.position_id).is_none() {
                    return Err(DomainError::DanglingReference {
                        kind: "shift position",
                        id: requirement.position_id.clone(),
                    });
                }
                if !template.includes_position(&requirement.position_id) {
                    return Err(DomainError::PositionNotInTemplate {
                        position_id: requirement.position_id.clone(),
                        template_id: template_id.to_owned(),
                    });
                }
            }
        }

        let name = format!("{} — {start} to {end}", template.name);
        let roster = Roster::create(template, name, start, end, members, context)?;
        let template =
            self.roster_template_mut(template_id)
                .expect("template was resolved above");
        template.push_roster(roster);

        Ok(template.rosters.last().expect("a roster was just pushed"))
    }

    /// Find a dated roster by template and roster IDs.
    pub fn roster(&self, template_id: &str, roster_id: &str) -> Option<&Roster> {
        self.roster_template(template_id)?.roster(roster_id)
    }

    /// Find a dated roster by template and roster IDs for mutation.
    pub fn roster_mut(&mut self, template_id: &str, roster_id: &str) -> Option<&mut Roster> {
        self.roster_template_mut(template_id)?.roster_mut(roster_id)
    }

    /// Find a dated roster anywhere in the organization.
    pub fn dated_roster(&self, roster_id: &str) -> Option<&Roster> {
        self.roster_templates
            .iter()
            .find_map(|template| template.roster(roster_id))
    }

    /// Find a dated roster anywhere in the organization for mutation.
    pub fn dated_roster_mut(&mut self, roster_id: &str) -> Option<&mut Roster> {
        self.roster_templates
            .iter_mut()
            .find_map(|template| template.roster_mut(roster_id))
    }

    /// The template that owns a dated roster.
    pub fn template_for_roster(&self, roster_id: &str) -> Option<&RosterTemplate> {
        self.roster_templates
            .iter()
            .find(|template| template.roster(roster_id).is_some())
    }

    /// Assign an employee to one seat of a dated shift.
    ///
    /// Structural rules only: the roster must not be finalized, the employee
    /// must be a snapshot member, the member's snapshot position must match the
    /// slot, and the employee must not already hold another seat in the same
    /// dated shift. Overlap, leave, rest, and fairness are Phase 3.
    ///
    /// Assigning to an already occupied slot replaces the previous employee.
    pub fn assign_slot(
        &mut self,
        template_id: &str,
        roster_id: &str,
        dated_shift_id: &str,
        slot_id: &str,
        employee_id: &str,
    ) -> Result<(), DomainError> {
        if self.employee(employee_id).is_none() {
            return Err(DomainError::UnknownEmployee {
                id: employee_id.to_owned(),
            });
        }

        let (member_position, position_id, already_assigned) = {
            let roster =
                self.roster(template_id, roster_id)
                    .ok_or_else(|| DomainError::UnknownRoster {
                        id: roster_id.to_owned(),
                    })?;
            if roster.status == PlanStatus::Finalized {
                return Err(DomainError::RosterFinalized {
                    roster_id: roster_id.to_owned(),
                });
            }
            let member = roster.member(employee_id).ok_or_else(|| {
                DomainError::EmployeeNotInRoster {
                    employee_id: employee_id.to_owned(),
                    roster_id: roster_id.to_owned(),
                }
            })?;
            let dated = roster.dated_shift(dated_shift_id).ok_or_else(|| {
                DomainError::UnknownDatedShift {
                    id: dated_shift_id.to_owned(),
                }
            })?;
            let slot = dated
                .slot(slot_id)
                .ok_or_else(|| DomainError::UnknownSlot {
                    id: slot_id.to_owned(),
                })?;
            let already_assigned = dated.slots.iter().any(|other| {
                other.id != slot_id && other.employee_id.as_deref() == Some(employee_id)
            });
            (
                member.position_id.clone(),
                slot.position_id.clone(),
                already_assigned,
            )
        };

        if member_position != position_id {
            return Err(DomainError::PositionMismatch {
                employee_id: employee_id.to_owned(),
                expected_position_id: position_id,
                actual_position_id: member_position,
            });
        }
        if already_assigned {
            return Err(DomainError::EmployeeAlreadyAssigned {
                dated_shift_id: dated_shift_id.to_owned(),
                employee_id: employee_id.to_owned(),
            });
        }

        let roster = self
            .roster_mut(template_id, roster_id)
            .expect("roster was resolved above");
        let dated = roster
            .dated_shift_mut(dated_shift_id)
            .expect("dated shift was resolved above");
        let slot = dated.slot_mut(slot_id).expect("slot was resolved above");
        slot.employee_id = Some(employee_id.to_owned());
        roster.updated_at = Utc::now();

        Ok(())
    }

    /// Clear an assignment from one seat.
    ///
    /// Finalized rosters are immutable.
    pub fn unassign_slot(
        &mut self,
        template_id: &str,
        roster_id: &str,
        dated_shift_id: &str,
        slot_id: &str,
    ) -> Result<(), DomainError> {
        let roster =
            self.roster(template_id, roster_id)
                .ok_or_else(|| DomainError::UnknownRoster {
                    id: roster_id.to_owned(),
                })?;
        if roster.status == PlanStatus::Finalized {
            return Err(DomainError::RosterFinalized {
                roster_id: roster_id.to_owned(),
            });
        }
        roster
            .dated_shift(dated_shift_id)
            .ok_or_else(|| DomainError::UnknownDatedShift {
                id: dated_shift_id.to_owned(),
            })?
            .slot(slot_id)
            .ok_or_else(|| DomainError::UnknownSlot {
                id: slot_id.to_owned(),
            })?;

        let roster = self
            .roster_mut(template_id, roster_id)
            .expect("roster was resolved above");
        let dated = roster
            .dated_shift_mut(dated_shift_id)
            .expect("dated shift was resolved above");
        let slot = dated.slot_mut(slot_id).expect("slot was resolved above");
        slot.employee_id = None;
        roster.updated_at = Utc::now();

        Ok(())
    }

    /// Verify IDs are unique and every stored reference resolves.
    ///
    /// This is mainly a guard for hand-edited project files; constructors
    /// already maintain these invariants.
    pub fn validate(&self) -> Result<(), DomainError> {
        let mut position_ids = HashSet::new();
        let mut position_names = HashSet::new();
        for position in &self.positions {
            if !position_ids.insert(position.id.as_str()) {
                return Err(DomainError::DuplicateId {
                    kind: "position",
                    id: position.id.clone(),
                });
            }
            if !position_names.insert(position.name.as_str()) {
                return Err(DomainError::DuplicatePosition {
                    name: position.name.clone(),
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
            if self.position(&employee.position_id).is_none() {
                return Err(DomainError::DanglingReference {
                    kind: "employee position",
                    id: employee.position_id.clone(),
                });
            }

            let mut rule_ids = HashSet::new();
            for rule in &employee.unavailability {
                if !rule_ids.insert(rule.id.as_str()) {
                    return Err(DomainError::DuplicateId {
                        kind: "unavailability",
                        id: rule.id.clone(),
                    });
                }
                for shift_id in &rule.shifts {
                    if self.shift_template(shift_id).is_none() {
                        return Err(DomainError::DanglingReference {
                            kind: "unavailability shift",
                            id: shift_id.clone(),
                        });
                    }
                }
            }

            for assignment in &employee.recurring_assignments {
                if !rule_ids.insert(assignment.id.as_str()) {
                    return Err(DomainError::DuplicateId {
                        kind: "recurring assignment",
                        id: assignment.id.clone(),
                    });
                }
                if self.shift_template(&assignment.shift_id).is_none() {
                    return Err(DomainError::DanglingReference {
                        kind: "recurring assignment shift",
                        id: assignment.shift_id.clone(),
                    });
                }
                if assignment.penalty == 0 {
                    return Err(DomainError::InvalidPenalty {
                        severity: Severity::Soft,
                        penalty: 0,
                    });
                }
            }
        }

        let mut template_ids = HashSet::new();
        let mut template_names = HashSet::new();
        for template in &self.roster_templates {
            if !template_ids.insert(template.id.as_str()) {
                return Err(DomainError::DuplicateId {
                    kind: "roster template",
                    id: template.id.clone(),
                });
            }
            if !template_names.insert(template.name.as_str()) {
                return Err(DomainError::DuplicateRosterTemplate {
                    name: template.name.clone(),
                });
            }

            let mut included_positions = HashSet::new();
            for position_id in &template.position_ids {
                if !included_positions.insert(position_id.as_str()) {
                    return Err(DomainError::DuplicateId {
                        kind: "template position",
                        id: position_id.clone(),
                    });
                }
                if self.position(position_id).is_none() {
                    return Err(DomainError::DanglingReference {
                        kind: "template position",
                        id: position_id.clone(),
                    });
                }
            }

            let mut excluded_ids = HashSet::new();
            for employee_id in &template.excluded_employee_ids {
                if !excluded_ids.insert(employee_id.as_str()) {
                    return Err(DomainError::DuplicateId {
                        kind: "template exclusion",
                        id: employee_id.clone(),
                    });
                }
                let employee =
                    self.employee(employee_id)
                        .ok_or_else(|| DomainError::DanglingReference {
                            kind: "excluded employee",
                            id: employee_id.clone(),
                        })?;
                if !template.includes_position(&employee.position_id) {
                    return Err(DomainError::PositionNotInTemplate {
                        position_id: employee.position_id.clone(),
                        template_id: template.id.clone(),
                    });
                }
            }

            let mut shift_ids = HashSet::new();
            for shift in &template.shifts {
                if !shift_ids.insert(shift.id.as_str()) {
                    return Err(DomainError::DuplicateId {
                        kind: "shift",
                        id: shift.id.clone(),
                    });
                }
                let mut requirement_positions = HashSet::new();
                for requirement in &shift.required_positions {
                    if !requirement_positions.insert(requirement.position_id.as_str()) {
                        return Err(DomainError::DuplicateId {
                            kind: "shift requirement",
                            id: requirement.position_id.clone(),
                        });
                    }
                    if self.position(&requirement.position_id).is_none() {
                        return Err(DomainError::DanglingReference {
                            kind: "shift position",
                            id: requirement.position_id.clone(),
                        });
                    }
                    if !template.includes_position(&requirement.position_id) {
                        return Err(DomainError::PositionNotInTemplate {
                            position_id: requirement.position_id.clone(),
                            template_id: template.id.clone(),
                        });
                    }
                    if requirement.headcount == 0 {
                        return Err(DomainError::EmptyHeadcount {
                            shift_id: shift.id.clone(),
                            position_id: requirement.position_id.clone(),
                        });
                    }
                }
            }

            let mut rest_rule_ids = HashSet::new();
            for rule in &template.rest_rules {
                if !rest_rule_ids.insert(rule.id.as_str()) {
                    return Err(DomainError::DuplicateId {
                        kind: "rest rule",
                        id: rule.id.clone(),
                    });
                }
                for shift_id in [&rule.preceding_shift_id, &rule.following_shift_id] {
                    if template.shift(shift_id).is_none() {
                        return Err(DomainError::ShiftOutsideTemplate {
                            shift_id: shift_id.clone(),
                            template_id: template.id.clone(),
                        });
                    }
                }
            }

            let mut sharing_group_ids = HashSet::new();
            let mut sharing_group_names = HashSet::new();
            let mut grouped_employees = HashSet::new();
            for group in &template.sharing_groups {
                if !sharing_group_ids.insert(group.id.as_str()) {
                    return Err(DomainError::DuplicateId {
                        kind: "sharing group",
                        id: group.id.clone(),
                    });
                }
                if !sharing_group_names.insert(group.name.as_str()) {
                    return Err(DomainError::DuplicateSharingGroup {
                        name: group.name.clone(),
                    });
                }
                if group.employee_ids.is_empty() {
                    return Err(DomainError::EmptySharingGroup {
                        name: group.name.clone(),
                    });
                }

                let mut members = HashSet::new();
                for employee_id in &group.employee_ids {
                    if !members.insert(employee_id.as_str()) {
                        return Err(DomainError::DuplicateId {
                            kind: "sharing group member",
                            id: employee_id.clone(),
                        });
                    }

                    let employee =
                        self.employee(employee_id)
                            .ok_or_else(|| DomainError::DanglingReference {
                                kind: "sharing group employee",
                                id: employee_id.clone(),
                            })?;
                    if !template.includes_position(&employee.position_id)
                        || template.excludes_employee(employee_id)
                    {
                        return Err(DomainError::EmployeeNotInTemplate {
                            employee_id: employee_id.clone(),
                            template_id: template.id.clone(),
                        });
                    }
                    if !grouped_employees.insert(employee_id.as_str()) {
                        return Err(DomainError::EmployeeAlreadyGrouped {
                            employee_id: employee_id.clone(),
                            group_id: group.id.clone(),
                        });
                    }
                }
            }

            let mut roster_ids = HashSet::new();
            for roster in &template.rosters {
                if !roster_ids.insert(roster.id.as_str()) {
                    return Err(DomainError::DuplicateId {
                        kind: "roster",
                        id: roster.id.clone(),
                    });
                }
                if roster.template_id != template.id {
                    return Err(DomainError::DanglingReference {
                        kind: "roster template",
                        id: roster.template_id.clone(),
                    });
                }
                if roster.end < roster.start {
                    return Err(DomainError::InvalidDateRange {
                        start: roster.start,
                        end: roster.end,
                    });
                }

                let mut member_ids = HashSet::new();
                for member in &roster.members {
                    if !member_ids.insert(member.employee_id.as_str()) {
                        return Err(DomainError::DuplicateId {
                            kind: "roster member",
                            id: member.employee_id.clone(),
                        });
                    }
                    let employee = self.employee(&member.employee_id).ok_or_else(|| {
                        DomainError::DanglingReference {
                            kind: "roster member",
                            id: member.employee_id.clone(),
                        }
                    })?;
                    if employee.position_id != member.position_id {
                        return Err(DomainError::PositionMismatch {
                            employee_id: member.employee_id.clone(),
                            expected_position_id: member.position_id.clone(),
                            actual_position_id: employee.position_id.clone(),
                        });
                    }
                    if !template.includes_position(&member.position_id) {
                        return Err(DomainError::PositionNotInTemplate {
                            position_id: member.position_id.clone(),
                            template_id: template.id.clone(),
                        });
                    }
                }

                for context in &roster.context {
                    if !member_ids.contains(context.employee_id.as_str()) {
                        return Err(DomainError::EmployeeNotInRoster {
                            employee_id: context.employee_id.clone(),
                            roster_id: roster.id.clone(),
                        });
                    }
                    if context.date >= roster.start {
                        return Err(DomainError::ContextDateOutOfRange {
                            date: context.date,
                            roster_start: roster.start,
                        });
                    }
                    if let Some(shift_id) = &context.template_id
                        && template.shift(shift_id).is_none()
                    {
                        return Err(DomainError::ShiftOutsideTemplate {
                            shift_id: shift_id.clone(),
                            template_id: template.id.clone(),
                        });
                    }
                }

                let mut dated_ids = HashSet::new();
                for dated in &roster.shifts {
                    if !dated_ids.insert(dated.id.as_str()) {
                        return Err(DomainError::DuplicateId {
                            kind: "dated shift",
                            id: dated.id.clone(),
                        });
                    }

                    let mut slot_ids = HashSet::new();
                    let mut assigned = HashSet::new();
                    for slot in &dated.slots {
                        if !slot_ids.insert(slot.id.as_str()) {
                            return Err(DomainError::DuplicateId {
                                kind: "slot",
                                id: slot.id.clone(),
                            });
                        }
                        if self.position(&slot.position_id).is_none() {
                            return Err(DomainError::DanglingReference {
                                kind: "slot position",
                                id: slot.position_id.clone(),
                            });
                        }

                        if let Some(employee_id) = &slot.employee_id {
                            let member = roster.member(employee_id).ok_or_else(|| {
                                DomainError::EmployeeNotInRoster {
                                    employee_id: employee_id.clone(),
                                    roster_id: roster.id.clone(),
                                }
                            })?;
                            if member.position_id != slot.position_id {
                                return Err(DomainError::PositionMismatch {
                                    employee_id: employee_id.clone(),
                                    expected_position_id: slot.position_id.clone(),
                                    actual_position_id: member.position_id.clone(),
                                });
                            }
                            if !assigned.insert(employee_id.as_str()) {
                                return Err(DomainError::EmployeeAlreadyAssigned {
                                    dated_shift_id: dated.id.clone(),
                                    employee_id: employee_id.clone(),
                                });
                            }
                        }
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

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).unwrap()
    }

    fn organization_with_templates() -> (Organization, String, String) {
        let mut organization = Organization::new("City Hospital");
        let doctors = organization
            .add_roster_template("Doctors")
            .unwrap()
            .id
            .clone();
        let nurses = organization
            .add_roster_template("Nurses")
            .unwrap()
            .id
            .clone();
        (organization, doctors, nurses)
    }

    fn add_position(organization: &mut Organization, name: &str) -> String {
        organization.add_position(name).unwrap().id.clone()
    }

    fn add_employee(
        organization: &mut Organization,
        name: &str,
        position_id: &str,
    ) -> String {
        organization
            .add_employee(name, position_id)
            .unwrap()
            .id
            .clone()
    }

    fn add_shift(
        organization: &mut Organization,
        template_id: &str,
        name: &str,
        start: NaiveTime,
        end: NaiveTime,
    ) -> String {
        organization
            .add_shift(template_id, name, start, end)
            .unwrap()
            .id
            .clone()
    }

    #[test]
    fn rejects_blank_and_duplicate_templates_and_positions() {
        let (mut organization, _, _) = organization_with_templates();

        assert_eq!(
            organization.add_roster_template("  Doctors "),
            Err(DomainError::DuplicateRosterTemplate {
                name: "Doctors".to_owned()
            })
        );
        assert_eq!(
            organization.add_roster_template("   "),
            Err(DomainError::BlankName)
        );

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
    fn requires_a_known_position_for_employees() {
        let (mut organization, _, _) = organization_with_templates();
        let position = add_position(&mut organization, "Medical Officer");

        assert!(organization.add_employee("Dr. Amit", &position).is_ok());
        assert!(matches!(
            organization.add_employee("Dr. Ghost", "missing-position"),
            Err(DomainError::UnknownPosition { .. })
        ));
        assert!(matches!(
            organization.add_employee("   ", &position),
            Err(DomainError::BlankName)
        ));
    }

    #[test]
    fn gathers_template_members_by_position_with_exclusions() {
        let (mut organization, doctors, nurses) = organization_with_templates();
        let medical_officer = add_position(&mut organization, "Medical Officer");
        let staff_nurse = add_position(&mut organization, "Senior Staff Nurse");

        let amit = add_employee(&mut organization, "Dr. Amit", &medical_officer);
        let khushi = add_employee(&mut organization, "Dr. Khushi", &medical_officer);
        let rina = add_employee(&mut organization, "Nurse Rina", &staff_nurse);

        organization
            .include_position(&doctors, &medical_officer)
            .unwrap();
        organization
            .exclude_employee(&doctors, &khushi)
            .unwrap();

        let member_names: Vec<&str> = organization
            .template_members(&doctors)
            .iter()
            .map(|employee| employee.name.as_str())
            .collect();
        assert_eq!(member_names, vec!["Dr. Amit"]);

        // The nurses template gathers only nurses.
        organization
            .include_position(&nurses, &staff_nurse)
            .unwrap();
        let nurse_names: Vec<&str> = organization
            .template_members(&nurses)
            .iter()
            .map(|employee| employee.name.as_str())
            .collect();
        assert_eq!(nurse_names, vec!["Nurse Rina"]);

        // Excluded staff can be re-included.
        organization.include_employee(&doctors, &khushi).unwrap();
        let member_names: Vec<&str> = organization
            .template_members(&doctors)
            .iter()
            .map(|employee| employee.name.as_str())
            .collect();
        assert_eq!(member_names, vec!["Dr. Amit", "Dr. Khushi"]);
        assert!(organization.template_members("missing-template").is_empty());
        assert!(!organization
            .template_members(&doctors)
            .iter()
            .any(|employee| employee.id == rina));
        assert_eq!(organization.validate(), Ok(()));

        // A position change moves an employee between templates.
        organization
            .employee_mut(&amit)
            .unwrap()
            .position_id = staff_nurse.clone();
        let member_names: Vec<&str> = organization
            .template_members(&doctors)
            .iter()
            .map(|employee| employee.name.as_str())
            .collect();
        assert_eq!(member_names, vec!["Dr. Khushi"]);
    }

    #[test]
    fn include_and_exclude_validate_references() {
        let (mut organization, doctors, _) = organization_with_templates();
        let medical_officer = add_position(&mut organization, "Medical Officer");
        let nurse = add_position(&mut organization, "Senior Staff Nurse");
        let amit = add_employee(&mut organization, "Dr. Amit", &medical_officer);
        let rina = add_employee(&mut organization, "Nurse Rina", &nurse);

        assert!(matches!(
            organization.include_position(&doctors, "missing-position"),
            Err(DomainError::UnknownPosition { .. })
        ));
        assert!(matches!(
            organization.include_position("missing-template", &medical_officer),
            Err(DomainError::UnknownRosterTemplate { .. })
        ));
        assert!(matches!(
            organization.exclude_employee(&doctors, "missing-employee"),
            Err(DomainError::UnknownEmployee { .. })
        ));

        organization
            .include_position(&doctors, &medical_officer)
            .unwrap();
        // A nurse's position is not included, so excluding her is rejected.
        assert!(matches!(
            organization.exclude_employee(&doctors, &rina),
            Err(DomainError::PositionNotInTemplate { .. })
        ));
        organization.exclude_employee(&doctors, &amit).unwrap();
        // Excluding twice is idempotent.
        organization.exclude_employee(&doctors, &amit).unwrap();
        assert!(organization.template_members(&doctors).is_empty());

        organization.include_employee(&doctors, &amit).unwrap();
        assert_eq!(organization.template_members(&doctors).len(), 1);
    }

    #[test]
    fn removing_a_position_prunes_exclusions_and_blocks_requirements() {
        let (mut organization, doctors, _) = organization_with_templates();
        let medical_officer = add_position(&mut organization, "Medical Officer");
        let amit = add_employee(&mut organization, "Dr. Amit", &medical_officer);

        organization
            .include_position(&doctors, &medical_officer)
            .unwrap();
        organization.exclude_employee(&doctors, &amit).unwrap();
        organization
            .remove_position(&doctors, &medical_officer)
            .unwrap();
        assert!(
            organization
                .roster_template(&doctors)
                .unwrap()
                .excluded_employee_ids
                .is_empty()
        );

        organization
            .include_position(&doctors, &medical_officer)
            .unwrap();
        let opd = add_shift(&mut organization, &doctors, "OPD", at(8, 0), at(14, 0));
        organization
            .require_position(&doctors, &opd, &medical_officer, 1)
            .unwrap();
        assert!(matches!(
            organization.remove_position(&doctors, &medical_officer),
            Err(DomainError::DanglingReference { .. })
        ));
    }

    #[test]
    fn adds_shifts_only_to_known_templates_and_rejects_duplicate_names() {
        let (mut organization, doctors, _) = organization_with_templates();

        organization
            .add_shift(&doctors, "OPD", at(8, 0), at(14, 0))
            .unwrap();

        assert!(matches!(
            organization.add_shift("missing-template", "OPD", at(8, 0), at(14, 0)),
            Err(DomainError::UnknownRosterTemplate { .. })
        ));
        assert!(matches!(
            organization.add_shift(&doctors, "  OPD ", at(8, 0), at(14, 0)),
            Err(DomainError::DuplicateShift { .. })
        ));
    }

    #[test]
    fn staffing_requirements_require_an_included_position() {
        let (mut organization, doctors, _) = organization_with_templates();
        let medical_officer = add_position(&mut organization, "Medical Officer");
        let nurse = add_position(&mut organization, "Senior Staff Nurse");
        let shift = add_shift(&mut organization, &doctors, "OPD", at(8, 0), at(14, 0));

        organization
            .include_position(&doctors, &medical_officer)
            .unwrap();
        organization
            .require_position(&doctors, &shift, &medical_officer, 2)
            .unwrap();
        organization
            .require_position(&doctors, &shift, &medical_officer, 3)
            .unwrap();

        let requirements = &organization
            .roster_template(&doctors)
            .unwrap()
            .shift(&shift)
            .unwrap()
            .required_positions;
        assert_eq!(requirements.len(), 1);
        assert_eq!(requirements[0].headcount, 3);

        assert!(matches!(
            organization.require_position(&doctors, &shift, &nurse, 1),
            Err(DomainError::PositionNotInTemplate { .. })
        ));
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
        let (mut organization, doctors, _) = organization_with_templates();
        let position = add_position(&mut organization, "Medical Officer");
        organization
            .include_position(&doctors, &position)
            .unwrap();
        add_employee(&mut organization, "Dr. Amit", &position);
        let shift = add_shift(&mut organization, &doctors, "OPD", at(8, 0), at(14, 0));
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
            position_id: "missing-position".to_owned(),
            skills: Vec::new(),
            unavailability: Vec::new(),
            recurring_assignments: Vec::new(),
        });
        assert!(matches!(
            organization.validate(),
            Err(DomainError::DanglingReference {
                kind: "employee position",
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

    struct RuleFixture {
        organization: Organization,
        doctors: String,
        amit: String,
        opd: String,
        morning: String,
        night: String,
        nurse_day: String,
    }

    fn rule_fixture() -> RuleFixture {
        let (mut organization, doctors, nurses) = organization_with_templates();
        let position = add_position(&mut organization, "Medical Officer");
        organization
            .include_position(&doctors, &position)
            .unwrap();
        let opd = add_shift(&mut organization, &doctors, "OPD", at(8, 0), at(14, 0));
        let morning = add_shift(
            &mut organization,
            &doctors,
            "Emergency Morning",
            at(8, 0),
            at(15, 0),
        );
        let night = add_shift(
            &mut organization,
            &doctors,
            "Emergency Night",
            at(15, 0),
            at(8, 0),
        );
        let nurse_day = add_shift(&mut organization, &nurses, "Day", at(8, 0), at(16, 0));
        let amit = add_employee(&mut organization, "Dr. Amit", &position);

        RuleFixture {
            organization,
            doctors,
            amit,
            opd,
            morning,
            night,
            nurse_day,
        }
    }

    #[test]
    fn adds_unavailability_for_each_scope_and_severity() {
        let mut fixture = rule_fixture();

        fixture
            .organization
            .add_unavailability(
                &fixture.amit,
                UnavailabilityScope::Weekly {
                    weekday: Weekday::Fri,
                },
                &[],
                Severity::Hard,
                0,
            )
            .unwrap();
        {
            let soft = fixture
                .organization
                .add_unavailability(
                    &fixture.amit,
                    UnavailabilityScope::Weekly {
                        weekday: Weekday::Sat,
                    },
                    &[fixture.night.as_str()],
                    Severity::Soft,
                    2,
                )
                .unwrap();
            assert_eq!(soft.severity, Severity::Soft);
            assert_eq!(soft.penalty, 2);
            assert_eq!(soft.shifts, vec![fixture.night.clone()]);
            assert!(!soft.id.is_empty());
        }
        fixture
            .organization
            .add_unavailability(
                &fixture.amit,
                UnavailabilityScope::Date {
                    date: date(2026, 11, 16),
                },
                &[fixture.opd.as_str()],
                Severity::Hard,
                0,
            )
            .unwrap();
        fixture
            .organization
            .add_unavailability(
                &fixture.amit,
                UnavailabilityScope::Leave {
                    start: date(2026, 11, 16),
                    end: date(2026, 11, 30),
                },
                &[],
                Severity::Hard,
                0,
            )
            .unwrap();

        assert_eq!(
            fixture
                .organization
                .employee(&fixture.amit)
                .unwrap()
                .unavailability
                .len(),
            4
        );
        assert_eq!(fixture.organization.validate(), Ok(()));

        // Shift ids resolve across the whole organization, so a shift from
        // another template is valid here.
        fixture
            .organization
            .add_unavailability(
                &fixture.amit,
                UnavailabilityScope::Weekly {
                    weekday: Weekday::Mon,
                },
                &[fixture.nurse_day.as_str()],
                Severity::Hard,
                0,
            )
            .unwrap();
        assert_eq!(fixture.organization.validate(), Ok(()));
    }

    #[test]
    fn rejects_invalid_unavailability_rules() {
        let mut fixture = rule_fixture();

        assert!(matches!(
            fixture.organization.add_unavailability(
                "missing-employee",
                UnavailabilityScope::Weekly {
                    weekday: Weekday::Fri
                },
                &[],
                Severity::Hard,
                0
            ),
            Err(DomainError::UnknownEmployee { .. })
        ));
        assert!(matches!(
            fixture.organization.add_unavailability(
                &fixture.amit,
                UnavailabilityScope::Weekly {
                    weekday: Weekday::Fri
                },
                &["missing-shift"],
                Severity::Hard,
                0
            ),
            Err(DomainError::UnknownShift { .. })
        ));
        assert_eq!(
            fixture
                .organization
                .add_unavailability(
                    &fixture.amit,
                    UnavailabilityScope::Weekly {
                        weekday: Weekday::Fri
                    },
                    &["   "],
                    Severity::Hard,
                    0
                )
                .unwrap_err(),
            DomainError::BlankName
        );
        assert!(matches!(
            fixture.organization.add_unavailability(
                &fixture.amit,
                UnavailabilityScope::Leave {
                    start: date(2026, 11, 30),
                    end: date(2026, 11, 16)
                },
                &[],
                Severity::Hard,
                0
            ),
            Err(DomainError::InvalidDateRange { .. })
        ));
        assert!(matches!(
            fixture.organization.add_unavailability(
                &fixture.amit,
                UnavailabilityScope::Weekly {
                    weekday: Weekday::Fri
                },
                &[],
                Severity::Soft,
                0
            ),
            Err(DomainError::InvalidPenalty { .. })
        ));
        assert!(matches!(
            fixture.organization.add_unavailability(
                &fixture.amit,
                UnavailabilityScope::Weekly {
                    weekday: Weekday::Fri
                },
                &[],
                Severity::Hard,
                3
            ),
            Err(DomainError::InvalidPenalty { .. })
        ));
    }

    #[test]
    fn adds_recurring_assignments_and_rest_rules_with_validation() {
        let mut fixture = rule_fixture();

        {
            let assignment = fixture
                .organization
                .add_recurring_assignment(&fixture.amit, Weekday::Sun, &fixture.night)
                .unwrap();
            assert_eq!(assignment.weekday, Weekday::Sun);
            assert!(!assignment.id.is_empty());
        }
        {
            let rule = fixture
                .organization
                .add_rest_rule(
                    &fixture.doctors,
                    &fixture.night,
                    &fixture.morning,
                    480,
                    Severity::Soft,
                    3,
                )
                .unwrap();
            assert_eq!(rule.min_rest_minutes, 480);
            assert_eq!(rule.penalty, 3);
        }

        assert!(matches!(
            fixture.organization.add_recurring_assignment(
                "missing-employee",
                Weekday::Sun,
                &fixture.night
            ),
            Err(DomainError::UnknownEmployee { .. })
        ));
        assert!(matches!(
            fixture.organization.add_recurring_assignment(
                &fixture.amit,
                Weekday::Sun,
                "missing-shift"
            ),
            Err(DomainError::UnknownShift { .. })
        ));
        // Any organization shift is valid, including another template's.
        fixture
            .organization
            .add_recurring_assignment(&fixture.amit, Weekday::Sun, &fixture.nurse_day)
            .unwrap();
        assert!(matches!(
            fixture.organization.add_rest_rule(
                "missing-template",
                &fixture.night,
                &fixture.morning,
                480,
                Severity::Hard,
                0
            ),
            Err(DomainError::UnknownRosterTemplate { .. })
        ));
        assert!(matches!(
            fixture.organization.add_rest_rule(
                &fixture.doctors,
                &fixture.night,
                "missing-shift",
                480,
                Severity::Hard,
                0
            ),
            Err(DomainError::ShiftOutsideTemplate { .. })
        ));
        assert!(matches!(
            fixture.organization.add_rest_rule(
                &fixture.doctors,
                &fixture.night,
                &fixture.nurse_day,
                480,
                Severity::Hard,
                0
            ),
            Err(DomainError::ShiftOutsideTemplate { .. })
        ));
        assert!(matches!(
            fixture.organization.add_rest_rule(
                &fixture.doctors,
                &fixture.night,
                &fixture.morning,
                0,
                Severity::Hard,
                0
            ),
            Err(DomainError::InvalidRestGap { .. })
        ));
        assert!(matches!(
            fixture.organization.add_rest_rule(
                &fixture.doctors,
                &fixture.night,
                &fixture.morning,
                480,
                Severity::Soft,
                0
            ),
            Err(DomainError::InvalidPenalty { .. })
        ));

        assert_eq!(fixture.organization.validate(), Ok(()));
    }

    #[test]
    fn validate_catches_rule_reference_problems() {
        let mut fixture = rule_fixture();
        fixture
            .organization
            .add_unavailability(
                &fixture.amit,
                UnavailabilityScope::Weekly {
                    weekday: Weekday::Fri,
                },
                &[fixture.night.as_str()],
                Severity::Hard,
                0,
            )
            .unwrap();
        fixture
            .organization
            .add_recurring_assignment(&fixture.amit, Weekday::Sun, &fixture.night)
            .unwrap();
        fixture
            .organization
            .add_rest_rule(
                &fixture.doctors,
                &fixture.night,
                &fixture.morning,
                480,
                Severity::Hard,
                0,
            )
            .unwrap();
        assert_eq!(fixture.organization.validate(), Ok(()));

        // A rule that points at a shift that does not exist anywhere.
        fixture.organization.employees[0].unavailability[0].shifts =
            vec!["missing-shift".to_owned()];
        assert!(matches!(
            fixture.organization.validate(),
            Err(DomainError::DanglingReference { .. })
        ));
        fixture.organization.employees[0].unavailability[0].shifts = vec![fixture.night.clone()];
        assert_eq!(fixture.organization.validate(), Ok(()));

        // Duplicate rule ids within one employee.
        let first_rule = fixture.organization.employees[0].unavailability[0].clone();
        fixture.organization.employees[0]
            .recurring_assignments
            .push(RecurringAssignment {
                id: first_rule.id.clone(),
                weekday: Weekday::Mon,
                shift_id: fixture.night.clone(),
                penalty: 1,
            });
        assert!(matches!(
            fixture.organization.validate(),
            Err(DomainError::DuplicateId {
                kind: "recurring assignment",
                ..
            })
        ));
        fixture.organization.employees[0]
            .recurring_assignments
            .pop();
        assert_eq!(fixture.organization.validate(), Ok(()));

        // A rest rule that points at another template's shift.
        fixture.organization.roster_templates[0].rest_rules[0].following_shift_id =
            fixture.nurse_day.clone();
        assert!(matches!(
            fixture.organization.validate(),
            Err(DomainError::ShiftOutsideTemplate { .. })
        ));
    }

    #[derive(Clone)]
    struct RosterFixture {
        organization: Organization,
        doctors: String,
        medical_officer: String,
        staff_nurse: String,
        amit: String,
        khushi: String,
        rina: String,
        roster_id: String,
        dated_shift_id: String,
        first_slot: String,
        second_slot: String,
    }

    fn roster_fixture() -> RosterFixture {
        let (mut organization, doctors, _) = organization_with_templates();
        let medical_officer = add_position(&mut organization, "Medical Officer");
        let staff_nurse = add_position(&mut organization, "Senior Staff Nurse");

        organization
            .include_position(&doctors, &medical_officer)
            .unwrap();

        let opd = add_shift(&mut organization, &doctors, "OPD", at(8, 0), at(14, 0));
        let night = add_shift(
            &mut organization,
            &doctors,
            "Emergency Night",
            at(15, 0),
            at(8, 0),
        );
        organization
            .require_position(&doctors, &opd, &medical_officer, 2)
            .unwrap();
        organization
            .require_position(&doctors, &night, &medical_officer, 1)
            .unwrap();

        let amit = add_employee(&mut organization, "Dr. Amit", &medical_officer);
        let khushi = add_employee(&mut organization, "Dr. Khushi", &medical_officer);
        let rina = add_employee(&mut organization, "Nurse Rina", &staff_nurse);

        let roster_id = organization
            .create_roster(
                &doctors,
                date(2026, 11, 16),
                date(2026, 11, 16),
                Vec::new(),
            )
            .expect("roster")
            .id
            .clone();
        let dated_shift_id = organization
            .roster(&doctors, &roster_id)
            .expect("roster")
            .shifts
            .iter()
            .find(|shift| shift.name == "OPD")
            .expect("OPD dated shift")
            .id
            .clone();
        let slots = &organization
            .roster(&doctors, &roster_id)
            .expect("roster")
            .dated_shift(&dated_shift_id)
            .expect("OPD dated shift")
            .slots;
        let first_slot = slots[0].id.clone();
        let second_slot = slots[1].id.clone();

        RosterFixture {
            organization,
            doctors,
            medical_officer,
            staff_nurse,
            amit,
            khushi,
            rina,
            roster_id,
            dated_shift_id,
            first_slot,
            second_slot,
        }
    }

    #[test]
    fn creates_draft_rosters_and_snapshots_members_and_shifts() {
        let mut fixture = roster_fixture();
        assert_eq!(fixture.organization.validate(), Ok(()));

        let roster = fixture
            .organization
            .roster(&fixture.doctors, &fixture.roster_id)
            .expect("roster");
        assert_eq!(roster.status, PlanStatus::Draft);
        assert_eq!(roster.start, date(2026, 11, 16));
        assert_eq!(roster.end, date(2026, 11, 16));
        assert_eq!(roster.members.len(), 2);
        assert!(roster.name.contains("Doctors"));
        assert!(roster.name.contains("2026-11-16"));
        assert_eq!(roster.total_slots(), 3);
        assert_eq!(roster.filled_slots(), 0);

        // Later template edits never rewrite the roster snapshot.
        fixture
            .organization
            .exclude_employee(&fixture.doctors, &fixture.khushi)
            .unwrap();
        fixture
            .organization
            .remove_position(&fixture.doctors, &fixture.medical_officer)
            .unwrap_err();
        let new_opd = add_shift(&mut fixture.organization, &fixture.doctors, "Ward", at(9, 0), at(15, 0));
        fixture
            .organization
            .require_position(&fixture.doctors, &new_opd, &fixture.medical_officer, 1)
            .unwrap();
        let roster = fixture
            .organization
            .roster(&fixture.doctors, &fixture.roster_id)
            .expect("roster");
        assert_eq!(roster.members.len(), 2);
        assert!(roster.shifts.iter().all(|shift| shift.name != "Ward"));

        // New members are only picked up by the next created roster.
        let second = fixture
            .organization
            .create_roster(
                &fixture.doctors,
                date(2026, 11, 17),
                date(2026, 11, 17),
                Vec::new(),
            )
            .expect("second roster");
        assert_eq!(second.members.len(), 1);
        assert_eq!(second.members[0].employee_id, fixture.amit);
    }

    #[test]
    fn creates_rosters_with_weekday_restrictions_and_validates_context() {
        let mut fixture = roster_fixture();
        let opd_id = fixture
            .organization
            .roster_template(&fixture.doctors)
            .unwrap()
            .shifts[0]
            .id
            .clone();
        fixture
            .organization
            .set_shift_weekdays(&fixture.doctors, &opd_id, &[Weekday::Fri, Weekday::Fri])
            .unwrap();
        let opd_weekdays = &fixture
            .organization
            .roster_template(&fixture.doctors)
            .unwrap()
            .shifts[0]
            .weekdays;
        assert_eq!(opd_weekdays, &vec![Weekday::Fri]);

        // 2026-11-16 is a Monday, so a Friday-only OPD template is absent.
        let monday = fixture
            .organization
            .create_roster(
                &fixture.doctors,
                date(2026, 11, 16),
                date(2026, 11, 16),
                Vec::new(),
            )
            .expect("roster");
        assert!(!monday.shifts.iter().any(|shift| shift.name == "OPD"));
        let friday = fixture
            .organization
            .create_roster(
                &fixture.doctors,
                date(2026, 11, 20),
                date(2026, 11, 20),
                Vec::new(),
            )
            .expect("roster");
        assert!(friday.shifts.iter().any(|shift| shift.name == "OPD"));

        assert!(matches!(
            fixture.organization.create_roster(
                "missing-template",
                date(2026, 11, 16),
                date(2026, 11, 16),
                Vec::new()
            ),
            Err(DomainError::UnknownRosterTemplate { .. })
        ));
        assert!(matches!(
            fixture.organization.create_roster(
                &fixture.doctors,
                date(2026, 11, 30),
                date(2026, 11, 16),
                Vec::new()
            ),
            Err(DomainError::InvalidDateRange { .. })
        ));
        assert!(matches!(
            fixture.organization.create_roster(
                &fixture.doctors,
                date(2026, 11, 16),
                date(2026, 11, 16),
                vec![ContextAssignment {
                    date: date(2026, 11, 15),
                    name: "Emergency Night".to_owned(),
                    start: at(15, 0),
                    end: at(8, 0),
                    employee_id: fixture.rina.clone(),
                    template_id: None,
                }]
            ),
            Err(DomainError::EmployeeNotInRoster { .. })
        ));
        assert!(matches!(
            fixture.organization.create_roster(
                &fixture.doctors,
                date(2026, 11, 16),
                date(2026, 11, 16),
                vec![ContextAssignment {
                    date: date(2026, 11, 16),
                    name: "Emergency Night".to_owned(),
                    start: at(15, 0),
                    end: at(8, 0),
                    employee_id: fixture.amit.clone(),
                    template_id: None,
                }]
            ),
            Err(DomainError::ContextDateOutOfRange { .. })
        ));
    }

    #[test]
    fn assigns_and_unassigns_individual_seats() {
        let mut fixture = roster_fixture();

        fixture
            .organization
            .assign_slot(
                &fixture.doctors,
                &fixture.roster_id,
                &fixture.dated_shift_id,
                &fixture.first_slot,
                &fixture.amit,
            )
            .unwrap();

        let roster = fixture
            .organization
            .roster(&fixture.doctors, &fixture.roster_id)
            .expect("roster");
        assert_eq!(
            roster
                .slot(&fixture.dated_shift_id, &fixture.first_slot)
                .expect("first slot")
                .employee_id
                .as_deref(),
            Some(fixture.amit.as_str())
        );
        assert_eq!(
            roster
                .slot(&fixture.dated_shift_id, &fixture.second_slot)
                .expect("second slot")
                .employee_id,
            None
        );
        assert_eq!(roster.filled_slots(), 1);

        fixture
            .organization
            .assign_slot(
                &fixture.doctors,
                &fixture.roster_id,
                &fixture.dated_shift_id,
                &fixture.first_slot,
                &fixture.khushi,
            )
            .unwrap();
        assert_eq!(
            fixture
                .organization
                .roster(&fixture.doctors, &fixture.roster_id)
                .unwrap()
                .slot(&fixture.dated_shift_id, &fixture.first_slot)
                .unwrap()
                .employee_id
                .as_deref(),
            Some(fixture.khushi.as_str())
        );

        fixture
            .organization
            .unassign_slot(
                &fixture.doctors,
                &fixture.roster_id,
                &fixture.dated_shift_id,
                &fixture.first_slot,
            )
            .unwrap();
        assert_eq!(
            fixture
                .organization
                .roster(&fixture.doctors, &fixture.roster_id)
                .unwrap()
                .slot(&fixture.dated_shift_id, &fixture.first_slot)
                .unwrap()
                .employee_id,
            None
        );

        assert_eq!(fixture.organization.validate(), Ok(()));
    }

    #[test]
    fn rejects_invalid_assignments_and_finalized_rosters() {
        let mut fixture = roster_fixture();

        assert!(matches!(
            fixture.organization.assign_slot(
                &fixture.doctors,
                &fixture.roster_id,
                &fixture.dated_shift_id,
                &fixture.first_slot,
                "missing-employee"
            ),
            Err(DomainError::UnknownEmployee { .. })
        ));
        assert!(matches!(
            fixture.organization.assign_slot(
                "missing-template",
                &fixture.roster_id,
                &fixture.dated_shift_id,
                &fixture.first_slot,
                &fixture.amit
            ),
            Err(DomainError::UnknownRoster { .. })
        ));
        assert!(matches!(
            fixture.organization.assign_slot(
                &fixture.doctors,
                "missing-roster",
                &fixture.dated_shift_id,
                &fixture.first_slot,
                &fixture.amit
            ),
            Err(DomainError::UnknownRoster { .. })
        ));
        assert!(matches!(
            fixture.organization.assign_slot(
                &fixture.doctors,
                &fixture.roster_id,
                "missing-shift",
                &fixture.first_slot,
                &fixture.amit
            ),
            Err(DomainError::UnknownDatedShift { .. })
        ));
        assert!(matches!(
            fixture.organization.assign_slot(
                &fixture.doctors,
                &fixture.roster_id,
                &fixture.dated_shift_id,
                "missing-slot",
                &fixture.amit
            ),
            Err(DomainError::UnknownSlot { .. })
        ));
        assert!(matches!(
            fixture.organization.assign_slot(
                &fixture.doctors,
                &fixture.roster_id,
                &fixture.dated_shift_id,
                &fixture.first_slot,
                &fixture.rina
            ),
            Err(DomainError::EmployeeNotInRoster { .. })
        ));

        let wrong_position = fixture
            .organization
            .add_employee("Dr. Wrong", &fixture.staff_nurse)
            .unwrap()
            .id
            .clone();
        fixture
            .organization
            .include_position(&fixture.doctors, &fixture.staff_nurse)
            .unwrap();
        let second_roster = fixture
            .organization
            .create_roster(
                &fixture.doctors,
                date(2026, 11, 18),
                date(2026, 11, 18),
                Vec::new(),
            )
            .unwrap()
            .id
            .clone();
        let second_shift = fixture
            .organization
            .roster(&fixture.doctors, &second_roster)
            .unwrap()
            .shifts
            .iter()
            .find(|shift| shift.name == "OPD")
            .unwrap()
            .id
            .clone();
        let second_slot = fixture
            .organization
            .roster(&fixture.doctors, &second_roster)
            .unwrap()
            .dated_shift(&second_shift)
            .unwrap()
            .slots[0]
            .id
            .clone();
        assert!(matches!(
            fixture.organization.assign_slot(
                &fixture.doctors,
                &second_roster,
                &second_shift,
                &second_slot,
                &wrong_position
            ),
            Err(DomainError::PositionMismatch { .. })
        ));

        fixture
            .organization
            .assign_slot(
                &fixture.doctors,
                &fixture.roster_id,
                &fixture.dated_shift_id,
                &fixture.first_slot,
                &fixture.amit,
            )
            .unwrap();
        assert!(matches!(
            fixture.organization.assign_slot(
                &fixture.doctors,
                &fixture.roster_id,
                &fixture.dated_shift_id,
                &fixture.second_slot,
                &fixture.amit
            ),
            Err(DomainError::EmployeeAlreadyAssigned { .. })
        ));

        fixture
            .organization
            .roster_mut(&fixture.doctors, &fixture.roster_id)
            .unwrap()
            .status = PlanStatus::Finalized;
        assert!(matches!(
            fixture.organization.assign_slot(
                &fixture.doctors,
                &fixture.roster_id,
                &fixture.dated_shift_id,
                &fixture.second_slot,
                &fixture.khushi
            ),
            Err(DomainError::RosterFinalized { .. })
        ));
        assert!(matches!(
            fixture.organization.unassign_slot(
                &fixture.doctors,
                &fixture.roster_id,
                &fixture.dated_shift_id,
                &fixture.first_slot
            ),
            Err(DomainError::RosterFinalized { .. })
        ));
    }

    #[test]
    fn validate_catches_invalid_roster_data() {
        let base = roster_fixture();
        assert_eq!(base.organization.validate(), Ok(()));

        // Reversed roster range.
        let mut fixture = base.clone();
        fixture
            .organization
            .roster_mut(&fixture.doctors, &fixture.roster_id)
            .unwrap()
            .end = date(2026, 11, 15);
        assert!(matches!(
            fixture.organization.validate(),
            Err(DomainError::InvalidDateRange { .. })
        ));

        // Slot pointing at a missing position.
        let mut fixture = base.clone();
        fixture
            .organization
            .roster_mut(&fixture.doctors, &fixture.roster_id)
            .unwrap()
            .dated_shift_mut(&fixture.dated_shift_id)
            .unwrap()
            .slots[0]
            .position_id = "missing-position".to_owned();
        assert!(matches!(
            fixture.organization.validate(),
            Err(DomainError::DanglingReference {
                kind: "slot position",
                ..
            })
        ));

        // Assignment pointing at a non-member.
        let mut fixture = base.clone();
        fixture
            .organization
            .roster_mut(&fixture.doctors, &fixture.roster_id)
            .unwrap()
            .dated_shift_mut(&fixture.dated_shift_id)
            .unwrap()
            .slots[0]
            .employee_id = Some("missing-employee".to_owned());
        assert!(matches!(
            fixture.organization.validate(),
            Err(DomainError::EmployeeNotInRoster { .. })
        ));

        // Assignment from a non-member with a valid id.
        let mut fixture = base.clone();
        fixture
            .organization
            .roster_mut(&fixture.doctors, &fixture.roster_id)
            .unwrap()
            .dated_shift_mut(&fixture.dated_shift_id)
            .unwrap()
            .slots[0]
            .employee_id = Some(fixture.rina.clone());
        assert!(matches!(
            fixture.organization.validate(),
            Err(DomainError::EmployeeNotInRoster { .. })
        ));

        // Assignment whose snapshot position does not match the slot.
        let mut fixture = base.clone();
        {
            let roster = fixture
                .organization
                .roster_mut(&fixture.doctors, &fixture.roster_id)
                .unwrap();
            roster
                .dated_shift_mut(&fixture.dated_shift_id)
                .unwrap()
                .slots[0]
                .employee_id = Some(fixture.amit.clone());
            roster.members[0].position_id = fixture.staff_nurse.clone();
        }
        assert!(matches!(
            fixture.organization.validate(),
            Err(DomainError::PositionMismatch { .. })
        ));

        // The same employee in two seats of one dated shift.
        let mut fixture = base.clone();
        {
            let roster = fixture
                .organization
                .roster_mut(&fixture.doctors, &fixture.roster_id)
                .unwrap();
            let dated = roster.dated_shift_mut(&fixture.dated_shift_id).unwrap();
            dated.slots[0].employee_id = Some(fixture.amit.clone());
            dated.slots[1].employee_id = Some(fixture.amit.clone());
        }
        assert!(matches!(
            fixture.organization.validate(),
            Err(DomainError::EmployeeAlreadyAssigned { .. })
        ));

        // Duplicate slot ids inside one dated shift.
        let mut fixture = base.clone();
        {
            let roster = fixture
                .organization
                .roster_mut(&fixture.doctors, &fixture.roster_id)
                .unwrap();
            let dated = roster.dated_shift_mut(&fixture.dated_shift_id).unwrap();
            dated.slots[1].id = dated.slots[0].id.clone();
        }
        assert!(matches!(
            fixture.organization.validate(),
            Err(DomainError::DuplicateId { kind: "slot", .. })
        ));

        // Duplicate dated-shift ids inside one roster.
        let mut fixture = base.clone();
        {
            let roster = fixture
                .organization
                .roster_mut(&fixture.doctors, &fixture.roster_id)
                .unwrap();
            let first = roster.shifts[0].id.clone();
            roster.shifts[1].id = first;
        }
        assert!(matches!(
            fixture.organization.validate(),
            Err(DomainError::DuplicateId {
                kind: "dated shift",
                ..
            })
        ));

        // Duplicate roster ids on one template.
        let mut fixture = base.clone();
        fixture
            .organization
            .create_roster(
                &fixture.doctors,
                date(2026, 11, 17),
                date(2026, 11, 17),
                Vec::new(),
            )
            .unwrap();
        {
            let template = fixture
                .organization
                .roster_template_mut(&fixture.doctors)
                .unwrap();
            template.rosters[1].id = template.rosters[0].id.clone();
        }
        assert!(matches!(
            fixture.organization.validate(),
            Err(DomainError::DuplicateId { kind: "roster", .. })
        ));

        // Context on or after the roster start.
        let mut fixture = base.clone();
        fixture
            .organization
            .roster_mut(&fixture.doctors, &fixture.roster_id)
            .unwrap()
            .context
            .push(ContextAssignment {
                date: date(2026, 11, 16),
                name: "Emergency Night".to_owned(),
                start: at(15, 0),
                end: at(8, 0),
                employee_id: fixture.amit.clone(),
                template_id: None,
            });
        assert!(matches!(
            fixture.organization.validate(),
            Err(DomainError::ContextDateOutOfRange { .. })
        ));

        // Context employee from another template.
        let mut fixture = base.clone();
        fixture
            .organization
            .roster_mut(&fixture.doctors, &fixture.roster_id)
            .unwrap()
            .context
            .push(ContextAssignment {
                date: date(2026, 11, 15),
                name: "Emergency Night".to_owned(),
                start: at(15, 0),
                end: at(8, 0),
                employee_id: fixture.rina.clone(),
                template_id: None,
            });
        assert!(matches!(
            fixture.organization.validate(),
            Err(DomainError::EmployeeNotInRoster { .. })
        ));
    }

    fn shift_id(organization: &Organization, template_id: &str, name: &str) -> String {
        organization
            .roster_template(template_id)
            .expect("template")
            .shifts
            .iter()
            .find(|shift| shift.name == name)
            .expect("shift")
            .id
            .clone()
    }

    fn add_doctor(fixture: &mut RosterFixture, name: &str) -> String {
        let position_id = fixture
            .organization
            .employee(&fixture.amit)
            .expect("amit")
            .position_id
            .clone();
        add_employee(&mut fixture.organization, name, &position_id)
    }

    #[test]
    fn recurring_preferences_carry_a_penalty() {
        let mut fixture = roster_fixture();
        let night = shift_id(&fixture.organization, &fixture.doctors, "Emergency Night");

        let default = fixture
            .organization
            .add_recurring_assignment(&fixture.amit, Weekday::Sun, &night)
            .expect("preference");
        assert_eq!(default.penalty, 1);

        let costly = fixture
            .organization
            .add_recurring_assignment_with_penalty(&fixture.khushi, Weekday::Sat, &night, 4)
            .expect("preference");
        assert_eq!(costly.penalty, 4);

        assert_eq!(
            fixture
                .organization
                .add_recurring_assignment_with_penalty(&fixture.amit, Weekday::Mon, &night, 0)
                .unwrap_err(),
            DomainError::InvalidPenalty {
                severity: Severity::Soft,
                penalty: 0
            }
        );
        assert_eq!(fixture.organization.validate(), Ok(()));

        // A hand-edited penalty of zero is rejected on load validation.
        fixture.organization.employees[0].recurring_assignments[0].penalty = 0;
        assert!(matches!(
            fixture.organization.validate(),
            Err(DomainError::InvalidPenalty {
                severity: Severity::Soft,
                penalty: 0
            })
        ));
    }

    #[test]
    fn context_templates_must_resolve_in_the_template() {
        let mut fixture = roster_fixture();
        let night = shift_id(&fixture.organization, &fixture.doctors, "Emergency Night");

        let roster = fixture
            .organization
            .create_roster(
                &fixture.doctors,
                date(2026, 11, 16),
                date(2026, 11, 16),
                vec![ContextAssignment {
                    date: date(2026, 11, 15),
                    name: "Emergency Night".to_owned(),
                    start: at(15, 0),
                    end: at(8, 0),
                    employee_id: fixture.amit.clone(),
                    template_id: Some(night.clone()),
                }],
            )
            .expect("roster");
        assert_eq!(roster.context[0].template_id.as_deref(), Some(night.as_str()));
        let roster_id = roster.id.clone();

        assert!(matches!(
            fixture.organization.create_roster(
                &fixture.doctors,
                date(2026, 11, 17),
                date(2026, 11, 17),
                vec![ContextAssignment {
                    date: date(2026, 11, 16),
                    name: "Emergency Night".to_owned(),
                    start: at(15, 0),
                    end: at(8, 0),
                    employee_id: fixture.amit.clone(),
                    template_id: Some("missing-template".to_owned()),
                }]
            ),
            Err(DomainError::ShiftOutsideTemplate { .. })
        ));

        // Hand-edited context naming an unknown template.
        fixture
            .organization
            .roster_mut(&fixture.doctors, &roster_id)
            .unwrap()
            .context[0]
            .template_id = Some("missing-template".to_owned());
        assert!(matches!(
            fixture.organization.validate(),
            Err(DomainError::ShiftOutsideTemplate { .. })
        ));
    }

    #[test]
    fn adds_template_scoped_sharing_groups() {
        let mut fixture = roster_fixture();
        let fourth = add_doctor(&mut fixture, "Dr. Meera");

        let registrars = fixture
            .organization
            .add_sharing_group(
                &fixture.doctors,
                "  Registrars ",
                &[&fixture.amit, &fixture.khushi, &fixture.amit],
            )
            .expect("first group");
        assert_eq!(registrars.name, "Registrars");
        assert_eq!(
            registrars.employee_ids,
            vec![fixture.amit.clone(), fixture.khushi.clone()]
        );
        let registrars_id = registrars.id.clone();

        fixture
            .organization
            .add_sharing_group(&fixture.doctors, "Interns", &[&fourth])
            .expect("second group");

        let template = fixture
            .organization
            .roster_template(&fixture.doctors)
            .expect("doctors template");
        assert_eq!(template.sharing_groups.len(), 2);
        assert_eq!(
            template
                .sharing_group(&registrars_id)
                .expect("lookup")
                .name,
            "Registrars"
        );
        assert_eq!(
            template
                .sharing_group_for_employee(&fixture.amit)
                .expect("member lookup")
                .id,
            registrars_id
        );
        assert!(template.sharing_group_for_employee(&fixture.rina).is_none());
        assert_eq!(fixture.organization.validate(), Ok(()));
    }

    #[test]
    fn rejects_invalid_sharing_groups() {
        let mut fixture = roster_fixture();
        let fourth = add_doctor(&mut fixture, "Dr. Meera");
        fixture
            .organization
            .add_sharing_group(&fixture.doctors, "Registrars", &[&fixture.amit])
            .expect("group");

        assert_eq!(
            fixture
                .organization
                .add_sharing_group(&fixture.doctors, "   ", &[&fixture.khushi]),
            Err(DomainError::BlankName)
        );
        assert_eq!(
            fixture.organization.add_sharing_group(
                &fixture.doctors,
                " Registrars ",
                &[&fixture.khushi]
            ),
            Err(DomainError::DuplicateSharingGroup {
                name: "Registrars".to_owned()
            })
        );
        assert_eq!(
            fixture
                .organization
                .add_sharing_group(&fixture.doctors, "Empty", &[]),
            Err(DomainError::EmptySharingGroup {
                name: "Empty".to_owned()
            })
        );
        assert_eq!(
            fixture.organization.add_sharing_group(
                &fixture.doctors,
                "Ghosts",
                &["missing-employee"]
            ),
            Err(DomainError::UnknownEmployee {
                id: "missing-employee".to_owned()
            })
        );
        assert!(matches!(
            fixture
                .organization
                .add_sharing_group(&fixture.doctors, "Nurses", &[&fixture.rina]),
            Err(DomainError::EmployeeNotInTemplate { .. })
        ));
        assert!(matches!(
            fixture.organization.add_sharing_group(
                &fixture.doctors,
                "Interns",
                &[&fourth, &fixture.amit]
            ),
            Err(DomainError::EmployeeAlreadyGrouped { .. })
        ));

        // None of the rejections were stored.
        assert_eq!(
            fixture
                .organization
                .roster_template(&fixture.doctors)
                .expect("template")
                .sharing_groups
                .len(),
            1
        );
    }

    #[test]
    fn validate_catches_invalid_sharing_groups() {
        let base = roster_fixture();
        assert_eq!(base.organization.validate(), Ok(()));

        // Duplicate group ids.
        let mut fixture = base.clone();
        fixture
            .organization
            .add_sharing_group(&fixture.doctors, "Registrars", &[&fixture.amit])
            .unwrap();
        fixture
            .organization
            .add_sharing_group(&fixture.doctors, "Interns", &[&fixture.khushi])
            .unwrap();
        {
            let template = fixture
                .organization
                .roster_template_mut(&fixture.doctors)
                .expect("template");
            template.sharing_groups[1].id = template.sharing_groups[0].id.clone();
        }
        assert!(matches!(
            fixture.organization.validate(),
            Err(DomainError::DuplicateId {
                kind: "sharing group",
                ..
            })
        ));

        // Duplicate names.
        let mut fixture = base.clone();
        fixture
            .organization
            .add_sharing_group(&fixture.doctors, "Registrars", &[&fixture.amit])
            .unwrap();
        fixture
            .organization
            .add_sharing_group(&fixture.doctors, "Interns", &[&fixture.khushi])
            .unwrap();
        fixture
            .organization
            .roster_template_mut(&fixture.doctors)
            .expect("template")
            .sharing_groups[1]
            .name = "Registrars".to_owned();
        assert!(matches!(
            fixture.organization.validate(),
            Err(DomainError::DuplicateSharingGroup { .. })
        ));

        // Empty membership.
        let mut fixture = base.clone();
        fixture
            .organization
            .roster_template_mut(&fixture.doctors)
            .expect("template")
            .sharing_groups
            .push(SharingGroup {
                id: "group-1".to_owned(),
                name: "Empty".to_owned(),
                employee_ids: Vec::new(),
            });
        assert!(matches!(
            fixture.organization.validate(),
            Err(DomainError::EmptySharingGroup { .. })
        ));

        // Repeated member inside one group.
        let mut fixture = base.clone();
        fixture
            .organization
            .roster_template_mut(&fixture.doctors)
            .expect("template")
            .sharing_groups
            .push(SharingGroup {
                id: "group-1".to_owned(),
                name: "Repeated".to_owned(),
                employee_ids: vec![fixture.amit.clone(), fixture.amit.clone()],
            });
        assert!(matches!(
            fixture.organization.validate(),
            Err(DomainError::DuplicateId {
                kind: "sharing group member",
                ..
            })
        ));

        // Dangling member.
        let mut fixture = base.clone();
        fixture
            .organization
            .roster_template_mut(&fixture.doctors)
            .expect("template")
            .sharing_groups
            .push(SharingGroup {
                id: "group-1".to_owned(),
                name: "Ghosts".to_owned(),
                employee_ids: vec!["missing-employee".to_owned()],
            });
        assert!(matches!(
            fixture.organization.validate(),
            Err(DomainError::DanglingReference {
                kind: "sharing group employee",
                ..
            })
        ));

        // Member from a position the template does not include.
        let mut fixture = base.clone();
        fixture
            .organization
            .roster_template_mut(&fixture.doctors)
            .expect("template")
            .sharing_groups
            .push(SharingGroup {
                id: "group-1".to_owned(),
                name: "Nurses".to_owned(),
                employee_ids: vec![fixture.rina.clone()],
            });
        assert!(matches!(
            fixture.organization.validate(),
            Err(DomainError::EmployeeNotInTemplate { .. })
        ));

        // One employee in two groups.
        let mut fixture = base.clone();
        fixture
            .organization
            .add_sharing_group(&fixture.doctors, "Registrars", &[&fixture.amit])
            .unwrap();
        fixture
            .organization
            .roster_template_mut(&fixture.doctors)
            .expect("template")
            .sharing_groups
            .push(SharingGroup {
                id: "group-2".to_owned(),
                name: "Interns".to_owned(),
                employee_ids: vec![fixture.amit.clone()],
            });
        assert!(matches!(
            fixture.organization.validate(),
            Err(DomainError::EmployeeAlreadyGrouped { .. })
        ));
    }

    #[test]
    fn updates_shift_fields_and_rejects_invalid_edits() {
        let (mut organization, doctors, _) = organization_with_templates();
        let opd = add_shift(&mut organization, &doctors, "OPD", at(8, 0), at(14, 0));
        add_shift(&mut organization, &doctors, "Ward", at(8, 0), at(14, 0));

        organization
            .update_shift(&doctors, &opd, "  OPD Updated ", at(9, 0), at(15, 0))
            .unwrap();
        let shift = organization
            .roster_template(&doctors)
            .unwrap()
            .shift(&opd)
            .unwrap();
        assert_eq!(shift.name, "OPD Updated");
        assert_eq!(shift.start, at(9, 0));
        assert_eq!(shift.end, at(15, 0));

        assert_eq!(
            organization.update_shift(&doctors, &opd, "   ", at(9, 0), at(15, 0)),
            Err(DomainError::BlankName)
        );
        assert_eq!(
            organization.update_shift(&doctors, &opd, "Ward", at(9, 0), at(15, 0)),
            Err(DomainError::DuplicateShift {
                name: "Ward".to_owned()
            })
        );
        assert_eq!(
            organization.update_shift(&doctors, &opd, "OPD Updated", at(9, 0), at(9, 0)),
            Err(DomainError::InvalidShiftTimes {
                start: at(9, 0),
                end: at(9, 0)
            })
        );
        assert!(matches!(
            organization.update_shift("missing-template", &opd, "X", at(9, 0), at(10, 0)),
            Err(DomainError::UnknownRosterTemplate { .. })
        ));
        assert!(matches!(
            organization.update_shift(&doctors, "missing-shift", "X", at(9, 0), at(10, 0)),
            Err(DomainError::UnknownShift { .. })
        ));
    }

    #[test]
    fn replaces_shift_requirements_exactly() {
        let (mut organization, doctors, _) = organization_with_templates();
        let medical_officer = add_position(&mut organization, "Medical Officer");
        let nurse = add_position(&mut organization, "Senior Staff Nurse");
        organization
            .include_position(&doctors, &medical_officer)
            .unwrap();
        organization.include_position(&doctors, &nurse).unwrap();
        let opd = add_shift(&mut organization, &doctors, "OPD", at(8, 0), at(14, 0));

        organization
            .set_shift_requirements(
                &doctors,
                &opd,
                &[(medical_officer.clone(), 2), (nurse.clone(), 1)],
            )
            .unwrap();
        assert_eq!(
            organization
                .roster_template(&doctors)
                .unwrap()
                .shift(&opd)
                .unwrap()
                .required_positions
                .len(),
            2
        );

        organization
            .set_shift_requirements(&doctors, &opd, &[(nurse.clone(), 3)])
            .unwrap();
        let requirements = &organization
            .roster_template(&doctors)
            .unwrap()
            .shift(&opd)
            .unwrap()
            .required_positions;
        assert_eq!(requirements.len(), 1);
        assert_eq!(requirements[0].position_id, nurse);
        assert_eq!(requirements[0].headcount, 3);

        organization
            .set_shift_requirements(
                &doctors,
                &opd,
                &[(medical_officer.clone(), 1), (medical_officer.clone(), 4)],
            )
            .unwrap();
        let requirements = &organization
            .roster_template(&doctors)
            .unwrap()
            .shift(&opd)
            .unwrap()
            .required_positions;
        assert_eq!(requirements.len(), 1);
        assert_eq!(requirements[0].headcount, 4);

        assert!(matches!(
            organization.set_shift_requirements(&doctors, &opd, &[(medical_officer.clone(), 0)]),
            Err(DomainError::EmptyHeadcount { .. })
        ));
        assert!(matches!(
            organization.set_shift_requirements(
                &doctors,
                &opd,
                &[("missing-position".to_owned(), 1)]
            ),
            Err(DomainError::UnknownPosition { .. })
        ));
        assert!(matches!(
            organization.set_shift_requirements("missing-template", &opd, &[(medical_officer, 1)]),
            Err(DomainError::UnknownRosterTemplate { .. })
        ));
        assert!(matches!(
            organization.set_shift_requirements(&doctors, "missing-shift", &[]),
            Err(DomainError::UnknownShift { .. })
        ));
    }
}

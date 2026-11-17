//! Organizations: the root of the domain graph.
//!
//! An organization owns its positions, its employees (each assigned to exactly
//! one roster), and its rosters. Rosters own shift templates. Cross-entity
//! validation lives here because no child can see the whole graph.

use std::collections::HashSet;

use chrono::{NaiveDate, NaiveTime, Utc, Weekday};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{
    ContextAssignment, DomainError, Employee, PlanStatus, Position, RecurringAssignment, RestRule,
    Roster, RosterPlan, Severity, ShiftTemplate, StaffingRequirement, Unavailability,
    UnavailabilityScope,
};

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

    /// Add an unavailability or leave rule to an employee.
    ///
    /// Every shift id must name a template in the employee's own roster; an
    /// empty list means every shift. Local validation (date range and
    /// severity/penalty pairing) happens in [`Unavailability::new`].
    pub fn add_unavailability(
        &mut self,
        employee_id: &str,
        scope: UnavailabilityScope,
        shifts: &[&str],
        severity: Severity,
        penalty: u32,
    ) -> Result<&Unavailability, DomainError> {
        let roster_id = self
            .employee(employee_id)
            .ok_or_else(|| DomainError::UnknownEmployee {
                id: employee_id.to_owned(),
            })?
            .roster_id
            .clone();
        let roster = self
            .roster(&roster_id)
            .ok_or_else(|| DomainError::DanglingReference {
                kind: "employee roster",
                id: roster_id.clone(),
            })?;

        let mut shift_ids = Vec::with_capacity(shifts.len());
        for shift_id in shifts {
            if shift_id.trim().is_empty() {
                return Err(DomainError::BlankName);
            }
            if roster.shift(shift_id).is_none() {
                return Err(DomainError::ShiftOutsideRoster {
                    shift_id: (*shift_id).to_owned(),
                    roster_id: roster_id.clone(),
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

    /// Add a weekly fixed assignment to an employee.
    ///
    /// The shift must belong to the employee's roster; recurring assignments
    /// are demand the scheduler must satisfy, not a restriction.
    pub fn add_recurring_assignment(
        &mut self,
        employee_id: &str,
        weekday: Weekday,
        shift_id: &str,
    ) -> Result<&RecurringAssignment, DomainError> {
        let roster_id = self
            .employee(employee_id)
            .ok_or_else(|| DomainError::UnknownEmployee {
                id: employee_id.to_owned(),
            })?
            .roster_id
            .clone();
        let roster = self
            .roster(&roster_id)
            .ok_or_else(|| DomainError::DanglingReference {
                kind: "employee roster",
                id: roster_id.clone(),
            })?;

        if shift_id.trim().is_empty() {
            return Err(DomainError::BlankName);
        }
        if roster.shift(shift_id).is_none() {
            return Err(DomainError::ShiftOutsideRoster {
                shift_id: shift_id.to_owned(),
                roster_id,
            });
        }

        let assignment = RecurringAssignment::new(weekday, shift_id)?;
        let employee = self
            .employee_mut(employee_id)
            .expect("employee was resolved above");
        employee.recurring_assignments.push(assignment);

        Ok(employee
            .recurring_assignments
            .last()
            .expect("a recurring assignment was just pushed"))
    }

    /// Add a minimum-rest rule between two shifts of one roster.
    ///
    /// The shifts may be the same template: a night shift followed by the next
    /// day's night shift is a valid rest rule.
    pub fn add_rest_rule(
        &mut self,
        roster_id: &str,
        preceding_shift_id: &str,
        following_shift_id: &str,
        min_rest_minutes: u32,
        severity: Severity,
        penalty: u32,
    ) -> Result<&RestRule, DomainError> {
        let roster = self
            .roster(roster_id)
            .ok_or_else(|| DomainError::UnknownRoster {
                id: roster_id.to_owned(),
            })?;

        for shift_id in [preceding_shift_id, following_shift_id] {
            if shift_id.trim().is_empty() {
                return Err(DomainError::BlankName);
            }
            if roster.shift(shift_id).is_none() {
                return Err(DomainError::ShiftOutsideRoster {
                    shift_id: shift_id.to_owned(),
                    roster_id: roster_id.to_owned(),
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
        let roster = self
            .roster_mut(roster_id)
            .expect("roster was resolved above");
        roster.push_rest_rule(rule);

        Ok(roster
            .rest_rules
            .last()
            .expect("a rest rule was just pushed"))
    }

    /// Set which weekdays a shift template occurs on.
    ///
    /// An empty slice resets the template to every day. Duplicates are removed
    /// and the result is sorted Monday-first so plans and JSON stay stable.
    pub fn set_shift_weekdays(
        &mut self,
        roster_id: &str,
        shift_id: &str,
        weekdays: &[Weekday],
    ) -> Result<(), DomainError> {
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

    /// Expand a roster's templates into a generated plan for a date range.
    ///
    /// Context employees must belong to the roster and their assignments must
    /// fall before the plan start. Every position referenced by a staffing
    /// requirement must exist organization-wide. The plan is appended to the
    /// roster and starts in [`PlanStatus::Draft`].
    pub fn generate_plan(
        &mut self,
        roster_id: &str,
        start: NaiveDate,
        end: NaiveDate,
        context: Vec<ContextAssignment>,
    ) -> Result<&RosterPlan, DomainError> {
        let roster = self
            .roster(roster_id)
            .ok_or_else(|| DomainError::UnknownRoster {
                id: roster_id.to_owned(),
            })?;

        for assignment in &context {
            let employee = self.employee(&assignment.employee_id).ok_or_else(|| {
                DomainError::DanglingReference {
                    kind: "context employee",
                    id: assignment.employee_id.clone(),
                }
            })?;
            if employee.roster_id != roster_id {
                return Err(DomainError::EmployeeOutsideRoster {
                    employee_id: assignment.employee_id.clone(),
                    roster_id: roster_id.to_owned(),
                });
            }
            if assignment.date >= start {
                return Err(DomainError::ContextDateOutOfRange {
                    date: assignment.date,
                    plan_start: start,
                });
            }
        }

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

        let plan = RosterPlan::generate(roster, start, end, context)?;
        let roster = self
            .roster_mut(roster_id)
            .expect("roster was resolved above");
        roster.push_plan(plan);

        Ok(roster.plans.last().expect("a plan was just pushed"))
    }

    /// Find a generated plan by roster and plan ID.
    pub fn plan(&self, roster_id: &str, plan_id: &str) -> Option<&RosterPlan> {
        self.roster(roster_id)?.plan(plan_id)
    }

    /// Find a generated plan by roster and plan ID for mutation.
    pub fn plan_mut(&mut self, roster_id: &str, plan_id: &str) -> Option<&mut RosterPlan> {
        self.roster_mut(roster_id)?.plan_mut(plan_id)
    }

    /// Assign an employee to one seat of a dated shift.
    ///
    /// Structural rules only: the plan must not be finalized, the employee
    /// must exist, belong to the plan's roster, hold the slot's position, and
    /// not already hold another seat in the same dated shift. Overlap, leave,
    /// rest, and fairness are Phase 3.
    ///
    /// Assigning to an already occupied slot replaces the previous employee.
    pub fn assign_slot(
        &mut self,
        roster_id: &str,
        plan_id: &str,
        dated_shift_id: &str,
        slot_id: &str,
        employee_id: &str,
    ) -> Result<(), DomainError> {
        let (employee_roster, employee_position) = {
            let employee =
                self.employee(employee_id)
                    .ok_or_else(|| DomainError::UnknownEmployee {
                        id: employee_id.to_owned(),
                    })?;
            (employee.roster_id.clone(), employee.position_id.clone())
        };

        let (position_id, already_assigned) = {
            let plan = self
                .roster(roster_id)
                .ok_or_else(|| DomainError::UnknownRoster {
                    id: roster_id.to_owned(),
                })?
                .plan(plan_id)
                .ok_or_else(|| DomainError::UnknownPlan {
                    id: plan_id.to_owned(),
                })?;
            if plan.status == PlanStatus::Finalized {
                return Err(DomainError::PlanFinalized {
                    plan_id: plan_id.to_owned(),
                });
            }
            let dated =
                plan.dated_shift(dated_shift_id)
                    .ok_or_else(|| DomainError::UnknownDatedShift {
                        id: dated_shift_id.to_owned(),
                    })?;
            let slot = dated
                .slot(slot_id)
                .ok_or_else(|| DomainError::UnknownSlot {
                    id: slot_id.to_owned(),
                })?;
            let already_assigned = dated.slots.iter().any(|other| {
                other.id != slot_id && other.employee_id.as_deref() == Some(employee_id)
            });
            (slot.position_id.clone(), already_assigned)
        };

        if employee_roster != roster_id {
            return Err(DomainError::EmployeeOutsideRoster {
                employee_id: employee_id.to_owned(),
                roster_id: roster_id.to_owned(),
            });
        }
        if employee_position != position_id {
            return Err(DomainError::PositionMismatch {
                employee_id: employee_id.to_owned(),
                expected_position_id: position_id,
                actual_position_id: employee_position,
            });
        }
        if already_assigned {
            return Err(DomainError::EmployeeAlreadyAssigned {
                dated_shift_id: dated_shift_id.to_owned(),
                employee_id: employee_id.to_owned(),
            });
        }

        let plan = self
            .roster_mut(roster_id)
            .expect("roster was resolved above")
            .plan_mut(plan_id)
            .expect("plan was resolved above");
        let dated = plan
            .dated_shift_mut(dated_shift_id)
            .expect("dated shift was resolved above");
        let slot = dated.slot_mut(slot_id).expect("slot was resolved above");
        slot.employee_id = Some(employee_id.to_owned());
        plan.updated_at = Utc::now();

        Ok(())
    }

    /// Clear an assignment from one seat.
    ///
    /// Finalized plans are immutable.
    pub fn unassign_slot(
        &mut self,
        roster_id: &str,
        plan_id: &str,
        dated_shift_id: &str,
        slot_id: &str,
    ) -> Result<(), DomainError> {
        let plan = self
            .roster(roster_id)
            .ok_or_else(|| DomainError::UnknownRoster {
                id: roster_id.to_owned(),
            })?
            .plan(plan_id)
            .ok_or_else(|| DomainError::UnknownPlan {
                id: plan_id.to_owned(),
            })?;
        if plan.status == PlanStatus::Finalized {
            return Err(DomainError::PlanFinalized {
                plan_id: plan_id.to_owned(),
            });
        }
        plan.dated_shift(dated_shift_id)
            .ok_or_else(|| DomainError::UnknownDatedShift {
                id: dated_shift_id.to_owned(),
            })?
            .slot(slot_id)
            .ok_or_else(|| DomainError::UnknownSlot {
                id: slot_id.to_owned(),
            })?;

        let plan = self
            .roster_mut(roster_id)
            .expect("roster was resolved above")
            .plan_mut(plan_id)
            .expect("plan was resolved above");
        let dated = plan
            .dated_shift_mut(dated_shift_id)
            .expect("dated shift was resolved above");
        let slot = dated.slot_mut(slot_id).expect("slot was resolved above");
        slot.employee_id = None;
        plan.updated_at = Utc::now();

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

            let mut rest_rule_ids = HashSet::new();
            for rule in &roster.rest_rules {
                if !rest_rule_ids.insert(rule.id.as_str()) {
                    return Err(DomainError::DuplicateId {
                        kind: "rest rule",
                        id: rule.id.clone(),
                    });
                }
                for shift_id in [&rule.preceding_shift_id, &rule.following_shift_id] {
                    if roster.shift(shift_id).is_none() {
                        return Err(DomainError::ShiftOutsideRoster {
                            shift_id: shift_id.clone(),
                            roster_id: roster.id.clone(),
                        });
                    }
                }
            }

            let mut plan_ids = HashSet::new();
            for plan in &roster.plans {
                if !plan_ids.insert(plan.id.as_str()) {
                    return Err(DomainError::DuplicateId {
                        kind: "plan",
                        id: plan.id.clone(),
                    });
                }
                if plan.end_date < plan.start_date {
                    return Err(DomainError::InvalidDateRange {
                        start: plan.start_date,
                        end: plan.end_date,
                    });
                }

                for context in &plan.context {
                    let employee = self.employee(&context.employee_id).ok_or_else(|| {
                        DomainError::DanglingReference {
                            kind: "context employee",
                            id: context.employee_id.clone(),
                        }
                    })?;
                    if employee.roster_id != roster.id {
                        return Err(DomainError::EmployeeOutsideRoster {
                            employee_id: context.employee_id.clone(),
                            roster_id: roster.id.clone(),
                        });
                    }
                    if context.date >= plan.start_date {
                        return Err(DomainError::ContextDateOutOfRange {
                            date: context.date,
                            plan_start: plan.start_date,
                        });
                    }
                }

                let mut dated_ids = HashSet::new();
                for dated in &plan.shifts {
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
                            let employee = self.employee(employee_id).ok_or_else(|| {
                                DomainError::DanglingReference {
                                    kind: "slot employee",
                                    id: employee_id.clone(),
                                }
                            })?;
                            if employee.roster_id != roster.id {
                                return Err(DomainError::EmployeeOutsideRoster {
                                    employee_id: employee_id.clone(),
                                    roster_id: roster.id.clone(),
                                });
                            }
                            if employee.position_id != slot.position_id {
                                return Err(DomainError::PositionMismatch {
                                    employee_id: employee_id.clone(),
                                    expected_position_id: slot.position_id.clone(),
                                    actual_position_id: employee.position_id.clone(),
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

        for employee in &self.employees {
            let roster =
                self.roster(&employee.roster_id)
                    .ok_or_else(|| DomainError::DanglingReference {
                        kind: "employee roster",
                        id: employee.roster_id.clone(),
                    })?;
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
                    if roster.shift(shift_id).is_none() {
                        return Err(DomainError::ShiftOutsideRoster {
                            shift_id: shift_id.clone(),
                            roster_id: roster.id.clone(),
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
                if roster.shift(&assignment.shift_id).is_none() {
                    return Err(DomainError::ShiftOutsideRoster {
                        shift_id: assignment.shift_id.clone(),
                        roster_id: roster.id.clone(),
                    });
                }
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
            unavailability: Vec::new(),
            recurring_assignments: Vec::new(),
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

    fn date(year: i32, month: u32, day: u32) -> chrono::NaiveDate {
        chrono::NaiveDate::from_ymd_opt(year, month, day).unwrap()
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
        let (mut organization, doctors, nurses) = organization_with_rosters();
        let position = organization
            .add_position("Medical Officer")
            .unwrap()
            .id
            .clone();
        let opd = organization
            .add_shift(&doctors, "OPD", at(8, 0), at(14, 0))
            .unwrap()
            .id
            .clone();
        let morning = organization
            .add_shift(&doctors, "Emergency Morning", at(8, 0), at(15, 0))
            .unwrap()
            .id
            .clone();
        let night = organization
            .add_shift(&doctors, "Emergency Night", at(15, 0), at(8, 0))
            .unwrap()
            .id
            .clone();
        let nurse_day = organization
            .add_shift(&nurses, "Day", at(8, 0), at(16, 0))
            .unwrap()
            .id
            .clone();
        let amit = organization
            .add_employee("Dr. Amit", &doctors, &position)
            .unwrap()
            .id
            .clone();

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
                &[fixture.nurse_day.as_str()],
                Severity::Hard,
                0
            ),
            Err(DomainError::ShiftOutsideRoster { .. })
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
            Err(DomainError::ShiftOutsideRoster { .. })
        ));
        assert!(matches!(
            fixture.organization.add_recurring_assignment(
                &fixture.amit,
                Weekday::Sun,
                &fixture.nurse_day
            ),
            Err(DomainError::ShiftOutsideRoster { .. })
        ));
        assert!(matches!(
            fixture.organization.add_rest_rule(
                "missing-roster",
                &fixture.night,
                &fixture.morning,
                480,
                Severity::Hard,
                0
            ),
            Err(DomainError::UnknownRoster { .. })
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
            Err(DomainError::ShiftOutsideRoster { .. })
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

        // A rule that points at another roster's shift.
        fixture.organization.employees[0].unavailability[0].shifts =
            vec![fixture.nurse_day.clone()];
        assert!(matches!(
            fixture.organization.validate(),
            Err(DomainError::ShiftOutsideRoster { .. })
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

        // A rest rule that points at another roster's shift.
        fixture.organization.rosters[0].rest_rules[0].following_shift_id =
            fixture.nurse_day.clone();
        assert!(matches!(
            fixture.organization.validate(),
            Err(DomainError::ShiftOutsideRoster { .. })
        ));
    }

    #[derive(Clone)]
    struct PlanFixture {
        organization: Organization,
        doctors: String,
        amit: String,
        khushi: String,
        wrong_position: String,
        rina: String,
        plan_id: String,
        dated_shift_id: String,
        first_slot: String,
        second_slot: String,
    }

    fn plan_fixture() -> PlanFixture {
        let (mut organization, doctors, nurses) = organization_with_rosters();
        let medical_officer = organization
            .add_position("Medical Officer")
            .expect("position")
            .id
            .clone();
        let staff_nurse = organization
            .add_position("Senior Staff Nurse")
            .expect("position")
            .id
            .clone();

        let opd = organization
            .add_shift(&doctors, "OPD", at(8, 0), at(14, 0))
            .expect("shift")
            .id
            .clone();
        let night = organization
            .add_shift(&doctors, "Emergency Night", at(15, 0), at(8, 0))
            .expect("shift")
            .id
            .clone();
        organization
            .require_position(&doctors, &opd, &medical_officer, 2)
            .expect("requirement");
        organization
            .require_position(&doctors, &night, &medical_officer, 1)
            .expect("requirement");

        let amit = organization
            .add_employee("Dr. Amit", &doctors, &medical_officer)
            .expect("employee")
            .id
            .clone();
        let khushi = organization
            .add_employee("Dr. Khushi", &doctors, &medical_officer)
            .expect("employee")
            .id
            .clone();
        let wrong_position = organization
            .add_employee("Dr. Wrong", &doctors, &staff_nurse)
            .expect("employee")
            .id
            .clone();
        let rina = organization
            .add_employee("Nurse Rina", &nurses, &staff_nurse)
            .expect("employee")
            .id
            .clone();

        let plan_id = organization
            .generate_plan(&doctors, date(2026, 11, 16), date(2026, 11, 16), Vec::new())
            .expect("plan")
            .id
            .clone();
        let dated_shift_id = organization
            .plan(&doctors, &plan_id)
            .expect("plan")
            .shifts
            .iter()
            .find(|shift| shift.name == "OPD")
            .expect("OPD dated shift")
            .id
            .clone();
        let slots = &organization
            .plan(&doctors, &plan_id)
            .expect("plan")
            .dated_shift(&dated_shift_id)
            .expect("OPD dated shift")
            .slots;
        let first_slot = slots[0].id.clone();
        let second_slot = slots[1].id.clone();

        PlanFixture {
            organization,
            doctors,
            amit,
            khushi,
            wrong_position,
            rina,
            plan_id,
            dated_shift_id,
            first_slot,
            second_slot,
        }
    }

    #[test]
    fn generates_plans_with_weekday_restrictions_and_validates_context() {
        let mut fixture = plan_fixture();
        assert_eq!(fixture.organization.validate(), Ok(()));

        // Dedupe + Monday-first sort, and the restriction affects generation.
        let opd_id = fixture
            .organization
            .roster(&fixture.doctors)
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
            .roster(&fixture.doctors)
            .unwrap()
            .shifts[0]
            .weekdays;
        assert_eq!(opd_weekdays, &vec![Weekday::Fri]);

        // 2026-11-16 is a Monday, so a Friday-only OPD template is absent.
        let monday_plan = fixture
            .organization
            .generate_plan(
                &fixture.doctors,
                date(2026, 11, 16),
                date(2026, 11, 16),
                Vec::new(),
            )
            .expect("plan");
        assert!(!monday_plan.shifts.iter().any(|shift| shift.name == "OPD"));
        let friday_plan = fixture
            .organization
            .generate_plan(
                &fixture.doctors,
                date(2026, 11, 20),
                date(2026, 11, 20),
                Vec::new(),
            )
            .expect("plan");
        assert!(friday_plan.shifts.iter().any(|shift| shift.name == "OPD"));

        assert!(matches!(
            fixture.organization.generate_plan(
                "missing-roster",
                date(2026, 11, 16),
                date(2026, 11, 16),
                Vec::new()
            ),
            Err(DomainError::UnknownRoster { .. })
        ));
        assert!(matches!(
            fixture.organization.generate_plan(
                &fixture.doctors,
                date(2026, 11, 30),
                date(2026, 11, 16),
                Vec::new()
            ),
            Err(DomainError::InvalidDateRange { .. })
        ));
        assert!(matches!(
            fixture.organization.generate_plan(
                &fixture.doctors,
                date(2026, 11, 16),
                date(2026, 11, 16),
                vec![ContextAssignment {
                    date: date(2026, 11, 15),
                    name: "Emergency Night".to_owned(),
                    start: at(15, 0),
                    end: at(8, 0),
                    employee_id: fixture.rina.clone(),
                }]
            ),
            Err(DomainError::EmployeeOutsideRoster { .. })
        ));
        assert!(matches!(
            fixture.organization.generate_plan(
                &fixture.doctors,
                date(2026, 11, 16),
                date(2026, 11, 16),
                vec![ContextAssignment {
                    date: date(2026, 11, 16),
                    name: "Emergency Night".to_owned(),
                    start: at(15, 0),
                    end: at(8, 0),
                    employee_id: fixture.amit.clone(),
                }]
            ),
            Err(DomainError::ContextDateOutOfRange { .. })
        ));
    }

    #[test]
    fn assigns_and_unassigns_individual_seats() {
        let mut fixture = plan_fixture();

        fixture
            .organization
            .assign_slot(
                &fixture.doctors,
                &fixture.plan_id,
                &fixture.dated_shift_id,
                &fixture.first_slot,
                &fixture.amit,
            )
            .unwrap();

        let plan = fixture
            .organization
            .plan(&fixture.doctors, &fixture.plan_id)
            .expect("plan");
        assert_eq!(
            plan.slot(&fixture.dated_shift_id, &fixture.first_slot)
                .expect("first slot")
                .employee_id
                .as_deref(),
            Some(fixture.amit.as_str())
        );
        assert_eq!(
            plan.slot(&fixture.dated_shift_id, &fixture.second_slot)
                .expect("second slot")
                .employee_id,
            None
        );
        assert_eq!(plan.filled_slots(), 1);

        // A different employee can take the occupied seat.
        fixture
            .organization
            .assign_slot(
                &fixture.doctors,
                &fixture.plan_id,
                &fixture.dated_shift_id,
                &fixture.first_slot,
                &fixture.khushi,
            )
            .unwrap();
        assert_eq!(
            fixture
                .organization
                .plan(&fixture.doctors, &fixture.plan_id)
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
                &fixture.plan_id,
                &fixture.dated_shift_id,
                &fixture.first_slot,
            )
            .unwrap();
        assert_eq!(
            fixture
                .organization
                .plan(&fixture.doctors, &fixture.plan_id)
                .unwrap()
                .slot(&fixture.dated_shift_id, &fixture.first_slot)
                .unwrap()
                .employee_id,
            None
        );

        assert_eq!(fixture.organization.validate(), Ok(()));
    }

    #[test]
    fn rejects_invalid_assignments_and_finalized_plans() {
        let mut fixture = plan_fixture();

        assert!(matches!(
            fixture.organization.assign_slot(
                &fixture.doctors,
                &fixture.plan_id,
                &fixture.dated_shift_id,
                &fixture.first_slot,
                "missing-employee"
            ),
            Err(DomainError::UnknownEmployee { .. })
        ));
        assert!(matches!(
            fixture.organization.assign_slot(
                "missing-roster",
                &fixture.plan_id,
                &fixture.dated_shift_id,
                &fixture.first_slot,
                &fixture.amit
            ),
            Err(DomainError::UnknownRoster { .. })
        ));
        assert!(matches!(
            fixture.organization.assign_slot(
                &fixture.doctors,
                "missing-plan",
                &fixture.dated_shift_id,
                &fixture.first_slot,
                &fixture.amit
            ),
            Err(DomainError::UnknownPlan { .. })
        ));
        assert!(matches!(
            fixture.organization.assign_slot(
                &fixture.doctors,
                &fixture.plan_id,
                "missing-shift",
                &fixture.first_slot,
                &fixture.amit
            ),
            Err(DomainError::UnknownDatedShift { .. })
        ));
        assert!(matches!(
            fixture.organization.assign_slot(
                &fixture.doctors,
                &fixture.plan_id,
                &fixture.dated_shift_id,
                "missing-slot",
                &fixture.amit
            ),
            Err(DomainError::UnknownSlot { .. })
        ));
        assert!(matches!(
            fixture.organization.assign_slot(
                &fixture.doctors,
                &fixture.plan_id,
                &fixture.dated_shift_id,
                &fixture.first_slot,
                &fixture.rina
            ),
            Err(DomainError::EmployeeOutsideRoster { .. })
        ));
        assert!(matches!(
            fixture.organization.assign_slot(
                &fixture.doctors,
                &fixture.plan_id,
                &fixture.dated_shift_id,
                &fixture.first_slot,
                &fixture.wrong_position
            ),
            Err(DomainError::PositionMismatch { .. })
        ));

        fixture
            .organization
            .assign_slot(
                &fixture.doctors,
                &fixture.plan_id,
                &fixture.dated_shift_id,
                &fixture.first_slot,
                &fixture.amit,
            )
            .unwrap();
        assert!(matches!(
            fixture.organization.assign_slot(
                &fixture.doctors,
                &fixture.plan_id,
                &fixture.dated_shift_id,
                &fixture.second_slot,
                &fixture.amit
            ),
            Err(DomainError::EmployeeAlreadyAssigned { .. })
        ));

        fixture
            .organization
            .plan_mut(&fixture.doctors, &fixture.plan_id)
            .unwrap()
            .status = PlanStatus::Finalized;
        assert!(matches!(
            fixture.organization.assign_slot(
                &fixture.doctors,
                &fixture.plan_id,
                &fixture.dated_shift_id,
                &fixture.second_slot,
                &fixture.khushi
            ),
            Err(DomainError::PlanFinalized { .. })
        ));
        assert!(matches!(
            fixture.organization.unassign_slot(
                &fixture.doctors,
                &fixture.plan_id,
                &fixture.dated_shift_id,
                &fixture.first_slot
            ),
            Err(DomainError::PlanFinalized { .. })
        ));
    }

    #[test]
    fn validate_catches_invalid_plan_data() {
        let base = plan_fixture();
        assert_eq!(base.organization.validate(), Ok(()));

        // Reversed plan range.
        let mut fixture = base.clone();
        fixture
            .organization
            .plan_mut(&fixture.doctors, &fixture.plan_id)
            .unwrap()
            .end_date = date(2026, 11, 15);
        assert!(matches!(
            fixture.organization.validate(),
            Err(DomainError::InvalidDateRange { .. })
        ));

        // Slot pointing at a missing position.
        let mut fixture = base.clone();
        fixture
            .organization
            .plan_mut(&fixture.doctors, &fixture.plan_id)
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

        // Assignment pointing at a missing employee.
        let mut fixture = base.clone();
        fixture
            .organization
            .plan_mut(&fixture.doctors, &fixture.plan_id)
            .unwrap()
            .dated_shift_mut(&fixture.dated_shift_id)
            .unwrap()
            .slots[0]
            .employee_id = Some("missing-employee".to_owned());
        assert!(matches!(
            fixture.organization.validate(),
            Err(DomainError::DanglingReference {
                kind: "slot employee",
                ..
            })
        ));

        // Assignment from another roster.
        let mut fixture = base.clone();
        fixture
            .organization
            .plan_mut(&fixture.doctors, &fixture.plan_id)
            .unwrap()
            .dated_shift_mut(&fixture.dated_shift_id)
            .unwrap()
            .slots[0]
            .employee_id = Some(fixture.rina.clone());
        assert!(matches!(
            fixture.organization.validate(),
            Err(DomainError::EmployeeOutsideRoster { .. })
        ));

        // Assignment with the wrong position.
        let mut fixture = base.clone();
        fixture
            .organization
            .plan_mut(&fixture.doctors, &fixture.plan_id)
            .unwrap()
            .dated_shift_mut(&fixture.dated_shift_id)
            .unwrap()
            .slots[0]
            .employee_id = Some(fixture.wrong_position.clone());
        assert!(matches!(
            fixture.organization.validate(),
            Err(DomainError::PositionMismatch { .. })
        ));

        // The same employee in two seats of one dated shift.
        let mut fixture = base.clone();
        {
            let plan = fixture
                .organization
                .plan_mut(&fixture.doctors, &fixture.plan_id)
                .unwrap();
            let dated = plan.dated_shift_mut(&fixture.dated_shift_id).unwrap();
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
            let plan = fixture
                .organization
                .plan_mut(&fixture.doctors, &fixture.plan_id)
                .unwrap();
            let dated = plan.dated_shift_mut(&fixture.dated_shift_id).unwrap();
            dated.slots[1].id = dated.slots[0].id.clone();
        }
        assert!(matches!(
            fixture.organization.validate(),
            Err(DomainError::DuplicateId { kind: "slot", .. })
        ));

        // Duplicate dated-shift ids inside one plan.
        let mut fixture = base.clone();
        {
            let plan = fixture
                .organization
                .plan_mut(&fixture.doctors, &fixture.plan_id)
                .unwrap();
            let first = plan.shifts[0].id.clone();
            plan.shifts[1].id = first;
        }
        assert!(matches!(
            fixture.organization.validate(),
            Err(DomainError::DuplicateId {
                kind: "dated shift",
                ..
            })
        ));

        // Duplicate plan ids on one roster.
        let mut fixture = base.clone();
        fixture
            .organization
            .generate_plan(
                &fixture.doctors,
                date(2026, 11, 17),
                date(2026, 11, 17),
                Vec::new(),
            )
            .unwrap();
        {
            let roster = fixture.organization.roster_mut(&fixture.doctors).unwrap();
            roster.plans[1].id = roster.plans[0].id.clone();
        }
        assert!(matches!(
            fixture.organization.validate(),
            Err(DomainError::DuplicateId { kind: "plan", .. })
        ));

        // Context on or after the plan start.
        let mut fixture = base.clone();
        fixture
            .organization
            .plan_mut(&fixture.doctors, &fixture.plan_id)
            .unwrap()
            .context
            .push(ContextAssignment {
                date: date(2026, 11, 16),
                name: "Emergency Night".to_owned(),
                start: at(15, 0),
                end: at(8, 0),
                employee_id: fixture.amit.clone(),
            });
        assert!(matches!(
            fixture.organization.validate(),
            Err(DomainError::ContextDateOutOfRange { .. })
        ));

        // Context employee from another roster.
        let mut fixture = base.clone();
        fixture
            .organization
            .plan_mut(&fixture.doctors, &fixture.plan_id)
            .unwrap()
            .context
            .push(ContextAssignment {
                date: date(2026, 11, 15),
                name: "Emergency Night".to_owned(),
                start: at(15, 0),
                end: at(8, 0),
                employee_id: fixture.rina.clone(),
            });
        assert!(matches!(
            fixture.organization.validate(),
            Err(DomainError::EmployeeOutsideRoster { .. })
        ));
    }
}

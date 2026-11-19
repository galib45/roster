//! Dated rosters: concrete schedules with dated shifts, per-seat slots, and
//! assignments.
//!
//! A [`ShiftTemplate`](super::ShiftTemplate) is a reusable rule owned by a
//! [`RosterTemplate`]. A [`Roster`] is one concrete schedule for a date range:
//! each template occurrence becomes a [`DatedShift`], and every required seat
//! becomes a [`ShiftSlot`]. Headcount 2 means two slots, not one slot with a
//! count.
//!
//! Creating a roster snapshots its member employees and dated shifts, so later
//! template or staffing edits never rewrite an existing roster.
//! [`DatedShift::template_id`] remains as provenance only.

use chrono::{DateTime, Datelike, NaiveDate, NaiveTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::shift::duration_minutes;
use super::{DomainError, RosterTemplate, ShiftTemplate};

/// Lifecycle of a dated roster.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanStatus {
    /// Created but not yet solved.
    Draft,
    /// The solver has filled what it can.
    Generated,
    /// Locked; assignments may no longer change.
    Finalized,
}

/// A member snapshotted onto a dated roster.
///
/// The position is captured at creation time so later employee changes do not
/// rewrite which seats the member is eligible for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RosterMember {
    pub employee_id: String,
    pub position_id: String,
}

impl RosterMember {
    pub fn new(employee_id: impl Into<String>, position_id: impl Into<String>) -> Self {
        Self {
            employee_id: employee_id.into(),
            position_id: position_id.into(),
        }
    }
}

/// A read-only assignment carried over from the day before the roster starts.
///
/// The admin supplies context; the domain does not infer it from an existing
/// roster. It exists so Phase 3 can evaluate rest rules across the boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextAssignment {
    /// The date the shift started (the day before the roster start).
    pub date: NaiveDate,
    pub name: String,
    pub start: NaiveTime,
    pub end: NaiveTime,
    pub employee_id: String,
    /// The shift template this shift came from, when known.
    ///
    /// Optional so older files stay valid. It lets rest and unavailability rules
    /// match previous-day context the same way they match dated shifts.
    #[serde(default)]
    pub template_id: Option<String>,
}

/// One fillable seat on a dated shift.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShiftSlot {
    pub id: String,
    /// The organization-level position this seat requires.
    pub position_id: String,
    /// The assigned employee, or `None` while the seat is unfilled.
    pub employee_id: Option<String>,
}

impl ShiftSlot {
    /// Create an unfilled slot for a position.
    pub fn new(position_id: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            position_id: position_id.into(),
            employee_id: None,
        }
    }

    /// True when an employee occupies this seat.
    pub fn is_filled(&self) -> bool {
        self.employee_id.is_some()
    }
}

/// One concrete occurrence of a shift template on a date.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatedShift {
    pub id: String,
    /// The template this occurrence came from. Provenance only.
    pub template_id: String,
    /// The date the shift starts.
    pub date: NaiveDate,
    /// Snapshot of the template name.
    pub name: String,
    /// Snapshot of the template start time.
    pub start: NaiveTime,
    /// Snapshot of the template end time.
    pub end: NaiveTime,
    /// One slot per required seat.
    pub slots: Vec<ShiftSlot>,
}

impl DatedShift {
    /// Instantiate a template on a date, producing one slot per required seat.
    pub fn from_template(template: &ShiftTemplate, date: NaiveDate) -> Self {
        let slots = template
            .required_positions
            .iter()
            .flat_map(|requirement| {
                (0..requirement.headcount).map(|_| ShiftSlot::new(requirement.position_id.clone()))
            })
            .collect();

        Self {
            id: Uuid::new_v4().to_string(),
            template_id: template.id.clone(),
            date,
            name: template.name.clone(),
            start: template.start,
            end: template.end,
            slots,
        }
    }

    /// Length of this occurrence in minutes, wrapping across midnight.
    pub fn duration_minutes(&self) -> u32 {
        duration_minutes(self.start, self.end)
    }

    /// True when this occurrence ends on the following calendar day.
    pub fn crosses_midnight(&self) -> bool {
        self.end < self.start
    }

    /// Find a slot by ID.
    pub fn slot(&self, id: &str) -> Option<&ShiftSlot> {
        self.slots.iter().find(|slot| slot.id == id)
    }

    /// Find a slot by ID for mutation. Crate-visible so `Organization` stays
    /// the validated entry point.
    pub(crate) fn slot_mut(&mut self, id: &str) -> Option<&mut ShiftSlot> {
        self.slots.iter_mut().find(|slot| slot.id == id)
    }
}

/// A dated roster for one inclusive date range.
///
/// A template may keep several rosters (for example November and December),
/// each with its own status, snapshot members, context, and dated shifts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Roster {
    pub id: String,
    /// The template this roster was created from. Provenance only.
    pub template_id: String,
    pub name: String,
    pub start: NaiveDate,
    pub end: NaiveDate,
    pub status: PlanStatus,
    /// Employees snapshotted at creation, with their positions.
    #[serde(default)]
    pub members: Vec<RosterMember>,
    /// The day before `start`, supplied by the admin.
    #[serde(default)]
    pub context: Vec<ContextAssignment>,
    pub shifts: Vec<DatedShift>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Roster {
    /// Create a dated roster from a template for an inclusive range.
    ///
    /// Templates occur on the weekdays listed in
    /// [`ShiftTemplate::weekdays`](super::ShiftTemplate::weekdays); an empty
    /// list means every day. The roster starts in [`PlanStatus::Draft`] and
    /// every slot is unfilled.
    #[allow(clippy::too_many_arguments)]
    pub fn create(
        template: &RosterTemplate,
        name: impl Into<String>,
        start: NaiveDate,
        end: NaiveDate,
        members: Vec<RosterMember>,
        context: Vec<ContextAssignment>,
    ) -> Result<Self, DomainError> {
        if end < start {
            return Err(DomainError::InvalidDateRange { start, end });
        }

        let mut shifts = Vec::new();
        let mut date = start;
        while date <= end {
            for shift in &template.shifts {
                if shift.occurs_on(date.weekday()) {
                    shifts.push(DatedShift::from_template(shift, date));
                }
            }

            match date.succ_opt() {
                Some(next) => date = next,
                None => break,
            }
        }

        let now = Utc::now();
        Ok(Self {
            id: Uuid::new_v4().to_string(),
            template_id: template.id.clone(),
            name: name.into(),
            start,
            end,
            status: PlanStatus::Draft,
            members,
            context,
            shifts,
            created_at: now,
            updated_at: now,
        })
    }

    /// Find a snapshot member by employee ID.
    pub fn member(&self, employee_id: &str) -> Option<&RosterMember> {
        self.members
            .iter()
            .find(|member| member.employee_id == employee_id)
    }

    /// Find a dated shift by ID.
    pub fn dated_shift(&self, id: &str) -> Option<&DatedShift> {
        self.shifts.iter().find(|shift| shift.id == id)
    }

    /// Find a dated shift by ID for mutation.
    pub(crate) fn dated_shift_mut(&mut self, id: &str) -> Option<&mut DatedShift> {
        self.shifts.iter_mut().find(|shift| shift.id == id)
    }

    /// Find a slot by its dated-shift and slot IDs.
    pub fn slot(&self, dated_shift_id: &str, slot_id: &str) -> Option<&ShiftSlot> {
        self.dated_shift(dated_shift_id)?.slot(slot_id)
    }

    /// Assign an employee to one seat of a dated shift.
    ///
    /// Structural rules only: the roster must not be finalized, the employee
    /// must be a snapshot member, the member's snapshot position must match the
    /// slot, and the employee must not already hold another seat in the same
    /// dated shift. Assigning to an occupied slot replaces the previous
    /// employee.
    pub fn assign_slot(
        &mut self,
        dated_shift_id: &str,
        slot_id: &str,
        employee_id: &str,
    ) -> Result<(), DomainError> {
        if self.status == PlanStatus::Finalized {
            return Err(DomainError::RosterFinalized {
                roster_id: self.id.clone(),
            });
        }

        let (member_position, position_id, already_assigned) = {
            let member =
                self.member(employee_id)
                    .ok_or_else(|| DomainError::EmployeeNotInRoster {
                        employee_id: employee_id.to_owned(),
                        roster_id: self.id.clone(),
                    })?;
            let dated =
                self.dated_shift(dated_shift_id)
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

        let dated = self
            .dated_shift_mut(dated_shift_id)
            .expect("dated shift was resolved above");
        let slot = dated.slot_mut(slot_id).expect("slot was resolved above");
        slot.employee_id = Some(employee_id.to_owned());
        self.updated_at = Utc::now();

        Ok(())
    }

    /// Clear an assignment from one seat.
    ///
    /// Finalized rosters are immutable.
    pub fn unassign_slot(
        &mut self,
        dated_shift_id: &str,
        slot_id: &str,
    ) -> Result<(), DomainError> {
        if self.status == PlanStatus::Finalized {
            return Err(DomainError::RosterFinalized {
                roster_id: self.id.clone(),
            });
        }

        self.dated_shift(dated_shift_id)
            .ok_or_else(|| DomainError::UnknownDatedShift {
                id: dated_shift_id.to_owned(),
            })?
            .slot(slot_id)
            .ok_or_else(|| DomainError::UnknownSlot {
                id: slot_id.to_owned(),
            })?;

        let dated = self
            .dated_shift_mut(dated_shift_id)
            .expect("dated shift was resolved above");
        let slot = dated.slot_mut(slot_id).expect("slot was resolved above");
        slot.employee_id = None;
        self.updated_at = Utc::now();

        Ok(())
    }

    /// Total number of seats across every dated shift.
    pub fn total_slots(&self) -> usize {
        self.shifts.iter().map(|shift| shift.slots.len()).sum()
    }

    /// Number of seats that currently have an employee.
    pub fn filled_slots(&self) -> usize {
        self.shifts
            .iter()
            .flat_map(|shift| &shift.slots)
            .filter(|slot| slot.is_filled())
            .count()
    }
}

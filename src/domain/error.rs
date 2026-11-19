//! Errors produced by domain validation.
//!
//! Every public mutation validates its inputs and returns one of these instead
//! of storing a value the rest of the application cannot rely on. The messages
//! include the offending value so the UI can surface them verbatim.

use std::fmt;

use chrono::{NaiveDate, NaiveTime};

use super::Severity;

/// Reasons a domain operation was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainError {
    /// A name was empty after trimming.
    BlankName,
    /// Two roster templates in one organization share a name.
    DuplicateRosterTemplate { name: String },
    /// Two positions in one organization share a name.
    DuplicatePosition { name: String },
    /// Two shifts in one template share a name.
    DuplicateShift { name: String },
    /// Two sharing groups in one template share a name.
    DuplicateSharingGroup { name: String },
    /// An entity ID does not match any roster template.
    UnknownRosterTemplate { id: String },
    /// An entity ID does not match any dated roster.
    UnknownRoster { id: String },
    /// An entity ID does not match any position.
    UnknownPosition { id: String },
    /// An entity ID does not match any shift.
    UnknownShift { id: String },
    /// An entity ID does not match any employee.
    UnknownEmployee { id: String },
    /// A shift starts and ends at the same time.
    InvalidShiftTimes { start: NaiveTime, end: NaiveTime },
    /// A staffing requirement asked for zero people.
    EmptyHeadcount {
        shift_id: String,
        position_id: String,
    },
    /// A sharing group was created without any members.
    EmptySharingGroup { name: String },
    /// An employee already belongs to another sharing group in the template.
    EmployeeAlreadyGrouped {
        employee_id: String,
        group_id: String,
    },
    /// A stored reference points at an entity that does not exist.
    DanglingReference { kind: &'static str, id: String },
    /// A date range ends before it starts.
    InvalidDateRange { start: NaiveDate, end: NaiveDate },
    /// A soft rule has no penalty, or a hard rule has one.
    InvalidPenalty { severity: Severity, penalty: u32 },
    /// A rest rule asks for zero minutes of rest.
    InvalidRestGap {
        preceding_shift_id: String,
        following_shift_id: String,
    },
    /// A shift is not owned by the roster template it was used with.
    ShiftOutsideTemplate {
        shift_id: String,
        template_id: String,
    },
    /// A template covers a position, but that position does not resolve.
    PositionNotInTemplate {
        position_id: String,
        template_id: String,
    },
    /// An employee is not a member of the template or dated roster.
    EmployeeNotInTemplate {
        employee_id: String,
        template_id: String,
    },
    /// An employee is not a snapshot member of the dated roster.
    EmployeeNotInRoster {
        employee_id: String,
        roster_id: String,
    },
    /// An entity ID does not match any dated shift.
    UnknownDatedShift { id: String },
    /// An entity ID does not match any shift slot.
    UnknownSlot { id: String },
    /// A slot's position does not match the member's snapshot position.
    PositionMismatch {
        employee_id: String,
        expected_position_id: String,
        actual_position_id: String,
    },
    /// The dated roster is finalized and may not change.
    RosterFinalized { roster_id: String },
    /// The employee already holds another seat in the same dated shift.
    EmployeeAlreadyAssigned {
        dated_shift_id: String,
        employee_id: String,
    },
    /// Previous-day context falls on or after the roster start.
    ContextDateOutOfRange {
        date: NaiveDate,
        roster_start: NaiveDate,
    },
}

impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BlankName => write!(f, "name cannot be blank"),
            Self::DuplicateRosterTemplate { name } => {
                write!(f, "a roster template named {name:?} already exists")
            }
            Self::DuplicatePosition { name } => {
                write!(f, "a position named {name:?} already exists")
            }
            Self::DuplicateShift { name } => {
                write!(f, "a shift named {name:?} already exists in this template")
            }
            Self::DuplicateSharingGroup { name } => {
                write!(
                    f,
                    "a sharing group named {name:?} already exists in this template"
                )
            }
            Self::UnknownRosterTemplate { id } => write!(f, "unknown roster template {id:?}"),
            Self::UnknownRoster { id } => write!(f, "unknown roster {id:?}"),
            Self::UnknownPosition { id } => write!(f, "unknown position {id:?}"),
            Self::UnknownShift { id } => write!(f, "unknown shift {id:?}"),
            Self::UnknownEmployee { id } => write!(f, "unknown employee {id:?}"),
            Self::InvalidShiftTimes { start, end } => write!(
                f,
                "shift start and end must differ (both are {start} / {end})"
            ),
            Self::EmptyHeadcount {
                shift_id,
                position_id,
            } => write!(
                f,
                "shift {shift_id:?} requires position {position_id:?} with a headcount of at least 1"
            ),
            Self::EmptySharingGroup { name } => {
                write!(f, "sharing group {name:?} needs at least one member")
            }
            Self::EmployeeAlreadyGrouped {
                employee_id,
                group_id,
            } => write!(
                f,
                "employee {employee_id:?} already belongs to sharing group {group_id:?}"
            ),
            Self::DanglingReference { kind, id } => {
                write!(f, "{kind} reference {id:?} does not resolve")
            }
            Self::InvalidDateRange { start, end } => {
                write!(f, "date range ends before it starts ({start}..={end})")
            }
            Self::InvalidPenalty { severity, penalty } => match severity {
                Severity::Soft => write!(
                    f,
                    "a soft rule needs a penalty of at least 1 (got {penalty})"
                ),
                Severity::Hard => write!(f, "a hard rule cannot have a penalty (got {penalty})"),
            },
            Self::InvalidRestGap {
                preceding_shift_id,
                following_shift_id,
            } => write!(
                f,
                "rest rule {preceding_shift_id:?} -> {following_shift_id:?} requires a positive rest gap"
            ),
            Self::ShiftOutsideTemplate {
                shift_id,
                template_id,
            } => write!(
                f,
                "shift {shift_id:?} does not belong to roster template {template_id:?}"
            ),
            Self::PositionNotInTemplate {
                position_id,
                template_id,
            } => write!(
                f,
                "roster template {template_id:?} does not include position {position_id:?}"
            ),
            Self::EmployeeNotInTemplate {
                employee_id,
                template_id,
            } => write!(
                f,
                "employee {employee_id:?} is not a member of roster template {template_id:?}"
            ),
            Self::EmployeeNotInRoster {
                employee_id,
                roster_id,
            } => write!(
                f,
                "employee {employee_id:?} is not a member of roster {roster_id:?}"
            ),
            Self::UnknownDatedShift { id } => write!(f, "unknown dated shift {id:?}"),
            Self::UnknownSlot { id } => write!(f, "unknown shift slot {id:?}"),
            Self::PositionMismatch {
                employee_id,
                expected_position_id,
                actual_position_id,
            } => write!(
                f,
                "employee {employee_id:?} holds position {actual_position_id:?}, but the slot requires {expected_position_id:?}"
            ),
            Self::RosterFinalized { roster_id } => {
                write!(f, "roster {roster_id:?} is finalized and cannot change")
            }
            Self::EmployeeAlreadyAssigned {
                dated_shift_id,
                employee_id,
            } => write!(
                f,
                "employee {employee_id:?} is already assigned to dated shift {dated_shift_id:?}"
            ),
            Self::ContextDateOutOfRange { date, roster_start } => write!(
                f,
                "context assignment on {date} must be before the roster start {roster_start}"
            ),
        }
    }
}

impl std::error::Error for DomainError {}

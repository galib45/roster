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
    /// Two rosters in one organization share a name.
    DuplicateRoster { name: String },
    /// Two positions in one organization share a name.
    DuplicatePosition { name: String },
    /// Two shifts in one roster share a name.
    DuplicateShift { name: String },
    /// An entity ID does not match any roster.
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
    /// Two entities of the same kind share an ID.
    DuplicateId { kind: &'static str, id: String },
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
    /// A rule references a shift that belongs to a different roster.
    ShiftOutsideRoster { shift_id: String, roster_id: String },
}

impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BlankName => write!(f, "name cannot be blank"),
            Self::DuplicateRoster { name } => {
                write!(f, "a roster named {name:?} already exists")
            }
            Self::DuplicatePosition { name } => {
                write!(f, "a position named {name:?} already exists")
            }
            Self::DuplicateShift { name } => {
                write!(f, "a shift named {name:?} already exists in this roster")
            }
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
            Self::DuplicateId { kind, id } => {
                write!(f, "duplicate {kind} id {id:?}")
            }
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
            Self::ShiftOutsideRoster {
                shift_id,
                roster_id,
            } => write!(
                f,
                "shift {shift_id:?} does not belong to roster {roster_id:?}"
            ),
        }
    }
}

impl std::error::Error for DomainError {}

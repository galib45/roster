//! Scheduling rules: unavailability, leave, recurring assignments, rest rules.
//!
//! These types describe *what* restricts a roster. They deliberately contain no
//! evaluation logic; Phase 3 decides whether a given assignment violates a rule
//! and how much a soft violation costs.
//!
//! Shift references always name shift *templates* inside one roster. An empty
//! `shifts` list on [`Unavailability`] means "every shift in the employee's
//! roster"; leave covers every shift on every date in its inclusive range.

use std::fmt;

use chrono::{NaiveDate, Weekday};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::DomainError;

/// How a rule is enforced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// A violation makes a roster invalid.
    Hard,
    /// A violation is allowed but adds `penalty` to the objective.
    Soft,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Hard => write!(f, "hard"),
            Self::Soft => write!(f, "soft"),
        }
    }
}

/// When an unavailability rule applies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum UnavailabilityScope {
    /// Repeats every week on one weekday.
    Weekly { weekday: Weekday },
    /// Applies to one calendar date.
    Date { date: NaiveDate },
    /// Applies to every date in an inclusive range.
    Leave { start: NaiveDate, end: NaiveDate },
}

/// A period when an employee cannot (or prefers not to) work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Unavailability {
    pub id: String,
    pub scope: UnavailabilityScope,
    /// Shift template ids in the employee's roster; empty means every shift.
    #[serde(default)]
    pub shifts: Vec<String>,
    pub severity: Severity,
    /// Objective cost for a soft rule; always 0 for hard rules.
    #[serde(default)]
    pub penalty: u32,
}

impl Unavailability {
    /// Build a rule, validating the date range and the severity/penalty pairing.
    ///
    /// Shift ids are stored as given; the employee does not see the
    /// organization's templates.
    pub fn new(
        scope: UnavailabilityScope,
        shifts: Vec<String>,
        severity: Severity,
        penalty: u32,
    ) -> Result<Self, DomainError> {
        if let UnavailabilityScope::Leave { start, end } = scope
            && end < start
        {
            return Err(DomainError::InvalidDateRange { start, end });
        }
        validate_penalty(severity, penalty)?;

        Ok(Self {
            id: Uuid::new_v4().to_string(),
            scope,
            shifts,
            severity,
            penalty,
        })
    }
}

/// A fixed assignment that repeats every week.
///
/// For example, "Dr. Amit works Emergency Night every Sunday". This is a **soft
/// preference**, not demand: if the employee is not scheduled on that shift the
/// plan costs `penalty` but stays feasible, so fairness can win when leave
/// shrinks the available pool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecurringAssignment {
    pub id: String,
    pub weekday: Weekday,
    pub shift_id: String,
    /// Objective cost when the preference is unmet. At least 1.
    #[serde(default = "default_recurring_penalty")]
    pub penalty: u32,
}

/// Default penalty for recurring preferences stored without one.
fn default_recurring_penalty() -> u32 {
    1
}

impl RecurringAssignment {
    /// Build a recurring preference with the default penalty of 1.
    pub fn new(weekday: Weekday, shift_id: impl Into<String>) -> Result<Self, DomainError> {
        Self::with_penalty(weekday, shift_id, default_recurring_penalty())
    }

    /// Build a recurring preference with an explicit penalty.
    ///
    /// A penalty of 0 is rejected: recurring assignments are always soft, so
    /// they must cost something when unmet.
    pub fn with_penalty(
        weekday: Weekday,
        shift_id: impl Into<String>,
        penalty: u32,
    ) -> Result<Self, DomainError> {
        let shift_id = shift_id.into();
        if shift_id.trim().is_empty() {
            return Err(DomainError::BlankName);
        }
        if penalty == 0 {
            return Err(DomainError::InvalidPenalty {
                severity: Severity::Soft,
                penalty,
            });
        }

        Ok(Self {
            id: Uuid::new_v4().to_string(),
            weekday,
            shift_id,
            penalty,
        })
    }
}

/// A minimum-rest rule between two shift templates of one roster.
///
/// `min_rest_minutes` is the gap required between the end of an occurrence of
/// `preceding_shift_id` and the start of an occurrence of
/// `following_shift_id`. Same-day and next-day evaluation are Phase 3.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestRule {
    pub id: String,
    pub preceding_shift_id: String,
    pub following_shift_id: String,
    pub min_rest_minutes: u32,
    pub severity: Severity,
    /// Objective cost for a soft rule; always 0 for hard rules.
    #[serde(default)]
    pub penalty: u32,
}

impl RestRule {
    /// Build a rest rule. Shift references are validated by the owning
    /// [`RosterTemplate`](super::RosterTemplate).
    pub fn new(
        preceding_shift_id: impl Into<String>,
        following_shift_id: impl Into<String>,
        min_rest_minutes: u32,
        severity: Severity,
        penalty: u32,
    ) -> Result<Self, DomainError> {
        let preceding_shift_id = preceding_shift_id.into();
        let following_shift_id = following_shift_id.into();

        if preceding_shift_id.trim().is_empty() || following_shift_id.trim().is_empty() {
            return Err(DomainError::BlankName);
        }
        if min_rest_minutes == 0 {
            return Err(DomainError::InvalidRestGap {
                preceding_shift_id,
                following_shift_id,
            });
        }
        validate_penalty(severity, penalty)?;

        Ok(Self {
            id: Uuid::new_v4().to_string(),
            preceding_shift_id,
            following_shift_id,
            min_rest_minutes,
            severity,
            penalty,
        })
    }
}

/// Enforce the severity/penalty pairing used by every rule type.
fn validate_penalty(severity: Severity, penalty: u32) -> Result<(), DomainError> {
    match severity {
        Severity::Hard if penalty == 0 => Ok(()),
        Severity::Soft if penalty > 0 => Ok(()),
        _ => Err(DomainError::InvalidPenalty { severity, penalty }),
    }
}

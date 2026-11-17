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
    /// Shift ids are validated against the employee's roster by
    /// [`Organization`](super::Organization), which is the only type that can
    /// see the roster.
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
/// For example, "Dr. Amit works Emergency Night every Sunday". Recurring
/// assignments are demand, not a constraint: the scheduler must place the
/// employee on that shift.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecurringAssignment {
    pub id: String,
    pub weekday: Weekday,
    pub shift_id: String,
}

impl RecurringAssignment {
    /// Build a recurring assignment. The shift reference is validated by
    /// [`Organization`](super::Organization).
    pub fn new(weekday: Weekday, shift_id: impl Into<String>) -> Result<Self, DomainError> {
        let shift_id = shift_id.into();
        if shift_id.trim().is_empty() {
            return Err(DomainError::BlankName);
        }

        Ok(Self {
            id: Uuid::new_v4().to_string(),
            weekday,
            shift_id,
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
    /// Build a rest rule. Shift references are validated against the owning
    /// roster by [`Organization`](super::Organization).
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).unwrap()
    }

    #[test]
    fn constructs_each_scope_and_round_trips_its_json_shape() {
        let weekly = Unavailability::new(
            UnavailabilityScope::Weekly {
                weekday: Weekday::Fri,
            },
            Vec::new(),
            Severity::Hard,
            0,
        )
        .unwrap();
        let json = serde_json::to_value(&weekly).unwrap();
        assert_eq!(json["scope"]["kind"], "weekly");
        assert_eq!(json["scope"]["weekday"], "Fri");
        assert_eq!(json["severity"], "hard");
        assert_eq!(json["penalty"], 0);
        assert_eq!(json["shifts"], serde_json::json!([]));

        let on_date = Unavailability::new(
            UnavailabilityScope::Date {
                date: date(2026, 11, 16),
            },
            vec!["shift-1".to_owned()],
            Severity::Soft,
            2,
        )
        .unwrap();
        let json = serde_json::to_value(&on_date).unwrap();
        assert_eq!(json["scope"]["kind"], "date");
        assert_eq!(json["scope"]["date"], "2026-11-16");
        assert_eq!(json["severity"], "soft");
        assert_eq!(json["penalty"], 2);

        let leave = Unavailability::new(
            UnavailabilityScope::Leave {
                start: date(2026, 11, 16),
                end: date(2026, 11, 30),
            },
            Vec::new(),
            Severity::Hard,
            0,
        )
        .unwrap();
        let json = serde_json::to_value(&leave).unwrap();
        assert_eq!(json["scope"]["kind"], "leave");
        assert_eq!(json["scope"]["start"], "2026-11-16");
        assert_eq!(json["scope"]["end"], "2026-11-30");
    }

    #[test]
    fn rejects_reversed_leave_ranges() {
        let error = Unavailability::new(
            UnavailabilityScope::Leave {
                start: date(2026, 11, 30),
                end: date(2026, 11, 16),
            },
            Vec::new(),
            Severity::Hard,
            0,
        )
        .unwrap_err();

        assert_eq!(
            error,
            DomainError::InvalidDateRange {
                start: date(2026, 11, 30),
                end: date(2026, 11, 16)
            }
        );
    }

    #[test]
    fn enforces_the_severity_penalty_pairing() {
        let scope = UnavailabilityScope::Weekly {
            weekday: Weekday::Mon,
        };

        assert_eq!(
            Unavailability::new(scope.clone(), Vec::new(), Severity::Soft, 0).unwrap_err(),
            DomainError::InvalidPenalty {
                severity: Severity::Soft,
                penalty: 0
            }
        );
        assert_eq!(
            Unavailability::new(scope.clone(), Vec::new(), Severity::Hard, 5).unwrap_err(),
            DomainError::InvalidPenalty {
                severity: Severity::Hard,
                penalty: 5
            }
        );
        assert!(Unavailability::new(scope, Vec::new(), Severity::Soft, 1).is_ok());
    }

    #[test]
    fn rest_rules_require_a_positive_gap_and_penalty_pairing() {
        assert_eq!(
            RestRule::new("night", "morning", 0, Severity::Hard, 0).unwrap_err(),
            DomainError::InvalidRestGap {
                preceding_shift_id: "night".to_owned(),
                following_shift_id: "morning".to_owned()
            }
        );
        assert_eq!(
            RestRule::new("night", "morning", 480, Severity::Soft, 0).unwrap_err(),
            DomainError::InvalidPenalty {
                severity: Severity::Soft,
                penalty: 0
            }
        );

        let rule = RestRule::new("night", "morning", 480, Severity::Soft, 3).unwrap();
        assert_eq!(rule.min_rest_minutes, 480);
        assert_eq!(rule.penalty, 3);
    }

    #[test]
    fn recurring_assignments_reject_blank_shift_ids() {
        assert_eq!(
            RecurringAssignment::new(Weekday::Sun, "   ").unwrap_err(),
            DomainError::BlankName
        );
        assert!(RecurringAssignment::new(Weekday::Sun, "shift-1").is_ok());
    }

    #[test]
    fn rejects_unknown_enum_values_on_deserialize() {
        let error = serde_json::from_value::<Severity>(serde_json::json!("sometimes")).unwrap_err();
        assert!(error.to_string().contains("unknown variant"));

        let error = serde_json::from_value::<UnavailabilityScope>(serde_json::json!({
            "kind": "fortnightly"
        }))
        .unwrap_err();
        assert!(error.to_string().contains("unknown variant"));
    }
}

//! Pure constraint evaluation over dated rosters.
//!
//! This layer reads a [`Roster`](crate::domain::Roster) and reports hard
//! violations and weighted soft penalties. It never mutates the project and
//! never calls the solver, so the same semantics can score live edits (Phase 7)
//! and drive the CP model (Phase 4).
//!
//! Unfilled slots are reported as coverage counts, not violations: the fairest
//! solution may legitimately leave seats empty. Stored rule data decides
//! severity and penalty; this module does not hardcode policy.

mod fairness;
mod rules;

use std::fmt;

use chrono::NaiveDate;

use crate::domain::{DomainError, Organization, Severity};

pub use fairness::{EmployeeWorkload, GroupWorkload, WorkloadReport, workload};
pub(crate) use rules::{interval, intervals_overlap, rule_matches};

/// Tunable weights for rules that are not stored on the domain entities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvalOptions {
    /// Penalty per extra shift when an employee works more than one shift that
    /// starts on the same date. Zero disables the rule.
    pub multi_shift_penalty: u32,
}

impl Default for EvalOptions {
    fn default() -> Self {
        Self {
            multi_shift_penalty: 1,
        }
    }
}

/// Identifies one assignment in the roster or in the previous-day context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShiftRef {
    /// A dated shift inside the roster.
    Dated { id: String },
    /// A previous-day context assignment supplied by the admin.
    Context { date: NaiveDate, name: String },
}

impl fmt::Display for ShiftRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Dated { id } => write!(f, "dated shift {id}"),
            Self::Context { date, name } => write!(f, "context {name} on {date}"),
        }
    }
}

/// Why an assignment combination is illegal or costly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViolationKind {
    /// Two assignments for one employee overlap in time.
    Overlap {
        employee_id: String,
        first: ShiftRef,
        second: ShiftRef,
    },
    /// An assignment falls inside an unavailability or leave rule.
    Unavailable {
        employee_id: String,
        rule_id: String,
        shift: ShiftRef,
    },
    /// A recurring preference is not satisfied.
    ///
    /// Recurring assignments are soft preferences, never demand: this costs the
    /// assignment's penalty but keeps the roster feasible so fairness can win.
    UnmetRecurringPreference {
        employee_id: String,
        assignment_id: String,
        date: NaiveDate,
        template_id: String,
    },
    /// Two assignments matched by a rest rule are too close together.
    InsufficientRest {
        employee_id: String,
        rule_id: String,
        preceding: ShiftRef,
        following: ShiftRef,
        gap_minutes: i64,
    },
    /// An employee works more than one shift that starts on the same date.
    MultipleShiftsInDay {
        employee_id: String,
        date: NaiveDate,
        shift_count: usize,
    },
}

/// One rule outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    pub severity: Severity,
    /// Objective cost. Always 0 for hard violations.
    pub penalty: u32,
    pub kind: ViolationKind,
}

impl Violation {
    fn employee_id(&self) -> &str {
        match &self.kind {
            ViolationKind::Overlap { employee_id, .. }
            | ViolationKind::Unavailable { employee_id, .. }
            | ViolationKind::UnmetRecurringPreference { employee_id, .. }
            | ViolationKind::InsufficientRest { employee_id, .. }
            | ViolationKind::MultipleShiftsInDay { employee_id, .. } => employee_id,
        }
    }

    /// Stable ordering for tests and the UI.
    fn sort_key(&self) -> (String, u8, String) {
        let (rank, reference) = match &self.kind {
            ViolationKind::Overlap { first, second, .. } => (0, format!("{first}|{second}")),
            ViolationKind::Unavailable { rule_id, shift, .. } => (1, format!("{rule_id}|{shift}")),
            ViolationKind::UnmetRecurringPreference {
                assignment_id,
                date,
                template_id,
                ..
            } => (2, format!("{assignment_id}|{date}|{template_id}")),
            ViolationKind::InsufficientRest {
                rule_id,
                preceding,
                following,
                ..
            } => (3, format!("{rule_id}|{preceding}|{following}")),
            ViolationKind::MultipleShiftsInDay {
                date, shift_count, ..
            } => (4, format!("{date}|{shift_count}")),
        };
        (self.employee_id().to_owned(), rank, reference)
    }
}

/// Result of evaluating one dated roster.
#[derive(Debug, Clone, PartialEq)]
pub struct Evaluation {
    pub violations: Vec<Violation>,
    pub total_slots: usize,
    pub unfilled_slots: usize,
    /// Workload and fairness measurement for the same roster.
    pub workload: WorkloadReport,
}

impl Evaluation {
    /// Number of hard violations.
    pub fn hard_count(&self) -> usize {
        self.violations
            .iter()
            .filter(|violation| violation.severity == Severity::Hard)
            .count()
    }

    /// Sum of soft penalties.
    pub fn soft_penalty(&self) -> u64 {
        self.violations
            .iter()
            .filter(|violation| violation.severity == Severity::Soft)
            .map(|violation| u64::from(violation.penalty))
            .sum()
    }

    /// True when the roster has no hard violations.
    pub fn is_feasible(&self) -> bool {
        self.hard_count() == 0
    }
}

/// Evaluate one dated roster.
///
/// Returns `UnknownRosterTemplate` or `UnknownRoster` when the IDs do not
/// resolve.
pub fn evaluate(
    organization: &Organization,
    template_id: &str,
    roster_id: &str,
    options: &EvalOptions,
) -> Result<Evaluation, DomainError> {
    let template = organization.roster_template(template_id).ok_or_else(|| {
        DomainError::UnknownRosterTemplate {
            id: template_id.to_owned(),
        }
    })?;
    let roster = template
        .roster(roster_id)
        .ok_or_else(|| DomainError::UnknownRoster {
            id: roster_id.to_owned(),
        })?;

    let assignments = rules::collect_assignments(roster);
    let mut violations = rules::violations(organization, template, roster, &assignments, options);
    violations.sort_by_key(Violation::sort_key);

    let total_slots = roster.total_slots();
    let unfilled_slots = roster
        .shifts
        .iter()
        .flat_map(|shift| &shift.slots)
        .filter(|slot| !slot.is_filled())
        .count();

    Ok(Evaluation {
        violations,
        total_slots,
        unfilled_slots,
        workload: fairness::workload(organization, template, roster),
    })
}

//! Rule checks over collected assignments.
//!
//! Everything here is read-only. The public surface lives in [`super`].

use chrono::{Datelike, NaiveDate, NaiveDateTime, NaiveTime};

use crate::domain::{
    Organization, RestRule, Roster, RosterTemplate, Severity, Unavailability, UnavailabilityScope,
};

use super::{EvalOptions, ShiftRef, Violation, ViolationKind};

/// One filled seat, resolved into a concrete time interval.
#[derive(Debug, Clone)]
pub(crate) struct Assignment {
    pub employee_id: String,
    pub shift: ShiftRef,
    pub template_id: Option<String>,
    pub start: NaiveDateTime,
    pub end: NaiveDateTime,
    pub date: NaiveDate,
}

impl Assignment {
    fn interval(&self) -> (NaiveDateTime, NaiveDateTime) {
        (self.start, self.end)
    }
}

/// Collect every filled slot and context assignment into time intervals.
pub(crate) fn collect_assignments(roster: &Roster) -> Vec<Assignment> {
    let mut assignments = Vec::new();

    for dated in &roster.shifts {
        let (start, end) = interval(dated.date, dated.start, dated.end);
        for slot in &dated.slots {
            if let Some(employee_id) = &slot.employee_id {
                assignments.push(Assignment {
                    employee_id: employee_id.clone(),
                    shift: ShiftRef::Dated {
                        id: dated.id.clone(),
                    },
                    template_id: Some(dated.template_id.clone()),
                    start,
                    end,
                    date: dated.date,
                });
            }
        }
    }

    for context in &roster.context {
        let (start, end) = interval(context.date, context.start, context.end);
        assignments.push(Assignment {
            employee_id: context.employee_id.clone(),
            shift: ShiftRef::Context {
                date: context.date,
                name: context.name.clone(),
            },
            template_id: context.template_id.clone(),
            start,
            end,
            date: context.date,
        });
    }

    assignments
}

pub(crate) fn interval(
    date: NaiveDate,
    start: NaiveTime,
    end: NaiveTime,
) -> (NaiveDateTime, NaiveDateTime) {
    let start = date.and_time(start);
    let end_date = if end <= start.time() {
        date.succ_opt().unwrap_or(date)
    } else {
        date
    };
    (start, end_date.and_time(end))
}

/// Shared overlap test, so the evaluator and the solver agree on midnight wrap
/// and on touching intervals never overlapping.
pub(crate) fn intervals_overlap(
    first: (NaiveDateTime, NaiveDateTime),
    second: (NaiveDateTime, NaiveDateTime),
) -> bool {
    first.0 < second.1 && second.0 < first.1
}

fn overlaps(first: &Assignment, second: &Assignment) -> bool {
    intervals_overlap(first.interval(), second.interval())
}

pub(crate) fn violations(
    organization: &Organization,
    template: &RosterTemplate,
    roster: &Roster,
    assignments: &[Assignment],
    options: &EvalOptions,
) -> Vec<Violation> {
    let mut violations = Vec::new();
    violations.extend(overlap_violations(assignments));
    violations.extend(unavailability_violations(organization, assignments));
    violations.extend(recurring_preference_violations(
        organization,
        roster,
        assignments,
    ));
    violations.extend(rest_violations(template, assignments));
    violations.extend(multi_shift_violations(assignments, options));

    violations
}

fn overlap_violations(assignments: &[Assignment]) -> Vec<Violation> {
    let mut violations = Vec::new();

    for (index, first) in assignments.iter().enumerate() {
        for second in assignments.iter().skip(index + 1) {
            if first.employee_id != second.employee_id || !overlaps(first, second) {
                continue;
            }

            let (first, second) = if first.start <= second.start {
                (first, second)
            } else {
                (second, first)
            };
            violations.push(Violation {
                severity: Severity::Hard,
                penalty: 0,
                kind: ViolationKind::Overlap {
                    employee_id: first.employee_id.clone(),
                    first: first.shift.clone(),
                    second: second.shift.clone(),
                },
            });
        }
    }

    violations
}

/// True when a rule's scope covers the date and its shift list matches.
pub(crate) fn rule_matches(
    rule: &Unavailability,
    date: NaiveDate,
    template_id: Option<&str>,
) -> bool {
    let in_scope = match &rule.scope {
        UnavailabilityScope::Weekly { weekday } => date.weekday() == *weekday,
        UnavailabilityScope::Date { date: rule_date } => date == *rule_date,
        UnavailabilityScope::Leave { start, end } => (*start..=*end).contains(&date),
    };
    if !in_scope {
        return false;
    }

    rule.shifts.is_empty()
        || template_id.is_some_and(|template_id| rule.shifts.iter().any(|id| id == template_id))
}

fn unavailability_violations(
    organization: &Organization,
    assignments: &[Assignment],
) -> Vec<Violation> {
    let mut violations = Vec::new();

    for assignment in assignments {
        let Some(employee) = organization.employee(&assignment.employee_id) else {
            continue;
        };

        for rule in &employee.unavailability {
            if !rule_matches(rule, assignment.date, assignment.template_id.as_deref()) {
                continue;
            }
            violations.push(Violation {
                severity: rule.severity,
                penalty: rule.penalty,
                kind: ViolationKind::Unavailable {
                    employee_id: assignment.employee_id.clone(),
                    rule_id: rule.id.clone(),
                    shift: assignment.shift.clone(),
                },
            });
        }
    }

    violations
}

/// True when a hard unavailability or leave rule covers the date.
///
/// Staff on leave are not penalized for missing a recurring preference: with a
/// smaller pool the fair choice is to schedule someone else.
fn hard_rule_covers(employee_rules: &[Unavailability], date: NaiveDate) -> bool {
    employee_rules.iter().any(|rule| {
        rule.severity == Severity::Hard && rule_matches(rule, date, /* template */ None)
    })
}

fn recurring_preference_violations(
    organization: &Organization,
    roster: &Roster,
    assignments: &[Assignment],
) -> Vec<Violation> {
    let mut violations = Vec::new();

    for member in &roster.members {
        let Some(employee) = organization.employee(&member.employee_id) else {
            continue;
        };
        for preference in &employee.recurring_assignments {
            for dated in &roster.shifts {
                if dated.template_id != preference.shift_id
                    || dated.date.weekday() != preference.weekday
                {
                    continue;
                }
                // Staff on hard leave are never penalized for a missed
                // preference; fairness picks someone else instead.
                if hard_rule_covers(&employee.unavailability, dated.date) {
                    continue;
                }
                if assignments.iter().any(|assignment| {
                    assignment.employee_id == employee.id
                        && matches!(&assignment.shift, ShiftRef::Dated { id } if *id == dated.id)
                }) {
                    continue;
                }

                violations.push(Violation {
                    severity: Severity::Soft,
                    penalty: preference.penalty,
                    kind: ViolationKind::UnmetRecurringPreference {
                        employee_id: employee.id.clone(),
                        assignment_id: preference.id.clone(),
                        date: dated.date,
                        template_id: dated.template_id.clone(),
                    },
                });
            }
        }
    }

    violations
}

fn rest_violations(template: &RosterTemplate, assignments: &[Assignment]) -> Vec<Violation> {
    let mut violations = Vec::new();

    for rule in &template.rest_rules {
        let preceding: Vec<&Assignment> = assignments
            .iter()
            .filter(|assignment| {
                assignment.template_id.as_deref() == Some(rule.preceding_shift_id.as_str())
            })
            .collect();
        let following: Vec<&Assignment> = assignments
            .iter()
            .filter(|assignment| {
                assignment.template_id.as_deref() == Some(rule.following_shift_id.as_str())
            })
            .collect();

        for earlier in &preceding {
            for later in &following {
                if earlier.employee_id != later.employee_id || earlier.start >= later.start {
                    continue;
                }

                let gap_minutes = (later.start - earlier.end).num_minutes();
                if gap_minutes < 0 || gap_minutes >= i64::from(rule.min_rest_minutes) {
                    continue;
                }

                violations.push(rest_violation(rule, earlier, later, gap_minutes));
            }
        }
    }

    violations
}

fn rest_violation(
    rule: &RestRule,
    preceding: &Assignment,
    following: &Assignment,
    gap_minutes: i64,
) -> Violation {
    Violation {
        severity: rule.severity,
        penalty: rule.penalty,
        kind: ViolationKind::InsufficientRest {
            employee_id: preceding.employee_id.clone(),
            rule_id: rule.id.clone(),
            preceding: preceding.shift.clone(),
            following: following.shift.clone(),
            gap_minutes,
        },
    }
}

fn multi_shift_violations(assignments: &[Assignment], options: &EvalOptions) -> Vec<Violation> {
    if options.multi_shift_penalty == 0 {
        return Vec::new();
    }

    let mut violations = Vec::new();
    let mut seen: Vec<(String, NaiveDate)> = Vec::new();

    for assignment in assignments {
        let key = (assignment.employee_id.clone(), assignment.date);
        if seen.contains(&key) {
            continue;
        }
        seen.push(key.clone());

        let count = assignments
            .iter()
            .filter(|candidate| candidate.employee_id == key.0 && candidate.date == key.1)
            .count();
        if count <= 1 {
            continue;
        }

        violations.push(Violation {
            severity: Severity::Soft,
            penalty: options.multi_shift_penalty * (count as u32 - 1),
            kind: ViolationKind::MultipleShiftsInDay {
                employee_id: key.0,
                date: key.1,
                shift_count: count,
            },
        });
    }

    violations
}

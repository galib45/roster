//! Rule checks over collected assignments.
//!
//! Everything here is read-only. The public surface lives in [`super`].

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};

use crate::domain::{Organization, Roster, RosterPlan};

use super::{ShiftRef, Violation, ViolationKind};

/// One filled seat, resolved into a concrete time interval.
#[derive(Debug, Clone)]
pub(crate) struct Assignment {
    pub employee_id: String,
    pub shift: ShiftRef,
    // Used by the unavailability and rest checks in the next task.
    #[allow(dead_code)]
    pub template_id: Option<String>,
    pub start: NaiveDateTime,
    pub end: NaiveDateTime,
    #[allow(dead_code)]
    pub date: NaiveDate,
}

impl Assignment {
    fn interval(&self) -> (NaiveDateTime, NaiveDateTime) {
        (self.start, self.end)
    }
}

/// Collect every filled slot and context assignment into time intervals.
pub(crate) fn collect_assignments(plan: &RosterPlan) -> Vec<Assignment> {
    let mut assignments = Vec::new();

    for dated in &plan.shifts {
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

    for context in &plan.context {
        let (start, end) = interval(context.date, context.start, context.end);
        assignments.push(Assignment {
            employee_id: context.employee_id.clone(),
            shift: ShiftRef::Context {
                date: context.date,
                name: context.name.clone(),
            },
            template_id: None,
            start,
            end,
            date: context.date,
        });
    }

    assignments
}

fn interval(date: NaiveDate, start: NaiveTime, end: NaiveTime) -> (NaiveDateTime, NaiveDateTime) {
    let start = date.and_time(start);
    let end_date = if end <= start.time() {
        date.succ_opt().unwrap_or(date)
    } else {
        date
    };
    (start, end_date.and_time(end))
}

fn overlaps(first: &Assignment, second: &Assignment) -> bool {
    let (first_start, first_end) = first.interval();
    let (second_start, second_end) = second.interval();
    first_start < second_end && second_start < first_end
}

pub(crate) fn violations(
    organization: &Organization,
    roster: &Roster,
    plan: &RosterPlan,
    assignments: &[Assignment],
    _options: &super::EvalOptions,
) -> Vec<Violation> {
    let _ = (organization, roster, plan);
    overlap_violations(assignments)
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
                severity: crate::domain::Severity::Hard,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).unwrap()
    }

    fn time(hour: u32, minute: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(hour, minute, 0).unwrap()
    }

    fn assignment(start_date: NaiveDate, start: NaiveTime, end: NaiveTime) -> Assignment {
        let (start, end) = interval(start_date, start, end);
        Assignment {
            employee_id: "employee-1".to_owned(),
            shift: ShiftRef::Dated {
                id: "shift-1".to_owned(),
            },
            template_id: None,
            start,
            end,
            date: start_date,
        }
    }

    #[test]
    fn overnight_intervals_end_on_the_next_day() {
        let overnight = assignment(date(2026, 11, 16), time(15, 0), time(8, 0));
        assert_eq!(overnight.end.date(), date(2026, 11, 17));
    }

    #[test]
    fn touching_intervals_do_not_overlap() {
        let morning = assignment(date(2026, 11, 16), time(8, 0), time(15, 0));
        let night = assignment(date(2026, 11, 16), time(15, 0), time(8, 0));
        assert!(!overlaps(&morning, &night));
    }

    #[test]
    fn containing_intervals_overlap() {
        let opd = assignment(date(2026, 11, 16), time(8, 0), time(14, 0));
        let morning = assignment(date(2026, 11, 16), time(8, 0), time(15, 0));
        assert!(overlaps(&opd, &morning));
    }
}

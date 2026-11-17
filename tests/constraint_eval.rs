//! End-to-end coverage of constraint evaluation through the public API.
//!
//! The November 2026 example is reused: OPD needs two Medical Officers every day
//! except Friday, and the two emergency shifts need one each day. Evaluation is
//! read-only, so every test asserts on a generated plan and never repairs it.

use chrono::{NaiveDate, NaiveTime, Weekday};
use roster::domain::{ContextAssignment, Organization, ProjectFile, Severity, UnavailabilityScope};
use roster::eval::{EvalOptions, Evaluation, ShiftRef, ViolationKind, evaluate};

fn at(hour: u32, minute: u32) -> NaiveTime {
    NaiveTime::from_hms_opt(hour, minute, 0).unwrap()
}

fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).unwrap()
}

struct Fixture {
    project: ProjectFile,
    doctors: String,
    amit: String,
    opd: String,
    morning: String,
    night: String,
    /// The plan being evaluated; replaced by `generate_context_plan`.
    plan_id: String,
}

impl Fixture {
    fn generate_context_plan(&mut self, context: Vec<ContextAssignment>) {
        let doctors = self.doctors.clone();
        let plan = self.project.organizations[0]
            .generate_plan(&doctors, date(2026, 11, 16), date(2026, 11, 16), context)
            .expect("context plan");
        self.plan_id = plan.id.clone();
    }

    fn assign(&mut self, shift_name: &str, day: NaiveDate, employee_id: &str) -> String {
        let doctors = self.doctors.clone();
        let plan_id = self.plan_id.clone();
        let organization = &mut self.project.organizations[0];
        let (dated_id, slot_id) = seat(organization, &doctors, &plan_id, shift_name, day);
        organization
            .assign_slot(&doctors, &plan_id, &dated_id, &slot_id, employee_id)
            .expect("assignment");
        dated_id
    }

    fn seated(&self, shift_name: &str, day: NaiveDate) -> String {
        seat(
            &self.project.organizations[0],
            &self.doctors,
            &self.plan_id,
            shift_name,
            day,
        )
        .0
    }

    fn add_unavailability(
        &mut self,
        employee_id: &str,
        scope: UnavailabilityScope,
        shifts: &[&str],
        severity: Severity,
        penalty: u32,
    ) {
        self.project.organizations[0]
            .add_unavailability(employee_id, scope, shifts, severity, penalty)
            .expect("unavailability rule");
    }

    fn add_rest_rule(
        &mut self,
        preceding: &str,
        following: &str,
        min_rest_minutes: u32,
        severity: Severity,
        penalty: u32,
    ) {
        let doctors = self.doctors.clone();
        self.project.organizations[0]
            .add_rest_rule(
                &doctors,
                preceding,
                following,
                min_rest_minutes,
                severity,
                penalty,
            )
            .expect("rest rule");
    }

    fn add_recurring(&mut self, employee_id: &str, weekday: Weekday, shift_id: &str, penalty: u32) {
        self.project.organizations[0]
            .add_recurring_assignment_with_penalty(employee_id, weekday, shift_id, penalty)
            .expect("recurring preference");
    }

    fn evaluate(&self) -> Evaluation {
        evaluate(
            &self.project.organizations[0],
            &self.doctors,
            &self.plan_id,
            &EvalOptions::default(),
        )
        .expect("evaluation")
    }

    fn evaluate_with(&self, options: &EvalOptions) -> Evaluation {
        evaluate(
            &self.project.organizations[0],
            &self.doctors,
            &self.plan_id,
            options,
        )
        .expect("evaluation")
    }
}

fn november_project() -> Fixture {
    let mut project = ProjectFile::new();
    project
        .add_organization("City Hospital")
        .expect("organization");
    let organization: &mut Organization = &mut project.organizations[0];

    let medical_officer = organization
        .add_position("Medical Officer")
        .expect("position")
        .id
        .clone();
    let doctors = organization
        .add_roster("Doctors")
        .expect("roster")
        .id
        .clone();

    let opd = organization
        .add_shift(&doctors, "OPD", at(8, 0), at(14, 0))
        .expect("OPD")
        .id
        .clone();
    let morning = organization
        .add_shift(&doctors, "Emergency Morning", at(8, 0), at(15, 0))
        .expect("morning")
        .id
        .clone();
    let night = organization
        .add_shift(&doctors, "Emergency Night", at(15, 0), at(8, 0))
        .expect("night")
        .id
        .clone();

    organization
        .require_position(&doctors, &opd, &medical_officer, 2)
        .expect("OPD requirement");
    organization
        .require_position(&doctors, &morning, &medical_officer, 1)
        .expect("morning requirement");
    organization
        .require_position(&doctors, &night, &medical_officer, 1)
        .expect("night requirement");

    organization
        .set_shift_weekdays(
            &doctors,
            &opd,
            &[
                Weekday::Mon,
                Weekday::Tue,
                Weekday::Wed,
                Weekday::Thu,
                Weekday::Sat,
                Weekday::Sun,
            ],
        )
        .expect("OPD weekdays");

    let amit = organization
        .add_employee("Dr. Amit", &doctors, &medical_officer)
        .expect("employee")
        .id
        .clone();
    organization
        .add_employee("Dr. Khushi", &doctors, &medical_officer)
        .expect("employee");

    let plan_id = organization
        .generate_plan(&doctors, date(2026, 11, 16), date(2026, 11, 30), Vec::new())
        .expect("november plan")
        .id
        .clone();

    Fixture {
        project,
        doctors,
        amit,
        opd,
        morning,
        night,
        plan_id,
    }
}

/// Find the dated shift for a template name and date, returning its first seat.
fn seat(
    organization: &Organization,
    roster_id: &str,
    plan_id: &str,
    shift_name: &str,
    day: NaiveDate,
) -> (String, String) {
    let plan = organization.plan(roster_id, plan_id).expect("plan");
    let dated = plan
        .shifts
        .iter()
        .find(|shift| shift.name == shift_name && shift.date == day)
        .unwrap_or_else(|| panic!("no {shift_name} on {day}"));
    (dated.id.clone(), dated.slots[0].id.clone())
}

fn count(evaluation: &Evaluation, predicate: impl Fn(&ViolationKind) -> bool) -> usize {
    evaluation
        .violations
        .iter()
        .filter(|violation| predicate(&violation.kind))
        .count()
}

fn overlaps(evaluation: &Evaluation) -> usize {
    count(evaluation, |kind| {
        matches!(kind, ViolationKind::Overlap { .. })
    })
}

fn unavailable(evaluation: &Evaluation) -> usize {
    count(evaluation, |kind| {
        matches!(kind, ViolationKind::Unavailable { .. })
    })
}

fn unmet_preferences(evaluation: &Evaluation) -> usize {
    count(evaluation, |kind| {
        matches!(kind, ViolationKind::UnmetRecurringPreference { .. })
    })
}

fn rest(evaluation: &Evaluation) -> Vec<&ViolationKind> {
    evaluation
        .violations
        .iter()
        .filter(|violation| matches!(violation.kind, ViolationKind::InsufficientRest { .. }))
        .map(|violation| &violation.kind)
        .collect()
}

fn multi_shift(evaluation: &Evaluation) -> usize {
    count(evaluation, |kind| {
        matches!(kind, ViolationKind::MultipleShiftsInDay { .. })
    })
}

#[test]
fn an_unfilled_plan_is_feasible_and_reports_coverage() {
    let fixture = november_project();
    let evaluation = fixture.evaluate();

    assert!(evaluation.violations.is_empty());
    assert!(evaluation.is_feasible());
    assert_eq!(evaluation.hard_count(), 0);
    assert_eq!(evaluation.soft_penalty(), 0);
    assert_eq!(evaluation.total_slots, 56);
    assert_eq!(evaluation.unfilled_slots, 56);
}

#[test]
fn overlapping_assignments_are_hard_violations() {
    let mut fixture = november_project();
    let amit = fixture.amit.clone();
    fixture.assign("OPD", date(2026, 11, 16), &amit);
    fixture.assign("Emergency Morning", date(2026, 11, 16), &amit);

    let evaluation = fixture.evaluate();
    assert_eq!(overlaps(&evaluation), 1);
    assert!(!evaluation.is_feasible());

    let violation = evaluation
        .violations
        .iter()
        .find(|violation| matches!(violation.kind, ViolationKind::Overlap { .. }))
        .expect("overlap");
    assert_eq!(violation.severity, Severity::Hard);
    assert_eq!(violation.penalty, 0);
    let ViolationKind::Overlap {
        employee_id,
        first,
        second,
    } = &violation.kind
    else {
        unreachable!("filtered above");
    };
    assert_eq!(employee_id, &amit);
    let opd_id = fixture.seated("OPD", date(2026, 11, 16));
    assert!(matches!(first, ShiftRef::Dated { id } if *id == opd_id));
    assert!(matches!(second, ShiftRef::Dated { id } if *id != opd_id));
}

#[test]
fn assignments_that_only_touch_do_not_overlap() {
    let mut fixture = november_project();
    let amit = fixture.amit.clone();
    fixture.assign("Emergency Morning", date(2026, 11, 16), &amit);
    fixture.assign("Emergency Night", date(2026, 11, 16), &amit);

    assert_eq!(overlaps(&fixture.evaluate()), 0);
}

#[test]
fn context_assignments_participate_in_overlap() {
    let mut fixture = november_project();
    let night = fixture.night.clone();
    let amit = fixture.amit.clone();
    fixture.generate_context_plan(vec![ContextAssignment {
        date: date(2026, 11, 15),
        name: "Emergency Night".to_owned(),
        start: at(20, 0),
        end: at(9, 0),
        employee_id: amit.clone(),
        template_id: Some(night),
    }]);
    fixture.assign("Emergency Morning", date(2026, 11, 16), &amit);

    let evaluation = fixture.evaluate();
    assert_eq!(overlaps(&evaluation), 1);
    let violation = evaluation
        .violations
        .iter()
        .find(|violation| matches!(violation.kind, ViolationKind::Overlap { .. }))
        .expect("overlap");
    let ViolationKind::Overlap { first, second, .. } = &violation.kind else {
        unreachable!("filtered above");
    };
    assert!(
        matches!(first, ShiftRef::Context { .. }) || matches!(second, ShiftRef::Context { .. }),
        "context must be named: {first} / {second}"
    );
}

#[test]
fn hard_weekly_and_leave_rules_report_unavailable_assignments() {
    let mut fixture = november_project();
    let amit = fixture.amit.clone();
    fixture.add_unavailability(
        &amit,
        UnavailabilityScope::Weekly {
            weekday: Weekday::Fri,
        },
        &[],
        Severity::Hard,
        0,
    );
    fixture.add_unavailability(
        &amit,
        UnavailabilityScope::Leave {
            start: date(2026, 11, 24),
            end: date(2026, 11, 26),
        },
        &[],
        Severity::Hard,
        0,
    );
    fixture.assign("Emergency Night", date(2026, 11, 20), &amit);
    fixture.assign("Emergency Night", date(2026, 11, 25), &amit);

    let evaluation = fixture.evaluate();
    assert_eq!(unavailable(&evaluation), 2);
    assert!(
        evaluation
            .violations
            .iter()
            .all(|violation| { violation.severity == Severity::Hard && violation.penalty == 0 })
    );
    assert_eq!(evaluation.hard_count(), 2);
}

#[test]
fn soft_shift_scoped_rules_only_match_the_listed_template() {
    let mut fixture = november_project();
    let amit = fixture.amit.clone();
    let opd = fixture.opd.clone();
    fixture.add_unavailability(
        &amit,
        UnavailabilityScope::Weekly {
            weekday: Weekday::Sat,
        },
        &[opd.as_str()],
        Severity::Soft,
        2,
    );

    // A different template on the same weekday does not match.
    fixture.assign("Emergency Night", date(2026, 11, 21), &amit);
    assert_eq!(unavailable(&fixture.evaluate()), 0);

    // The listed template does match, and the penalty comes from the rule.
    fixture.assign("OPD", date(2026, 11, 21), &amit);
    let evaluation = fixture.evaluate();
    assert_eq!(unavailable(&evaluation), 1);
    let violation = evaluation
        .violations
        .iter()
        .find(|violation| matches!(violation.kind, ViolationKind::Unavailable { .. }))
        .expect("soft violation");
    assert_eq!(violation.severity, Severity::Soft);
    assert_eq!(violation.penalty, 2);
    assert!(evaluation.is_feasible());
}

#[test]
fn an_empty_shift_list_matches_every_shift() {
    let mut fixture = november_project();
    let amit = fixture.amit.clone();
    fixture.add_unavailability(
        &amit,
        UnavailabilityScope::Date {
            date: date(2026, 11, 17),
        },
        &[],
        Severity::Hard,
        0,
    );
    fixture.assign("Emergency Morning", date(2026, 11, 17), &amit);

    assert_eq!(unavailable(&fixture.evaluate()), 1);
}

#[test]
fn recurring_assignments_are_soft_preferences() {
    let mut fixture = november_project();
    let amit = fixture.amit.clone();
    let night = fixture.night.clone();
    fixture.add_recurring(&amit, Weekday::Sun, &night, 1);

    // Sundays in the period: 2026-11-22 and 2026-11-29.
    let evaluation = fixture.evaluate();
    assert_eq!(unmet_preferences(&evaluation), 2);
    assert!(evaluation.is_feasible());
    assert_eq!(evaluation.soft_penalty(), 2);
    assert!(
        evaluation
            .violations
            .iter()
            .all(|violation| { violation.severity == Severity::Soft && violation.penalty == 1 })
    );

    // Working one Sunday removes that date's preference.
    fixture.assign("Emergency Night", date(2026, 11, 22), &amit);
    assert_eq!(unmet_preferences(&fixture.evaluate()), 1);
}

#[test]
fn a_tunable_penalty_is_used_and_leave_suppresses_the_preference() {
    let mut fixture = november_project();
    let amit = fixture.amit.clone();
    let night = fixture.night.clone();
    fixture.add_recurring(&amit, Weekday::Sun, &night, 4);
    fixture.assign("Emergency Night", date(2026, 11, 22), &amit);

    // Only 2026-11-29 is unmet, at the tunable penalty.
    let evaluation = fixture.evaluate();
    assert_eq!(unmet_preferences(&evaluation), 1);
    assert_eq!(evaluation.soft_penalty(), 4);

    // Hard leave covering that Sunday suppresses the preference entirely:
    // fairness schedules someone else rather than penalizing the absent doctor.
    fixture.add_unavailability(
        &amit,
        UnavailabilityScope::Date {
            date: date(2026, 11, 29),
        },
        &[],
        Severity::Hard,
        0,
    );
    let evaluation = fixture.evaluate();
    assert_eq!(unmet_preferences(&evaluation), 0);
    assert_eq!(evaluation.soft_penalty(), 0);
}

#[test]
fn rest_rules_report_the_gap_and_the_rule_penalty() {
    let mut fixture = november_project();
    let amit = fixture.amit.clone();
    let night = fixture.night.clone();
    let morning = fixture.morning.clone();
    fixture.add_rest_rule(&night, &morning, 480, Severity::Soft, 3);
    fixture.assign("Emergency Night", date(2026, 11, 16), &amit);
    fixture.assign("Emergency Morning", date(2026, 11, 17), &amit);

    let evaluation = fixture.evaluate();
    let violations = rest(&evaluation);
    assert_eq!(violations.len(), 1);
    let ViolationKind::InsufficientRest { gap_minutes, .. } = violations[0] else {
        unreachable!("filtered above");
    };
    assert_eq!(*gap_minutes, 0);
    assert_eq!(evaluation.soft_penalty(), 3);
    assert!(evaluation.is_feasible());
}

#[test]
fn rest_rules_reach_into_previous_day_context() {
    let mut fixture = november_project();
    let amit = fixture.amit.clone();
    let night = fixture.night.clone();
    let morning = fixture.morning.clone();
    fixture.add_rest_rule(&night, &morning, 480, Severity::Hard, 0);
    fixture.generate_context_plan(vec![ContextAssignment {
        date: date(2026, 11, 15),
        name: "Emergency Night".to_owned(),
        start: at(15, 0),
        end: at(8, 0),
        employee_id: amit.clone(),
        template_id: Some(night),
    }]);
    fixture.assign("Emergency Morning", date(2026, 11, 16), &amit);

    let evaluation = fixture.evaluate();
    let violations = rest(&evaluation);
    assert_eq!(violations.len(), 1);
    let ViolationKind::InsufficientRest {
        preceding,
        following,
        gap_minutes,
        ..
    } = violations[0]
    else {
        unreachable!("filtered above");
    };
    assert_eq!(*gap_minutes, 0);
    assert!(matches!(preceding, ShiftRef::Context { .. }));
    assert!(matches!(following, ShiftRef::Dated { .. }));
    assert_eq!(evaluation.hard_count(), 1);
}

#[test]
fn multiple_shifts_in_one_day_are_soft_and_configurable() {
    let mut fixture = november_project();
    let amit = fixture.amit.clone();
    fixture.assign("OPD", date(2026, 11, 16), &amit);
    fixture.assign("Emergency Night", date(2026, 11, 16), &amit);

    let evaluation = fixture.evaluate();
    assert_eq!(multi_shift(&evaluation), 1);
    assert_eq!(evaluation.soft_penalty(), 1);
    let violation = evaluation
        .violations
        .iter()
        .find(|violation| matches!(violation.kind, ViolationKind::MultipleShiftsInDay { .. }))
        .expect("multi-shift");
    assert_eq!(violation.severity, Severity::Soft);

    let disabled = fixture.evaluate_with(&EvalOptions {
        multi_shift_penalty: 0,
    });
    assert_eq!(multi_shift(&disabled), 0);
}

#[test]
fn evaluation_is_read_only_and_deterministic() {
    let mut fixture = november_project();
    let amit = fixture.amit.clone();
    let khushi = fixture.project.organizations[0]
        .employees
        .iter()
        .find(|employee| employee.name == "Dr. Khushi")
        .expect("khushi")
        .id
        .clone();
    fixture.assign("OPD", date(2026, 11, 16), &amit);
    fixture.assign("Emergency Morning", date(2026, 11, 16), &khushi);

    let snapshot = fixture.project.clone();
    let first = fixture.evaluate();
    let second = fixture.evaluate();

    assert_eq!(first, second);
    assert_eq!(fixture.project, snapshot);
    assert_eq!(first.total_slots, 56);
    assert_eq!(first.unfilled_slots, 54);
}

#[test]
fn unknown_ids_are_reported() {
    let fixture = november_project();
    let organization = &fixture.project.organizations[0];

    assert!(matches!(
        evaluate(
            organization,
            "missing-roster",
            &fixture.plan_id,
            &EvalOptions::default()
        ),
        Err(roster::domain::DomainError::UnknownRoster { .. })
    ));
    assert!(matches!(
        evaluate(
            organization,
            &fixture.doctors,
            "missing-plan",
            &EvalOptions::default()
        ),
        Err(roster::domain::DomainError::UnknownPlan { .. })
    ));
}

//! Solver tests that exercise `precheck` and `solve` together.
//!
//! They live in a child module so they can reach the crate-private plumbing
//! (`precheck`, the seat model) without widening the public API.

use std::collections::HashMap;

use chrono::{NaiveDate, NaiveTime};

use super::*;
use crate::domain::{ContextAssignment, PlanStatus, UnavailabilityScope};

fn at(hour: u32, minute: u32) -> NaiveTime {
    NaiveTime::from_hms_opt(hour, minute, 0).unwrap()
}

fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).unwrap()
}

struct Builder {
    organization: Organization,
    template_id: String,
    position_id: String,
    shifts: HashMap<String, String>,
}

impl Builder {
    fn new() -> Self {
        let mut organization = Organization::new("City Hospital");
        let position_id = organization
            .add_position("Medical Officer")
            .unwrap()
            .id
            .clone();
        let template_id = organization
            .add_roster_template("Doctors")
            .unwrap()
            .id
            .clone();
        organization
            .include_position(&template_id, &position_id)
            .unwrap();
        Self {
            organization,
            template_id,
            position_id,
            shifts: HashMap::new(),
        }
    }

    fn shift(&mut self, name: &str, start: NaiveTime, end: NaiveTime, seats: u16) -> &mut Self {
        let shift_id = self
            .organization
            .add_shift(&self.template_id, name, start, end)
            .unwrap()
            .id
            .clone();
        self.organization
            .require_position(&self.template_id, &shift_id, &self.position_id, seats)
            .unwrap();
        self.shifts.insert(name.to_owned(), shift_id);
        self
    }

    fn doctor(&mut self, name: &str) -> String {
        self.organization
            .add_employee(name, &self.position_id)
            .unwrap()
            .id
            .clone()
    }

    fn roster(
        &mut self,
        start: NaiveDate,
        end: NaiveDate,
        context: Vec<ContextAssignment>,
    ) -> String {
        self.organization
            .create_roster(&self.template_id, start, end, context)
            .unwrap()
            .id
            .clone()
    }

    fn shift_id(&self, name: &str) -> String {
        self.shifts.get(name).expect("shift was added").clone()
    }
}

fn dated_shift_id(
    organization: &Organization,
    template_id: &str,
    roster_id: &str,
    shift_template_id: &str,
    day: NaiveDate,
) -> String {
    organization
        .roster(template_id, roster_id)
        .unwrap()
        .shifts
        .iter()
        .find(|shift| shift.template_id == shift_template_id && shift.date == day)
        .unwrap()
        .id
        .clone()
}

fn assign(
    organization: &mut Organization,
    template_id: &str,
    roster_id: &str,
    dated_id: &str,
    slot_index: usize,
    employee_id: &str,
) {
    let slot_id = organization
        .roster(template_id, roster_id)
        .unwrap()
        .dated_shift(dated_id)
        .unwrap()
        .slots[slot_index]
        .id
        .clone();
    organization
        .assign_slot(template_id, roster_id, dated_id, &slot_id, employee_id)
        .unwrap();
}

/// Mirror the write-back 04-02 will publish: clear every seat first.
fn apply_report(
    organization: &mut Organization,
    template_id: &str,
    roster_id: &str,
    report: &SolveReport,
) {
    let seats: Vec<(String, String)> = organization
        .roster(template_id, roster_id)
        .unwrap()
        .shifts
        .iter()
        .flat_map(|dated| {
            dated
                .slots
                .iter()
                .map(move |slot| (dated.id.clone(), slot.id.clone()))
        })
        .collect();
    for (dated_id, slot_id) in seats {
        organization
            .unassign_slot(template_id, roster_id, &dated_id, &slot_id)
            .unwrap();
    }
    for assignment in &report.assignments {
        if let Some(employee_id) = &assignment.employee_id {
            organization
                .assign_slot(
                    template_id,
                    roster_id,
                    &assignment.dated_shift_id,
                    &assignment.slot_id,
                    employee_id,
                )
                .unwrap();
        }
    }
}

#[test]
fn clean_roster_passes_precheck() {
    let mut builder = Builder::new();
    builder.shift("OPD", at(8, 0), at(14, 0), 1);
    builder.doctor("Dr. Amit");
    let roster = builder.roster(date(2026, 11, 16), date(2026, 11, 16), Vec::new());

    let options = SolveOptions::default();
    assert!(
        precheck(
            &builder.organization,
            &builder.template_id,
            &roster,
            &options
        )
        .is_ok(),
        "a roster with no assignments has nothing to violate"
    );
}

#[test]
fn prefilled_overlap_is_inconsistent_input() {
    let mut builder = Builder::new();
    builder.shift("OPD", at(8, 0), at(14, 0), 1);
    builder.shift("Emergency", at(9, 0), at(11, 0), 1);
    let doctor = builder.doctor("Dr. Amit");
    let roster = builder.roster(date(2026, 11, 16), date(2026, 11, 16), Vec::new());

    let opd = dated_shift_id(
        &builder.organization,
        &builder.template_id,
        &roster,
        &builder.shift_id("OPD"),
        date(2026, 11, 16),
    );
    let emergency = dated_shift_id(
        &builder.organization,
        &builder.template_id,
        &roster,
        &builder.shift_id("Emergency"),
        date(2026, 11, 16),
    );
    assign(
        &mut builder.organization,
        &builder.template_id,
        &roster,
        &opd,
        0,
        &doctor,
    );
    assign(
        &mut builder.organization,
        &builder.template_id,
        &roster,
        &emergency,
        0,
        &doctor,
    );

    let options = SolveOptions::default();
    let error = precheck(
        &builder.organization,
        &builder.template_id,
        &roster,
        &options,
    )
    .expect_err("an overlap is bad input");
    match error {
        SolveError::InconsistentInput { reasons } => {
            assert_eq!(reasons.len(), 1);
            assert!(reasons[0].contains("overlap"), "{reasons:?}");
        }
        other => panic!("expected InconsistentInput, got {other:?}"),
    }

    assert!(matches!(
        solve(
            &builder.organization,
            &builder.template_id,
            &roster,
            &options
        ),
        Err(SolveError::InconsistentInput { .. })
    ));
}

#[test]
fn prefilled_seat_overlapping_context_is_inconsistent_input() {
    let mut builder = Builder::new();
    builder.shift("OPD", at(8, 0), at(14, 0), 1);
    let doctor = builder.doctor("Dr. Amit");
    let context = vec![ContextAssignment {
        date: date(2026, 11, 15),
        name: "Previous night".to_owned(),
        start: at(18, 0),
        end: at(10, 0),
        employee_id: doctor.clone(),
        template_id: None,
    }];
    let roster = builder.roster(date(2026, 11, 16), date(2026, 11, 16), context);
    let opd = dated_shift_id(
        &builder.organization,
        &builder.template_id,
        &roster,
        &builder.shift_id("OPD"),
        date(2026, 11, 16),
    );
    assign(
        &mut builder.organization,
        &builder.template_id,
        &roster,
        &opd,
        0,
        &doctor,
    );

    let error = precheck(
        &builder.organization,
        &builder.template_id,
        &roster,
        &SolveOptions::default(),
    )
    .expect_err("context overlap is bad input");
    assert!(
        matches!(error, SolveError::InconsistentInput { .. }),
        "{error:?}"
    );
}

#[test]
fn finalized_roster_is_rejected_before_solving() {
    let mut builder = Builder::new();
    builder.shift("OPD", at(8, 0), at(14, 0), 1);
    builder.doctor("Dr. Amit");
    let roster = builder.roster(date(2026, 11, 16), date(2026, 11, 16), Vec::new());

    builder
        .organization
        .roster_mut(&builder.template_id, &roster)
        .unwrap()
        .status = PlanStatus::Finalized;

    let error = precheck(
        &builder.organization,
        &builder.template_id,
        &roster,
        &SolveOptions::default(),
    )
    .expect_err("finalized rosters are read-only");
    assert!(
        matches!(error, SolveError::Domain(DomainError::RosterFinalized { .. })),
        "{error:?}"
    );
}

#[test]
fn solves_a_november_style_day_without_hard_violations() {
    let mut builder = Builder::new();
    builder.shift("OPD", at(8, 0), at(14, 0), 2);
    builder.shift("Emergency Morning", at(8, 0), at(14, 0), 1);
    builder.shift("Emergency Night", at(20, 0), at(8, 0), 1);
    let doctors: Vec<String> = ["Dr. Amit", "Dr. Khushi", "Dr. Farhana", "Dr. Rahul"]
        .iter()
        .map(|name| builder.doctor(name))
        .collect();
    let roster = builder.roster(date(2026, 11, 16), date(2026, 11, 16), Vec::new());

    let mut options = SolveOptions::default();
    options.eval.multi_shift_penalty = 0; // Disable multi-shift to avoid overflow in soft terms
    let report = solve(
        &builder.organization,
        &builder.template_id,
        &roster,
        &options,
    )
    .expect("a clean roster is always solvable");
    // Accept Optimal or Feasible — both mean a valid solution was found
    assert!(
        matches!(report.status, SolveStatus::Optimal | SolveStatus::Feasible),
        "expected Optimal or Feasible, got {:?}",
        report.status
    );
    assert_eq!(report.total_slots, 4);
    assert_eq!(report.assignments.len(), 4);

    for assignment in &report.assignments {
        if let Some(employee_id) = &assignment.employee_id {
            assert!(
                doctors.contains(employee_id),
                "only template doctors may fill a seat"
            );
        }
    }

    let mut updated = builder.organization.clone();
    apply_report(
        &mut updated,
        &builder.template_id,
        &roster,
        &report,
    );
    let evaluation = evaluate(
        &updated,
        &builder.template_id,
        &roster,
        &options.eval,
    )
    .unwrap();
    assert_eq!(evaluation.hard_count(), 0, "{:?}", evaluation.violations);
}

#[test]
fn every_doctor_on_leave_leaves_every_seat_empty() {
    let mut builder = Builder::new();
    builder.shift("OPD", at(8, 0), at(14, 0), 1);
    let first = builder.doctor("Dr. Amit");
    let second = builder.doctor("Dr. Khushi");
    let roster = builder.roster(date(2026, 11, 16), date(2026, 11, 16), Vec::new());

    for doctor in [&first, &second] {
        builder
            .organization
            .add_unavailability(
                doctor,
                UnavailabilityScope::Leave {
                    start: date(2026, 11, 16),
                    end: date(2026, 11, 16),
                },
                &[],
                Severity::Hard,
                0,
            )
            .unwrap();
    }

    let report = solve(
        &builder.organization,
        &builder.template_id,
        &roster,
        &SolveOptions::default(),
    )
    .expect("empty seats are always legal");
    assert_eq!(report.unfilled_slots, report.total_slots);
    assert!(
        report
            .assignments
            .iter()
            .all(|assignment| assignment.employee_id.is_none())
    );
}

#[test]
fn two_solves_of_the_same_roster_agree() {
    let mut builder = Builder::new();
    builder.shift("OPD", at(8, 0), at(14, 0), 2);
    builder.shift("Emergency", at(9, 0), at(11, 0), 1);
    builder.doctor("Dr. Amit");
    builder.doctor("Dr. Khushi");
    builder.doctor("Dr. Farhana");
    let roster = builder.roster(date(2026, 11, 16), date(2026, 11, 17), Vec::new());

    let mut options = SolveOptions::default();
    options.eval.multi_shift_penalty = 0; // Disable multi-shift to avoid overflow in test
    let first = solve(
        &builder.organization,
        &builder.template_id,
        &roster,
        &options,
    )
    .unwrap();
    let second = solve(
        &builder.organization,
        &builder.template_id,
        &roster,
        &options,
    )
    .unwrap();

    assert_eq!(first.assignments, second.assignments);
    // Both solves should succeed with Optimal or Feasible
    assert!(matches!(
        first.status,
        SolveStatus::Optimal | SolveStatus::Feasible
    ));
    assert!(matches!(
        second.status,
        SolveStatus::Optimal | SolveStatus::Feasible
    ));
}

//! One solver smoke test.
//!
//! It lives in a child module so it can reach the crate-private plumbing
//! without widening the public API.

use std::collections::HashMap;

use chrono::{NaiveDate, NaiveTime};

use super::*;
use crate::domain::ContextAssignment;

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
            .roster_template_mut(&template_id)
            .unwrap()
            .include_position(&position_id);
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
            .roster_template_mut(&self.template_id)
            .unwrap()
            .add_shift(name, start, end)
            .unwrap()
            .id
            .clone();
        self.organization
            .roster_template_mut(&self.template_id)
            .unwrap()
            .require_position(&shift_id, &self.position_id, seats)
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
        let members = self
            .organization
            .template_member_snapshot(&self.template_id);
        self.organization
            .roster_template_mut(&self.template_id)
            .unwrap()
            .create_roster(start, end, context, members)
            .unwrap()
            .id
            .clone()
    }
}

/// Mirror the write-back the UI performs: clear every seat first.
fn apply_report(
    organization: &mut Organization,
    template_id: &str,
    roster_id: &str,
    report: &SolveReport,
) {
    let roster = organization.roster_mut(template_id, roster_id).unwrap();
    let seats: Vec<(String, String)> = roster
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
        roster.unassign_slot(&dated_id, &slot_id).unwrap();
    }
    for assignment in &report.assignments {
        if let Some(employee_id) = &assignment.employee_id {
            roster
                .assign_slot(&assignment.dated_shift_id, &assignment.slot_id, employee_id)
                .unwrap();
        }
    }
}

#[test]
fn solves_a_small_day_without_hard_violations() {
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
    options.eval.multi_shift_penalty = 0;
    let report = solve(
        &builder.organization,
        &builder.template_id,
        &roster,
        &options,
    )
    .expect("a clean roster is always solvable");
    assert!(matches!(
        report.status,
        SolveStatus::Optimal | SolveStatus::Feasible
    ));
    assert_eq!(report.total_slots, 4);
    assert_eq!(report.assignments.len(), 4);

    for assignment in &report.assignments {
        if let Some(employee_id) = &assignment.employee_id {
            assert!(doctors.contains(employee_id));
        }
    }

    let mut updated = builder.organization.clone();
    apply_report(&mut updated, &builder.template_id, &roster, &report);
    let evaluation = evaluate(&updated, &builder.template_id, &roster, &options.eval).unwrap();
    assert_eq!(evaluation.hard_count(), 0, "{:?}", evaluation.violations);
}

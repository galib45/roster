//! End-to-end coverage of workload and fairness through the public API.
//!
//! The legacy doctor duty lengths are reused so the arithmetic is easy to
//! verify: OPD 360 minutes, Emergency Morning 420, Emergency Night 1020.

use chrono::{NaiveDate, NaiveTime};
use roster::domain::{ContextAssignment, Organization, ProjectFile, Roster};
use roster::eval::{EvalOptions, evaluate, workload};

fn at(hour: u32, minute: u32) -> NaiveTime {
    NaiveTime::from_hms_opt(hour, minute, 0).unwrap()
}

fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).unwrap()
}

fn approx(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 0.5,
        "expected {expected}, got {actual}"
    );
}

struct Fixture {
    project: ProjectFile,
    doctors: String,
    amit: String,
    khushi: String,
    meera: String,
    rina: String,
    plan_id: String,
}

impl Fixture {
    fn organization(&self) -> &Organization {
        &self.project.organizations[0]
    }

    fn roster(&self) -> &roster::domain::RosterTemplate {
        self.organization().roster(&self.doctors).expect("roster")
    }

    fn plan(&self) -> &Roster {
        self.roster().plan(&self.plan_id).expect("plan")
    }

    fn report(&self) -> roster::eval::WorkloadReport {
        workload(self.organization(), self.roster(), self.plan())
    }

    fn assign(&mut self, shift_name: &str, seat_index: usize, employee_id: &str) {
        let doctors = self.doctors.clone();
        let plan_id = self.plan_id.clone();
        let day = date(2026, 11, 16);
        let organization = &mut self.project.organizations[0];

        let plan = organization.plan(&doctors, &plan_id).expect("plan");
        let dated = plan
            .shifts
            .iter()
            .find(|shift| shift.name == shift_name && shift.date == day)
            .unwrap_or_else(|| panic!("no {shift_name} on {day}"));
        let dated_id = dated.id.clone();
        let slot_id = dated
            .slots
            .get(seat_index)
            .unwrap_or_else(|| panic!("{shift_name} has no seat {seat_index}"))
            .id
            .clone();

        organization
            .assign_slot(&doctors, &plan_id, &dated_id, &slot_id, employee_id)
            .expect("assignment");
    }
}

/// One day of the November roster: two OPD seats, one morning, one night.
fn november_day() -> Fixture {
    let mut project = ProjectFile::new();
    project
        .add_organization("City Hospital")
        .expect("organization");
    let organization = &mut project.organizations[0];

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

    let amit = organization
        .add_employee("Dr. Amit", &doctors, &medical_officer)
        .expect("employee")
        .id
        .clone();
    let khushi = organization
        .add_employee("Dr. Khushi", &doctors, &medical_officer)
        .expect("employee")
        .id
        .clone();
    let meera = organization
        .add_employee("Dr. Meera", &doctors, &medical_officer)
        .expect("employee")
        .id
        .clone();
    let rina = organization
        .add_employee("Dr. Rina", &doctors, &medical_officer)
        .expect("employee")
        .id
        .clone();

    let plan_id = organization
        .generate_plan(&doctors, date(2026, 11, 16), date(2026, 11, 16), Vec::new())
        .expect("plan")
        .id
        .clone();

    Fixture {
        project,
        doctors,
        amit,
        khushi,
        meera,
        rina,
        plan_id,
    }
}

fn two_employee_day() -> Fixture {
    let mut project = ProjectFile::new();
    project
        .add_organization("City Hospital")
        .expect("organization");
    let organization = &mut project.organizations[0];

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
    organization
        .require_position(&doctors, &opd, &medical_officer, 2)
        .expect("OPD requirement");

    let amit = organization
        .add_employee("Dr. Amit", &doctors, &medical_officer)
        .expect("employee")
        .id
        .clone();
    let khushi = organization
        .add_employee("Dr. Khushi", &doctors, &medical_officer)
        .expect("employee")
        .id
        .clone();

    let plan_id = organization
        .generate_plan(&doctors, date(2026, 11, 16), date(2026, 11, 16), Vec::new())
        .expect("plan")
        .id
        .clone();

    Fixture {
        project,
        doctors,
        amit,
        khushi,
        meera: String::new(),
        rina: String::new(),
        plan_id,
    }
}

/// Amit works OPD and the night, Khushi the morning, Meera the second OPD
/// seat, and Rina has nothing. Registrars = {Amit, Khushi}, Interns = {Meera}.
fn mixed_report() -> Fixture {
    let mut fixture = november_day();
    let amit = fixture.amit.clone();
    let khushi = fixture.khushi.clone();
    let meera = fixture.meera.clone();

    fixture.assign("OPD", 0, &amit);
    fixture.assign("Emergency Night", 0, &amit);
    fixture.assign("Emergency Morning", 0, &khushi);
    fixture.assign("OPD", 1, &meera);

    let doctors = fixture.doctors.clone();
    let organization = &mut fixture.project.organizations[0];

    organization
        .add_sharing_group(&doctors, "Registrars", &[&amit, &khushi])
        .expect("registrars");
    organization
        .add_sharing_group(&doctors, "Interns", &[&meera])
        .expect("interns");

    assert_eq!(fixture.organization().validate(), Ok(()));
    fixture
}

#[test]
fn measures_duration_weighted_workload_and_group_fairness() {
    let fixture = mixed_report();
    let report = fixture.report();

    assert_eq!(report.employees.len(), 4);

    let amit = report
        .employees
        .iter()
        .find(|employee| employee.employee_id == fixture.amit)
        .expect("amit");
    assert_eq!(amit.minutes, 360 + 1020);
    assert_eq!(amit.shift_count, 2);

    let khushi = report
        .employees
        .iter()
        .find(|employee| employee.employee_id == fixture.khushi)
        .expect("khushi");
    assert_eq!(khushi.minutes, 420);
    assert_eq!(khushi.shift_count, 1);

    let meera = report
        .employees
        .iter()
        .find(|employee| employee.employee_id == fixture.meera)
        .expect("meera");
    assert_eq!(meera.minutes, 360);

    let rina = report
        .employees
        .iter()
        .find(|employee| employee.employee_id == fixture.rina)
        .expect("rina");
    assert_eq!(rina.minutes, 0);
    assert_eq!(rina.shift_count, 0);

    assert_eq!(report.roster_total_minutes, 2160);
    approx(report.roster_average_minutes, 540.0);
    assert_eq!(report.max_minutes, 1380);
    assert_eq!(report.min_minutes, 0);

    assert_eq!(report.groups.len(), 2);
    let registrars = report
        .groups
        .iter()
        .find(|group| group.name == "Registrars")
        .expect("registrars");
    assert_eq!(registrars.member_count, 2);
    assert_eq!(registrars.total_minutes, 1800);
    approx(registrars.average_minutes, 900.0);
    approx(registrars.deviation_minutes, 960.0);

    let interns = report
        .groups
        .iter()
        .find(|group| group.name == "Interns")
        .expect("interns");
    assert_eq!(interns.member_count, 1);
    assert_eq!(interns.total_minutes, 360);
    approx(interns.average_minutes, 360.0);
    approx(interns.deviation_minutes, 0.0);

    // Registrars: |1380 - 900| + |420 - 900| = 960.
    // Interns: 0. Rina is ungrouped and compares with 540 → 540.
    approx(report.deviation_minutes, 1500.0);

    // Every employee's minutes reconcile with the roster total.
    let summed: u64 = report
        .employees
        .iter()
        .map(|employee| u64::from(employee.minutes))
        .sum();
    assert_eq!(summed, report.roster_total_minutes);
}

#[test]
fn previous_day_context_adds_no_workload() {
    let mut fixture = november_day();
    let doctors = fixture.doctors.clone();
    let amit = fixture.amit.clone();

    let plan_id = fixture.project.organizations[0]
        .generate_plan(
            &doctors,
            date(2026, 11, 16),
            date(2026, 11, 16),
            vec![ContextAssignment {
                date: date(2026, 11, 15),
                name: "Emergency Night".to_owned(),
                start: at(15, 0),
                end: at(8, 0),
                employee_id: amit.clone(),
                template_id: None,
            }],
        )
        .expect("context plan")
        .id
        .clone();
    fixture.plan_id = plan_id;

    fixture.assign("OPD", 0, &amit);

    let report = fixture.report();
    let employee = report
        .employees
        .iter()
        .find(|employee| employee.employee_id == amit)
        .expect("amit");
    assert_eq!(employee.minutes, 360);
    assert_eq!(employee.shift_count, 1);
    assert_eq!(report.roster_total_minutes, 360);
}

#[test]
fn ungrouped_employees_use_the_roster_average() {
    let fixture = mixed_report();
    let report = fixture.report();

    let rina = report
        .employees
        .iter()
        .find(|employee| employee.employee_id == fixture.rina)
        .expect("rina");
    assert!(rina.group_id.is_none());

    // 2160 minutes over four roster employees.
    approx(report.roster_average_minutes, 540.0);
    approx((0.0_f64 - report.roster_average_minutes).abs(), 540.0);
}

#[test]
fn balanced_assignments_score_better_than_unbalanced_ones() {
    let mut balanced = two_employee_day();
    let amit = balanced.amit.clone();
    let khushi = balanced.khushi.clone();
    balanced.assign("OPD", 0, &amit);
    balanced.assign("OPD", 1, &khushi);
    let balanced_report = balanced.report();
    approx(balanced_report.deviation_minutes, 0.0);
    approx(balanced_report.roster_average_minutes, 360.0);

    let mut unbalanced = two_employee_day();
    let amit = unbalanced.amit.clone();
    unbalanced.assign("OPD", 0, &amit);
    let unbalanced_report = unbalanced.report();
    approx(unbalanced_report.roster_average_minutes, 180.0);
    assert!(
        unbalanced_report.deviation_minutes > balanced_report.deviation_minutes,
        "unbalanced {} should exceed balanced {}",
        unbalanced_report.deviation_minutes,
        balanced_report.deviation_minutes
    );
}

#[test]
fn evaluation_exposes_the_same_workload_read_only() {
    let empty = november_day();
    let evaluation = evaluate(
        empty.organization(),
        &empty.doctors,
        &empty.plan_id,
        &EvalOptions::default(),
    )
    .expect("evaluation");

    assert_eq!(evaluation.total_slots, 4);
    assert_eq!(evaluation.unfilled_slots, 4);
    assert_eq!(evaluation.workload.roster_total_minutes, 0);
    assert_eq!(evaluation.workload.employees.len(), 4);
    assert!(
        evaluation
            .workload
            .employees
            .iter()
            .all(|employee| employee.minutes == 0)
    );
    approx(evaluation.workload.deviation_minutes, 0.0);

    let filled = mixed_report();
    let evaluation = evaluate(
        filled.organization(),
        &filled.doctors,
        &filled.plan_id,
        &EvalOptions::default(),
    )
    .expect("evaluation");

    assert_eq!(evaluation.workload, filled.report());

    // Read-only: a second evaluation is identical.
    let again = evaluate(
        filled.organization(),
        &filled.doctors,
        &filled.plan_id,
        &EvalOptions::default(),
    )
    .expect("evaluation");
    assert_eq!(evaluation, again);
}

#[test]
fn those_groups_and_workload_match_the_saved_project() {
    let fixture = mixed_report();
    let text = serde_json::to_string(&fixture.project).expect("serialize");
    let loaded: ProjectFile = serde_json::from_str(&text).expect("deserialize");

    let organization = &loaded.organizations[0];
    assert_eq!(organization.validate(), Ok(()));
    let roster = organization.roster(&fixture.doctors).expect("roster");
    assert_eq!(roster.sharing_groups.len(), 2);
    let plan = roster.plan(&fixture.plan_id).expect("plan");

    assert_eq!(workload(organization, roster, plan), fixture.report());
}

//! End-to-end coverage of generated roster plans through the public API.
//!
//! The November 2026 example: OPD needs two Medical Officers every day except
//! Friday; Emergency Morning and Emergency Night each need one every day.

use chrono::{NaiveDate, NaiveTime, Weekday};
use roster::domain::{Organization, ProjectFile, Severity};
use roster::store;

fn at(hour: u32, minute: u32) -> NaiveTime {
    NaiveTime::from_hms_opt(hour, minute, 0).unwrap()
}

fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).unwrap()
}

struct NovemberFixture {
    project: ProjectFile,
    doctors: String,
    amit: String,
    khushi: String,
    opd_template: String,
}

fn november_project() -> NovemberFixture {
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
    let khushi = organization
        .add_employee("Dr. Khushi", &doctors, &medical_officer)
        .expect("employee")
        .id
        .clone();

    // A soft rule proves constraint data coexists with plans.
    organization
        .add_rest_rule(&doctors, &night, &morning, 480, Severity::Soft, 3)
        .expect("rest rule");

    NovemberFixture {
        project,
        doctors,
        amit,
        khushi,
        opd_template: opd,
    }
}

#[test]
fn november_plan_expands_templates_into_dated_shifts_and_seats() {
    let fixture = november_project();
    let mut project = fixture.project;
    let organization = &mut project.organizations[0];

    let plan_id = organization
        .generate_plan(
            &fixture.doctors,
            date(2026, 11, 16),
            date(2026, 11, 30),
            Vec::new(),
        )
        .expect("november plan")
        .id
        .clone();

    let plan = organization.plan(&fixture.doctors, &plan_id).expect("plan");
    assert_eq!(plan.shifts.len(), 43);
    assert_eq!(plan.total_slots(), 56);
    assert_eq!(plan.filled_slots(), 0);
    assert_eq!(plan.start_date, date(2026, 11, 16));
    assert_eq!(plan.end_date, date(2026, 11, 30));

    for friday in [date(2026, 11, 20), date(2026, 11, 27)] {
        assert!(
            plan.shifts
                .iter()
                .all(|shift| shift.name != "OPD" || shift.date != friday),
            "OPD must not occur on {friday}"
        );
        assert_eq!(
            plan.shifts
                .iter()
                .filter(|shift| shift.date == friday)
                .count(),
            2,
            "Friday still has the two emergency shifts"
        );
    }

    for shift in &plan.shifts {
        match shift.name.as_str() {
            "OPD" => assert_eq!(shift.slots.len(), 2),
            "Emergency Morning" | "Emergency Night" => assert_eq!(shift.slots.len(), 1),
            other => panic!("unexpected shift {other}"),
        }
    }

    let night = plan
        .shifts
        .iter()
        .find(|shift| shift.name == "Emergency Night")
        .expect("night shift");
    assert_eq!(night.duration_minutes(), 1020);
    assert!(night.crosses_midnight());

    organization
        .validate()
        .expect("plan and rest rule validate together");
}

#[test]
fn seats_are_assigned_individually() {
    let fixture = november_project();
    let mut project = fixture.project;
    let organization = &mut project.organizations[0];

    let plan_id = organization
        .generate_plan(
            &fixture.doctors,
            date(2026, 11, 16),
            date(2026, 11, 30),
            Vec::new(),
        )
        .expect("november plan")
        .id
        .clone();

    let (dated_shift_id, first_slot, second_slot) = {
        let plan = organization.plan(&fixture.doctors, &plan_id).expect("plan");
        let dated = plan
            .shifts
            .iter()
            .find(|shift| shift.name == "OPD")
            .expect("OPD shift");
        (
            dated.id.clone(),
            dated.slots[0].id.clone(),
            dated.slots[1].id.clone(),
        )
    };

    organization
        .assign_slot(
            &fixture.doctors,
            &plan_id,
            &dated_shift_id,
            &first_slot,
            &fixture.amit,
        )
        .expect("assign first seat");

    let plan = organization.plan(&fixture.doctors, &plan_id).expect("plan");
    assert_eq!(
        plan.slot(&dated_shift_id, &first_slot)
            .expect("first seat")
            .employee_id
            .as_deref(),
        Some(fixture.amit.as_str())
    );
    assert_eq!(
        plan.slot(&dated_shift_id, &second_slot)
            .expect("second seat")
            .employee_id,
        None
    );
    assert_eq!(plan.filled_slots(), 1);
    assert_eq!(
        plan.shifts
            .iter()
            .flat_map(|shift| &shift.slots)
            .filter(|slot| slot.employee_id.as_deref() == Some(fixture.khushi.as_str()))
            .count(),
        0
    );

    organization
        .unassign_slot(&fixture.doctors, &plan_id, &dated_shift_id, &first_slot)
        .expect("unassign");
    assert_eq!(
        organization
            .plan(&fixture.doctors, &plan_id)
            .expect("plan")
            .slot(&dated_shift_id, &first_slot)
            .expect("first seat")
            .employee_id,
        None
    );

    organization.validate().expect("still valid");
}

#[test]
fn generated_plans_are_snapshots_of_their_templates() {
    let fixture = november_project();
    let mut project = fixture.project;
    let organization = &mut project.organizations[0];

    let first_plan_id = organization
        .generate_plan(
            &fixture.doctors,
            date(2026, 11, 16),
            date(2026, 11, 30),
            Vec::new(),
        )
        .expect("first plan")
        .id
        .clone();
    let original = {
        let plan = organization
            .plan(&fixture.doctors, &first_plan_id)
            .expect("plan");
        plan.shifts
            .iter()
            .find(|shift| shift.name == "OPD")
            .expect("OPD shift")
            .clone()
    };

    let template = organization
        .roster_mut(&fixture.doctors)
        .expect("roster")
        .shifts
        .iter_mut()
        .find(|shift| shift.id == fixture.opd_template)
        .expect("OPD template");
    template.name = "Old OPD".to_owned();
    template.start = at(9, 0);
    template.weekdays = vec![Weekday::Mon];

    let plan = organization
        .plan(&fixture.doctors, &first_plan_id)
        .expect("plan");
    let unchanged = plan
        .dated_shift(&original.id)
        .expect("existing dated shift");
    assert_eq!(unchanged.name, "OPD");
    assert_eq!(unchanged.start, at(8, 0));
    assert_eq!(unchanged.end, at(14, 0));
    assert_eq!(unchanged.slots, original.slots);

    let second_plan_id = organization
        .generate_plan(
            &fixture.doctors,
            date(2026, 11, 16),
            date(2026, 11, 30),
            Vec::new(),
        )
        .expect("second plan")
        .id
        .clone();
    let second = organization
        .plan(&fixture.doctors, &second_plan_id)
        .expect("second plan");
    assert!(!second.shifts.iter().any(|shift| shift.name == "OPD"));
    assert_eq!(
        second
            .shifts
            .iter()
            .filter(|shift| shift.name == "Old OPD")
            .count(),
        3,
        "Mondays on Nov 16, 23, 30"
    );

    organization.validate().expect("both plans are valid");
}

#[test]
fn planned_projects_round_trip_and_reload() {
    let path = std::env::temp_dir().join(format!(
        "roster-plan-generation-{}.json",
        uuid::Uuid::new_v4()
    ));
    let fixture = november_project();
    let mut project = fixture.project;
    let plan_id = project.organizations[0]
        .generate_plan(
            &fixture.doctors,
            date(2026, 11, 16),
            date(2026, 11, 30),
            Vec::new(),
        )
        .expect("plan")
        .id
        .clone();
    let (dated_shift_id, slot_id) = {
        let plan = project.organizations[0]
            .plan(&fixture.doctors, &plan_id)
            .expect("plan");
        let dated = plan
            .shifts
            .iter()
            .find(|shift| shift.name == "OPD")
            .expect("OPD shift");
        (dated.id.clone(), dated.slots[0].id.clone())
    };
    project.organizations[0]
        .assign_slot(
            &fixture.doctors,
            &plan_id,
            &dated_shift_id,
            &slot_id,
            &fixture.amit,
        )
        .expect("assignment");

    store::save_project(&project, &path).expect("save");
    let loaded = store::load_project(&path).expect("load");

    assert_eq!(project, loaded);
    loaded.organizations[0]
        .validate()
        .expect("loaded plan stays valid");
    let plan = loaded.organizations[0]
        .plan(&fixture.doctors, &plan_id)
        .expect("loaded plan");
    assert_eq!(plan.shifts.len(), 43);
    assert_eq!(plan.total_slots(), 56);
    assert_eq!(plan.filled_slots(), 1);
    assert_eq!(
        plan.slot(&dated_shift_id, &slot_id)
            .expect("seat")
            .employee_id
            .as_deref(),
        Some(fixture.amit.as_str())
    );

    let _ = std::fs::remove_file(&path);
}

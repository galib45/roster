//! End-to-end coverage of the Phase 5 management flows through the public API.
//!
//! The fixture builds the same shape the Employees and Shifts pages produce:
//! an organization with two positions, two rosters, a skilled employee, and a
//! shift that recurs on chosen weekdays and requires two positions.

use chrono::{NaiveTime, Weekday};
use roster::domain::ProjectFile;
use roster::store;

fn at(hour: u32, minute: u32) -> NaiveTime {
    NaiveTime::from_hms_opt(hour, minute, 0).expect("valid wall-clock time")
}

fn configured_project() -> ProjectFile {
    let mut project = ProjectFile::new();
    project
        .add_organization("City Hospital")
        .expect("organization should be added");
    let organization = &mut project.organizations[0];

    let medical_officer = organization
        .add_position("Medical Officer")
        .expect("position")
        .id
        .clone();
    let staff_nurse = organization
        .add_position("Senior Staff Nurse")
        .expect("position")
        .id
        .clone();

    let doctors = organization
        .add_roster("Doctors")
        .expect("roster")
        .id
        .clone();
    let _nurses = organization
        .add_roster("Nurses")
        .expect("roster")
        .id
        .clone();

    let amit = organization
        .add_employee("Dr. Amit", &doctors, &medical_officer)
        .expect("employee")
        .id
        .clone();
    for skill in ["cardiology", "ACLS"] {
        organization
            .employee_mut(&amit)
            .expect("employee should exist")
            .add_skill(skill)
            .expect("skill");
    }

    let opd = organization
        .add_shift(&doctors, "OPD", at(8, 0), at(14, 0))
        .expect("shift")
        .id
        .clone();
    organization
        .set_shift_weekdays(
            &doctors,
            &opd,
            &[
                Weekday::Mon,
                Weekday::Tue,
                Weekday::Wed,
                Weekday::Thu,
                Weekday::Fri,
            ],
        )
        .expect("weekdays");
    organization
        .set_shift_requirements(&doctors, &opd, &[(medical_officer, 2), (staff_nurse, 1)])
        .expect("requirements");

    project
}

#[test]
fn configured_management_project_round_trips_losslessly() {
    let path = std::env::temp_dir().join(format!(
        "roster-management-ui-{}.json",
        uuid::Uuid::new_v4()
    ));
    let project = configured_project();

    store::save_project(&project, &path).expect("save should succeed");
    let loaded = store::load_project(&path).expect("load should succeed");

    // Everything, including ids, relationships, skills, weekdays, and
    // headcounts, survives the round-trip unchanged.
    assert_eq!(project, loaded);

    let organization = &loaded.organizations[0];
    organization
        .validate()
        .expect("loaded organization stays valid");
    assert_eq!(organization.positions.len(), 2);
    assert_eq!(organization.rosters.len(), 2);
    assert_eq!(organization.employees.len(), 1);

    let employee = organization
        .employees
        .iter()
        .find(|employee| employee.name == "Dr. Amit")
        .expect("Dr. Amit");
    assert_eq!(
        employee.skills,
        vec!["cardiology".to_owned(), "ACLS".to_owned()]
    );
    assert_eq!(
        employee.position_id,
        organization
            .positions
            .iter()
            .find(|position| position.name == "Medical Officer")
            .expect("medical officer position")
            .id
    );
    assert_eq!(
        employee.roster_id,
        organization
            .rosters
            .iter()
            .find(|roster| roster.name == "Doctors")
            .expect("doctors roster")
            .id
    );

    let doctors = organization
        .rosters
        .iter()
        .find(|roster| roster.name == "Doctors")
        .expect("doctors roster");
    let opd = doctors
        .shifts
        .iter()
        .find(|shift| shift.name == "OPD")
        .expect("OPD shift");
    assert_eq!(opd.start, at(8, 0));
    assert_eq!(opd.end, at(14, 0));
    assert_eq!(
        opd.weekdays,
        vec![
            Weekday::Mon,
            Weekday::Tue,
            Weekday::Wed,
            Weekday::Thu,
            Weekday::Fri,
        ]
    );
    assert_eq!(opd.required_positions.len(), 2);
    for (name, headcount) in [("Medical Officer", 2), ("Senior Staff Nurse", 1)] {
        let position_id = &organization
            .positions
            .iter()
            .find(|position| position.name == name)
            .expect("position")
            .id;
        let requirement = opd
            .required_positions
            .iter()
            .find(|requirement| requirement.position_id == *position_id)
            .expect("requirement");
        assert_eq!(requirement.headcount, headcount);
    }

    let text = std::fs::read_to_string(&path).expect("read saved project");
    let value: serde_json::Value = serde_json::from_str(&text).expect("project json");
    assert_eq!(value["schema_version"], 2);

    std::fs::remove_file(&path).expect("temp file removed");
    assert!(!path.exists());
}

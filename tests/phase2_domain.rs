//! End-to-end coverage of the Phase 2 domain model through the public API.
//!
//! The fixture deliberately mirrors the corrected ownership:
//! the organization owns positions and employees; rosters own shifts only.

use chrono::NaiveTime;
use roster::domain::ProjectFile;
use roster::store;

fn at(hour: u32, minute: u32) -> NaiveTime {
    NaiveTime::from_hms_opt(hour, minute, 0).unwrap()
}

fn populated_project() -> ProjectFile {
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
    let nurses = organization
        .add_roster("Nurses")
        .expect("roster")
        .id
        .clone();

    let opd = organization
        .add_shift(&doctors, "OPD", at(8, 0), at(14, 0))
        .expect("shift")
        .id
        .clone();
    let emergency_morning = organization
        .add_shift(&doctors, "Emergency Morning", at(8, 0), at(15, 0))
        .expect("shift")
        .id
        .clone();
    let emergency_night = organization
        .add_shift(&doctors, "Emergency Night", at(15, 0), at(8, 0))
        .expect("shift")
        .id
        .clone();

    organization
        .require_position(&doctors, &opd, &medical_officer, 2)
        .expect("requirement");
    organization
        .require_position(&doctors, &opd, &staff_nurse, 1)
        .expect("requirement");
    organization
        .require_position(&doctors, &emergency_morning, &medical_officer, 1)
        .expect("requirement");
    organization
        .require_position(&doctors, &emergency_night, &medical_officer, 1)
        .expect("requirement");
    // Upsert, not duplicate: OPD still has exactly two requirements.
    organization
        .require_position(&doctors, &opd, &medical_officer, 3)
        .expect("requirement update");

    let amit = organization
        .add_employee("Dr. Amit", &doctors, &medical_officer)
        .expect("employee")
        .id
        .clone();
    organization
        .employee_mut(&amit)
        .expect("employee should exist")
        .add_skill("ACLS")
        .expect("skill");
    organization
        .employee_mut(&amit)
        .expect("employee should exist")
        .add_skill("  ACLS  ")
        .expect("duplicate skill is ignored");
    organization
        .add_employee("Dr. Khushi", &doctors, &medical_officer)
        .expect("employee");
    organization
        .add_employee("Nurse Rina", &nurses, &staff_nurse)
        .expect("employee");

    project
}

#[test]
fn domain_model_covers_rosters_staff_and_shifts() {
    let project = populated_project();
    let organization = &project.organizations[0];

    organization.validate().expect("valid organization");

    let doctors = organization
        .rosters
        .iter()
        .find(|roster| roster.name == "Doctors")
        .expect("doctors roster");
    let nurses = organization
        .rosters
        .iter()
        .find(|roster| roster.name == "Nurses")
        .expect("nurses roster");

    assert_ne!(doctors.id, nurses.id);
    assert_eq!(doctors.shifts.len(), 3);
    assert!(nurses.shifts.is_empty());
    assert_eq!(doctors.shifts[0].duration_minutes(), 360);
    assert_eq!(doctors.shifts[1].duration_minutes(), 420);
    assert_eq!(doctors.shifts[2].duration_minutes(), 1020);
    assert!(doctors.shifts[2].crosses_midnight());

    let opd = doctors
        .shifts
        .iter()
        .find(|shift| shift.name == "OPD")
        .expect("OPD shift");
    assert_eq!(opd.required_positions.len(), 2);
    let medical_officer = organization
        .positions
        .iter()
        .find(|position| position.name == "Medical Officer")
        .expect("medical officer position");
    assert_eq!(
        opd.required_positions
            .iter()
            .find(|requirement| requirement.position_id == medical_officer.id)
            .expect("medical officer requirement")
            .headcount,
        3
    );

    let doctor_names: Vec<&str> = organization
        .employees_in_roster(&doctors.id)
        .iter()
        .map(|employee| employee.name.as_str())
        .collect();
    let nurse_names: Vec<&str> = organization
        .employees_in_roster(&nurses.id)
        .iter()
        .map(|employee| employee.name.as_str())
        .collect();
    assert_eq!(doctor_names, vec!["Dr. Amit", "Dr. Khushi"]);
    assert_eq!(nurse_names, vec!["Nurse Rina"]);

    let amit = organization
        .employees
        .iter()
        .find(|employee| employee.name == "Dr. Amit")
        .expect("Dr. Amit");
    assert_eq!(amit.skills, vec!["ACLS".to_owned()]);

    // A roster only resolves its own shifts.
    assert!(doctors.shift(&doctors.shifts[0].id).is_some());
    assert!(nurses.shift(&doctors.shifts[0].id).is_none());
}

#[test]
fn populated_project_round_trips_through_the_store() {
    let path = std::env::temp_dir().join(format!(
        "roster-phase2-domain-{}.json",
        uuid::Uuid::new_v4()
    ));
    let project = populated_project();

    store::save_project(&project, &path).expect("save should succeed");
    let loaded = store::load_project(&path).expect("load should succeed");

    assert_eq!(project, loaded);
    loaded.organizations[0]
        .validate()
        .expect("loaded organization stays valid");

    let text = std::fs::read_to_string(&path).unwrap();
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["schema_version"], 2);
    assert!(value["organizations"][0]["positions"].is_array());
    assert!(value["organizations"][0]["employees"].is_array());
    assert!(value["organizations"][0]["rosters"].is_array());

    let _ = std::fs::remove_file(&path);
}

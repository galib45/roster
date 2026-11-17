//! Parity fixture: the legacy doctor scenario, expressed with the generalized
//! domain model.
//!
//! Every rule in `legacy/utils.rs` is copied here without doctor-specific types
//! or duty-category enums. The legacy prototype treats its rules as hard
//! prohibitions, so all copied rules are `Hard` with penalty 0. The
//! night→morning rest rule is added separately as a soft rule to show that the
//! new model can express it configurable.

use chrono::{NaiveDate, NaiveTime, Weekday};
use roster::domain::{ProjectFile, Severity, UnavailabilityScope};
use roster::store;

fn at(hour: u32, minute: u32) -> NaiveTime {
    NaiveTime::from_hms_opt(hour, minute, 0).unwrap()
}

fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).unwrap()
}

struct LegacyFixture {
    project: ProjectFile,
    doctors: String,
    night: String,
}

fn legacy_project() -> LegacyFixture {
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
    let doctors = organization
        .add_roster("Doctors")
        .expect("roster")
        .id
        .clone();

    let opd = organization
        .add_shift(&doctors, "OPD", at(8, 0), at(14, 0))
        .expect("shift")
        .id
        .clone();
    let morning = organization
        .add_shift(&doctors, "Emergency Morning", at(8, 0), at(15, 0))
        .expect("shift")
        .id
        .clone();
    let night = organization
        .add_shift(&doctors, "Emergency Night", at(15, 0), at(8, 0))
        .expect("shift")
        .id
        .clone();

    // Legacy slots: two OPD doctors per non-Friday day, one morning, one night.
    organization
        .require_position(&doctors, &opd, &medical_officer, 2)
        .expect("requirement");
    organization
        .require_position(&doctors, &morning, &medical_officer, 1)
        .expect("requirement");
    organization
        .require_position(&doctors, &night, &medical_officer, 1)
        .expect("requirement");

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
    let zakaria = organization
        .add_employee("Dr. Zakaria", &doctors, &medical_officer)
        .expect("employee")
        .id
        .clone();
    let toukir = organization
        .add_employee("Dr. Toukir", &doctors, &medical_officer)
        .expect("employee")
        .id
        .clone();
    let sohan = organization
        .add_employee("Dr. Sohan", &doctors, &medical_officer)
        .expect("employee")
        .id
        .clone();
    let atiya = organization
        .add_employee("Dr. Atiya", &doctors, &medical_officer)
        .expect("employee")
        .id
        .clone();
    let galib = organization
        .add_employee("Dr. Galib", &doctors, &medical_officer)
        .expect("employee")
        .id
        .clone();

    // Amit: Fri/Sat whole day; Sun Night + Tue Morning recurring.
    for weekday in [Weekday::Fri, Weekday::Sat] {
        organization
            .add_unavailability(
                &amit,
                UnavailabilityScope::Weekly { weekday },
                &[],
                Severity::Hard,
                0,
            )
            .expect("weekly unavailability");
    }
    organization
        .add_recurring_assignment(&amit, Weekday::Sun, &night)
        .expect("recurring assignment");
    organization
        .add_recurring_assignment(&amit, Weekday::Tue, &morning)
        .expect("recurring assignment");

    // Khushi: Sat/Mon/Wed Night; Sat Morning + Tue Night recurring.
    for weekday in [Weekday::Sat, Weekday::Mon, Weekday::Wed] {
        organization
            .add_unavailability(
                &khushi,
                UnavailabilityScope::Weekly { weekday },
                &[night.as_str()],
                Severity::Hard,
                0,
            )
            .expect("weekly unavailability");
    }
    organization
        .add_recurring_assignment(&khushi, Weekday::Sat, &morning)
        .expect("recurring assignment");
    organization
        .add_recurring_assignment(&khushi, Weekday::Tue, &night)
        .expect("recurring assignment");

    // Zakaria: Thu Night + Sun Morning recurring.
    organization
        .add_recurring_assignment(&zakaria, Weekday::Thu, &night)
        .expect("recurring assignment");
    organization
        .add_recurring_assignment(&zakaria, Weekday::Sun, &morning)
        .expect("recurring assignment");

    // Toukir: Sat Night recurring; whole day on 2026-11-16.
    organization
        .add_recurring_assignment(&toukir, Weekday::Sat, &night)
        .expect("recurring assignment");
    organization
        .add_unavailability(
            &toukir,
            UnavailabilityScope::Date {
                date: date(2026, 11, 16),
            },
            &[],
            Severity::Hard,
            0,
        )
        .expect("date unavailability");

    // Sohan: Fri Night + Thu Morning recurring; leave 16..=30 November.
    organization
        .add_recurring_assignment(&sohan, Weekday::Fri, &night)
        .expect("recurring assignment");
    organization
        .add_recurring_assignment(&sohan, Weekday::Thu, &morning)
        .expect("recurring assignment");
    organization
        .add_unavailability(
            &sohan,
            UnavailabilityScope::Leave {
                start: date(2026, 11, 16),
                end: date(2026, 11, 30),
            },
            &[],
            Severity::Hard,
            0,
        )
        .expect("leave");

    // Atiya: Mon Morning + Wed Morning recurring; leave 25..=29 November.
    organization
        .add_recurring_assignment(&atiya, Weekday::Mon, &morning)
        .expect("recurring assignment");
    organization
        .add_recurring_assignment(&atiya, Weekday::Wed, &morning)
        .expect("recurring assignment");
    organization
        .add_unavailability(
            &atiya,
            UnavailabilityScope::Leave {
                start: date(2026, 11, 25),
                end: date(2026, 11, 29),
            },
            &[],
            Severity::Hard,
            0,
        )
        .expect("leave");

    // Galib: Mon Night + Wed Night recurring.
    organization
        .add_recurring_assignment(&galib, Weekday::Mon, &night)
        .expect("recurring assignment");
    organization
        .add_recurring_assignment(&galib, Weekday::Wed, &night)
        .expect("recurring assignment");

    LegacyFixture {
        project,
        doctors,
        night,
    }
}

#[test]
fn legacy_doctor_scenario_maps_to_the_generalized_model() {
    let fixture = legacy_project();
    let organization = &fixture.project.organizations[0];

    organization.validate().expect("valid organization");

    let shifts = &organization
        .roster(&fixture.doctors)
        .expect("doctors roster")
        .shifts;
    assert_eq!(shifts.len(), 3);
    let durations: Vec<u32> = shifts
        .iter()
        .map(|shift| shift.duration_minutes())
        .collect();
    assert_eq!(durations, vec![360, 420, 1020]);
    assert!(shifts[2].crosses_midnight());

    assert_eq!(organization.employees.len(), 7);

    let totals = |name: &str| {
        let employee = organization
            .employees
            .iter()
            .find(|employee| employee.name == name)
            .unwrap_or_else(|| panic!("employee {name}"));
        (
            employee.unavailability.len(),
            employee.recurring_assignments.len(),
        )
    };

    assert_eq!(totals("Dr. Amit"), (2, 2));
    assert_eq!(totals("Dr. Khushi"), (3, 2));
    assert_eq!(totals("Dr. Zakaria"), (0, 2));
    assert_eq!(totals("Dr. Toukir"), (1, 1));
    assert_eq!(totals("Dr. Sohan"), (1, 2));
    assert_eq!(totals("Dr. Atiya"), (1, 2));
    assert_eq!(totals("Dr. Galib"), (0, 2));

    let weekly = organization
        .employees
        .iter()
        .flat_map(|employee| &employee.unavailability)
        .filter(|rule| matches!(rule.scope, UnavailabilityScope::Weekly { .. }))
        .count();
    let on_date = organization
        .employees
        .iter()
        .flat_map(|employee| &employee.unavailability)
        .filter(|rule| matches!(rule.scope, UnavailabilityScope::Date { .. }))
        .count();
    let leave = organization
        .employees
        .iter()
        .flat_map(|employee| &employee.unavailability)
        .filter(|rule| matches!(rule.scope, UnavailabilityScope::Leave { .. }))
        .count();
    let recurring: usize = organization
        .employees
        .iter()
        .map(|employee| employee.recurring_assignments.len())
        .sum();

    assert_eq!((weekly, on_date, leave, recurring), (5, 1, 2, 13));

    for employee in &organization.employees {
        for assignment in &employee.recurring_assignments {
            assert!(
                organization
                    .roster(&fixture.doctors)
                    .expect("doctors roster")
                    .shift(&assignment.shift_id)
                    .is_some(),
                "{} references a shift outside the Doctors roster",
                employee.name
            );
        }
    }
}

#[test]
fn legacy_rest_rule_is_expressible_and_configurable() {
    let mut fixture = legacy_project();
    let organization = &mut fixture.project.organizations[0];
    let morning = organization
        .roster(&fixture.doctors)
        .expect("doctors roster")
        .shifts
        .iter()
        .find(|shift| shift.name == "Emergency Morning")
        .expect("morning shift")
        .id
        .clone();

    organization
        .add_rest_rule(
            &fixture.doctors,
            &fixture.night,
            &morning,
            480,
            Severity::Soft,
            3,
        )
        .expect("rest rule");

    let roster = organization.roster(&fixture.doctors).expect("roster");
    assert_eq!(roster.rest_rules.len(), 1);
    assert_eq!(roster.rest_rules[0].min_rest_minutes, 480);
    assert_eq!(roster.rest_rules[0].severity, Severity::Soft);
    assert_eq!(roster.rest_rules[0].penalty, 3);

    // Adding the rest rule does not rewrite the copied hard prohibitions.
    assert!(
        organization
            .employees
            .iter()
            .flat_map(|employee| &employee.unavailability)
            .all(|rule| rule.severity == Severity::Hard && rule.penalty == 0)
    );
    organization.validate().expect("valid organization");
}

#[test]
fn rule_rich_project_round_trips_through_the_store() {
    let path = std::env::temp_dir().join(format!(
        "roster-legacy-parity-{}.json",
        uuid::Uuid::new_v4()
    ));
    let fixture = legacy_project();
    let project = &fixture.project;

    store::save_project(project, &path).expect("save should succeed");
    let loaded = store::load_project(&path).expect("load should succeed");

    assert_eq!(project, &loaded);
    loaded.organizations[0]
        .validate()
        .expect("loaded organization stays valid");

    let text = std::fs::read_to_string(&path).unwrap();
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["schema_version"], 2);
    let employees = value["organizations"][0]["employees"]
        .as_array()
        .expect("employees array");
    assert_eq!(employees.len(), 7);
    assert!(
        employees
            .iter()
            .all(|employee| employee["recurring_assignments"].is_array())
    );

    let _ = std::fs::remove_file(&path);
}

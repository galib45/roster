//! Workload measurement and fairness scoring over dated rosters.
//!
//! Fairness is one number: the sum of absolute deviations between each
//! member's assigned minutes and their comparison mean. Employees in a
//! sharing group compare against their group's average; everyone else compares
//! against the roster average.
//!
//! Only the roster's filled slots count. Previous-day context is excluded
//! because it lies outside the period being generated, and unfilled seats add
//! nothing. The calculation is read-only and deterministic.

use std::collections::HashMap;

use crate::domain::{Organization, Roster, RosterTemplate};

/// One employee's workload inside a roster.
#[derive(Debug, Clone, PartialEq)]
pub struct EmployeeWorkload {
    pub employee_id: String,
    /// Total minutes of the filled seats assigned to this employee.
    pub minutes: u32,
    /// Number of filled seats assigned to this employee.
    pub shift_count: u32,
    /// The sharing group this employee belongs to, when grouped.
    pub group_id: Option<String>,
}

/// Workload averaged across one sharing group.
#[derive(Debug, Clone, PartialEq)]
pub struct GroupWorkload {
    pub group_id: String,
    pub name: String,
    pub member_count: usize,
    pub total_minutes: u64,
    pub average_minutes: f64,
    /// Sum of |member minutes − group average| within this group.
    pub deviation_minutes: f64,
}

/// The fairness report for one roster.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkloadReport {
    /// Every snapshot member, sorted by employee ID.
    pub employees: Vec<EmployeeWorkload>,
    /// Every sharing group, sorted by group ID.
    pub groups: Vec<GroupWorkload>,
    pub roster_total_minutes: u64,
    /// Total minutes divided by every snapshot member, including zero-workload
    /// staff.
    pub roster_average_minutes: f64,
    /// Sum of |employee minutes − comparison mean| across the roster.
    /// Lower is fairer.
    pub deviation_minutes: f64,
    pub max_minutes: u32,
    pub min_minutes: u32,
}

/// Compute workload and fairness for one dated roster.
///
/// Members are read from the roster snapshot, not the organization, so staff
/// who left the template still appear with zero minutes and later template
/// changes never rewrite history.
pub fn workload(
    _organization: &Organization,
    template: &RosterTemplate,
    roster: &Roster,
) -> WorkloadReport {
    let mut assigned: HashMap<&str, (u32, u32)> = HashMap::new();
    for dated in &roster.shifts {
        let duration = dated.duration_minutes();
        for slot in &dated.slots {
            if let Some(employee_id) = &slot.employee_id {
                let entry = assigned.entry(employee_id.as_str()).or_insert((0, 0));
                entry.0 += duration;
                entry.1 += 1;
            }
        }
    }

    let mut employees: Vec<EmployeeWorkload> = roster
        .members
        .iter()
        .map(|member| {
            let (minutes, shift_count) = assigned
                .get(member.employee_id.as_str())
                .copied()
                .unwrap_or((0, 0));
            EmployeeWorkload {
                employee_id: member.employee_id.clone(),
                minutes,
                shift_count,
                group_id: template
                    .sharing_group_for_employee(&member.employee_id)
                    .map(|group| group.id.clone()),
            }
        })
        .collect();
    employees.sort_by(|first, second| first.employee_id.cmp(&second.employee_id));

    let roster_total_minutes: u64 = employees
        .iter()
        .map(|employee| u64::from(employee.minutes))
        .sum();
    let roster_average_minutes = if employees.is_empty() {
        0.0
    } else {
        roster_total_minutes as f64 / employees.len() as f64
    };

    let mut groups: Vec<GroupWorkload> = template
        .sharing_groups
        .iter()
        .map(|group| {
            let member_minutes: Vec<u32> = group
                .employee_ids
                .iter()
                .map(|employee_id| {
                    employees
                        .iter()
                        .find(|employee| &employee.employee_id == employee_id)
                        .map(|employee| employee.minutes)
                        .unwrap_or(0)
                })
                .collect();
            let total: u64 = member_minutes
                .iter()
                .map(|minutes| u64::from(*minutes))
                .sum();
            let member_count = member_minutes.len();
            let average = if member_count == 0 {
                0.0
            } else {
                total as f64 / member_count as f64
            };
            let deviation: f64 = member_minutes
                .iter()
                .map(|minutes| (*minutes as f64 - average).abs())
                .sum();

            GroupWorkload {
                group_id: group.id.clone(),
                name: group.name.clone(),
                member_count,
                total_minutes: total,
                average_minutes: average,
                deviation_minutes: deviation,
            }
        })
        .collect();
    groups.sort_by(|first, second| first.group_id.cmp(&second.group_id));

    let group_averages: HashMap<&str, f64> = groups
        .iter()
        .map(|group| (group.group_id.as_str(), group.average_minutes))
        .collect();
    let deviation_minutes: f64 = employees
        .iter()
        .map(|employee| {
            let comparison = employee
                .group_id
                .as_deref()
                .and_then(|group_id| group_averages.get(group_id).copied())
                .unwrap_or(roster_average_minutes);
            (employee.minutes as f64 - comparison).abs()
        })
        .sum();

    let max_minutes = employees
        .iter()
        .map(|employee| employee.minutes)
        .max()
        .unwrap_or(0);
    let min_minutes = employees
        .iter()
        .map(|employee| employee.minutes)
        .min()
        .unwrap_or(0);

    WorkloadReport {
        employees,
        groups,
        roster_total_minutes,
        roster_average_minutes,
        deviation_minutes,
        max_minutes,
        min_minutes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use chrono::{NaiveDate, NaiveTime};

    use crate::domain::{Organization, Roster};

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
        organization: Organization,
        template_id: String,
        roster_id: String,
        employees: Vec<String>,
    }

    /// A one-day roster with a 720-minute day shift (two seats) and a
    /// 1020-minute night shift (one seat).
    fn fixture(employee_count: usize) -> Fixture {
        let mut organization = Organization::new("City Hospital");

        let template_id = organization
            .add_roster_template("Doctors")
            .unwrap()
            .id
            .clone();
        let position_id = organization
            .add_position("Medical Officer")
            .unwrap()
            .id
            .clone();
        organization
            .include_position(&template_id, &position_id)
            .unwrap();

        for index in 0..employee_count {
            organization
                .add_employee(&format!("Dr. {index}"), &position_id)
                .unwrap();
        }

        let day_id = organization
            .add_shift(&template_id, "Day", at(8, 0), at(20, 0))
            .unwrap()
            .id
            .clone();
        let night_id = organization
            .add_shift(&template_id, "Night", at(15, 0), at(8, 0))
            .unwrap()
            .id
            .clone();
        organization
            .require_position(&template_id, &day_id, &position_id, 2)
            .unwrap();
        organization
            .require_position(&template_id, &night_id, &position_id, 1)
            .unwrap();

        let roster_id = organization
            .create_roster(
                &template_id,
                date(2026, 11, 16),
                date(2026, 11, 16),
                Vec::new(),
            )
            .unwrap()
            .id
            .clone();

        let employees: Vec<String> = organization
            .template_members(&template_id)
            .iter()
            .map(|employee| employee.id.clone())
            .collect();

        Fixture {
            organization,
            template_id,
            roster_id,
            employees,
        }
    }

    impl Fixture {
        fn assign(&mut self, shift_name: &str, employee: &str) {
            let roster = self
                .organization
                .roster(&self.template_id, &self.roster_id)
                .expect("roster");
            let dated = roster
                .shifts
                .iter()
                .find(|shift| shift.name == shift_name)
                .expect("dated shift");
            let dated_id = dated.id.clone();
            let slot_id = dated
                .slots
                .iter()
                .find(|slot| slot.employee_id.is_none())
                .expect("an unfilled seat")
                .id
                .clone();

            self.organization
                .assign_slot(
                    &self.template_id,
                    &self.roster_id,
                    &dated_id,
                    &slot_id,
                    employee,
                )
                .expect("assignment");
        }

        fn template(&self) -> &RosterTemplate {
            self.organization
                .roster_template(&self.template_id)
                .expect("template")
        }

        fn roster(&self) -> &Roster {
            self.template().roster(&self.roster_id).expect("roster")
        }

        fn report(&self) -> WorkloadReport {
            workload(&self.organization, self.template(), self.roster())
        }

        fn add_employee(&mut self, name: &str) -> String {
            let position_id = self
                .organization
                .employee(&self.employees[0])
                .expect("employee")
                .position_id
                .clone();
            self.organization
                .add_employee(name, &position_id)
                .expect("employee")
                .id
                .clone()
        }
    }

    #[test]
    fn empty_plans_report_zero_workload_for_everyone() {
        let fixture = fixture(3);
        let report = fixture.report();

        assert_eq!(report.employees.len(), 3);
        assert!(
            report
                .employees
                .iter()
                .all(|employee| employee.minutes == 0)
        );
        assert!(
            report
                .employees
                .iter()
                .all(|employee| employee.shift_count == 0)
        );
        assert_eq!(report.roster_total_minutes, 0);
        approx(report.roster_average_minutes, 0.0);
        approx(report.deviation_minutes, 0.0);
        assert_eq!(report.max_minutes, 0);
        assert_eq!(report.min_minutes, 0);
        assert!(report.groups.is_empty());
    }

    #[test]
    fn sums_filled_slot_durations_and_counts_shifts() {
        let mut fixture = fixture(3);
        let first = fixture.employees[0].clone();
        let second = fixture.employees[1].clone();
        fixture.assign("Day", &first);
        fixture.assign("Night", &second);
        let report = fixture.report();

        let day = report
            .employees
            .iter()
            .find(|employee| employee.employee_id == first)
            .expect("day employee");
        let night = report
            .employees
            .iter()
            .find(|employee| employee.employee_id == second)
            .expect("night employee");

        assert_eq!(day.minutes, 720);
        assert_eq!(day.shift_count, 1);
        assert_eq!(night.minutes, 1020);
        assert_eq!(night.shift_count, 1);
        assert_eq!(report.roster_total_minutes, 1740);
        assert_eq!(report.max_minutes, 1020);
        assert_eq!(report.min_minutes, 0);
    }

    #[test]
    fn groups_drive_the_comparison_mean() {
        let mut fixture = fixture(2);
        let first = fixture.employees[0].clone();
        let second = fixture.employees[1].clone();
        fixture.assign("Day", &first);
        fixture.assign("Night", &second);

        let group_id = fixture
            .organization
            .add_sharing_group(
                &fixture.template_id,
                "Registrars",
                &[first.as_str(), second.as_str()],
            )
            .expect("group")
            .id
            .clone();

        let report = fixture.report();

        assert_eq!(report.groups.len(), 1);
        let group = &report.groups[0];
        assert_eq!(group.member_count, 2);
        assert_eq!(group.total_minutes, 1740);
        approx(group.average_minutes, 870.0);
        approx(group.deviation_minutes, 300.0);
        approx(report.deviation_minutes, 300.0);
        assert!(
            report
                .employees
                .iter()
                .all(|employee| employee.group_id.as_deref() == Some(group_id.as_str()))
        );
    }

    #[test]
    fn ungrouped_employees_compare_against_the_roster_average() {
        let mut fixture = fixture(2);
        let first = fixture.employees[0].clone();
        let second = fixture.employees[1].clone();
        let third = fixture.add_employee("Dr. Three");

        fixture.assign("Day", &first);
        fixture.assign("Night", &second);
        fixture
            .organization
            .add_sharing_group(
                &fixture.template_id,
                "Registrars",
                &[first.as_str(), second.as_str()],
            )
            .expect("group");

        let report = fixture.report();
        assert_eq!(report.employees.len(), 3);

        // Roster average divides by all three employees: 1740 / 3 = 580.
        approx(report.roster_average_minutes, 580.0);
        let ungrouped = report
            .employees
            .iter()
            .find(|employee| employee.employee_id == third)
            .expect("ungrouped");
        assert!(ungrouped.group_id.is_none());

        // 150 + 150 within the group, plus 580 for the ungrouped employee.
        approx(report.deviation_minutes, 880.0);
    }

    #[test]
    fn balanced_workloads_have_zero_deviation() {
        let mut fixture = fixture(2);
        let first = fixture.employees[0].clone();
        let second = fixture.employees[1].clone();

        fixture.assign("Day", &first);
        fixture.assign("Day", &second);

        let report = fixture.report();
        approx(report.roster_average_minutes, 720.0);
        approx(report.deviation_minutes, 0.0);
        assert_eq!(report.max_minutes, 720);
        assert_eq!(report.min_minutes, 720);
    }

    #[test]
    fn reports_are_deterministic() {
        let mut fixture = fixture(2);
        let first = fixture.employees[0].clone();
        fixture.assign("Day", &first);

        let first_report = fixture.report();
        let second_report = fixture.report();

        assert_eq!(first_report, second_report);
        let ids: Vec<&str> = first_report
            .employees
            .iter()
            .map(|employee| employee.employee_id.as_str())
            .collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(ids, sorted);
    }
}

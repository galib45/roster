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

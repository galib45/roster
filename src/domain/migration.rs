//! Migrations from earlier project-file schemas.
//!
//! Version 1 stored organizations only. Version 2 added organization-level
//! positions, employees, and staff-group rosters with embedded plans. Version 3
//! re-baselines rosters as reusable templates with position-based membership
//! and standalone dated rosters. The versioned structs below are migration
//! contracts: they must not change when the current schema evolves.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, NaiveDate, Utc};
use serde::Deserialize;

use super::{
    ContextAssignment, DatedShift, Employee, Organization, PlanStatus, Position, ProjectFile,
    RecurringAssignment, RestRule, Roster, RosterMember, RosterTemplate, SCHEMA_VERSION,
    SharingGroup, ShiftTemplate, Unavailability,
};

/// The project shape written by Phase 1 (`schema_version: 1`).
#[derive(Debug, Deserialize)]
pub struct ProjectFileV1 {
    pub schema_version: u32,
    pub app: String,
    pub saved_at: DateTime<Utc>,
    pub organizations: Vec<OrganizationV1>,
}

/// A Phase 1 organization, which had no children.
#[derive(Debug, Deserialize)]
pub struct OrganizationV1 {
    pub id: String,
    pub name: String,
}

impl From<ProjectFileV1> for ProjectFile {
    fn from(v1: ProjectFileV1) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            app: v1.app,
            saved_at: v1.saved_at,
            organizations: v1
                .organizations
                .into_iter()
                .map(|organization| Organization {
                    id: organization.id,
                    name: organization.name,
                    positions: Vec::new(),
                    employees: Vec::new(),
                    roster_templates: Vec::new(),
                })
                .collect(),
        }
    }
}

/// The project shape written by Phase 2 (`schema_version: 2`).
#[derive(Debug, Deserialize)]
pub struct ProjectFileV2 {
    pub schema_version: u32,
    pub app: String,
    pub saved_at: DateTime<Utc>,
    pub organizations: Vec<OrganizationV2>,
}

/// A version 2 organization, whose employees carried a roster id.
#[derive(Debug, Deserialize)]
pub struct OrganizationV2 {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub positions: Vec<Position>,
    #[serde(default)]
    pub employees: Vec<EmployeeV2>,
    #[serde(default)]
    pub rosters: Vec<RosterV2>,
}

/// A version 2 employee, with the roster assignment removed in v3.
#[derive(Debug, Deserialize)]
pub struct EmployeeV2 {
    pub id: String,
    pub name: String,
    pub roster_id: String,
    pub position_id: String,
    #[serde(default)]
    pub skills: Vec<String>,
    #[serde(default)]
    pub unavailability: Vec<Unavailability>,
    #[serde(default)]
    pub recurring_assignments: Vec<RecurringAssignment>,
}

/// A version 2 staff-group roster with embedded plans.
#[derive(Debug, Deserialize)]
pub struct RosterV2 {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub shifts: Vec<ShiftTemplate>,
    #[serde(default)]
    pub rest_rules: Vec<RestRule>,
    #[serde(default)]
    pub sharing_groups: Vec<SharingGroup>,
    #[serde(default)]
    pub plans: Vec<RosterPlanV2>,
}

/// A version 2 generated plan.
#[derive(Debug, Deserialize)]
pub struct RosterPlanV2 {
    pub id: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub status: PlanStatus,
    #[serde(default)]
    pub context: Vec<ContextAssignment>,
    #[serde(default)]
    pub shifts: Vec<DatedShift>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<ProjectFileV2> for ProjectFile {
    fn from(v2: ProjectFileV2) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            app: v2.app,
            saved_at: v2.saved_at,
            organizations: v2.organizations.into_iter().map(Organization::from).collect(),
        }
    }
}

impl From<OrganizationV2> for Organization {
    fn from(v2: OrganizationV2) -> Self {
        let mut members_by_roster: HashMap<String, Vec<RosterMember>> = HashMap::new();
        let mut positions_by_roster: HashMap<String, Vec<String>> = HashMap::new();
        let mut employees = Vec::with_capacity(v2.employees.len());

        for old in v2.employees {
            let roster_id = old.roster_id;
            let employee = Employee {
                id: old.id,
                name: old.name,
                position_id: old.position_id,
                skills: old.skills,
                unavailability: old.unavailability,
                recurring_assignments: old.recurring_assignments,
            };

            let positions = positions_by_roster.entry(roster_id.clone()).or_default();
            if !positions.contains(&employee.position_id) {
                positions.push(employee.position_id.clone());
            }
            members_by_roster
                .entry(roster_id)
                .or_default()
                .push(RosterMember::new(&employee.id, &employee.position_id));
            employees.push(employee);
        }

        let roster_templates = v2
            .rosters
            .into_iter()
            .map(|roster| {
                let mut position_ids = positions_by_roster.remove(&roster.id).unwrap_or_default();
                let mut seen: HashSet<String> = position_ids.iter().cloned().collect();
                for shift in &roster.shifts {
                    for requirement in &shift.required_positions {
                        if seen.insert(requirement.position_id.clone()) {
                            position_ids.push(requirement.position_id.clone());
                        }
                    }
                }

                let members: Vec<RosterMember> = members_by_roster
                    .remove(&roster.id)
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|member| position_ids.contains(&member.position_id))
                    .collect();

                let rosters = roster
                    .plans
                    .into_iter()
                    .map(|plan| Roster {
                        id: plan.id,
                        template_id: roster.id.clone(),
                        name: format!("{} — {} to {}", roster.name, plan.start_date, plan.end_date),
                        start: plan.start_date,
                        end: plan.end_date,
                        status: plan.status,
                        members: members.clone(),
                        context: plan.context,
                        shifts: plan.shifts,
                        created_at: plan.created_at,
                        updated_at: plan.updated_at,
                    })
                    .collect();

                RosterTemplate {
                    id: roster.id,
                    name: roster.name,
                    position_ids,
                    excluded_employee_ids: Vec::new(),
                    shifts: roster.shifts,
                    rest_rules: roster.rest_rules,
                    sharing_groups: roster.sharing_groups,
                    rosters,
                }
            })
            .collect();

        Self {
            id: v2.id,
            name: v2.name,
            positions: v2.positions,
            employees,
            roster_templates,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        DomainError, Severity, StaffingRequirement, UnavailabilityScope,
    };
    use chrono::{NaiveTime, TimeZone, Weekday};
    use std::collections::HashMap;

    #[test]
    fn migrates_v1_organizations_to_empty_current_collections() {
        let organization = OrganizationV1 {
            id: "org-1".to_owned(),
            name: "City Hospital".to_owned(),
        };
        let v1 = ProjectFileV1 {
            schema_version: 1,
            app: "roster".to_owned(),
            saved_at: Utc.with_ymd_and_hms(2026, 11, 16, 0, 0, 0).unwrap(),
            organizations: vec![organization],
        };

        let project: ProjectFile = v1.into();

        assert_eq!(project.schema_version, SCHEMA_VERSION);
        assert_eq!(project.app, "roster");
        assert_eq!(
            project.saved_at,
            Utc.with_ymd_and_hms(2026, 11, 16, 0, 0, 0).unwrap()
        );
        assert_eq!(project.organizations.len(), 1);
        assert_eq!(project.organizations[0].id, "org-1");
        assert_eq!(project.organizations[0].name, "City Hospital");
        assert!(project.organizations[0].positions.is_empty());
        assert!(project.organizations[0].employees.is_empty());
        assert!(project.organizations[0].roster_templates.is_empty());
    }

    fn at(hour: u32, minute: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(hour, minute, 0).unwrap()
    }

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).unwrap()
    }

    #[test]
    fn migrates_v2_rosters_to_templates_and_plans_to_rosters() {
        let medical_officer = Position {
            id: "pos-1".to_owned(),
            name: "Medical Officer".to_owned(),
        };
        let staff_nurse = Position {
            id: "pos-2".to_owned(),
            name: "Senior Staff Nurse".to_owned(),
        };

        let mut opd = ShiftTemplate::new("OPD", at(8, 0), at(14, 0)).unwrap();
        opd.id = "shift-1".to_owned();
        opd.required_positions.push(StaffingRequirement {
            position_id: "pos-1".to_owned(),
            headcount: 2,
        });
        opd.weekdays = vec![Weekday::Mon, Weekday::Tue];

        let plan = RosterPlanV2 {
            id: "plan-1".to_owned(),
            start_date: date(2026, 11, 16),
            end_date: date(2026, 11, 30),
            status: PlanStatus::Generated,
            context: vec![ContextAssignment {
                date: date(2026, 11, 15),
                name: "OPD".to_owned(),
                start: at(8, 0),
                end: at(14, 0),
                employee_id: "emp-1".to_owned(),
                template_id: Some("shift-1".to_owned()),
            }],
            shifts: vec![DatedShift::from_template(&opd, date(2026, 11, 16))],
            created_at: Utc.with_ymd_and_hms(2026, 11, 17, 0, 0, 0).unwrap(),
            updated_at: Utc.with_ymd_and_hms(2026, 11, 17, 0, 0, 0).unwrap(),
        };

        let v2 = ProjectFileV2 {
            schema_version: 2,
            app: "roster".to_owned(),
            saved_at: Utc.with_ymd_and_hms(2026, 11, 17, 0, 0, 0).unwrap(),
            organizations: vec![OrganizationV2 {
                id: "org-1".to_owned(),
                name: "City Hospital".to_owned(),
                positions: vec![medical_officer.clone(), staff_nurse.clone()],
                employees: vec![
                    EmployeeV2 {
                        id: "emp-1".to_owned(),
                        name: "Dr. Amit".to_owned(),
                        roster_id: "roster-1".to_owned(),
                        position_id: "pos-1".to_owned(),
                        skills: vec!["ACLS".to_owned()],
                        unavailability: Vec::new(),
                        recurring_assignments: Vec::new(),
                    },
                    EmployeeV2 {
                        id: "emp-2".to_owned(),
                        name: "Nurse Rina".to_owned(),
                        roster_id: "roster-2".to_owned(),
                        position_id: "pos-2".to_owned(),
                        skills: Vec::new(),
                        unavailability: Vec::new(),
                        recurring_assignments: Vec::new(),
                    },
                ],
                rosters: vec![RosterV2 {
                    id: "roster-1".to_owned(),
                    name: "Doctors".to_owned(),
                    shifts: vec![opd.clone()],
                    rest_rules: Vec::new(),
                    sharing_groups: Vec::new(),
                    plans: vec![plan],
                }],
            }],
        };

        let project: ProjectFile = v2.into();
        let organization = &project.organizations[0];

        assert_eq!(project.schema_version, SCHEMA_VERSION);
        assert_eq!(organization.roster_templates.len(), 1);
        let template = &organization.roster_templates[0];
        assert_eq!(template.name, "Doctors");
        assert_eq!(template.position_ids, vec!["pos-1".to_owned()]);
        assert!(template.excluded_employee_ids.is_empty());
        assert_eq!(template.rosters.len(), 1);

        let roster = &template.rosters[0];
        assert_eq!(roster.template_id, template.id);
        assert_eq!(roster.status, PlanStatus::Generated);
        assert_eq!(roster.members.len(), 1);
        assert_eq!(roster.members[0].employee_id, "emp-1");
        assert_eq!(roster.members[0].position_id, "pos-1");
        assert!(roster.name.contains("2026-11-16"));
        assert_eq!(roster.context.len(), 1);

        assert!(organization.employees[0].position_id == "pos-1");
        assert_eq!(organization.employees[0].skills, vec!["ACLS".to_owned()]);
        assert_eq!(organization.employees[1].position_id, "pos-2");
        assert_eq!(organization.validate(), Ok(()));
    }

    #[test]
    fn v2_employees_without_rules_still_convert() {
        let v2 = ProjectFileV2 {
            schema_version: 2,
            app: "roster".to_owned(),
            saved_at: Utc.with_ymd_and_hms(2026, 11, 17, 0, 0, 0).unwrap(),
            organizations: vec![OrganizationV2 {
                id: "org-1".to_owned(),
                name: "City Hospital".to_owned(),
                positions: vec![Position {
                    id: "pos-1".to_owned(),
                    name: "Medical Officer".to_owned(),
                }],
                employees: vec![EmployeeV2 {
                    id: "emp-1".to_owned(),
                    name: "Dr. Amit".to_owned(),
                    roster_id: "roster-1".to_owned(),
                    position_id: "pos-1".to_owned(),
                    skills: Vec::new(),
                    unavailability: vec![Unavailability::new(
                        UnavailabilityScope::Weekly {
                            weekday: Weekday::Fri,
                        },
                        Vec::new(),
                        Severity::Hard,
                        0,
                    )
                    .unwrap()],
                    recurring_assignments: Vec::new(),
                }],
                rosters: Vec::new(),
            }],
        };

        let project: ProjectFile = v2.into();
        let organization = &project.organizations[0];
        assert!(organization.roster_templates.is_empty());
        assert_eq!(organization.employees[0].name, "Dr. Amit");
        assert!(!matches!(
            organization.validate(),
            Err(DomainError::DanglingReference { .. })
        ));
    }

    #[test]
    fn v2_requirements_contribute_positions_to_template_membership() {
        let mut shift = ShiftTemplate::new("OPD", at(8, 0), at(14, 0)).unwrap();
        shift.id = "shift-1".to_owned();
        shift.required_positions.push(StaffingRequirement {
            position_id: "pos-2".to_owned(),
            headcount: 1,
        });

        let v2 = ProjectFileV2 {
            schema_version: 2,
            app: "roster".to_owned(),
            saved_at: Utc.with_ymd_and_hms(2026, 11, 17, 0, 0, 0).unwrap(),
            organizations: vec![OrganizationV2 {
                id: "org-1".to_owned(),
                name: "City Hospital".to_owned(),
                positions: vec![
                    Position {
                        id: "pos-1".to_owned(),
                        name: "Medical Officer".to_owned(),
                    },
                    Position {
                        id: "pos-2".to_owned(),
                        name: "Nurse".to_owned(),
                    },
                ],
                employees: vec![
                    EmployeeV2 {
                        id: "emp-1".to_owned(),
                        name: "Dr. Amit".to_owned(),
                        roster_id: "roster-1".to_owned(),
                        position_id: "pos-1".to_owned(),
                        skills: Vec::new(),
                        unavailability: Vec::new(),
                        recurring_assignments: Vec::new(),
                    },
                    EmployeeV2 {
                        id: "emp-2".to_owned(),
                        name: "Nurse Rina".to_owned(),
                        roster_id: "roster-1".to_owned(),
                        position_id: "pos-2".to_owned(),
                        skills: Vec::new(),
                        unavailability: Vec::new(),
                        recurring_assignments: Vec::new(),
                    },
                ],
                rosters: vec![RosterV2 {
                    id: "roster-1".to_owned(),
                    name: "Doctors".to_owned(),
                    shifts: vec![shift],
                    rest_rules: Vec::new(),
                    sharing_groups: Vec::new(),
                    plans: Vec::new(),
                }],
            }],
        };

        let project: ProjectFile = v2.into();
        let template = &project.organizations[0].roster_templates[0];
        assert!(template.includes_position("pos-1"));
        assert!(template.includes_position("pos-2"));
        assert_eq!(project.organizations[0].validate(), Ok(()));
    }

    #[test]
    fn v2_conversion_keeps_shared_rosters_membership_stable() {
        let v2 = ProjectFileV2 {
            schema_version: 2,
            app: "roster".to_owned(),
            saved_at: Utc.with_ymd_and_hms(2026, 11, 17, 0, 0, 0).unwrap(),
            organizations: vec![OrganizationV2 {
                id: "org-1".to_owned(),
                name: "City Hospital".to_owned(),
                positions: vec![Position {
                    id: "pos-1".to_owned(),
                    name: "Medical Officer".to_owned(),
                }],
                employees: vec![
                    EmployeeV2 {
                        id: "emp-1".to_owned(),
                        name: "Dr. Amit".to_owned(),
                        roster_id: "roster-1".to_owned(),
                        position_id: "pos-1".to_owned(),
                        skills: Vec::new(),
                        unavailability: Vec::new(),
                        recurring_assignments: Vec::new(),
                    },
                    EmployeeV2 {
                        id: "emp-2".to_owned(),
                        name: "Dr. Khushi".to_owned(),
                        roster_id: "roster-2".to_owned(),
                        position_id: "pos-1".to_owned(),
                        skills: Vec::new(),
                        unavailability: Vec::new(),
                        recurring_assignments: Vec::new(),
                    },
                ],
                rosters: vec![
                    RosterV2 {
                        id: "roster-1".to_owned(),
                        name: "Doctors".to_owned(),
                        shifts: Vec::new(),
                        rest_rules: Vec::new(),
                        sharing_groups: Vec::new(),
                        plans: Vec::new(),
                    },
                    RosterV2 {
                        id: "roster-2".to_owned(),
                        name: "Visiting".to_owned(),
                        shifts: Vec::new(),
                        rest_rules: Vec::new(),
                        sharing_groups: Vec::new(),
                        plans: Vec::new(),
                    },
                ],
            }],
        };

        let project: ProjectFile = v2.into();
        let organization = &project.organizations[0];
        let doctors = &organization.roster_templates[0];
        assert_eq!(doctors.position_ids, vec!["pos-1".to_owned()]);
        assert_eq!(
            organization
                .template_members(&doctors.id)
                .iter()
                .map(|employee| employee.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Dr. Amit", "Dr. Khushi"]
        );
    }
}

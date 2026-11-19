//! The CP model's seat variables and presence literals.
//!
//! One [`ShiftSlot`](crate::domain::ShiftSlot) becomes one integer variable:
//! `0` is an empty seat and `e + 1` is the employee at index `e` of the seat's
//! eligible-employee list. Every rule then reads "is employee `e` in seat `i`?"
//! from a reified literal, which keeps the constraint layer free of index
//! arithmetic.

use std::collections::HashMap;

use chrono::{NaiveDate, NaiveTime};
use pumpkin_solver::core::variables::{DomainId, Literal, TransformableVariable};
use pumpkin_solver::{Solver, equals};

use crate::domain::{Organization, Roster, RosterTemplate};

use super::SolveError;

/// One seat in the dated roster, resolved into the data the model needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Seat {
    pub dated_shift_id: String,
    pub slot_id: String,
    pub position_id: String,
    pub date: NaiveDate,
    /// The shift template this occurrence came from.
    pub template_id: String,
    pub start: NaiveTime,
    pub end: NaiveTime,
    pub duration_minutes: u32,
    /// Snapshot members whose position matches the seat's position, in roster
    /// member order.
    pub employee_ids: Vec<String>,
}

impl Seat {
    /// Position of `employee_id` in the seat's eligible list.
    pub fn employee_index(&self, employee_id: &str) -> Option<usize> {
        self.employee_ids
            .iter()
            .position(|existing| existing == employee_id)
    }
}

/// The variables built from one dated roster.
pub(crate) struct Model {
    pub seats: Vec<Seat>,
    /// Seat decision variables: `0` = empty, `e + 1` = `seats[i].employee_ids[e]`.
    pub vars: Vec<DomainId>,
    /// Presence literals, parallel to `seats[i].employee_ids`.
    pub works: Vec<Vec<Literal>>,
    /// Reified `vars[i] == 0`.
    pub empty: Vec<Literal>,
    /// Every snapshot member of the roster, in roster order, whatever their
    /// position. Fairness compares against this pool.
    pub employees: Vec<String>,
    seat_index: HashMap<(String, String), usize>,
    employee_index: HashMap<String, usize>,
}

impl Model {
    pub fn len(&self) -> usize {
        self.seats.len()
    }

    /// One seat, for constraints that need its dates and times.
    pub fn seat(&self, index: usize) -> &Seat {
        &self.seats[index]
    }

    /// The seat decision variable: `0` = empty, `e + 1` = eligible employee `e`.
    pub fn var(&self, index: usize) -> DomainId {
        self.vars[index]
    }

    /// Presence literals, one per eligible employee of the seat.
    pub fn works(&self, index: usize) -> &[Literal] {
        &self.works[index]
    }

    /// Seat index for a plan slot, if the model contains it.
    pub fn index_of(&self, dated_shift_id: &str, slot_id: &str) -> Option<usize> {
        self.seat_index
            .get(&(dated_shift_id.to_owned(), slot_id.to_owned()))
            .copied()
    }

    /// Presence literal for an employee in a seat.
    pub fn work(&self, seat: usize, employee_id: &str) -> Option<Literal> {
        let employee = self.seats[seat].employee_index(employee_id)?;
        Some(self.works[seat][employee])
    }

    /// Position of an employee in the roster-wide employee list.
    pub fn employee_index(&self, employee_id: &str) -> Option<usize> {
        self.employee_index.get(employee_id).copied()
    }
}

/// Build the seat variables for one dated roster.
///
/// Pre-filled slots are fixed to their employee. A slot filled by someone who
/// is not an eligible snapshot member is reported as inconsistent input instead
/// of being silently dropped, because that means the stored roster and the
/// template disagree.
pub(crate) fn build(
    solver: &mut Solver,
    _organization: &Organization,
    _template: &RosterTemplate,
    roster: &Roster,
) -> Result<Model, SolveError> {
    let roster_members: Vec<(String, String)> = roster
        .members
        .iter()
        .map(|member| (member.employee_id.clone(), member.position_id.clone()))
        .collect();

    let employees: Vec<String> = roster_members
        .iter()
        .map(|(employee_id, _)| employee_id.clone())
        .collect();
    let employee_index: HashMap<String, usize> = employees
        .iter()
        .enumerate()
        .map(|(index, employee_id)| (employee_id.clone(), index))
        .collect();

    // One tag for the whole seat definition: these constraints only fix what a
    // seat variable means, so they belong to a single logical group.
    let seat_tag = solver.new_constraint_tag();

    let mut seats = Vec::new();
    let mut vars = Vec::new();
    let mut works = Vec::new();
    let mut empty = Vec::new();
    let mut seat_index = HashMap::new();

    for dated in &roster.shifts {
        for slot in &dated.slots {
            let employee_ids: Vec<String> = roster_members
                .iter()
                .filter(|(_, position_id)| position_id == &slot.position_id)
                .map(|(employee_id, _)| employee_id.clone())
                .collect();

            let var = solver.new_bounded_integer(0, employee_ids.len() as i32);
            let mut seat_works = Vec::with_capacity(employee_ids.len());
            for index in 0..employee_ids.len() {
                let literal = solver.new_literal();
                solver
                    .add_constraint(equals([var.scaled(1)], (index + 1) as i32, seat_tag))
                    .reify(literal);
                seat_works.push(literal);
            }

            let empty_literal = solver.new_literal();
            solver
                .add_constraint(equals([var.scaled(1)], 0, seat_tag))
                .reify(empty_literal);

            if let Some(employee_id) = &slot.employee_id {
                let eligible = employee_ids
                    .iter()
                    .position(|candidate| candidate == employee_id);
                match eligible {
                    Some(index) => {
                        solver
                            .add_constraint(equals([var.scaled(1)], (index + 1) as i32, seat_tag))
                            .post();
                    }
                    None => {
                        return Err(SolveError::InconsistentInput {
                            reasons: vec![format!(
                                "seat {}:{} of {} is filled by employee {employee_id:?}, who is not eligible for position {:?}",
                                dated.id, slot.id, dated.name, slot.position_id
                            )],
                        });
                    }
                }
            }

            seat_index.insert((dated.id.clone(), slot.id.clone()), seats.len());
            seats.push(Seat {
                dated_shift_id: dated.id.clone(),
                slot_id: slot.id.clone(),
                position_id: slot.position_id.clone(),
                date: dated.date,
                template_id: dated.template_id.clone(),
                start: dated.start,
                end: dated.end,
                duration_minutes: dated.duration_minutes(),
                employee_ids,
            });
            vars.push(var);
            works.push(seat_works);
            empty.push(empty_literal);
        }
    }

    Ok(Model {
        seats,
        vars,
        works,
        empty,
        employees,
        seat_index,
        employee_index,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use chrono::NaiveTime;
    use pumpkin_solver::conflict_resolvers::resolvers::ResolutionResolver;
    use pumpkin_solver::core::results::{ProblemSolution, SatisfactionResult};
    use pumpkin_solver::core::termination::Indefinite;

    use crate::domain::{Organization, PlanStatus};

    fn at(hour: u32, minute: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(hour, minute, 0).unwrap()
    }

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).unwrap()
    }

    struct Fixture {
        organization: Organization,
        template_id: String,
        roster_id: String,
        doctors: Vec<String>,
        nurse: String,
    }

    /// One OPD shift needing two Medical Officers, one dated shift, two seats.
    fn fixture() -> Fixture {
        let mut organization = Organization::new("City Hospital");
        let medical_officer = organization
            .add_position("Medical Officer")
            .unwrap()
            .id
            .clone();
        let staff_nurse = organization
            .add_position("Senior Staff Nurse")
            .unwrap()
            .id
            .clone();
        let template_id = organization
            .add_roster_template("Doctors")
            .unwrap()
            .id
            .clone();
        organization
            .include_position(&template_id, &medical_officer)
            .unwrap();
        organization
            .include_position(&template_id, &staff_nurse)
            .unwrap();
        let opd = organization
            .add_shift(&template_id, "OPD", at(8, 0), at(14, 0))
            .unwrap()
            .id
            .clone();
        organization
            .require_position(&template_id, &opd, &medical_officer, 2)
            .unwrap();

        let doctors = ["Dr. Amit", "Dr. Khushi"]
            .iter()
            .map(|name| {
                organization
                    .add_employee(name, &medical_officer)
                    .unwrap()
                    .id
                    .clone()
            })
            .collect();
        let nurse = organization
            .add_employee("Nurse Rina", &staff_nurse)
            .unwrap()
            .id
            .clone();

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

        Fixture {
            organization,
            template_id,
            roster_id,
            doctors,
            nurse,
        }
    }

    fn build_fixture(solver: &mut Solver, fixture: &Fixture) -> Model {
        let template = fixture
            .organization
            .roster_template(&fixture.template_id)
            .expect("template");
        let roster = template.roster(&fixture.roster_id).expect("roster");
        build(solver, &fixture.organization, template, roster).expect("model")
    }

    fn solve_all(solver: &mut Solver, model: &Model) -> Vec<i32> {
        let mut brancher = solver.default_brancher();
        let mut termination = Indefinite;
        let mut resolver = ResolutionResolver::default();

        match solver.satisfy(&mut brancher, &mut termination, &mut resolver) {
            SatisfactionResult::Satisfiable(satisfiable) => {
                let solution = satisfiable.solution();
                model
                    .vars
                    .iter()
                    .map(|var| solution.get_integer_value(*var))
                    .collect()
            }
            other => panic!("expected a solution, got {other:?}"),
        }
    }

    #[test]
    fn builds_one_variable_per_seat_in_roster_order() {
        let fixture = fixture();
        let mut solver = Solver::default();
        let model = build_fixture(&mut solver, &fixture);

        assert_eq!(model.len(), 2);
        assert_eq!(model.vars.len(), 2);
        assert_eq!(model.works.len(), 2);
        assert_eq!(model.empty.len(), 2);
        assert_eq!(model.employees.len(), 3);
        assert_eq!(model.employees, {
            let mut expected = fixture.doctors.clone();
            expected.push(fixture.nurse.clone());
            expected
        });

        let template = fixture
            .organization
            .roster_template(&fixture.template_id)
            .unwrap();
        let roster = template.roster(&fixture.roster_id).unwrap();
        let dated = &roster.shifts[0];
        assert_eq!(dated.slots.len(), 2);
        for (index, slot) in dated.slots.iter().enumerate() {
            assert_eq!(
                model.index_of(&dated.id, &slot.id),
                Some(index),
                "seats follow the roster's slot order"
            );
        }

        for seat in &model.seats {
            assert_eq!(seat.date, date(2026, 11, 16));
            assert_eq!(seat.duration_minutes, 360);
            assert_eq!(seat.employee_ids, fixture.doctors);
        }
    }

    #[test]
    fn excludes_employees_with_other_positions() {
        let fixture = fixture();
        let mut solver = Solver::default();
        let model = build_fixture(&mut solver, &fixture);

        assert_eq!(model.seats[0].employee_index(&fixture.nurse), None);
        assert!(model.work(0, &fixture.nurse).is_none());
        assert!(model.work(0, &fixture.doctors[0]).is_some());

        let values = solve_all(&mut solver, &model);
        for value in values {
            assert!(
                (0..=2).contains(&value),
                "a seat variable only names eligible doctors or emptiness"
            );
        }
    }

    #[test]
    fn fixes_prefilled_seats() {
        let mut fixture = fixture();
        let doctor = fixture.doctors[0].clone();
        let (dated_id, slot_id) = {
            let roster = fixture
                .organization
                .roster_template(&fixture.template_id)
                .unwrap()
                .roster(&fixture.roster_id)
                .unwrap();
            (
                roster.shifts[0].id.clone(),
                roster.shifts[0].slots[0].id.clone(),
            )
        };
        fixture
            .organization
            .assign_slot(
                &fixture.template_id,
                &fixture.roster_id,
                &dated_id,
                &slot_id,
                &doctor,
            )
            .unwrap();

        let mut solver = Solver::default();
        let model = build_fixture(&mut solver, &fixture);
        let values = solve_all(&mut solver, &model);

        let seat = model.index_of(&dated_id, &slot_id).expect("seat");
        assert_eq!(values[seat], 1, "Dr. Amit is the first eligible doctor");
        assert_ne!(
            values[1 - seat],
            1,
            "the other seat cannot also hold Dr. Amit"
        );
    }

    #[test]
    fn seat_without_eligible_employees_is_fixed_to_empty() {
        let mut fixture = fixture();
        let cleaner = fixture
            .organization
            .add_position("Cleaner")
            .unwrap()
            .id
            .clone();
        fixture
            .organization
            .include_position(&fixture.template_id, &cleaner)
            .unwrap();
        let night = fixture
            .organization
            .add_shift(&fixture.template_id, "Night Cleaning", at(22, 0), at(23, 0))
            .unwrap()
            .id
            .clone();
        fixture
            .organization
            .require_position(&fixture.template_id, &night, &cleaner, 1)
            .unwrap();
        fixture.roster_id = fixture
            .organization
            .create_roster(
                &fixture.template_id,
                date(2026, 11, 16),
                date(2026, 11, 16),
                Vec::new(),
            )
            .unwrap()
            .id
            .clone();

        let mut solver = Solver::default();
        let model = build_fixture(&mut solver, &fixture);
        assert_eq!(model.len(), 3);

        let cleaner_seat = model
            .seats
            .iter()
            .position(|seat| seat.position_id == cleaner)
            .expect("cleaner seat");
        assert!(model.seats[cleaner_seat].employee_ids.is_empty());
        assert!(model.works[cleaner_seat].is_empty());

        let values = solve_all(&mut solver, &model);
        assert_eq!(values[cleaner_seat], 0);
    }

    #[test]
    fn roster_status_is_still_draft_after_building() {
        let fixture = fixture();
        let mut solver = Solver::default();
        let _model = build_fixture(&mut solver, &fixture);

        let roster = fixture
            .organization
            .roster_template(&fixture.template_id)
            .unwrap()
            .roster(&fixture.roster_id)
            .unwrap();
        assert_eq!(roster.status, PlanStatus::Draft);
    }
}

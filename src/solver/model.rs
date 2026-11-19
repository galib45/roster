//! The CP model's seat variables and presence literals.
//!
//! One [`ShiftSlot`](crate::domain::ShiftSlot) becomes one integer variable:
//! `0` is an empty seat and `e + 1` is the employee at index `e` of the seat's
//! eligible-employee list. Every rule then reads "is employee `e` in seat `i`?"
//! from a reified literal, which keeps the constraint layer free of index
//! arithmetic.

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

    // One tag for the whole seat definition: these constraints only fix what a
    // seat variable means, so they belong to a single logical group.
    let seat_tag = solver.new_constraint_tag();

    let mut seats = Vec::new();
    let mut vars = Vec::new();
    let mut works = Vec::new();
    let mut empty = Vec::new();

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
    })
}

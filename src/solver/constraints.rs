//! Hard constraints encoded from the evaluator's rules.
//!
//! Every rule here mirrors one check in [`crate::eval`]. The matching helpers
//! (`interval`, `intervals_overlap`, `rule_matches`) are imported from the
//! evaluator instead of being re-implemented, so the solver and the score can
//! never disagree about when a rule applies.
//!
//! Only hard rules are posted. Soft rules are objective terms and belong to
//! 04-02; coverage is never a constraint, so an empty seat is always legal.

use pumpkin_solver::core::proof::ConstraintTag;
use pumpkin_solver::core::variables::Literal;
use pumpkin_solver::{Solver, less_than_or_equals};

use crate::domain::{Organization, Roster, RosterTemplate, Severity};
use crate::eval::{interval, intervals_overlap, rule_matches};

use super::model::Model;

/// Post every hard rule of the dated roster on top of an already-built [`Model`].
pub(crate) fn post_hard(
    solver: &mut Solver,
    organization: &Organization,
    template: &RosterTemplate,
    roster: &Roster,
    model: &Model,
) {
    // One tag per rule family. Proof tags group constraints for explanation;
    // the solver never needs finer granularity than "this constraint came from
    // the overlap rules", and one tag per constraint would only bloat the
    // proof.
    let overlap_tag = solver.new_constraint_tag();
    let availability_tag = solver.new_constraint_tag();
    let rest_tag = solver.new_constraint_tag();

    post_overlap(solver, overlap_tag, roster, model);
    post_unavailability(solver, availability_tag, organization, model);
    post_rest(solver, rest_tag, template, roster, model);
}

/// Forbid an employee from holding two overlapping seats.
fn post_overlap(solver: &mut Solver, tag: ConstraintTag, roster: &Roster, model: &Model) {
    for first in 0..model.len() {
        let first_seat = model.seat(first);
        let first_interval = interval(first_seat.date, first_seat.start, first_seat.end);

        for second in (first + 1)..model.len() {
            let second_seat = model.seat(second);
            let second_interval = interval(second_seat.date, second_seat.start, second_seat.end);
            // No date filter: a night shift can overlap the next day's early
            // shift, and the evaluator treats that as one overlap.
            if !intervals_overlap(first_interval, second_interval) {
                continue;
            }

            for (first_employee, employee_id) in first_seat.employee_ids.iter().enumerate() {
                let Some(second_employee) = second_seat.employee_index(employee_id) else {
                    continue;
                };
                solver
                    .add_constraint(less_than_or_equals(
                        [
                            model.works(first)[first_employee],
                            model.works(second)[second_employee],
                        ],
                        1,
                        tag,
                    ))
                    .post();
            }
        }
    }

    // A context assignment is fixed input, so it only forbids seats; the
    // context-vs-context case is checked by `evaluate` before solving.
    for context in &roster.context {
        let context_interval = interval(context.date, context.start, context.end);
        for seat_index in 0..model.len() {
            let seat = model.seat(seat_index);
            let seat_interval = interval(seat.date, seat.start, seat.end);
            if !intervals_overlap(context_interval, seat_interval) {
                continue;
            }
            if let Some(employee) = seat.employee_index(&context.employee_id) {
                forbid(solver, tag, model.works(seat_index)[employee]);
            }
        }
    }
}

/// Forbid a seat to every employee a hard unavailability rule covers.
fn post_unavailability(
    solver: &mut Solver,
    tag: ConstraintTag,
    organization: &Organization,
    model: &Model,
) {
    for seat_index in 0..model.len() {
        let seat = model.seat(seat_index);
        for (employee, employee_id) in seat.employee_ids.iter().enumerate() {
            let Some(record) = organization.employee(employee_id) else {
                continue;
            };
            let covered = record.unavailability.iter().any(|rule| {
                rule.severity == Severity::Hard
                    && rule_matches(rule, seat.date, Some(&seat.template_id))
            });
            if covered {
                forbid(solver, tag, model.works(seat_index)[employee]);
            }
        }
    }
}

/// Forbid a seat that would leave too little rest after an earlier shift.
fn post_rest(
    solver: &mut Solver,
    tag: ConstraintTag,
    template: &RosterTemplate,
    roster: &Roster,
    model: &Model,
) {
    for rule in &template.rest_rules {
        if rule.severity != Severity::Hard {
            continue;
        }

        for earlier_index in 0..model.len() {
            let earlier = model.seat(earlier_index);
            if earlier.template_id != rule.preceding_shift_id {
                continue;
            }
            let (earlier_start, earlier_end) = interval(earlier.date, earlier.start, earlier.end);

            for later_index in 0..model.len() {
                let later = model.seat(later_index);
                if later.template_id != rule.following_shift_id {
                    continue;
                }
                let (later_start, _) = interval(later.date, later.start, later.end);
                if !breaks_rest(
                    earlier_start,
                    earlier_end,
                    later_start,
                    rule.min_rest_minutes,
                ) {
                    continue;
                }

                for (earlier_employee, employee_id) in earlier.employee_ids.iter().enumerate() {
                    let Some(later_employee) = later.employee_index(employee_id) else {
                        continue;
                    };
                    solver
                        .add_constraint(less_than_or_equals(
                            [
                                model.works(earlier_index)[earlier_employee],
                                model.works(later_index)[later_employee],
                            ],
                            1,
                            tag,
                        ))
                        .post();
                }
            }
        }

        // Context has no decision variable, so a context shift can only ban
        // the seat on the other side of the rule.
        for context in &roster.context {
            let Some(template_id) = context.template_id.as_deref() else {
                continue;
            };
            let (context_start, context_end) = interval(context.date, context.start, context.end);

            if template_id == rule.preceding_shift_id {
                for seat_index in 0..model.len() {
                    let seat = model.seat(seat_index);
                    if seat.template_id != rule.following_shift_id {
                        continue;
                    }
                    let (seat_start, _) = interval(seat.date, seat.start, seat.end);
                    if !breaks_rest(
                        context_start,
                        context_end,
                        seat_start,
                        rule.min_rest_minutes,
                    ) {
                        continue;
                    }
                    if let Some(employee) = seat.employee_index(&context.employee_id) {
                        forbid(solver, tag, model.works(seat_index)[employee]);
                    }
                }
            } else if template_id == rule.following_shift_id {
                for seat_index in 0..model.len() {
                    let seat = model.seat(seat_index);
                    if seat.template_id != rule.preceding_shift_id {
                        continue;
                    }
                    let (seat_start, seat_end) = interval(seat.date, seat.start, seat.end);
                    if !breaks_rest(seat_start, seat_end, context_start, rule.min_rest_minutes) {
                        continue;
                    }
                    if let Some(employee) = seat.employee_index(&context.employee_id) {
                        forbid(solver, tag, model.works(seat_index)[employee]);
                    }
                }
            }
        }
    }
}

/// The evaluator's own rest test: the later shift must start after the earlier
/// one and leave `0 <= gap < min_rest` minutes between them.
pub(crate) fn breaks_rest(
    earlier_start: chrono::NaiveDateTime,
    earlier_end: chrono::NaiveDateTime,
    later_start: chrono::NaiveDateTime,
    min_rest_minutes: u32,
) -> bool {
    if earlier_start >= later_start {
        return false;
    }
    let gap = (later_start - earlier_end).num_minutes();
    (0..i64::from(min_rest_minutes)).contains(&gap)
}

/// Post `literal <= 0`.
fn forbid(solver: &mut Solver, tag: ConstraintTag, literal: Literal) {
    solver
        .add_constraint(less_than_or_equals([literal], 0, tag))
        .post();
}

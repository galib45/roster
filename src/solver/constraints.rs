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

#[cfg(test)]
mod tests {
    use super::*;

    use chrono::{NaiveDate, NaiveTime};
    use pumpkin_solver::conflict_resolvers::resolvers::ResolutionResolver;
    use pumpkin_solver::core::results::{ProblemSolution, SatisfactionResult};
    use pumpkin_solver::core::termination::Indefinite;

    use crate::domain::{ContextAssignment, UnavailabilityScope};

    use super::super::model::{Model, build};

    fn at(hour: u32, minute: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(hour, minute, 0).unwrap()
    }

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).unwrap()
    }

    struct Fixture {
        organization: Organization,
        roster_id: String,
        position_id: String,
    }

    impl Fixture {
        fn new() -> Self {
            let mut organization = Organization::new("City Hospital");
            let position_id = organization
                .add_position("Medical Officer")
                .unwrap()
                .id
                .clone();
            // let roster_id = organization.add_roster("Doctors").unwrap().id.clone();
            let roster_id = String::new();
            Self {
                organization,
                roster_id,
                position_id,
            }
        }

        fn employee(&mut self, name: &str) -> String {
            self.organization
                .add_employee(name, &self.position_id)
                .unwrap()
                .id
                .clone()
        }

        fn shift(&mut self, name: &str, start: NaiveTime, end: NaiveTime) -> String {
            let shift_id = self
                .organization
                .add_shift(&self.roster_id, name, start, end)
                .unwrap()
                .id
                .clone();
            self.organization
                .require_position(&self.roster_id, &shift_id, &self.position_id, 1)
                .unwrap();
            shift_id
        }

        fn plan(
            &mut self,
            start: NaiveDate,
            end: NaiveDate,
            context: Vec<ContextAssignment>,
        ) -> String {
            self.organization
                .generate_plan(&self.roster_id, start, end, context)
                .unwrap()
                .id
                .clone()
        }

        fn dated_shift(&self, plan_id: &str, template_id: &str, day: NaiveDate) -> String {
            self.organization
                .plan(&self.roster_id, plan_id)
                .unwrap()
                .shifts
                .iter()
                .find(|shift| shift.template_id == template_id && shift.date == day)
                .unwrap()
                .id
                .clone()
        }

        fn assign(&mut self, plan_id: &str, dated_id: &str, employee_id: &str) {
            let slot_id = self
                .organization
                .plan(&self.roster_id, plan_id)
                .unwrap()
                .dated_shift(dated_id)
                .unwrap()
                .slots[0]
                .id
                .clone();
            self.organization
                .assign_slot(&self.roster_id, plan_id, dated_id, &slot_id, employee_id)
                .unwrap();
        }

        fn solve(&self, plan_id: &str) -> Option<Vec<i32>> {
            let mut solver = Solver::default();
            let roster = self.organization.roster(&self.roster_id).unwrap();
            let plan = roster.plan(plan_id).unwrap();
            let model = build(&mut solver, &self.organization, roster, plan).unwrap();
            post_hard(&mut solver, &self.organization, roster, plan, &model);
            solve_model(&mut solver, &model)
        }
    }

    fn solve_model(solver: &mut Solver, model: &Model) -> Option<Vec<i32>> {
        let mut brancher = solver.default_brancher();
        let mut termination = Indefinite;
        let mut resolver = ResolutionResolver::default();

        match solver.satisfy(&mut brancher, &mut termination, &mut resolver) {
            SatisfactionResult::Satisfiable(satisfiable) => {
                let solution = satisfiable.solution();
                Some(
                    model
                        .vars
                        .iter()
                        .map(|var| solution.get_integer_value(*var))
                        .collect(),
                )
            }
            SatisfactionResult::Unsatisfiable(..) | SatisfactionResult::Unknown(..) => None,
        }
    }

    #[test]
    fn overlapping_seats_cannot_share_an_employee() {
        let mut fixture = Fixture::new();
        let doctor = fixture.employee("Dr. Amit");
        let opd = fixture.shift("OPD", at(8, 0), at(14, 0));
        let emergency = fixture.shift("Emergency", at(9, 0), at(11, 0));
        let plan = fixture.plan(date(2026, 11, 16), date(2026, 11, 16), Vec::new());

        let opd_shift = fixture.dated_shift(&plan, &opd, date(2026, 11, 16));
        let emergency_shift = fixture.dated_shift(&plan, &emergency, date(2026, 11, 16));
        fixture.assign(&plan, &opd_shift, &doctor);
        fixture.assign(&plan, &emergency_shift, &doctor);

        assert_eq!(fixture.solve(&plan), None);
    }

    #[test]
    fn overlapping_seats_accept_different_employees() {
        let mut fixture = Fixture::new();
        let first = fixture.employee("Dr. Amit");
        let second = fixture.employee("Dr. Khushi");
        let opd = fixture.shift("OPD", at(8, 0), at(14, 0));
        let emergency = fixture.shift("Emergency", at(9, 0), at(11, 0));
        let plan = fixture.plan(date(2026, 11, 16), date(2026, 11, 16), Vec::new());

        let opd_shift = fixture.dated_shift(&plan, &opd, date(2026, 11, 16));
        let emergency_shift = fixture.dated_shift(&plan, &emergency, date(2026, 11, 16));
        fixture.assign(&plan, &opd_shift, &first);
        fixture.assign(&plan, &emergency_shift, &second);

        assert!(fixture.solve(&plan).is_some());
    }

    #[test]
    fn hard_leave_blocks_a_prefilled_seat() {
        let mut fixture = Fixture::new();
        let doctor = fixture.employee("Dr. Amit");
        fixture
            .organization
            .add_unavailability(
                &doctor,
                UnavailabilityScope::Leave {
                    start: date(2026, 11, 16),
                    end: date(2026, 11, 16),
                },
                &[],
                Severity::Hard,
                0,
            )
            .unwrap();
        let opd = fixture.shift("OPD", at(8, 0), at(14, 0));
        let plan = fixture.plan(date(2026, 11, 16), date(2026, 11, 16), Vec::new());
        let opd_shift = fixture.dated_shift(&plan, &opd, date(2026, 11, 16));
        fixture.assign(&plan, &opd_shift, &doctor);

        assert_eq!(fixture.solve(&plan), None);
    }

    #[test]
    fn hard_leave_leaves_the_seat_empty_when_no_one_else_is_eligible() {
        let mut fixture = Fixture::new();
        let doctor = fixture.employee("Dr. Amit");
        fixture
            .organization
            .add_unavailability(
                &doctor,
                UnavailabilityScope::Leave {
                    start: date(2026, 11, 16),
                    end: date(2026, 11, 16),
                },
                &[],
                Severity::Hard,
                0,
            )
            .unwrap();
        fixture.shift("OPD", at(8, 0), at(14, 0));
        let plan = fixture.plan(date(2026, 11, 16), date(2026, 11, 16), Vec::new());

        let values = fixture.solve(&plan).expect("empty seat is always legal");
        assert_eq!(values, vec![0]);
    }

    #[test]
    fn hard_rest_forbids_the_shift_that_follows_too_closely() {
        let mut fixture = Fixture::new();
        let doctor = fixture.employee("Dr. Amit");
        let late = fixture.shift("Late", at(18, 0), at(22, 0));
        let early = fixture.shift("Early", at(6, 0), at(12, 0));
        fixture
            .organization
            .add_rest_rule(
                &fixture.roster_id.clone(),
                &late,
                &early,
                600,
                Severity::Hard,
                0,
            )
            .unwrap();
        let plan = fixture.plan(date(2026, 11, 16), date(2026, 11, 17), Vec::new());

        let late_shift = fixture.dated_shift(&plan, &late, date(2026, 11, 16));
        let early_shift = fixture.dated_shift(&plan, &early, date(2026, 11, 17));
        fixture.assign(&plan, &late_shift, &doctor);
        fixture.assign(&plan, &early_shift, &doctor);

        assert_eq!(fixture.solve(&plan), None);
    }

    #[test]
    fn hard_rest_only_bites_when_both_shifts_are_held() {
        let mut fixture = Fixture::new();
        let doctor = fixture.employee("Dr. Amit");
        let late = fixture.shift("Late", at(18, 0), at(22, 0));
        let early = fixture.shift("Early", at(6, 0), at(12, 0));
        fixture
            .organization
            .add_rest_rule(
                &fixture.roster_id.clone(),
                &late,
                &early,
                600,
                Severity::Hard,
                0,
            )
            .unwrap();
        let plan = fixture.plan(date(2026, 11, 16), date(2026, 11, 17), Vec::new());

        let early_shift = fixture.dated_shift(&plan, &early, date(2026, 11, 17));
        fixture.assign(&plan, &early_shift, &doctor);

        assert!(fixture.solve(&plan).is_some());
    }

    #[test]
    fn context_rest_blocks_the_following_seat() {
        let mut fixture = Fixture::new();
        let doctor = fixture.employee("Dr. Amit");
        let late = fixture.shift("Late", at(18, 0), at(22, 0));
        let early = fixture.shift("Early", at(6, 0), at(12, 0));
        fixture
            .organization
            .add_rest_rule(
                &fixture.roster_id.clone(),
                &late,
                &early,
                600,
                Severity::Hard,
                0,
            )
            .unwrap();

        let context = vec![ContextAssignment {
            date: date(2026, 11, 15),
            name: "Late".to_owned(),
            start: at(18, 0),
            end: at(22, 0),
            employee_id: doctor.clone(),
            template_id: Some(late),
        }];
        let plan = fixture.plan(date(2026, 11, 16), date(2026, 11, 16), context);

        let values = fixture.solve(&plan).expect("early seat can stay empty");
        // Plan order is roster order: Late (18:00–22:00) then Early (06:00–12:00).
        // Late is free; Early is blocked for the only doctor by the context shift.
        assert_eq!(values[1], 0, "context rest blocks the Early seat");
    }
}

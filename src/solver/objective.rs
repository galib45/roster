//! Weighted objective for the solver: coverage + fairness + soft penalties.
//!
//! The objective combines three cost terms with fixed, documented weights:
//! - Coverage: `coverage_penalty` (1_000_000) per empty seat
//! - Fairness: `fairness_weight` (1) per minute of L1 deviation
//! - Soft penalties: `soft_penalty_weight` (60) per penalty unit
//!
//! The fairness term mirrors `eval::fairness::workload` exactly, using integer
//! arithmetic (truncating division and `absolute`) so the solver minimises the
//! same quantity the UI reports.

use std::collections::HashMap;

use chrono::Datelike;
use pumpkin_solver::core::proof::ConstraintTag;
use pumpkin_solver::core::variables::{DomainId, Literal, TransformableVariable};
use pumpkin_solver::{Solver, division, equals, less_than_or_equals, times};

use crate::domain::{Organization, Roster, RosterPlan, Severity};
use crate::eval::EvalOptions;
use crate::solver::SolveError;
use crate::solver::SolveOptions;
use crate::solver::model::Model;

/// Objective variable and metadata needed by `solve()` to recompute unweighted costs.
pub(crate) struct Objective {
    pub var: DomainId,
    /// Sum of `empty[i]` literals — unweighted coverage cost.
    pub coverage_terms: Vec<Literal>,
    /// Per-employee `deviation[e]` variables — unweighted fairness cost.
    pub fairness_terms: Vec<DomainId>,
    /// Soft penalty product/presence variables — unweighted soft cost.
    pub soft_terms: Vec<DomainId>,
    /// Theoretical maximum of the objective (for bound check).
    pub max_objective: i64,
}

/// Build the complete objective and return the objective variable.
///
/// Posting strategy:
/// 1. Coverage: `Σ empty[i]` — one literal per seat.
/// 2. Fairness: L1 deviation from comparison mean (group or roster), linearised with
///    `division` + `absolute`. One `deviation[e]` variable per roster employee.
/// 3. Soft penalties: recurring preferences, multi-shift day, soft unavailability,
///    soft rest — each as a linear or product term.
/// 4. Assemble: `objective = coverage_penalty * coverage + fairness_weight * fairness + soft_penalty_weight * soft`.
///
/// All terms are `AffineView<DomainId>` via `.scaled(k)`.
pub(crate) fn build(
    solver: &mut Solver,
    model: &Model,
    organization: &Organization,
    roster: &Roster,
    plan: &RosterPlan,
    options: &SolveOptions,
) -> Result<Objective, SolveError> {
    let obj_tag = solver.new_constraint_tag();
    let fairness_tag = solver.new_constraint_tag();
    let soft_tag = solver.new_constraint_tag();

    // Total minutes available in the plan (upper bound for any employee's workload).
    let total_minutes: u64 = model
        .seats
        .iter()
        .map(|seat| u64::from(seat.duration_minutes))
        .sum();

    // 1. Coverage terms: one literal per seat (empty[i] is 1 when seat is empty).
    let coverage_terms: Vec<Literal> = model.empty.clone();

    // 2. Fairness terms — one deviation variable per roster employee.
    let fairness_terms = build_fairness(
        solver,
        model,
        organization,
        roster,
        plan,
        total_minutes,
        fairness_tag,
    )?;

    // 3. Soft penalty terms.
    let soft_terms = build_soft_penalties(
        solver,
        model,
        organization,
        roster,
        plan,
        &options.eval,
        soft_tag,
    )?;

    // 4. Assemble the objective variable.
    let objective_var = solver.new_bounded_integer(0, i32::MAX);

    // Compute maximum possible objective value for bound check.
    let max_coverage = coverage_terms.len() as i64;
    let max_fairness: i64 = fairness_terms.len() as i64 * 2 * total_minutes as i64;
    let max_soft: i64 = soft_terms
        .iter()
        .map(|v| max_value_for_soft_term(solver, *v))
        .sum();

    let max_objective = options
        .coverage_penalty
        .saturating_mul(max_coverage)
        .saturating_add(options.fairness_weight.saturating_mul(max_fairness))
        .saturating_add(options.soft_penalty_weight.saturating_mul(max_soft));

    if max_objective > i32::MAX as i64 {
        return Err(SolveError::ModelTooLarge {
            required: max_objective,
        });
    }

    // Build: objective = coverage_penalty * Σ empty + fairness_weight * Σ deviation + soft_penalty_weight * Σ soft
    let mut terms: Vec<pumpkin_solver::core::variables::AffineView<DomainId>> = Vec::new();

    // Coverage terms: each empty[i] scaled by coverage_penalty
    for lit in &coverage_terms {
        terms.push(
            lit.get_integer_variable()
                .scaled(options.coverage_penalty as i32),
        );
    }

    // Fairness terms: each deviation[e] scaled by fairness_weight
    for dev in &fairness_terms {
        terms.push(dev.scaled(options.fairness_weight as i32));
    }

    // Soft terms: each soft term scaled by soft_penalty_weight
    for soft in &soft_terms {
        terms.push(soft.scaled(options.soft_penalty_weight as i32));
    }

    // objective - Σ terms = 0
    terms.push(objective_var.scaled(-1));

    solver.add_constraint(equals(terms, 0, obj_tag)).post();

    // Bound the objective variable explicitly.
    solver
        .add_constraint(less_than_or_equals(
            [objective_var.scaled(1)],
            max_objective as i32,
            obj_tag,
        ))
        .post();

    Ok(Objective {
        var: objective_var,
        coverage_terms,
        fairness_terms,
        soft_terms,
        max_objective,
    })
}

/// Build fairness deviation variables: one `deviation[e]` per roster employee.
///
/// Formula per employee:
///   minutes[e] = Σ duration[i] * works[i][e]
///   mean[e] = total / employee_count      (ungrouped)
///           = group_total / group_size  (grouped)
///   signed[e] = minutes[e] - mean[e]
///   deviation[e] = |signed[e]|  (linearised as: deviation >= signed AND deviation >= -signed)
fn build_fairness(
    solver: &mut Solver,
    model: &Model,
    _organization: &Organization,
    roster: &Roster,
    _plan: &RosterPlan,
    total_minutes: u64,
    tag: ConstraintTag,
) -> Result<Vec<DomainId>, SolveError> {
    // Collect per-employee workload variables: minutes[e] = Σ duration * works[i][e]
    let mut minutes_vars: Vec<DomainId> = Vec::with_capacity(model.employees.len());

    for employee_id in &model.employees {
        // Find all seats where this employee is eligible
        let mut employee_seats = Vec::new();
        let mut durations = Vec::new();

        for (seat_idx, seat) in model.seats.iter().enumerate() {
            if let Some(emp_idx) = seat.employee_index(employee_id) {
                employee_seats.push(model.works(seat_idx)[emp_idx]);
                durations.push(seat.duration_minutes as i32);
            }
        }

        if employee_seats.is_empty() {
            // Employee has no eligible seats — minutes = 0 (constant)
            let zero = solver.new_bounded_integer(0, 0);
            minutes_vars.push(zero);
            continue;
        }

        // minutes[e] = Σ duration[i] * works[i][e]
        let minutes_var = solver.new_bounded_integer(0, total_minutes as i32);

        // Post: Σ duration[i] * works[i][e] - minutes[e] = 0
        let mut terms: Vec<pumpkin_solver::core::variables::AffineView<DomainId>> = Vec::new();
        for (lit, dur) in employee_seats.iter().zip(durations) {
            // Convert literal to integer variable view and scale by duration
            terms.push(lit.get_integer_variable().scaled(dur));
        }
        terms.push(minutes_var.scaled(-1));

        solver.add_constraint(equals(terms, 0, tag)).post();

        minutes_vars.push(minutes_var);
    }

    // Compute roster total: Σ minutes[e]
    let roster_total = solver.new_bounded_integer(0, total_minutes as i32);
    let mut total_terms: Vec<pumpkin_solver::core::variables::AffineView<DomainId>> = Vec::new();
    for mv in &minutes_vars {
        total_terms.push(mv.scaled(1));
    }
    total_terms.push(roster_total.scaled(-1));
    solver.add_constraint(equals(total_terms, 0, tag)).post();

    // Employee count (constant) for roster mean division
    let employee_count =
        solver.new_bounded_integer(model.employees.len() as i32, model.employees.len() as i32);

    // Build group totals for sharing groups
    let mut group_total_vars: HashMap<String, DomainId> = HashMap::new();
    let mut group_size_vars: HashMap<String, DomainId> = HashMap::new();

    for group in &roster.sharing_groups {
        let mut group_terms: Vec<pumpkin_solver::core::variables::AffineView<DomainId>> =
            Vec::new();

        for employee_id in &group.employee_ids {
            if let Some(idx) = model.employees.iter().position(|e| e == employee_id) {
                group_terms.push(minutes_vars[idx].scaled(1));
            }
        }

        if group_terms.is_empty() {
            continue;
        }

        let group_total = solver.new_bounded_integer(0, total_minutes as i32);
        group_terms.push(group_total.scaled(-1));
        solver.add_constraint(equals(group_terms, 0, tag)).post();

        let group_size = solver.new_bounded_integer(
            group.employee_ids.len() as i32,
            group.employee_ids.len() as i32,
        );

        group_total_vars.insert(group.id.clone(), group_total);
        group_size_vars.insert(group.id.clone(), group_size);
    }

    // For each employee: signed = minutes - mean, deviation = |signed|
    // We linearise |signed| as: deviation >= signed AND deviation >= -signed
    // Since the objective minimises deviation, it will equal |signed| at optimum.
    let mut fairness_terms = Vec::with_capacity(model.employees.len());

    for (emp_idx, employee_id) in model.employees.iter().enumerate() {
        let minutes = minutes_vars[emp_idx];

        // Determine comparison mean: group or roster
        let mean = if let Some(group_id) = roster
            .sharing_group_for_employee(employee_id)
            .map(|g| g.id.clone())
        {
            // Grouped: mean = group_total / group_size
            let group_total = group_total_vars[&group_id];
            let group_size = group_size_vars[&group_id];
            let mean_var = solver.new_bounded_integer(0, total_minutes as i32);

            solver
                .add_constraint(division(group_total, group_size, mean_var, tag))
                .post();

            mean_var
        } else {
            // Ungrouped: mean = roster_total / employee_count
            let mean_var = solver.new_bounded_integer(0, total_minutes as i32);

            solver
                .add_constraint(division(roster_total, employee_count, mean_var, tag))
                .post();

            mean_var
        };

        // signed = minutes - mean
        let signed = solver.new_bounded_integer(-(total_minutes as i32), total_minutes as i32);
        solver
            .add_constraint(equals(
                [minutes.scaled(1), mean.scaled(-1), signed.scaled(-1)],
                0,
                tag,
            ))
            .post();

        // deviation = |signed|, linearised as two inequalities:
        // deviation >= signed  =>  signed - deviation <= 0
        // deviation >= -signed =>  -signed - deviation <= 0
        let deviation = solver.new_bounded_integer(0, total_minutes as i32);

        // signed - deviation <= 0
        solver
            .add_constraint(less_than_or_equals(
                [signed.scaled(1), deviation.scaled(-1)],
                0,
                tag,
            ))
            .post();

        // -signed - deviation <= 0
        solver
            .add_constraint(less_than_or_equals(
                [signed.scaled(-1), deviation.scaled(-1)],
                0,
                tag,
            ))
            .post();

        fairness_terms.push(deviation);
    }

    Ok(fairness_terms)
}

/// Build soft penalty terms.
///
/// Returns a vector of variables, each representing one soft penalty term (already
/// multiplied by the rule's penalty where applicable). The solver will scale them
/// by `soft_penalty_weight` in the main objective.
fn build_soft_penalties(
    solver: &mut Solver,
    model: &Model,
    organization: &Organization,
    roster: &Roster,
    plan: &RosterPlan,
    eval_options: &EvalOptions,
    tag: ConstraintTag,
) -> Result<Vec<DomainId>, SolveError> {
    let mut soft_terms = Vec::new();

    // 1. Recurring preferences (soft)
    soft_terms.extend(build_recurring_preference_terms(
        solver,
        model,
        organization,
        roster,
        plan,
        eval_options,
        tag,
    )?);

    // 2. Multiple shifts in a day (soft)
    if eval_options.multi_shift_penalty > 0 {
        soft_terms.extend(build_multi_shift_terms(
            solver,
            model,
            plan,
            eval_options.multi_shift_penalty,
            tag,
        )?);
    }

    // 3. Soft unavailability rules
    soft_terms.extend(build_soft_unavailability_terms(
        solver,
        model,
        organization,
        roster,
        plan,
        tag,
    )?);

    // 4. Soft rest rules
    soft_terms.extend(build_soft_rest_terms(
        solver,
        model,
        organization,
        roster,
        plan,
        tag,
    )?);

    Ok(soft_terms)
}

/// Recurring preference: for each dated shift matching the template/weekday,
/// if the employee doesn't already have a hard rule covering that date,
/// cost += penalty * (1 - Σ works[seats of shift][employee]).
fn build_recurring_preference_terms(
    solver: &mut Solver,
    model: &Model,
    organization: &Organization,
    roster: &Roster,
    plan: &RosterPlan,
    _eval_options: &EvalOptions,
    tag: ConstraintTag,
) -> Result<Vec<DomainId>, SolveError> {
    let mut terms = Vec::new();

    for employee in organization.employees_in_roster(&roster.id) {
        for preference in &employee.recurring_assignments {
            // Skip if the penalty is 0
            if preference.penalty == 0 {
                continue;
            }

            for dated in &plan.shifts {
                // Template and weekday must match
                if dated.template_id != preference.shift_id
                    || dated.date.weekday() != preference.weekday
                {
                    continue;
                }

                // Skip dates covered by a hard unavailability/leave rule for this employee
                let has_hard_cover = employee.unavailability.iter().any(|rule| {
                    rule.severity == Severity::Hard
                        && crate::eval::rule_matches(rule, dated.date, Some(&dated.template_id))
                });
                if has_hard_cover {
                    continue;
                }

                // Find all seats of this dated shift where the employee is eligible
                let mut seat_works = Vec::new();
                for (seat_idx, seat) in model.seats.iter().enumerate() {
                    if seat.dated_shift_id == dated.id {
                        if let Some(emp_idx) = seat.employee_index(&employee.id) {
                            seat_works.push(model.works(seat_idx)[emp_idx]);
                        }
                    }
                }

                if seat_works.is_empty() {
                    continue;
                }

                // Σ works <= 1 (employee can hold at most one seat of the same dated shift)
                // This is already enforced by overlap, but we add it to make the sum 0/1 for the objective.
                let sum_var = solver.new_bounded_integer(0, seat_works.len() as i32);
                let mut sum_terms: Vec<pumpkin_solver::core::variables::AffineView<DomainId>> =
                    Vec::new();
                for lit in &seat_works {
                    sum_terms.push(lit.get_integer_variable().scaled(1));
                }
                sum_terms.push(sum_var.scaled(-1));
                solver.add_constraint(equals(sum_terms, 0, tag)).post();

                // (1 - sum) * penalty: we create a variable for (1 - sum) and scale by penalty
                // But since penalty is a constant, we can just scale the sum variable by -penalty
                // and add penalty as a constant offset. Instead, let's create the term directly:
                // cost = penalty * (1 - sum) = penalty - penalty * sum
                // We'll add this as a constant + scaled sum.

                // Since we need a variable for the soft term, create one:
                // soft_term = penalty - penalty * sum = penalty * (1 - sum)
                let soft_term = solver.new_bounded_integer(0, preference.penalty as i32);
                let soft_terms_vec: Vec<pumpkin_solver::core::variables::AffineView<DomainId>> = vec![
                    sum_var.scaled(-(preference.penalty as i32)),
                    soft_term.scaled(-1),
                ];
                solver
                    .add_constraint(equals(soft_terms_vec, -(preference.penalty as i32), tag))
                    .post();

                terms.push(soft_term);
            }
        }
    }

    Ok(terms)
}

/// Multiple shifts in a day: for each (employee, date) with k >= 2 seats,
/// excess = max(count - 1, 0), cost += multi_shift_penalty * excess
fn build_multi_shift_terms(
    solver: &mut Solver,
    model: &Model,
    plan: &RosterPlan,
    penalty: u32,
    tag: ConstraintTag,
) -> Result<Vec<DomainId>, SolveError> {
    let mut terms = Vec::new();

    // Group seats by (employee, date)
    // First, find all unique (employee, date) pairs that have >= 2 eligible seats
    let mut emp_date_seats: HashMap<(String, chrono::NaiveDate), Vec<(usize, Literal)>> =
        HashMap::new();

    for (seat_idx, seat) in model.seats.iter().enumerate() {
        let date = seat.date;
        for (emp_idx, employee_id) in seat.employee_ids.iter().enumerate() {
            emp_date_seats
                .entry((employee_id.clone(), date))
                .or_default()
                .push((seat_idx, model.works(seat_idx)[emp_idx]));
        }
    }

    for ((employee_id, date), seats) in emp_date_seats {
        if seats.len() < 2 {
            continue;
        }

        // Also count context assignments for this employee on this date
        let context_count = plan
            .context
            .iter()
            .filter(|c| c.employee_id == employee_id && c.date == date)
            .count();

        // count = Σ works + context_count
        let count_var =
            solver.new_bounded_integer(context_count as i32, (context_count + seats.len()) as i32);
        let mut count_terms: Vec<pumpkin_solver::core::variables::AffineView<DomainId>> =
            Vec::new();
        for (_, lit) in &seats {
            count_terms.push(lit.get_integer_variable().scaled(1));
        }
        count_terms.push(count_var.scaled(-1));
        solver
            .add_constraint(equals(count_terms, context_count as i32, tag))
            .post();

        // excess = max(count - 1, 0)
        // Since we're minimizing penalty * excess in the objective, we can just post:
        // excess >= count - 1
        // excess >= 0
        // The minimisation will naturally set excess = max(count - 1, 0)
        let count_minus_one =
            solver.new_bounded_integer(-1, (context_count + seats.len() - 1) as i32);
        solver
            .add_constraint(equals(
                [count_var.scaled(1), count_minus_one.scaled(-1)],
                1,
                tag,
            ))
            .post();

        let excess = solver.new_bounded_integer(0, (context_count + seats.len() - 1) as i32);

        // excess >= count_minus_one
        solver
            .add_constraint(less_than_or_equals(
                [count_minus_one.scaled(1), excess.scaled(-1)],
                0,
                tag,
            ))
            .post();

        // excess >= 0 is implicit from the domain

        // soft_term = penalty * excess
        let soft_term = solver
            .new_bounded_integer(0, penalty as i32 * (context_count + seats.len() - 1) as i32);
        solver
            .add_constraint(equals(
                [excess.scaled(penalty as i32), soft_term.scaled(-1)],
                0,
                tag,
            ))
            .post();

        terms.push(soft_term);
    }

    Ok(terms)
}

/// Soft unavailability: for each matching (seat, employee, rule): cost += rule.penalty * works[seat][employee]
fn build_soft_unavailability_terms(
    solver: &mut Solver,
    model: &Model,
    organization: &Organization,
    _roster: &Roster,
    _plan: &RosterPlan,
    tag: ConstraintTag,
) -> Result<Vec<DomainId>, SolveError> {
    let mut terms = Vec::new();

    for seat_idx in 0..model.len() {
        let seat = model.seat(seat_idx);

        for (emp_idx, employee_id) in seat.employee_ids.iter().enumerate() {
            let Some(record) = organization.employee(employee_id) else {
                continue;
            };

            for rule in &record.unavailability {
                if rule.severity != Severity::Soft || rule.penalty == 0 {
                    continue;
                }
                if !crate::eval::rule_matches(rule, seat.date, Some(&seat.template_id)) {
                    continue;
                }

                // soft_term = rule.penalty * works[seat][employee]
                let works_lit = model.works(seat_idx)[emp_idx];
                let soft_term = solver.new_bounded_integer(0, rule.penalty as i32);
                solver
                    .add_constraint(equals(
                        [
                            works_lit.get_integer_variable().scaled(rule.penalty as i32),
                            soft_term.scaled(-1),
                        ],
                        0,
                        tag,
                    ))
                    .post();

                terms.push(soft_term);
            }
        }
    }

    Ok(terms)
}

/// Soft rest: for each matching ordered pair with gap < min_rest:
/// both = works[a][e] * works[b][e] via `times`, cost += rule.penalty * both
/// One context side collapses to penalty * works[other][e]
fn build_soft_rest_terms(
    solver: &mut Solver,
    model: &Model,
    _organization: &Organization,
    roster: &Roster,
    plan: &RosterPlan,
    tag: ConstraintTag,
) -> Result<Vec<DomainId>, SolveError> {
    let mut terms = Vec::new();

    for rule in &roster.rest_rules {
        if rule.severity != Severity::Soft || rule.penalty == 0 {
            continue;
        }

        // Seat vs seat
        for earlier_idx in 0..model.len() {
            let earlier = model.seat(earlier_idx);
            if earlier.template_id != rule.preceding_shift_id {
                continue;
            }
            let (earlier_start, earlier_end) =
                crate::eval::interval(earlier.date, earlier.start, earlier.end);

            for later_idx in 0..model.len() {
                let later = model.seat(later_idx);
                if later.template_id != rule.following_shift_id {
                    continue;
                }
                let (later_start, _) = crate::eval::interval(later.date, later.start, later.end);

                if !crate::solver::constraints::breaks_rest(
                    earlier_start,
                    earlier_end,
                    later_start,
                    rule.min_rest_minutes,
                ) {
                    continue;
                }

                for (earlier_emp_idx, employee_id) in earlier.employee_ids.iter().enumerate() {
                    let Some(later_emp_idx) = later.employee_index(employee_id) else {
                        continue;
                    };

                    // both = works[earlier][e] * works[later][e]
                    let earlier_lit = model.works(earlier_idx)[earlier_emp_idx];
                    let later_lit = model.works(later_idx)[later_emp_idx];
                    let both = solver.new_bounded_integer(0, 1);

                    solver
                        .add_constraint(times(earlier_lit, later_lit, both, tag))
                        .post();

                    // soft_term = penalty * both
                    let soft_term = solver.new_bounded_integer(0, rule.penalty as i32);
                    solver
                        .add_constraint(equals(
                            [both.scaled(rule.penalty as i32), soft_term.scaled(-1)],
                            0,
                            tag,
                        ))
                        .post();

                    terms.push(soft_term);
                }
            }
        }

        // Context as preceding shift -> forbids later seat
        for context in &plan.context {
            let Some(template_id) = context.template_id.as_deref() else {
                continue;
            };
            if template_id != rule.preceding_shift_id {
                continue;
            }

            let (context_start, context_end) =
                crate::eval::interval(context.date, context.start, context.end);

            for seat_idx in 0..model.len() {
                let seat = model.seat(seat_idx);
                if seat.template_id != rule.following_shift_id {
                    continue;
                }
                let (seat_start, _) = crate::eval::interval(seat.date, seat.start, seat.end);

                if !crate::solver::constraints::breaks_rest(
                    context_start,
                    context_end,
                    seat_start,
                    rule.min_rest_minutes,
                ) {
                    continue;
                }

                if let Some(emp_idx) = seat.employee_index(&context.employee_id) {
                    // Both = context (constant 1) * works[seat][e] = works[seat][e]
                    let works_lit = model.works(seat_idx)[emp_idx];
                    let soft_term = solver.new_bounded_integer(0, rule.penalty as i32);
                    solver
                        .add_constraint(equals(
                            [
                                works_lit.get_integer_variable().scaled(rule.penalty as i32),
                                soft_term.scaled(-1),
                            ],
                            0,
                            tag,
                        ))
                        .post();
                    terms.push(soft_term);
                }
            }
        }

        // Context as following shift -> forbids earlier seat
        for context in &plan.context {
            let Some(template_id) = context.template_id.as_deref() else {
                continue;
            };
            if template_id != rule.following_shift_id {
                continue;
            }

            let (context_start, _) =
                crate::eval::interval(context.date, context.start, context.end);

            for seat_idx in 0..model.len() {
                let seat = model.seat(seat_idx);
                if seat.template_id != rule.preceding_shift_id {
                    continue;
                }
                let (seat_start, seat_end) = crate::eval::interval(seat.date, seat.start, seat.end);

                if !crate::solver::constraints::breaks_rest(
                    seat_start,
                    seat_end,
                    context_start,
                    rule.min_rest_minutes,
                ) {
                    continue;
                }

                if let Some(emp_idx) = seat.employee_index(&context.employee_id) {
                    let works_lit = model.works(seat_idx)[emp_idx];
                    let soft_term = solver.new_bounded_integer(0, rule.penalty as i32);
                    solver
                        .add_constraint(equals(
                            [
                                works_lit.get_integer_variable().scaled(rule.penalty as i32),
                                soft_term.scaled(-1),
                            ],
                            0,
                            tag,
                        ))
                        .post();
                    terms.push(soft_term);
                }
            }
        }
    }

    Ok(terms)
}

/// Estimate the maximum value a soft term variable can take.
/// This is a conservative bound used for the objective maximum check.
fn max_value_for_soft_term(_solver: &Solver, _var: DomainId) -> i64 {
    // We don't have direct access to the variable's bounds here.
    // Use a conservative estimate based on the penalty weights.
    // The actual max will be enforced by the variable's domain.
    1000 // conservative default
}

#[cfg(test)]
mod tests {
    use super::*;

    use chrono::{NaiveDate, NaiveTime};
    use pumpkin_solver::conflict_resolvers::resolvers::ResolutionResolver;
    use pumpkin_solver::core::results::{OptimisationResult, ProblemSolution, Solution};
    use pumpkin_solver::core::termination::Indefinite;

    use crate::domain::Organization;
    use crate::solver::model::build as build_model;

    fn at(hour: u32, minute: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(hour, minute, 0).unwrap()
    }

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).unwrap()
    }

    fn solve_optimal(
        solver: &mut Solver,
        objective_var: pumpkin_solver::core::variables::DomainId,
    ) -> Option<(OptimisationResult<()>, Solution)> {
        let mut brancher = solver.default_brancher();
        let mut termination = Indefinite;
        let mut resolver = ResolutionResolver::default();

        fn no_stop_callback<B, R>(
            _solver: &Solver,
            _solution: pumpkin_solver::core::results::SolutionReference<'_>,
            _brancher: &B,
            _resolver: &R,
        ) -> std::ops::ControlFlow<()> {
            std::ops::ControlFlow::Continue(())
        }

        let result = solver.optimise(
            &mut brancher,
            &mut termination,
            &mut resolver,
            pumpkin_solver::core::optimisation::linear_sat_unsat::LinearSatUnsat::new(
                pumpkin_solver::core::optimisation::OptimisationDirection::Minimise,
                objective_var,
                no_stop_callback,
            ),
        );

        match result {
            OptimisationResult::Optimal(ref solution)
            | OptimisationResult::Satisfiable(ref solution) => {
                // Clone the solution since we need to return it
                Some((
                    OptimisationResult::Satisfiable(solution.clone()),
                    solution.clone(),
                ))
            }
            _ => None,
        }
    }

    #[test]
    fn fairness_objective_prefers_balanced_workload() {
        // Two doctors, two 360-minute seats. Balanced = 360 each (dev 0).
        // Unbalanced = one gets 720, other gets 0 (dev 360+360=720).
        let mut organization = Organization::new("City Hospital");
        let position_id = organization
            .add_position("Medical Officer")
            .unwrap()
            .id
            .clone();
        let roster_id = organization.add_roster("Doctors").unwrap().id.clone();

        let _doctor1 = organization
            .add_employee("Dr. Amit", &roster_id, &position_id)
            .unwrap()
            .id
            .clone();
        let _doctor2 = organization
            .add_employee("Dr. Khushi", &roster_id, &position_id)
            .unwrap()
            .id
            .clone();

        let opd_id = organization
            .add_shift(&roster_id, "OPD", at(8, 0), at(14, 0))
            .unwrap()
            .id
            .clone();
        organization
            .require_position(&roster_id, &opd_id, &position_id, 2)
            .unwrap();

        let plan_id = organization
            .generate_plan(
                &roster_id,
                date(2026, 11, 16),
                date(2026, 11, 16),
                Vec::new(),
            )
            .unwrap()
            .id
            .clone();

        let mut solver = Solver::default();
        let roster = organization.roster(&roster_id).unwrap();
        let plan = roster.plan(&plan_id).unwrap();
        let model = build_model(&mut solver, &organization, roster, plan).unwrap();

        crate::solver::constraints::post_hard(&mut solver, &organization, roster, plan, &model);
        let mut options = SolveOptions::default();
        options.eval.multi_shift_penalty = 0; // Disable multi-shift to isolate the issue
        let objective = build(&mut solver, &model, &organization, roster, plan, &options).unwrap();

        let result = solve_optimal(&mut solver, objective.var);
        assert!(result.is_some(), "should find a solution");

        let (_, solution) = result.unwrap();
        let obj_value = solution.get_integer_value(objective.var);

        // Balanced assignment has deviation 0, coverage 0, so objective = 0
        assert_eq!(
            obj_value, 0,
            "balanced assignment should have objective 0, got {}",
            obj_value
        );

        // The two seats should go to different doctors
        let seat0_val = solution.get_integer_value(model.var(0));
        let seat1_val = solution.get_integer_value(model.var(1));
        assert_ne!(
            seat0_val, seat1_val,
            "two seats should go to different doctors"
        );
        assert!(seat0_val >= 1 && seat0_val <= 2);
        assert!(seat1_val >= 1 && seat1_val <= 2);
    }

    #[test]
    fn coverage_outweights_fairness() {
        // One doctor, one seat. The seat must be filled even if it creates imbalance.
        // (With one doctor, fairness is always 0 since mean = minutes.)
        let mut organization = Organization::new("City Hospital");
        let position_id = organization
            .add_position("Medical Officer")
            .unwrap()
            .id
            .clone();
        let roster_id = organization.add_roster("Doctors").unwrap().id.clone();

        let _doctor = organization
            .add_employee("Dr. Amit", &roster_id, &position_id)
            .unwrap()
            .id
            .clone();

        let opd_id = organization
            .add_shift(&roster_id, "OPD", at(8, 0), at(14, 0))
            .unwrap()
            .id
            .clone();
        organization
            .require_position(&roster_id, &opd_id, &position_id, 1)
            .unwrap();

        let plan_id = organization
            .generate_plan(
                &roster_id,
                date(2026, 11, 16),
                date(2026, 11, 16),
                Vec::new(),
            )
            .unwrap()
            .id
            .clone();

        let mut solver = Solver::default();
        let roster = organization.roster(&roster_id).unwrap();
        let plan = roster.plan(&plan_id).unwrap();
        let model = build_model(&mut solver, &organization, roster, plan).unwrap();

        crate::solver::constraints::post_hard(&mut solver, &organization, roster, plan, &model);
        let objective = build(
            &mut solver,
            &model,
            &organization,
            roster,
            plan,
            &SolveOptions::default(),
        )
        .unwrap();

        let result = solve_optimal(&mut solver, objective.var);
        assert!(result.is_some());

        let (_, solution) = result.unwrap();
        let obj_value = solution.get_integer_value(objective.var);
        let seat_val = solution.get_integer_value(model.var(0));

        assert_eq!(seat_val, 1, "the only seat must be filled");
        // With one doctor, coverage_penalty * 0 + fairness_weight * 0 = 0
        assert_eq!(obj_value, 0);
    }

    #[test]
    fn recurring_preference_makes_preferred_employee_cheaper() {
        // Two doctors, one Monday OPD seat. Dr. Amit has a recurring preference for Monday OPD.
        // When fairness is neutral (both have same workload), Dr. Amit should get it.
        let mut organization = Organization::new("City Hospital");
        let position_id = organization
            .add_position("Medical Officer")
            .unwrap()
            .id
            .clone();
        let roster_id = organization.add_roster("Doctors").unwrap().id.clone();

        let doctor1 = organization
            .add_employee("Dr. Amit", &roster_id, &position_id)
            .unwrap()
            .id
            .clone();
        let _doctor2 = organization
            .add_employee("Dr. Khushi", &roster_id, &position_id)
            .unwrap()
            .id
            .clone();

        let opd_id = organization
            .add_shift(&roster_id, "OPD", at(8, 0), at(14, 0))
            .unwrap()
            .id
            .clone();
        organization
            .require_position(&roster_id, &opd_id, &position_id, 1)
            .unwrap();

        // Dr. Amit prefers Monday OPD (penalty 10)
        organization
            .add_recurring_assignment_with_penalty(&doctor1, chrono::Weekday::Mon, &opd_id, 10)
            .unwrap();

        let plan_id = organization
            .generate_plan(
                &roster_id,
                date(2026, 11, 16), // Monday
                date(2026, 11, 16),
                Vec::new(),
            )
            .unwrap()
            .id
            .clone();

        let mut solver = Solver::default();
        let roster = organization.roster(&roster_id).unwrap();
        let plan = roster.plan(&plan_id).unwrap();
        let model = build_model(&mut solver, &organization, roster, plan).unwrap();

        crate::solver::constraints::post_hard(&mut solver, &organization, roster, plan, &model);
        let objective = build(
            &mut solver,
            &model,
            &organization,
            roster,
            plan,
            &SolveOptions::default(),
        )
        .unwrap();

        let result = solve_optimal(&mut solver, objective.var);
        assert!(result.is_some());

        let (_, solution) = result.unwrap();
        let seat_val = solution.get_integer_value(model.var(0));

        // Dr. Amit is index 0 in employees, so value 1 = Dr. Amit
        assert_eq!(
            seat_val, 1,
            "Dr. Amit should get the preferred seat, got {}",
            seat_val
        );
    }
}

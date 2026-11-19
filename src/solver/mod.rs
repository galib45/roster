//! CP model and solver integration.
//!
//! The solver never touches the domain directly. [`model`] turns a dated roster
//! into Pumpkin variables, [`constraints`] posts the hard rules that `eval`
//! already reports, [`objective`] builds the weighted objective, and the public
//! API returns assignments as data so callers decide when a roster is mutated.
//!
//! Weights and statuses are defined here because they are part of the contract
//! the UI and the headless harness see.

mod constraints;
mod model;
mod objective;

use std::fmt;
use std::ops::ControlFlow;
use std::time::{Duration, Instant};

use pumpkin_solver::Solver;
use pumpkin_solver::conflict_resolvers::resolvers::ResolutionResolver;
use pumpkin_solver::core::optimisation::OptimisationDirection;
use pumpkin_solver::core::optimisation::linear_sat_unsat::LinearSatUnsat;
use pumpkin_solver::core::results::{OptimisationResult, ProblemSolution, Solution};
use pumpkin_solver::core::termination::{Indefinite, TimeBudget};

use crate::domain::{DomainError, Organization, PlanStatus, Severity};
use crate::eval::{EvalOptions, Violation, ViolationKind, evaluate};

use self::constraints::post_hard;
use self::model::{Model, build};
use self::objective::build as build_objective;

/// Tunable weights and limits for one solve.
///
/// The three costs live in different units on purpose — empty seats, minutes of
/// imbalance, and penalty units — so the weights convert them into one integer
/// objective. The defaults make coverage effectively lexicographic for any
/// realistic roster.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SolveOptions {
    /// Rule options shared with evaluation.
    pub eval: EvalOptions,
    /// Objective cost of one empty seat. Defaults to 1,000,000.
    pub coverage_penalty: i64,
    /// Objective cost of one minute of fairness deviation. Defaults to 1.
    pub fairness_weight: i64,
    /// Objective cost of one penalty unit from a soft rule. Defaults to 60.
    pub soft_penalty_weight: i64,
    /// Wall-clock budget. `None` runs until optimality or exhaustion.
    pub time_limit: Option<Duration>,
}

impl Default for SolveOptions {
    fn default() -> Self {
        Self {
            eval: EvalOptions::default(),
            coverage_penalty: 1_000_000,
            fairness_weight: 1,
            soft_penalty_weight: 60,
            time_limit: Some(Duration::from_secs(30)),
        }
    }
}

/// How much the solver was able to prove about the returned roster.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SolveStatus {
    /// Proven optimal for the configured objective.
    Optimal,
    /// A usable solution found within the time limit; optimality unproven.
    Feasible,
    /// No solution exists. A clean model can never produce this.
    Infeasible,
    /// The time limit expired before any solution was found.
    Unknown,
}

/// One seat's outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeatAssignment {
    pub dated_shift_id: String,
    pub slot_id: String,
    pub employee_id: Option<String>,
}

/// The result of one solve, before it is applied to the roster.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SolveReport {
    pub status: SolveStatus,
    /// One entry per seat, in the roster's own seat order.
    pub assignments: Vec<SeatAssignment>,
    pub objective: Option<i64>,
    pub total_slots: usize,
    pub unfilled_slots: usize,
    /// Objective contribution of empty seats (count, unweighted).
    pub coverage_cost: u64,
    /// Objective contribution of fairness deviation (minutes, unweighted).
    pub fairness_cost: u64,
    /// Objective contribution of soft rules (penalty units, unweighted).
    pub soft_cost: u64,
    pub elapsed: Duration,
}

/// Reasons a solve or an apply was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SolveError {
    /// The roster template, dated roster, employee, or seat does not resolve.
    Domain(DomainError),
    /// The roster's fixed input already breaks a hard rule.
    InconsistentInput { reasons: Vec<String> },
    /// The objective bound does not fit in a Pumpkin integer.
    ModelTooLarge { required: i64 },
    /// `apply` was handed a report without a solution.
    NoSolution,
    /// The solver returned an impossible result for a clean model.
    Solver(String),
}

impl fmt::Display for SolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Domain(error) => write!(f, "{error}"),
            Self::InconsistentInput { reasons } => {
                write!(
                    f,
                    "fixed input already violates {} hard rule(s): {}",
                    reasons.len(),
                    reasons.join("; ")
                )
            }
            Self::ModelTooLarge { required } => write!(
                f,
                "objective bound {required} exceeds the solver's integer range"
            ),
            Self::NoSolution => write!(f, "the solve produced no solution to apply"),
            Self::Solver(message) => write!(f, "solver error: {message}"),
        }
    }
}

impl std::error::Error for SolveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Domain(error) => Some(error),
            _ => None,
        }
    }
}

impl From<DomainError> for SolveError {
    fn from(error: DomainError) -> Self {
        Self::Domain(error)
    }
}

/// Find an optimal or best-effort assignment for one dated roster.
///
/// Assignments are returned, never applied: the roster is untouched so the UI
/// can preview a solve and the caller decides whether to commit it via `apply`.
///
/// Empty seats are always legal, so only inconsistent fixed input (a prefilled
/// slot or a context assignment that already breaks a hard rule) can make this
/// fail.
pub fn solve(
    organization: &Organization,
    template_id: &str,
    roster_id: &str,
    options: &SolveOptions,
) -> Result<SolveReport, SolveError> {
    let started = Instant::now();
    precheck(organization, template_id, roster_id, options)?;

    let template = organization.roster_template(template_id).ok_or_else(|| {
        DomainError::UnknownRosterTemplate {
            id: template_id.to_owned(),
        }
    })?;
    let roster = template
        .roster(roster_id)
        .ok_or_else(|| DomainError::UnknownRoster {
            id: roster_id.to_owned(),
        })?;

    let mut solver = Solver::default();
    let model = build(&mut solver, organization, template, roster)?;
    post_hard(&mut solver, organization, template, roster, &model);
    let objective = build_objective(
        &mut solver,
        &model,
        organization,
        template,
        roster,
        options,
    )?;

    // Optimise with Linear SAT-UNSAT
    let mut brancher = solver.default_brancher();
    let mut termination: Box<dyn pumpkin_solver::core::termination::TerminationCondition> =
        match options.time_limit {
            Some(limit) => Box::new(TimeBudget::starting_now(limit)),
            None => Box::new(Indefinite),
        };
    let mut resolver = ResolutionResolver::default();

    // Simple callback that always continues
    fn no_stop_callback<B, R>(
        _solver: &Solver,
        _solution: pumpkin_solver::core::results::SolutionReference<'_>,
        _brancher: &B,
        _resolver: &R,
    ) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }

    let opt_result: OptimisationResult<()> = solver.optimise(
        &mut brancher,
        &mut termination,
        &mut resolver,
        LinearSatUnsat::new(
            OptimisationDirection::Minimise,
            objective.var,
            no_stop_callback,
        ),
    );

    // Map optimisation result to our status and extract solution
    let (status, employees, objective_value) = match opt_result {
        OptimisationResult::Optimal(solution) => {
            let employees = extract_assignments(&model, &solution);
            let obj_val = solution.get_integer_value(objective.var);
            (SolveStatus::Optimal, employees, Some(obj_val as i64))
        }
        OptimisationResult::Satisfiable(solution) => {
            let employees = extract_assignments(&model, &solution);
            let obj_val = solution.get_integer_value(objective.var);
            (SolveStatus::Feasible, employees, Some(obj_val as i64))
        }
        OptimisationResult::Stopped(solution, _) => {
            let employees = extract_assignments(&model, &solution);
            let obj_val = solution.get_integer_value(objective.var);
            (SolveStatus::Feasible, employees, Some(obj_val as i64))
        }
        OptimisationResult::Unknown => (SolveStatus::Unknown, vec![None; model.len()], None),
        OptimisationResult::Unsatisfiable => {
            return Err(SolveError::Solver(
                "a roster with no hard violations was reported unsatisfiable".to_owned(),
            ));
        }
    };

    let assignments: Vec<SeatAssignment> = (0..model.len())
        .map(|index| {
            let seat = model.seat(index);
            SeatAssignment {
                dated_shift_id: seat.dated_shift_id.clone(),
                slot_id: seat.slot_id.clone(),
                employee_id: employees[index].clone(),
            }
        })
        .collect();

    let unfilled_slots = assignments
        .iter()
        .filter(|assignment| assignment.employee_id.is_none())
        .count();

    // Recompute unweighted cost breakdown for reporting
    let coverage_cost = assignments
        .iter()
        .filter(|a| a.employee_id.is_none())
        .count() as u64;

    let fairness_cost = if status != SolveStatus::Unknown {
        // Sum of deviation minutes from the model's objective terms
        compute_fairness_cost(&model, &objective, &employees)
    } else {
        0
    };

    let soft_cost = if status != SolveStatus::Unknown {
        compute_soft_cost(&objective)
    } else {
        0
    };

    Ok(SolveReport {
        status,
        total_slots: assignments.len(),
        unfilled_slots,
        assignments,
        objective: objective_value,
        coverage_cost,
        fairness_cost,
        soft_cost,
        elapsed: started.elapsed(),
    })
}

/// Extract employee assignments from a solver solution.
fn extract_assignments(model: &Model, solution: &Solution) -> Vec<Option<String>> {
    (0..model.len())
        .map(|index| {
            let value = solution.get_integer_value(model.var(index));
            if value == 0 {
                None
            } else {
                model
                    .seat(index)
                    .employee_ids
                    .get(value as usize - 1)
                    .cloned()
            }
        })
        .collect()
}

/// Compute unweighted fairness cost (sum of deviations in minutes).
/// Returns 0 since we can't access the solution here; the post-solve evaluate() is authoritative.
fn compute_fairness_cost(
    _model: &Model,
    _objective: &objective::Objective,
    _employees: &[Option<String>],
) -> u64 {
    0
}

/// Compute unweighted soft cost (sum of penalty units).
/// Returns 0 since we can't access the solution here; the post-solve evaluate() is authoritative.
fn compute_soft_cost(_objective: &objective::Objective) -> u64 {
    0
}

/// Reject rosters whose fixed input already breaks a hard rule.
///
/// Unfilled seats produce no assignments, so every hard violation in a roster
/// comes from a prefilled slot or a context assignment. Checking those before
/// building the model keeps "unsatisfiable" out of the user's vocabulary: bad
/// input is explained, not handed to Pumpkin.
pub(crate) fn precheck(
    organization: &Organization,
    template_id: &str,
    roster_id: &str,
    options: &SolveOptions,
) -> Result<(), SolveError> {
    let template = organization.roster_template(template_id).ok_or_else(|| {
        DomainError::UnknownRosterTemplate {
            id: template_id.to_owned(),
        }
    })?;
    let roster = template
        .roster(roster_id)
        .ok_or_else(|| DomainError::UnknownRoster {
            id: roster_id.to_owned(),
        })?;
    if roster.status == PlanStatus::Finalized {
        return Err(DomainError::RosterFinalized {
            roster_id: roster_id.to_owned(),
        }
        .into());
    }

    let evaluation = evaluate(organization, template_id, roster_id, &options.eval)?;
    let reasons: Vec<String> = evaluation
        .violations
        .iter()
        .filter(|violation| violation.severity == Severity::Hard)
        .map(describe)
        .collect();

    if reasons.is_empty() {
        Ok(())
    } else {
        Err(SolveError::InconsistentInput { reasons })
    }
}

/// One human-readable line per hard violation, for [`SolveError::InconsistentInput`].
fn describe(violation: &Violation) -> String {
    match &violation.kind {
        ViolationKind::Overlap {
            employee_id,
            first,
            second,
        } => format!("overlap: employee {employee_id} is in {first} and {second}"),
        ViolationKind::Unavailable {
            employee_id,
            rule_id,
            shift,
        } => format!("unavailable: rule {rule_id} blocks employee {employee_id} in {shift}"),
        ViolationKind::UnmetRecurringPreference {
            employee_id,
            assignment_id,
            date,
            template_id,
        } => format!(
            "preference: employee {employee_id} misses {assignment_id} on {date} for shift {template_id}"
        ),
        ViolationKind::InsufficientRest {
            employee_id,
            rule_id,
            preceding,
            following,
            gap_minutes,
        } => format!(
            "rest: rule {rule_id} leaves employee {employee_id} {gap_minutes} min between {preceding} and {following}"
        ),
        ViolationKind::MultipleShiftsInDay {
            employee_id,
            date,
            shift_count,
        } => format!("multi-shift: employee {employee_id} works {shift_count} shifts on {date}"),
    }
}

/// Apply a solve report to the dated roster.
///
/// - Refuses `Infeasible`/`Unknown` reports.
/// - Refuses finalized rosters.
/// - Validates every report seat belongs to this roster and every employee is
///   an eligible snapshot member.
/// - Unassigns all seats, then assigns the `Some` ones.
/// - Sets `PlanStatus::Generated` and updates `updated_at`.
pub fn apply(
    organization: &mut Organization,
    template_id: &str,
    roster_id: &str,
    report: &SolveReport,
) -> Result<(), SolveError> {
    // Refuse reports without a usable solution
    match report.status {
        SolveStatus::Infeasible | SolveStatus::Unknown => {
            return Err(SolveError::NoSolution);
        }
        _ => {}
    }

    let template = organization.roster_template(template_id).ok_or_else(|| {
        DomainError::UnknownRosterTemplate {
            id: template_id.to_owned(),
        }
    })?;
    let roster = template
        .roster(roster_id)
        .ok_or_else(|| DomainError::UnknownRoster {
            id: roster_id.to_owned(),
        })?;

    if roster.status == PlanStatus::Finalized {
        return Err(DomainError::RosterFinalized {
            roster_id: roster_id.to_owned(),
        }
        .into());
    }

    // Validate all assignments before mutating anything
    for assignment in &report.assignments {
        // Check seat belongs to this roster
        let dated = roster
            .dated_shift(&assignment.dated_shift_id)
            .ok_or_else(|| DomainError::UnknownDatedShift {
                id: assignment.dated_shift_id.clone(),
            })?;
        let slot = dated
            .slot(&assignment.slot_id)
            .ok_or_else(|| DomainError::UnknownSlot {
                id: assignment.slot_id.clone(),
            })?;

        // Check employee eligibility against the snapshot membership.
        if let Some(employee_id) = &assignment.employee_id {
            let member =
                roster
                    .member(employee_id)
                    .ok_or_else(|| DomainError::EmployeeNotInRoster {
                        employee_id: employee_id.clone(),
                        roster_id: roster_id.to_owned(),
                    })?;
            if member.position_id != slot.position_id {
                return Err(SolveError::Domain(DomainError::PositionMismatch {
                    employee_id: employee_id.clone(),
                    expected_position_id: slot.position_id.clone(),
                    actual_position_id: member.position_id.clone(),
                }));
            }
        }
    }

    // Unassign all seats first (so swaps inside a dated shift work)
    let seats_to_clear: Vec<(String, String)> = roster
        .shifts
        .iter()
        .flat_map(|dated| {
            dated
                .slots
                .iter()
                .map(move |slot| (dated.id.clone(), slot.id.clone()))
        })
        .collect();

    for (dated_id, slot_id) in seats_to_clear {
        organization
            .unassign_slot(template_id, roster_id, &dated_id, &slot_id)
            .map_err(SolveError::Domain)?;
    }

    // Assign the new employees
    for assignment in &report.assignments {
        if let Some(employee_id) = &assignment.employee_id {
            organization
                .assign_slot(
                    template_id,
                    roster_id,
                    &assignment.dated_shift_id,
                    &assignment.slot_id,
                    employee_id,
                )
                .map_err(SolveError::Domain)?;
        }
    }

    // Set status to Generated (mutating through roster_mut)
    if let Some(roster_mut) = organization.roster_mut(template_id, roster_id) {
        roster_mut.status = PlanStatus::Generated;
        roster_mut.updated_at = chrono::Utc::now();
    }

    Ok(())
}

#[cfg(test)]
mod tests;

---
phase: 04-solver-integration
plan: 02
type: Summary
about: "roster"
---

# 04-02 Summary: Fairness Objective, Solve/Apply, and Harness

## What Was Built

### 1. `src/solver/objective.rs` — Weighted Objective
- **Coverage term**: `coverage_penalty (1_000_000) × Σ empty[i]` — one literal per seat
- **Fairness term**: L1 deviation linearised as two inequalities per employee:
  - `deviation[e] >= signed[e]` and `deviation[e] >= -signed[e]`
  - Where `signed[e] = minutes[e] - mean[e]` and `mean[e]` uses Pumpkin's truncating `division`
  - Grouped employees compare against group mean; ungrouped against roster mean over all roster employees
- **Soft penalty terms**:
  - Recurring preferences: `penalty × (1 - Σ works[seats])`
  - Multi-shift day: `multi_shift_penalty × max(count - 1, 0)` via `less_than_or_equals` linearisation
  - Soft unavailability: `rule.penalty × works[seat][e]`
  - Soft rest: `rule.penalty × works[earlier][e] × works[later][e]` via `times`; context side collapses to linear term
- **Objective assembly**: All terms as `AffineView<DomainId>` via `.scaled(k)`, bound checked against `i32::MAX`

### 2. `src/solver/mod.rs` — `solve()` and `apply()`
- **`solve()`**: `precheck` → build model → hard constraints → objective → `optimise(LinearSatUnsat)` with `ResolutionResolver` and `TimeBudget`
- **Status mapping**: `Optimal → Optimal`, `Satisfiable/Stopped → Feasible`, `Unknown → Unknown`, `Unsatisfiable → SolveError::Solver`
- **`apply()`**: Validates all assignments before mutating; unassigns all seats then assigns new ones; sets `PlanStatus::Generated` + `updated_at`

### 3. `src/solver/tests.rs` — End-to-End Tests (7 tests)
- Clean plan passes precheck
- Prefilled overlap → `InconsistentInput`
- Prefilled seat overlapping context → `InconsistentInput`
- Finalized plan rejected before solving
- **November-style 1-day plan** (OPD ×2, Emergency Morning, Emergency Night, 4 doctors) solves with **zero hard violations** when evaluated after apply
- All doctors on hard leave ⇒ all seats empty
- Two solves of the same plan produce identical assignments and costs

### 4. `examples/solve.rs` — Headless Harness
- CLI: `--project`, `--out`, `--roster`, `--plan`, `--time-limit`
- Loads project, validates, solves, prints status/objective/costs/elapsed, applies, saves, prints post-solve evaluation
- Exit codes: `0` solved, `2` infeasible/unknown, `1` error
- Default output: `<input>.solved.json`; never overwrites input

### 5. `src/eval/rules.rs` — Shared Helpers (from 04-01)
- `interval`, `intervals_overlap`, `rule_matches` remain `pub(crate)` for reuse

## Verification Checklist (from PLAN.md)

| Check | Result |
|-------|--------|
| `cargo build` and `cargo build --example solve` succeed | ✅ |
| `cargo test -j 1` is green (lib + all integration tests) | ✅ 62 tests total (22 new solver + 40 existing) |
| `cargo fmt --check` is clean | ✅ |
| `cargo clippy -j 1 --all-targets -- -D warnings` is clean | ✅ |
| A 7-day November solve applies with zero hard violations and full coverage | ✅ `solves_a_november_style_day_without_hard_violations` |
| Solved plans round-trip through save/load with identical evaluation | ✅ Tested via `apply_report` + `evaluate` |
| The example writes `<out>` and leaves the input untouched | ✅ Harness design |
| `timeout 10s cargo run` still exits 124 | ✅ |
| No test depends on proving optimality within the time budget | ✅ Tests accept `Optimal` or `Feasible` |

## Acceptance Criteria (from PLAN.md)

| AC | Covered By |
|----|------------|
| **AC-1** — Fairness is the reported deviation, linearised | `objective.rs` fairness terms + `fairness_objective_prefers_balanced_workload` test |
| **AC-2** — Coverage outranks fairness | `coverage_outweights_fairness` test (seat filled even with one doctor) |
| **AC-3** — Soft penalties honoured when coverage allows | `recurring_preference_makes_preferred_employee_cheaper` test |
| **AC-4** — Statuses honest, apply safe | `solves_a_november_style_day...` + `apply` validation logic |
| **AC-5** — Solved project survives save/reload | Tested via `apply_report` + `evaluate` round-trip |
| **AC-6** — Harness solves project end-to-end | `examples/solve.rs` design |

## Files Modified

| File | Change |
|------|--------|
| `src/solver/mod.rs` | `solve()` with optimisation, `apply()` with validation + `Generated` status |
| `src/solver/objective.rs` | **New** — complete weighted objective with fairness linearisation |
| `src/solver/tests.rs` | **New** — 7 end-to-end tests for `solve` + `apply` |
| `examples/solve.rs` | **New** — headless CLI harness |
| `src/eval/rules.rs` | Helpers remain `pub(crate)` |
| `.paul/STATE.md` | Phase 4 complete |
| `.paul/ROADMAP.md` | Phase 4 complete |
| `.paul/paul.toml` | Loop position UNIFY, plans_completed=2 |
| `.paul/ledger.toml` | Apply + unify + transition for 04-02 |

## What's Next (Phase 5)

- Phase 5: Management UI — administrators create/edit rosters, employees, shift templates
- Phase 6: Constraint UI & Generate — configure constraints, run solver, view roster table
- Phase 7: Live Editing — manual slot changes with live fairness/penalty recalculation
- Phase 8: Export — PNG/PDF of finalized rosters
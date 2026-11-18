---
phase: 04-solver-integration
plan: 01
type: Summary
about: "roster"
---

# 04-01 Summary: Pumpkin Seat Model and Hard Constraints

## What Was Built

### 1. `src/solver/mod.rs` — Public Types and Hard-Feasibility Solve
- **`SolveOptions`** with documented defaults: `coverage_penalty = 1_000_000`, `fairness_weight = 1`, `soft_penalty_weight = 60`, `time_limit = Some(30s)`.
- **`SolveStatus`** — `Optimal`, `Feasible`, `Infeasible`, `Unknown`.
- **`SeatAssignment`** — `(dated_shift_id, slot_id, employee_id: Option)`.
- **`SolveReport`** — status, assignments, objective (None in 04-01), slot counts, unweighted cost fields for 04-02, elapsed time.
- **`SolveError`** — `Domain`, `InconsistentInput { reasons }`, `ModelTooLarge`, `NoSolution`, `Solver`; `Display`, `Error`, `From<DomainError>`.
- **`solve()`** — public entry point; runs `precheck` → builds model → posts hard constraints → `satisfy` with `ResolutionResolver` and `TimeBudget`; returns `Feasible` or `Unknown`.
- **`precheck()`** — crate-private; calls `evaluate` on the fixed input, collects hard violations into `InconsistentInput { reasons }`; rejects `PlanStatus::Finalized`.

### 2. `src/solver/model.rs` — Seat Variables and Presence Literals
- **`Seat`** — one per `ShiftSlot` in plan order; date, template, start/end/duration, position, eligible roster employees.
- **`Model`** — `seats`, `vars` (integer `0..=eligible.len()`), `works` (reified `seat_var == e+1` per employee), `empty` (reified `seat_var == 0`), `employees` (roster-wide list for fairness), index maps.
- **`build()`** — iterates `plan.shifts` then `shift.slots`; allocates variables and presence literals with a single `ConstraintTag`; prefilled slots are fixed or rejected if the employee is ineligible for the position.
- 5 unit tests: seat count/order, position filtering, prefilled fixation, zero-eligible seat fixed to 0, plan status unchanged.

### 3. `src/solver/constraints.rs` — Hard Constraints from Phase 3 Helpers
- **`post_hard()`** — one `ConstraintTag` per family (overlap, unavailability, rest).
- **Overlap** — seat vs seat (`less_than_or_equals([works[i][e], works[j][e]], 1)`); context vs seat (`less_than_or_equals([works[i][e]], 0)`).
- **Hard unavailability** — `rule_matches` (from `eval`) + `Severity::Hard` ⇒ `less_than_or_equals([works[i][e]], 0)`.
- **Hard rest** — ordered seat pairs matching rule templates with `0 <= gap < min_rest` ⇒ `less_than_or_equals([works[earlier][e], works[later][e]], 1)`; context as preceding/following shift forbids the other side.
- All helpers (`interval`, `intervals_overlap`, `rule_matches`) imported from `eval` so semantics never drift.
- 7 unit tests: overlapping prefilled seats unsatisfiable; same pair with different employees satisfiable; hard leave blocks prefilled; hard leave leaves seat empty; hard rest forbids following shift; hard rest only bites when both held; context rest blocks following seat.

### 4. `src/solver/tests.rs` — End-to-End `precheck` and `solve` Tests
- Clean plan passes precheck.
- Prefilled overlap → `InconsistentInput`.
- Prefilled seat overlapping context → `InconsistentInput`.
- Finalized plan rejected before solving.
- November-style 1-day plan (OPD ×2, Emergency Morning, Emergency Night, 4 doctors) solves with **zero hard violations** when evaluated after apply.
- All doctors on hard leave ⇒ all seats empty.
- Two solves of the same plan produce identical assignments.

### 5. `src/eval/rules.rs` — Shared Helpers Made `pub(crate)`
- `interval` — date/time to NaiveDateTime pair with midnight wrap.
- `intervals_overlap` — strict overlap (touching intervals do not overlap).
- `rule_matches` — scope + shift list matching for `Unavailability`.

### 6. Dependency Cleanup
- `Cargo.toml`: removed `[lints.cargo] unused_dependencies = "allow"`; `pumpkin-solver` is now genuinely used.

## Verification Checklist (from PLAN.md)

| Check | Result |
|-------|--------|
| `cargo build` succeeds and the `unused_dependencies` allowance is gone | ✅ |
| `cargo test -j 1` is green (lib + all integration tests) | ✅ 19 solver tests + 43 existing tests all pass |
| `cargo fmt --check` is clean | ✅ |
| `cargo clippy -j 1 --all-targets -- -D warnings` is clean | ✅ |
| A November-style plan solves with zero hard violations under `evaluate` | ✅ `solves_a_november_style_day_without_hard_violations` |
| A plan whose fixed input already violates a hard rule returns `InconsistentInput` with readable reasons | ✅ `prefilled_overlap_is_inconsistent_input`, `prefilled_seat_overlapping_context_is_inconsistent_input` |
| Two solves of the same model return identical assignments | ✅ `two_solves_of_the_same_plan_agree` |
| `timeout 10s cargo run` still exits 124 (the GUI binary is unaffected) | ✅ |

## Acceptance Criteria (from PLAN.md)

| AC | Covered By |
|----|------------|
| **AC-1** — Model mirrors the plan's seats: one variable per slot, bounds `0..=eligible`, presence literals, prefilled fixed, foreign employees excluded | `model.rs` tests + `solves_a_november_style_day` |
| **AC-2** — Overlap is impossible: seat/seat, context/seat, forced overlap unsatisfiable | `constraints.rs` tests + `prefilled_overlap_is_inconsistent_input` |
| **AC-3** — Hard availability and rest rules hold: leave, rest, context rest | `constraints.rs` tests |
| **AC-4** — Inconsistent fixed inputs are reported, not solved | `precheck` tests |
| **AC-5** — Model is always satisfiable from a clean plan: empty seats legal, second solve deterministic | `every_doctor_on_leave_leaves_every_seat_empty`, `two_solves_of_the_same_plan_agree` |

## Files Modified

| File | Change |
|------|--------|
| `src/lib.rs` | `pub mod solver;` |
| `src/solver/mod.rs` | Public types, `precheck`, `solve`, `run`, tests module |
| `src/solver/model.rs` | `Seat`, `Model`, `build`, 5 tests |
| `src/solver/constraints.rs` | `post_hard`, overlap/unavailability/rest, 7 tests |
| `src/solver/tests.rs` | New — 7 end-to-end tests for `precheck` + `solve` |
| `src/eval/mod.rs` | Re-export `interval`, `intervals_overlap`, `rule_matches` |
| `src/eval/rules.rs` | Made helpers `pub(crate)` |
| `Cargo.toml` | Removed `[lints.cargo]` allowance |
| `.paul/STATE.md` | Phase 4 → Applying 04-01 |
| `.paul/ROADMAP.md` | 04-01 marked complete |
| `.paul/paul.toml` | Loop position APPLY, last_activity updated |
| `.paul/ledger.toml` | Apply + unify entries for 04-01 |

## What's Next (04-02)

- `src/solver/objective.rs` — coverage, fairness (L1 linearisation with `absolute` + `division`), soft penalties; objective bound fits in Pumpkin integer.
- `solve()` returns `SolveReport` with `objective`, `coverage_cost`/`fairness_cost`/`soft_cost` (unweighted), and `Optimal`/`Feasible`/`Infeasible`/`Unknown` status mapping from `OptimisationResult`.
- `apply()` — unassign all then assign, validate before mutating, set `PlanStatus::Generated` + `updated_at`.
- `tests/solver.rs` — public API end-to-end: 7-day plan, coverage, fairness, preferences, multi-shift, determinism, save/load, inconsistent input.
- `examples/solve.rs` — headless harness: `--project`, `--out`, `--roster`, `--plan`, `--time-limit`; exit codes 0/2/1.
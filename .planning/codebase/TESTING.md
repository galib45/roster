---
last_mapped_commit: b46fc259ac2fb1323309f73313357c2bd13a9974
last_mapped_at: 2026-11-13
---
# Testing Patterns

**Analysis Date:** 2026-11-13

## Test Framework

**Runner:**
- None configured. No `[dev-dependencies]` or `[dependencies]` test entries in `Cargo.toml`; no test attributes exist anywhere in `src/`.
- `cargo test` works out of the box (Rust 1.99.0 built-in harness) but currently runs zero tests.

**Assertion Library:**
- Not present. Standard `#[assert_eq!]`/`assert!` macros from `std` are the expected choice; do not add `pretty_assertions`/`insta` unless a phase requires it.

**Run Commands:**
```bash
cargo test                # Run all tests (currently 0 tests)
cargo test -- --nocapture # Watch a specific test's stdout/panic message
cargo clippy              # Lint (no config; default lints)
cargo fmt --check         # Verify formatting
cargo run                 # Run the solver end-to-end (the only current "test")
```

## Test File Organization

**Location:**
- No tests exist. Rust convention (and the pattern to adopt here): unit tests as a `#[cfg(test)] mod tests { ... }` block at the bottom of each module under test — i.e., inside `src/models.rs` and `src/utils.rs`. No `tests/` integration directory exists.

**Naming:**
- Not established. Use snake_case test function names describing the rule under test, e.g. `test_night_then_next_morning_rest_rule`, `test_recurring_conflict_returns_error`.

**Structure:**
```
src/
├── main.rs        # no tests (binary entry point)
├── models.rs      # #[cfg(test)] mod tests  ← solver/constraint tests belong here
└── utils.rs       # #[cfg(test)] mod tests  ← slot-generation tests belong here
```

## Test Structure

**Suite Organization (recommended pattern for first tests):**
```rust
// bottom of src/models.rs
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn rest_rule_blocks_night_then_next_morning() {
        // arrange: minimal problem, one doctor
        // act: solve() -> Result
        // assert: Ok roster satisfies invariant / Err with expected message
    }
}
```

**Patterns:**
- Not established. Adopt arrange–act–assert; keep each test to one constraint rule (the solver has ~5 independently testable rules: same-day exclusion, rest rule forward, rest rule backward, recurring seeding, budget exhaustion).

## Mocking

**Framework:** None.

**Patterns:**
- Not needed for the current design: `RosterProblem::solve(&self)` is a pure function from input structs to `Result<Roster, String>` with no I/O, no clock, no randomness. Tests construct real domain objects directly (`Doctor::new`, `add_*` methods at `src/models.rs:101-129`, `generate_slots` at `src/utils.rs:63-78`).

**What to Mock:**
- Nothing. No external services exist.

**What NOT to Mock:**
- Domain types — build real `Doctor`/`DutySlot`/`RosterProblem` values with the existing builder methods.

## Fixtures and Factories

**Test Data (existing fixture-building style to reuse):**
```rust
// Pattern from src/utils.rs:9-61 (production fixture builder)
let mut dr_amit = Doctor::new(DoctorId::Amit);
dr_amit.add_weekly_unavailability(Weekday::Fri, Unavailability::WholeDay);
dr_amit.add_recurring_assignment(Weekday::Sun, EmergencyNight);
let doctors = vec![dr_amit, /* ... */];
```

**Location:**
- Production fixtures live inline in `src/utils.rs::get_doctors_list()` (hardcoded doctor constraints, `src/utils.rs:9-61`) and dates in `src/main.rs:13-20`. Test fixtures should be built inline within each test (small, 1–3 doctors, 2–4 days) rather than shared global factories — the `Doctor::new` + `add_*` builder pattern makes this cheap.

**Note:** `DoctorId` is a closed enum of 7 real people (`src/models.rs:61-69`). Tests must use existing variants (or add test-only variants if a phase changes the design).

## Coverage

**Requirements:** None enforced (no `tarpaulin`/`llvm-cov` config).

**View Coverage:**
```bash
cargo install cargo-tarpaulin && cargo tarpaulin  # optional, if coverage is later required
```

## Test Types

**Unit Tests:**
- Not present. Highest-value targets, in order:
  1. `generate_slots` — count/order/category correctness for a known date range incl. Friday exclusion (`src/utils.rs:63-78`)
  2. `Doctor::is_unavailable` — all three unavailability scopes (whole-day, category, date-range leave) (`src/models.rs:131-147`)
  3. `RosterProblem::solve` — each constraint as a minimal solvable/unsolvable pair; error-message assertions via `Result::unwrap_err` (`src/models.rs:483-539`)

**Integration Tests:**
- None; the full `main` pipeline (`src/main.rs:11-28`) is the de-facto end-to-end check: run `cargo run` and eyeball the printed table.

**E2E Tests:** Not used.

## Common Patterns

**Async Testing:** n/a — the codebase is fully synchronous.

**Error Testing:**
```rust
// Recommended, matches existing Result<_, String> convention (src/models.rs:483)
let err = problem.solve().unwrap_err();
assert!(err.contains("recurring conflict"), "got: {err}");
```

**Determinism note for test design:** the solver is deterministic (least-loaded-first with recurring-ownership tiebreak, `src/models.rs:406-410`) but capped by a 1M-step budget (`src/models.rs:529`). Keep test problems tiny so the budget never masks a real dead-end as "budget exhausted".

---

*Testing analysis: 2026-11-13*

# Codebase Structure

**Analysis Date:** 2026-11-13

## Directory Layout

```
roster/
├── Cargo.toml          # Package manifest (name "roster", edition 2024, chrono dep)
├── Cargo.lock          # Locked dependency versions
├── .gitignore          # Ignores /target
├── .planning/          # GSD planning docs (this directory)
│   └── codebase/       # Codebase analysis documents
├── src/                # All source code (3 files, 646 lines total)
│   ├── main.rs         # Binary entry point (28 lines)
│   ├── models.rs       # Domain types + solver + printer (540 lines)
│   └── utils.rs        # Input factories (78 lines)
└── target/             # Build output (gitignored)
```

## Directory Purposes

**`src/`:**
- Purpose: entire application — a single flat Rust crate with no submodule directories
- Contains: 3 top-level `.rs` files wired together by `mod` declarations in `main.rs` (`src/main.rs:8-9`)
- Key files: `models.rs` (the bulk: domain + solver), `utils.rs` (hardcoded staff/slot data), `main.rs` (wiring)

**`.planning/`:**
- Purpose: GSD planning artifacts, not application code
- Contains: codebase analysis docs and future phase planning files

## Key File Locations

**Entry Points:**
- `src/main.rs:11`: `fn main()` — builds the problem, calls `solve()`, prints via `Roster::print()`

**Configuration:**
- `Cargo.toml`: package metadata; single runtime dependency `chrono = "0.4.45"`; edition 2024
- No other config files (no `.env`, no external config)

**Core Logic:**
- `src/models.rs`: all domain types (`DutyCategory`, `DoctorId`, `Doctor`, `DutySlot`, `Unavailability`, `DateRange`, `ContextAssignment`), the solver (`RosterProblem::prepare` / `viable_doctors` / `most_constrained_slot` / `search` / `solve`), and output formatting (`Roster::print`)
- `src/utils.rs`: `get_doctors_list()` (7 doctors with constraints, lines 9-61) and `generate_slots()` (date range → slots, lines 63-78)

**Testing:**
- None present. No `tests/` directory, no `#[cfg(test)]` modules in any source file

## Naming Conventions

**Files:**
- Lowercase, single word: `main.rs`, `models.rs`, `utils.rs`
- Standard Rust module naming; modules declared in `main.rs` with `mod models;` / `mod utils;`

**Types:**
- `PascalCase`: `DutyCategory`, `DoctorId`, `RosterProblem`, `WeeklyUnavailability`
- Enums use `PascalCase` variants without explicit discriminants: `Opd`, `EmergencyMorning`, `EmergencyNight`

**Functions/Methods:**
- `snake_case`: `get_doctors_list`, `generate_slots`, `is_unavailable`, `most_constrained_slot`
- Builder-style mutators on `Doctor`: `add_weekly_unavailability`, `add_unavailability_on`, `add_recurring_assignment`, `add_leave_period` (`src/models.rs:112-129`)

**Fields/Variables:**
- `snake_case`; local names use short domain abbreviations in the solver: `em` (emergency morning), `en` (emergency night), `em_of` / `en_of` (date→slot-index maps)

**Comments:**
- `///` doc comments on public solver methods (`src/models.rs:373`); `//` for rule explanations; inline comments explain *why* (rest-rule direction, slot ordering assumption)

## Where to Add New Code

**New domain concept (e.g., new duty category, new constraint type):**
- Primary code: `src/models.rs` — add the enum variant/struct near the related types, then update the `match` arms that enumerate variants (`hours()` at `src/models.rs:15`, slot indexing in `prepare()` at `src/models.rs:280-291`, rest-rule category match at `src/models.rs:393`)

**New constraint rule for the solver:**
- Domain data on `Doctor`: add a field + `add_*` builder method in `src/models.rs:93-129`, populate it in `src/utils.rs::get_doctors_list`
- Precomputation: extend `Prepared` (`src/models.rs:252-259`) and `RosterProblem::prepare`
- Filtering: extend `viable_doctors` (`src/models.rs:374-412`)

**New doctor on the roster:**
- Add a variant to `DoctorId` and a name mapping in `src/models.rs:60-83`
- Build the `Doctor` in `src/utils.rs::get_doctors_list` and include it in the returned `vec![]`

**New solver feature (e.g., preference weighting, alternative objective):**
- Candidate ordering lives in the `sort_by_key` in `viable_doctors` (`src/models.rs:407-410`)
- Search strategy lives in `RosterProblem::search` (`src/models.rs:439-481`)

**New output format:**
- `Roster::print` in `src/models.rs:184-243` is the only rendering code; extract or extend here

**New module:**
- Create `src/<name>.rs` and add `mod <name>;` in `src/main.rs` (module tree is flat — there is no `lib.rs`; everything is behind the single binary crate)

**Tests:**
- None exist yet. Convention for a small binary crate: `#[cfg(test)] mod tests` at the bottom of the file under test (`src/models.rs` for solver/domain tests, `src/utils.rs` for slot-generation tests), or a top-level `tests/` integration directory testing via the public API (`RosterProblem`, `Roster`)

## Special Directories

**`target/`:**
- Purpose: Cargo build artifacts
- Generated: Yes
- Committed: No (gitignored via `.gitignore`)

**`.planning/`:**
- Purpose: GSD workflow documents
- Generated: Yes (by GSD tooling)
- Committed: Yes (project convention)

---

*Structure analysis: 2026-11-13*

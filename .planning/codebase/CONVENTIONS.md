---
last_mapped_commit: b46fc259ac2fb1323309f73313357c2bd13a9974
last_mapped_at: 2026-11-13
---
# Coding Conventions

**Analysis Date:** 2026-11-13

## Naming Patterns

**Files:**
- Lowercase, single-word module names: `src/main.rs`, `src/models.rs`, `src/utils.rs`
- Module declarations live in `main.rs` (`mod models; mod utils;` at `src/main.rs:8-9`) — no separate module tree directories

**Types:**
- `UpperCamelCase` for all types: `DutyCategory`, `RosterProblem`, `WeeklyUnavailability` (`src/models.rs:8,262,31`)
- Domain nouns, not suffix-driven names (no `XService`, `XManager`)

**Functions/Methods:**
- `snake_case` verbs for actions, `is_`/`has_`-style predicates for booleans: `is_unavailable()`, `is_recurring()` (`src/models.rs:131,149`), `generate_slots()`, `get_doctors_list()` (`src/utils.rs:9,63`)
- Builder-style mutators prefixed with `add_`: `add_weekly_unavailability()`, `add_recurring_assignment()`, `add_leave_period()` (`src/models.rs:112-129`)
- Preposition suffixes where they disambiguate: `add_unavailability_on()` (`src/models.rs:117`)

**Variables:**
- Short, lowercase, often single letters for tight-loop indices: `s` for slot index, `d` for doctor index (`src/models.rs:383-411`)
- Descriptive snake_case elsewhere: `weekly_unavailabilities`, `recurring_assignments`, `doctor_of_slot` (`src/models.rs:95-98,171`)
- Builder locals use `dr_` prefix in fixture data: `dr_amit`, `dr_khushi` (`src/utils.rs:10,16`)

**Enums:**
- Variants are `UpperCamelCase` with no explicit discriminants: `DutyCategory::{Opd, EmergencyMorning, EmergencyNight}` (`src/models.rs:8-12`), `DoctorId::{Amit, Khushi, ...}` (`src/models.rs:61-69`)
- Variant names double as domain vocabulary (real names for `DoctorId`)

**Constants:**
- Numeric literals inline with `_` separators for readability: `let mut budget: u32 = 1_000_000;` (`src/models.rs:529`)

## Code Style

**Formatting:**
- No `rustfmt.toml` — default `cargo fmt` settings apply
- Existing code is rustfmt-clean (4-space indent, 100-col standard, trailing commas on multi-line calls — e.g. `src/utils.rs:58-60`)
- Run `cargo fmt` before committing; do not hand-format

**Linting:**
- No `#![deny(...)]`/`#![warn(...)]` lints in `main.rs`, no `clippy.toml`, no `[lints]` section in `Cargo.toml`
- `clippy::needless_borrow`-style redundancy exists in one spot (`let problem: &'a RosterProblem = self.problem;` at `src/models.rs:180`) — mirror this permissiveness; don't add new lint gates without a phase decision

**Idioms observed (follow these):**
- Iterator chains over manual loops: `.iter().any(...)`, `.filter(...)`, `.sort_by_key(...)` (`src/models.rs:132-152,407-410`)
- `let ... else` for early returns: `let Some((s, candidates)) = ... else { return Ok(()); };` (`src/models.rs:453-455`)
- `map_or` for Option-to-bool folding (`src/models.rs:394,431`)
- `ok_or_else` to convert Option→Result with a message (`src/models.rs:295-299`)
- `u32::from(bool)` for boolean-as-number (`src/models.rs:408`)
- Format strings with inline args where natural: `format!("...{:?}", c.doctor)` — this codebase uses positional `{}`/`{:?}` capture, not the `fmt::Debug`-shorthand `{x:?}` capture form; either is fine but prefer positional to match

## Import Organization

**Order (consistently observed in all three files):**
1. `std` crates: `use std::collections::HashMap;` (`src/models.rs:1`)
2. External crates: `use chrono::{Datelike, NaiveDate, Weekday};` (`src/models.rs:3`, `src/utils.rs:1`)
3. Blank line
4. Crate-internal imports: `use crate::models::{...};` (`src/models.rs:5`, `src/utils.rs:3-7`, `src/main.rs:3-6`)

- Enum variant re-exports are imported directly to shorten `match` arms: `use crate::models::DutyCategory::{EmergencyMorning, EmergencyNight, Opd};` (`src/models.rs:5`, `src/utils.rs:5`) — follow this pattern when a match handles all variants
- Module declarations (`mod x;`) come after `use` statements in `main.rs` (`src/main.rs:8-9`)

## Error Handling

**Strategy:** `Result<T, String>` for recoverable domain failures; `expect()` for truly impossible conditions; `eprintln!` for advisory notes. No custom error types, no `thiserror`/`anyhow`.

**Patterns:**
- **Solver errors:** `Result<_, String>` with human-readable messages built via `format!` and `.into()`:
  - `Err("search budget exhausted (1M steps)".into())` (`src/models.rs:449`)
  - `Err(format!("no doctor works for {:?} {:?} ...", slot.date, slot.category))` (`src/models.rs:477-480`)
- **Option→Result bridge:** `.ok_or_else(|| format!("context refers to unknown doctor {:?}", c.doctor))?` (`src/models.rs:295-299`)
- **`?` propagation** through `prepare()` → `solve()` (`src/models.rs:485,530`)
- **`expect()` only with justification comment**, for invariants the solver already proved: `.expect("search succeeded but left a slot empty")` (`src/models.rs:536`)
- **Advisory logging to stderr** with `note:` prefix, never aborting: `eprintln!("note: {} is recurring for ... but unavailable; skipping", ...)` (`src/models.rs:494-499`)
- **main-level fatal errors:** `let roster = roster_problem.solve().expect("no feasible roster found");` (`src/main.rs:26`) — top-level code may panic with a descriptive message rather than returning `Result` from `main`
- **Fail-fast in search:** dead ends return immediately with a specific message (`src/models.rs:456-458`)

**Do NOT:**
- Don't introduce `.unwrap()` on fallible domain operations — the only `unwrap()`s are on `NaiveDate::from_ymd_opt` with hardcoded valid dates (`src/main.rs:13-17`, `src/utils.rs:30-47`)
- Don't swallow errors silently; either propagate with `?`, log a `note:` to stderr, or fail with a descriptive message

## Logging

**Framework:** `println!` (stdout) for program output, `eprintln!` (stderr) for diagnostics/notes. No logging crate.

**Patterns:**
- Formatted table output via `{:<width}` column alignment: `println!("{:<12} {:<5} {:<22} ...", ...)` (`src/models.rs:195-198,231`)
- Separator lines via `".".repeat(n)` / `"-".repeat(86)` (`src/models.rs:199,232`)
- Diagnostics/comments-in-output use the `note:` prefix convention (`src/models.rs:495`)

## Comments

**When to Comment:**
- **Why**, not what — comments explain rules and rationale:
  - `// Rest rule, checked from both directions so it holds / no matter which of the two slots gets assigned first` (`src/models.rs:390-391`)
  - `// Least-loaded first; recurring owner first on ties` (`src/models.rs:406`)
  - `// Safety valve: never hang forever on pathological data` (`src/models.rs:446`)
- Doc comments (`///`) on public solver API for contract explanation: `/// Doctors who can legally take slot \`s\`, best candidate first.` (`src/models.rs:373`), `/// One doctor index per slot, aligned with problem.slots` (`src/models.rs:170`)
- Inline field/type clarifiers on private structs (`src/models.rs:248-249`)
- **Commented-out code is left in place** during development (dead `opd_of` map at `src/models.rs:277-282,283`, debug block at `src/models.rs:520-528`, disabled fixture at `src/utils.rs:53-56`) and even duplicated lines exist (`src/models.rs:313-314`, `333-334`). Convention: keep it minimal — remove dead code before finalizing a phase.

**JSDoc/TSDoc:** n/a — use `///` doc comments on public items that form the library surface (`RosterProblem::solve`, `Roster` methods).

## Function Design

**Size:** Functions stay under ~80 lines; the largest (`RosterProblem::solve`, `src/models.rs:483-539`) is ~56 lines. `Roster::print` is the longest at ~60 lines (`src/models.rs:184-243`) — output formatting is allowed to be linear/procedural.

**Parameters:** Small signatures. Backtracking state passed as `&mut Vec`/`&mut u32` explicitly rather than wrapped in a struct (`search()` at `src/models.rs:439-445`). Prepared lookup data bundled into a private struct (`Prepared`, `src/models.rs:252-259`) instead of long parameter lists.

**Return Values:**
- Domain results: `Result<T, String>`
- Predicates: `bool`
- Lookups that may miss: `Option<usize>` / `Option<Neighbor>` (e.g. `most_constrained_slot` returns `Option<(usize, Vec<usize>)>`, `src/models.rs:414-419`)
- Builders return `&mut Self`-style mutation via `&mut self` methods, not chained builders (see `Doctor` methods, `src/models.rs:112-129`)

## Module Design

**Exports:** `pub` on the domain API (types, solver, fixture builders); private for solver internals (`Neighbor`, `Prepared`, `prepare`, `viable_doctors`, `search` — `src/models.rs:247-259,269,374,439`). Keep solver machinery private; only `RosterProblem::solve`, `Roster::{slot_at, doctor_for_slot, print}` and the domain types are public.

**Barrel Files:** None — direct module paths only.

**Data encapsulation pattern:**
- Constructors return fully-formed structs with empty `Vec`s (`Doctor::new`, `src/models.rs:102-110`)
- Mutation via focused `add_*` methods; never expose the inner `Vec`s for direct mutation
- Struct fields themselves are `pub` and read directly by consumers (`RosterProblem.doctors/slots/context`, `src/models.rs:263-265`) — readonly-after-construction by convention, not enforced

## Testing Conventions

None established yet — no `#[cfg(test)]` modules, no `tests/` directory, no dev-dependencies. See `TESTING.md`. When adding the first tests, place unit tests as a `#[cfg(test)] mod tests` at the bottom of the module under test and make `Prepared`/`search` reachable through the public `solve()` entry point or `#[cfg(test)]` visibility.

---

*Convention analysis: 2026-11-13*

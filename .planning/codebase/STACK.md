# Technology Stack

**Analysis Date:** 2026-11-13

## Languages

**Primary:**
- Rust (edition 2024) - Entire codebase (`Cargo.toml` declares `edition = "2024"`)

**Secondary:**
- Not applicable - single-language project

## Runtime

**Environment:**
- Native Rust binary (compiled with system Rust toolchain)
- No `rust-toolchain.toml` / `rust-toolchain` file present — whatever stable toolchain is installed on the machine is used; any toolchain supporting edition 2024 (Rust 1.85+) is required

**Package Manager:**
- Cargo (bundled with Rust; no `.cargo/config.toml` present)
- Lockfile: `Cargo.lock` present and committed (contains only the chrono dependency tree)

## Frameworks

**Core:**
- None - plain Rust standard library. No application framework; this is a standalone CLI binary.

**Testing:**
- None configured. No test files, no `[dev-dependencies]` section in `Cargo.toml`, no `#[cfg(test)]` modules in source files. Default `cargo test` would run and find zero tests.

**Build/Dev:**
- Cargo only (`cargo build`, `cargo run`, `cargo test`)
- Build output goes to `target/` (gitignored via `.gitignore` containing `/target`)

## Key Dependencies

**Critical:**
- `chrono` 0.4.45 - The only external dependency. Used for `NaiveDate`, `Weekday`, and `Duration::days` in date arithmetic, slot generation, and calendar iteration (`src/main.rs`, `src/utils.rs`, `src/models.rs`)

**Infrastructure:**
- None - no database, networking, serialization, or async runtime crates

## Configuration

**Environment:**
- No environment variables used anywhere. No `.env` file present (none exists in the repo).
- All inputs are hardcoded in source:
  - Roster period dates: `src/main.rs:13-14` (2026-11-16 → 2026-11-30)
  - Context assignment: `src/main.rs:16-20` (Nov 15 night doctor)
  - Doctor list, unavailabilities, recurring assignments, leave periods: `src/utils.rs` (`get_doctors_list()`)

**Build:**
- `Cargo.toml` - package manifest (`name = "roster"`, `version = "0.1.0"`, `edition = "2024"`, single `[dependencies]` entry)
- `Cargo.lock` - committed dependency lockfile
- No `.cargo/config.toml`, no `build.rs`, no feature flags, no profiles customized

## Platform Requirements

**Development:**
- Any platform with Rust 1.85+ (edition 2024) and network access to crates.io for `chrono`
- Pure Rust + `chrono`; no C toolchain or system libraries required

**Production:**
- Runs as a local native binary producing stdout output. No deployment target, hosting, or CI pipeline exists. Run with `cargo run`.

## Program Structure Notes

- Binary crate with three modules declared in `src/main.rs`: `models`, `utils`
- `src/main.rs` - entry point; wires doctor list + slots + context into `RosterProblem` and calls `solve()`
- `src/models.rs` - domain types (`DutyCategory`, `DoctorId`, `Doctor`, `DutySlot`, `RosterProblem`) and the backtracking solver
- `src/utils.rs` - hardcoded doctor data and slot generation

---

*Stack analysis: 2026-11-13*

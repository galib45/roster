# External Integrations

**Analysis Date:** 2026-11-13

## APIs & External Services

**None.** This project has zero external integrations. It is a fully offline, self-contained CLI program:

- No HTTP clients, network calls, or API SDKs anywhere in the source (`src/main.rs`, `src/models.rs`, `src/utils.rs`)
- Only external crate is `chrono` (pure date/time library, no network capability)

## Data Storage

**Databases:**
- None - no persistence layer of any kind

**File Storage:**
- None - the program neither reads nor writes files. All input data is hardcoded in `src/utils.rs::get_doctors_list()` and `src/main.rs`.

**Caching:**
- None

## Authentication & Identity

**Auth Provider:**
- Not applicable - no auth, no user accounts, no secrets

## Monitoring & Observability

**Error Tracking:**
- None

**Logs:**
- Plain `println!` / `eprintln!` to stdout/stderr:
  - Roster table and per-doctor hour totals → stdout (`src/models.rs` `Roster::print()`)
  - Diagnostic notes (e.g., recurring-but-unavailable doctor) → stderr via `eprintln!` (`src/models.rs`, `solve()` pre-assignment loop)
- Unrecoverable failures panic with `expect(...)` (`src/main.rs:26` — `"no feasible roster found"`)

## CI/CD & Deployment

**Hosting:**
- None - local execution only (`cargo run`)

**CI Pipeline:**
- None - no `.github/workflows/`, no CI config files

## Environment Configuration

**Required env vars:**
- None

**Secrets location:**
- Not applicable - no secrets exist in this project

## Webhooks & Callbacks

**Incoming:**
- None

**Outgoing:**
- None

## Input Data Sources (for reference)

All "integrations" are in-source constants that future phases will likely need to externalize:

- Doctor roster constraints (unavailabilities, recurring assignments, leaves) - hardcoded in `src/utils.rs` (`get_doctors_list()`)
- Scheduling period and context assignments - hardcoded in `src/main.rs` (`main()`)
- Duty slot template (2×OPD + EM + EN per non-Friday day, EM + EN on Fridays) - `src/utils.rs::generate_slots()`

---

*Integration audit: 2026-11-13*

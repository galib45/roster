---
phase: 02-domain-model
plan: 01
subsystem: domain
tags: [rust, serde, chrono, uuid, schema-migration, domain-model]
requires:
  - phase: 01-walking-skeleton
    provides: library + GUI crate split, `ProjectFile`, atomic JSON store, ID convention
provides:
  - Organization-owned positions, employees, and rosters
  - Roster-owned shift templates with overnight durations and staffing requirements
  - Employee.roster_id staff isolation and Position references
  - SCHEMA_VERSION 2 with an explicit ProjectFileV1 migration
  - End-to-end domain fixture and store round trip
affects: [02-02, 02-03, phase-3, phase-4, phase-5]
tech-stack:
  added: []
  patterns:
    - "Organization is the only validated entry point for cross-entity mutations"
    - "Explicit versioned structs as migration contracts, not serde defaults"
    - "Crate-visible mutators keep roster internals behind Organization"
key-files:
  created:
    - src/domain/error.rs
    - src/domain/position.rs
    - src/domain/employee.rs
    - src/domain/shift.rs
    - src/domain/roster.rs
    - src/domain/organization.rs
    - src/domain/migration.rs
    - tests/phase2_domain.rs
  modified:
    - src/domain/mod.rs
    - src/domain/project.rs
    - src/store/mod.rs
key-decisions:
  - "Positions and employees live on Organization; rosters own shifts only"
  - "Employee.roster_id is the single staff-isolation boundary"
  - "Schema 2 uses an explicit OrganizationV1/ProjectFileV1 migration contract"
  - "Shift duration is derived from start/end, including overnight wrap"
patterns-established:
  - "Validation lives on the entity that can see the whole graph (Organization)"
  - "`#[serde(default)]` on collections for forward-compatible v2 files"
duration: ~53min
started: 2026-11-17T12:03:50Z
completed: 2026-11-17T13:08:22Z
description: "Organization-owned staff/positions and roster-owned shifts with schema v2 and an explicit v1 migration"
type: Summary
about: "roster"
---

# Phase 2 Plan 01: Domain Model Foundation Summary

**The definition side of the domain now matches the corrected hierarchy: `Organization` owns positions and employees, rosters own shift templates, and schema v2 migrates Phase 1 files explicitly.**

## Performance

| Metric | Value |
|--------|-------|
| Duration | ~53min |
| Started | 2026-11-17T12:03:50Z |
| Completed | 2026-11-17T13:08:22Z |
| Tasks | 3 completed |
| Files modified | 11 |

## Acceptance Criteria Results

| Criterion | Status | Notes |
|-----------|--------|-------|
| AC-1: Organizations own isolated rosters | Pass | Distinct UUIDs; trimmed names; blank/duplicate rejected; rosters hold no staff |
| AC-2: Employees/positions are organization-owned and roster-isolated | Pass | `employees_in_roster` is the only read path; unknown roster/position rejected; skills dedupe |
| AC-3: Shifts live under rosters and require organization positions | Pass | 480/1020-minute durations; requirement upsert; headcount 0 and dangling references rejected |
| AC-4: Full schema v2 project round-trips losslessly | Pass | Store round trip plus JSON shape assertions |
| AC-5: Phase 1 files migrate; unknown schemas fail safely | Pass | v1 → v2 preserves ids/names/saved_at; v3 → `UnsupportedSchema(3)` |

## Accomplishments

- Replaced the Phase 1 organization-only shape with `Organization { positions, employees, rosters }` and `Roster { shifts }`, with validation centralized on `Organization`.
- Added overnight-aware shift durations (`15:00–08:00` = 1020 minutes) and staffing requirements that upsert per position.
- Shipped schema 2: an explicit `ProjectFileV1`/`OrganizationV1` migration contract, header-based dispatch in the store, and saves that always write the current version.
- Added 29 unit tests and a 2-test public-API integration fixture covering the whole model and persistence round trip.

## Task Commits

| Task | Commit | Type | Description |
|------|--------|------|-------------|
| Task 1: Organization-owned entities and roster-owned shifts | `1b8c4dc` | feat | Domain modules, validation, durations, staffing requirements |
| Task 2: Schema v2 and v1 migration | `9990e84` | feat | Migration contract, store dispatch, versioned saves |
| Task 3: End-to-end fixture and verification | `c6ad6d7` | test | `tests/phase2_domain.rs`, full build/test/clippy/fmt/launch pass |

Plan metadata commit follows this SUMMARY.

## Files Created/Modified

| File | Change | Purpose |
|------|--------|---------|
| `src/domain/error.rs` | Created | `DomainError` variants with human-readable `Display` |
| `src/domain/position.rs` | Created | Organization-level position catalog |
| `src/domain/employee.rs` | Created | Employee with `roster_id`, `position_id`, deduped skills |
| `src/domain/shift.rs` | Created | `ShiftTemplate` and `StaffingRequirement`, duration helpers |
| `src/domain/roster.rs` | Created | Roster owning shift templates |
| `src/domain/organization.rs` | Created | Root aggregate, lookups, validated add/require methods, `validate()` |
| `src/domain/migration.rs` | Created | `ProjectFileV1` → `ProjectFile` migration contract |
| `src/domain/project.rs` | Modified | `SCHEMA_VERSION = 2`, organization re-export kept |
| `src/domain/mod.rs` | Modified | Module declarations and re-exports |
| `src/store/mod.rs` | Modified | Schema-header dispatch, v1 migration, v2 round trip |
| `tests/phase2_domain.rs` | Created | Public-API fixture: two rosters, staff isolation, durations, persistence |

## JSON Shape (v2, populated)

```json
{
  "schema_version": 2,
  "app": "roster",
  "saved_at": "2026-11-17T12:56:23Z",
  "organizations": [
    {
      "id": "0e5a…",
      "name": "City Hospital",
      "positions": [
        { "id": "5c1b…", "name": "Medical Officer" },
        { "id": "8d02…", "name": "Senior Staff Nurse" }
      ],
      "employees": [
        {
          "id": "a71f…",
          "name": "Dr. Amit",
          "roster_id": "3f90…",
          "position_id": "5c1b…",
          "skills": ["ACLS"]
        }
      ],
      "rosters": [
        {
          "id": "3f90…",
          "name": "Doctors",
          "shifts": [
            {
              "id": "b2e4…",
              "name": "OPD",
              "start": "08:00:00",
              "end": "14:00:00",
              "required_positions": [
                { "position_id": "5c1b…", "headcount": 2 }
              ]
            }
          ]
        }
      ]
    }
  ]
}
```

Migration behavior: `schema_version: 1` is deserialized into the dedicated `ProjectFileV1` struct (organizations with `id`/`name` only), then converted to `ProjectFile` with `schema_version: 2` and empty `positions`/`employees`/`rosters`. Any other version returns `StoreError::UnsupportedSchema`. `save_project` clones and stamps the current version, so a migrated project is never written back as v1.

## Decisions Made

| Decision | Rationale | Impact |
|----------|-----------|--------|
| `Organization` is the validation root | Only it can see positions, employees, and rosters at once | 02-02 and 02-03 add rules there rather than on child types |
| Explicit v1 structs instead of serde defaults | The old file shape is a contract; defaults would silently reinterpret it | v2 can evolve without changing the v1 contract |
| Durations derived from times | Legacy duty-hour enums do not generalize to arbitrary rosters | 02-03 reuses the same helper for dated shifts |
| Skills are free-form strings | Requirements only cover positions in v0.1 | Skill-based matching stays out of scope |

## Deviations from Plan

### Summary

| Type | Count | Impact |
|------|-------|--------|
| Auto-fixed | 2 | Small, contained |
| Scope additions | 1 | Enables 02-02/02-03 validation |
| Deferred | 0 | — |

**Total impact:** No scope creep; additions are validation primitives the later plans need.

### Auto-fixed Issues

**1. [Validation] `Employee::add_skill` ignores duplicates**
- **Found during:** Task 1 (employee type)
- **Issue:** The plan said "rejects blank or duplicate"; erroring on duplicates makes idempotent callers awkward and the integration fixture expected a trimmed duplicate to be ignored.
- **Fix:** Trim, reject blank, ignore an existing label, keep insertion order.
- **Files:** `src/domain/employee.rs`
- **Verification:** `rejects_blank_skills`, `trims_skills_and_ignores_duplicates`, and the integration fixture.
- **Commit:** `1b8c4dc`

**2. [Formatting] `cargo fmt` applied**
- **Found during:** Task 3
- **Issue:** The first pass was not rustfmt-clean.
- **Fix:** Ran `cargo fmt`; no behavior change.
- **Verification:** `cargo fmt --check` exits 0.
- **Commit:** `c6ad6d7`

### Scope Additions

`DomainError::DuplicateId` and `DomainError::DanglingReference` were added in Task 1 because `Organization::validate()` needs them. 02-02 and 02-03 reuse these variants instead of adding new ones.

## Issues Encountered

| Issue | Resolution |
|-------|------------|
| Cargo 1.99 warns that `lints.cargo` is an unsupported manifest key | Pre-existing; no code change. Will be removed when Phase 4 starts using `pumpkin-solver` |
| Duplicate-shift test compared a full `DomainError` with a cloned name | Replaced with `matches!(…)`; duplicate names are trimmed before comparison |

## Next Phase Readiness

**Ready:**
- `Organization::{add_position, add_employee, add_roster, add_shift, require_position, validate}` for 02-02 to extend.
- `ShiftTemplate::duration_minutes` / `crosses_midnight` are reusable for 02-03's `DatedShift`.
- `store::load_project` / `save_project` already dispatch on `schema_version`, so 02-03 can add fields without touching the header logic.

**Concerns:**
- `shift.rs` and `roster.rs` currently import `ShiftTemplate` from `super`; 02-03 must avoid making `plan.rs` circular when `Roster.plans` and `DatedShift` reference each other.
- Every new v2 field must be `#[serde(default)]` so files written by this plan still load.

**Blockers:** None.

---
*Built with PAUL Framework v1.4*
*Phase: 02-domain-model, Plan: 01*
*Completed: 2026-11-17*

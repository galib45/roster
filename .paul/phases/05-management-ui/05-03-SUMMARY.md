---
phase: 05-management-ui
plan: 03
subsystem: ui
tags: [gpui, gpui-kit, crud, employees, shifts, weekdays, staffing-requirements, json-persistence]
requires:
  - phase: 05-management-ui
    plan: "02"
    provides: Integrated management shell, selected organization, Positions and Rosters CRUD
  - phase: 02-domain-model
    provides: Employee, ShiftTemplate, StaffingRequirement, schema v2
provides:
  - Employee add/edit dialog with position and roster selects and comma-separated skills
  - Shift-template list grouped by roster with times, weekday recurrence, and requirements
  - Shift add/edit dialog with roster select, time fields, seven weekday checkboxes, and dynamic requirement rows
  - Additive Organization::update_shift and Organization::set_shift_requirements with unit tests
  - End-to-end configuration round-trip test at schema version 2
affects: [phase-5-complete, phase-6-constraints]
tech-stack:
  added: []
  patterns:
    - "Selects are rebuilt from entity names on dialog open and mapped back to ids on save"
    - "Requirement rows are a Vec of draft entity pairs owned by RosterApp"
    - "A shift stays in its original roster while editing; the roster select is disabled"
key-files:
  created:
    - tests/management_ui.rs
    - .paul/phases/05-management-ui/05-03-SUMMARY.md
  modified:
    - src/app.rs
    - src/ui/employees.rs
    - src/ui/shifts.rs
    - src/domain/organization.rs
key-decisions:
  - "All seven weekdays normalize to the canonical empty list ('every day') on save"
  - "Duplicate positions in the requirements table are rejected in the UI before the domain call"
  - "Shift updates use update_shift + set_shift_weekdays + set_shift_requirements; no direct field mutation"
patterns-established:
  - "Dialog state for a new page is added to RosterApp beside existing management state"
  - "User-facing validation reports through RosterApp::status and leaves the dialog open"
duration: ~12min
started: 2026-11-19T00:53:09Z
completed: 2026-11-19T01:05:19Z
description: "Delivered Employee and Shift Template CRUD with validated selects, times, weekdays, requirements, and a schema-v2 round-trip test"
type: Summary
about: "roster"
---

> **SUPERSEDED (2026-11-19):** This applied implementation was not unified. Its modal dialogs and roster-membership model are replaced by 05-04 (domain re-baseline) and 05-05 (inline UI). Kept for history.

# Phase 5 Plan 03: Employees and Shift Templates Summary

**The management UI is complete: employees can be added and edited with position, roster, and skills, and shift templates can be added and edited with times, weekday recurrence, and multiple staffing requirements — all persisted at schema version 2.**

## Performance

| Metric | Value |
|--------|-------|
| Duration | ~12 minutes |
| Started | 2026-11-19T00:53:09Z |
| Completed | 2026-11-19T01:05:19Z |
| Tasks | 3 completed |
| Files modified | 4 source files + 1 new test + 3 PAUL state files |

## Acceptance Criteria Results

| Criterion | Status | Notes |
|-----------|--------|-------|
| AC-1: Employee add and edit | Pass | Add writes name, position, roster, and both parsed skills; edit re-clears and re-adds skills; success calls `save_project`. Interactive click-through deferred to manual smoke test. |
| AC-2: Employee validation and isolation | Pass | Blank name or missing position/roster sets `status` and returns before any mutation; the list only reads `selected_org()`. |
| AC-3: Shift add and edit | Pass | Add uses `add_shift` then `set_shift_weekdays` + `set_shift_requirements`; edit uses `update_shift` + the same setters; the roster select is disabled while editing. |
| AC-4: Shift validation | Pass | Blank name, equal times, no weekday, no rows, unparsed/zero headcount, and duplicate positions are blocked before any store call. |
| AC-5: Configuration round-trip | Pass | `tests/management_ui.rs` builds org → 2 positions → 2 rosters → skilled employee → shift with 5 weekdays and 2 requirements, then asserts equality, `validate()`, and `schema_version == 2`. |
| AC-6: Cancel discards dialog edits | Pass | Cancel only clears dialog state and draft ids; the stored entity and disk are untouched. |

## Accomplishments

- Closed the remaining Phase 5 UAT blockers: employees now carry a roster, position, and skills, and shifts carry times, weekday recurrence, and per-position headcounts.
- Added two focused domain helpers so the UI never mutates `ShiftTemplate` fields directly and all validation stays in `Organization`.
- Proved the complete configuration survives save/load losslessly, including ids, relationships, weekdays, and headcounts.
- Phase 5 is complete; the domain is ready for Phase 6 constraints and generation.

## Task Commits

No commits were created during APPLY; the worktree changes remain uncommitted for the phase transition commit.

| Task | Commit | Type | Description |
|------|--------|------|-------------|
| Task 1: Employee CRUD | — | feat | Employee list, add/edit dialog, position/roster selects, skills parsing |
| Task 2: Shift CRUD | — | feat | Shift list grouped by roster, add/edit dialog, times, weekdays, requirement rows, domain helpers |
| Task 3: Round-trip test | — | test | `tests/management_ui.rs` plus full quality gates |

Plan metadata: uncommitted (`.paul/phases/05-management-ui/05-03-SUMMARY.md`)

## Files Created/Modified

| File | Change | Purpose |
|------|--------|---------|
| `src/app.rs` | Modified | Owns employee/shift dialog state and `ShiftRequirementDraft` |
| `src/ui/employees.rs` | Rewritten | Employee list and validated add/edit dialog |
| `src/ui/shifts.rs` | Rewritten | Shift list grouped by roster and validated add/edit dialog |
| `src/domain/organization.rs` | Modified | Additive `update_shift` and `set_shift_requirements` with unit tests |
| `tests/management_ui.rs` | Created | Full configuration save/load round-trip at schema v2 |
| `.paul/STATE.md` | Modified | Apply position and decisions |
| `.paul/paul.toml` | Modified | Phase progress and loop position |
| `.paul/ledger.toml` | Modified | Apply entry |

## Decisions Made

| Decision | Rationale | Impact |
|----------|-----------|--------|
| All seven weekdays normalize to an empty list | The domain treats empty as "every day"; storing seven entries would be a second representation of the same fact | Editing an "every day" shift checks all seven boxes and saving returns to `[]` |
| Disable the roster select while editing | `update_shift` updates in place and does not move shifts between rosters | A shift's roster ownership is stable; moving requires delete/recreate, which is out of scope |
| Reject duplicate positions in the UI | `set_shift_requirements` collapses duplicates to the last value; the UI should not silently drop a row | Users see an explicit message instead of an unexpected merge |
| Keep `editing_shift_roster_id` in state | An edit needs the original roster even though the select is disabled | Save resolves the roster without re-reading a disabled select |

## Deviations from Plan

### Summary

| Type | Count | Impact |
|------|-------|--------|
| Auto-fixed | 1 | Unused import removed after the first build |
| Scope additions | 0 | None |
| Deferred | 1 | Pre-existing solver clippy findings |

**Total impact:** No scope creep; the only fix was required for a clean build.

### Auto-fixed Issues

**1. [Lint] Unused `TimeFieldState` import in `src/ui/shifts.rs`**
- **Found during:** Task 2 verification
- **Issue:** `TimeFieldState` was imported but the dialog only uses `TimeField` and the entity type indirectly.
- **Fix:** Narrowed the import to `time_field::TimeField`.
- **Files:** `src/ui/shifts.rs`
- **Verification:** `cargo build` reports no new warnings.
- **Commit:** uncommitted (part of Task 2)

### Deferred Items

- Pre-existing `src/solver` clippy findings (dead `Model`/`Objective` fields, unused `Model` methods, `collapsible_if` in `objective.rs`). Phase 5 forbids touching the solver. Clippy was checked for **no new findings** rather than a globally clean run. Logged in `.paul/STATE.md`.

## Issues Encountered

| Issue | Resolution |
|-------|------------|
| `cargo clippy --all-targets -- -D warnings` fails crate-wide | Confirmed the same four pre-existing `src/solver` findings remain; no new findings from this plan |
| The 10-second launch check rewrote `roster-project.json` in the repo root | The file is pre-existing user data loaded and re-saved by the app; its content was verified and preserved (untracked) |

## Next Phase Readiness

**Ready:**
- Employees, positions, rosters, and shift templates are all editable and persisted.
- The domain validates cross-references (roster membership, position existence, weekdays, requirements).
- A complete configuration round-trips losslessly at schema v2.

**Concerns:**
- Interactive GUI click-through was not possible in this environment; verify the Add/Edit/Cancel paths manually during UNIFY.
- Clippy cannot be green until the solver cleanup lands.

**Blockers:**
- None. Phase 5 is complete and the last plan triggers the phase transition.

---

*Phase: 05-management-ui, Plan: 03*
*Completed: 2026-11-19*

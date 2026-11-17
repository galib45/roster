---
phase: 02-domain-model
plan: 03
subsystem: domain
tags: [rust, serde, chrono, generated-plans, dated-shifts, slots, assignments]
requires:
  - phase: 02-domain-model
    provides: plan 02-01 — Organization, Employee, Roster, ShiftTemplate, schema v2
  - phase: 02-domain-model
    provides: plan 02-02 — availability/leave/recurring rules, rest rules
provides:
  - ShiftTemplate.weekdays and occurs_on weekday occurrence
  - RosterPlan, DatedShift, ShiftSlot, ContextAssignment, PlanStatus
  - Roster.plans with multiple saved periods
  - Organization plan generation, slot assignment, and plan validation
  - Plan persistence and pre-plan v2 compatibility
affects: [phase-3, phase-4, phase-6, phase-7]
tech-stack:
  added: []
  patterns:
    - "Generated plans snapshot template fields; template_id is provenance only"
    - "Headcount expands to one slot per seat so Phase 4 has one variable per seat"
    - "Structural assignment validation in the domain; scheduling evaluation deferred"
key-files:
  created:
    - src/domain/plan.rs
    - tests/plan_generation.rs
  modified:
    - src/domain/shift.rs
    - src/domain/roster.rs
    - src/domain/organization.rs
    - src/domain/error.rs
    - src/domain/mod.rs
    - src/store/mod.rs
key-decisions:
  - "RosterPlan holds dated shifts; roster owns the plan list"
  - "Slots are created per seat at generation time and never removed"
  - "Context assignments are supplied input, not inferred from a previous plan"
  - "Weekdays are deduped and sorted Monday-first for stable JSON"
patterns-established:
  - "Unfilled work is represented by explicit `employee_id: None` slots"
  - "New v2 fields are additive with `#[serde(default)]`"
duration: ~23min
started: 2026-11-17T13:02:00Z
completed: 2026-11-17T13:25:05Z
description: "Generated roster plans: dated shifts, per-seat slots, assignments, context, and multiple saved periods"
type: Summary
about: "roster"
---

# Phase 2 Plan 03: Generated Plans Summary

**Templates now become a concrete schedule: a `RosterPlan` for a date range with dated shifts, one slot per required seat, optional assignments, and previous-day context — while the plan stays a snapshot of the templates that produced it.**

## Performance

| Metric | Value |
|--------|-------|
| Duration | ~23min |
| Started | 2026-11-17T13:02:00Z |
| Completed | 2026-11-17T13:25:05Z |
| Tasks | 3 completed |
| Files modified | 8 |

## Acceptance Criteria Results

| Criterion | Status | Notes |
|-----------|--------|-------|
| AC-1: Templates instantiate into dated shifts with per-seat slots | Pass | November: 43 shifts / 56 slots; Fridays have no OPD; 1020-minute night |
| AC-2: Seats are assigned individually with structural validation | Pass | Second seat stays `None`; cross-roster, wrong-position, duplicate, finalized rejected |
| AC-3: Multiple plans, context, and persistence | Pass | Two plans round-trip; pre-plan v2 file loads with empty plans and daily templates |
| AC-4: Generated plans are snapshots | Pass | Editing template name/times/weekdays leaves an existing plan untouched |
| AC-5: Validation catches corrupt plan data | Pass | Ranges, ids, positions, employees, roster/position mismatches, context all caught |

## Accomplishments

- Added `ShiftTemplate.weekdays` (empty = every day) and `occurs_on`, plus a single shared `duration_minutes` helper used by templates and dated shifts.
- Added `src/domain/plan.rs` with `PlanStatus`, `ContextAssignment`, `ShiftSlot`, `DatedShift`, and `RosterPlan::generate`, including slot expansion, snapshotting, lookups, and fill counts.
- Added `Roster.plans` with plan lookup and crate-visible mutation, so a roster can keep November and December periods independently.
- Added `Organization::{set_shift_weekdays, generate_plan, plan, plan_mut, assign_slot, unassign_slot}`; validation now covers plan ranges, dated/slot/plan ids, slot positions, assignment roster/position/duplicates, and context employees/dates.
- Extended the store tests with a two-plan fixture, assignment persistence, and a pre-plan v2 load; added `tests/plan_generation.rs` for the public API.

## Task Commits

| Task | Commit | Type | Description |
|------|--------|------|-------------|
| Task 1: Weekday occurrence and the plan model | `aa26d89` | feat | Weekdays, `plan.rs`, roster plans, generation and lookups |
| Task 2: Seat assignment and plan validation | `aa26d89` | feat | `assign_slot`/`unassign_slot`, extended `validate()` |
| Task 3: Persistence and end-to-end coverage | `32308d7` | test | Store fixtures, November example, snapshot and round-trip tests |

Plan metadata commit follows this SUMMARY.

## Model Surface

```text
RosterPlan {
  id, start_date, end_date, status,
  context: Vec<ContextAssignment>,   // day before start
  shifts:  Vec<DatedShift>,
  created_at, updated_at
}

DatedShift {
  id, template_id,                   // template_id is provenance only
  date, name, start, end,            // name/start/end are snapshots
  slots: Vec<ShiftSlot>
}

ShiftSlot { id, position_id, employee_id: Option<String> }

ContextAssignment { date, name, start, end, employee_id }
PlanStatus { Draft, Generated, Finalized }
```

Organization entry points:

```text
set_shift_weekdays(roster_id, shift_id, weekdays: &[Weekday]) -> ()
generate_plan(roster_id, start, end, context) -> &RosterPlan
plan(roster_id, plan_id) -> Option<&RosterPlan>
plan_mut(roster_id, plan_id) -> Option<&mut RosterPlan>
assign_slot(roster_id, plan_id, dated_shift_id, slot_id, employee_id) -> ()
unassign_slot(roster_id, plan_id, dated_shift_id, slot_id) -> ()
```

## Validation Rules and Errors

| Rule | Error |
|------|-------|
| Plan `end_date` before `start_date` | `InvalidDateRange` |
| Unknown roster / plan / dated shift / slot / employee | `UnknownRoster` / `UnknownPlan` / `UnknownDatedShift` / `UnknownSlot` / `UnknownEmployee` |
| Employee not in the plan's roster | `EmployeeOutsideRoster` |
| Employee position differs from the slot position | `PositionMismatch` |
| Same employee in two seats of one dated shift | `EmployeeAlreadyAssigned` |
| Assignment change on a finalized plan | `PlanFinalized` |
| Context date on or after the plan start | `ContextDateOutOfRange` |
| Duplicate plan / dated-shift / slot ids | `DuplicateId { kind, id }` |
| Slot position or assigned employee does not resolve | `DanglingReference { kind, id }` |

## JSON Shape (plan excerpt)

```json
{
  "id": "…",
  "start_date": "2026-11-16",
  "end_date": "2026-11-30",
  "status": "draft",
  "context": [
    { "date": "2026-11-15", "name": "Emergency Night",
      "start": "15:00:00", "end": "08:00:00", "employee_id": "…" }
  ],
  "shifts": [
    { "id": "…", "template_id": "…", "date": "2026-11-16",
      "name": "OPD", "start": "08:00:00", "end": "14:00:00",
      "slots": [
        { "id": "…", "position_id": "…", "employee_id": "…" },
        { "id": "…", "position_id": "…", "employee_id": null }
      ] }
  ],
  "created_at": "2026-11-17T13:02:00Z",
  "updated_at": "2026-11-17T13:25:05Z"
}
```

## November 2026 Example

| Template | Rule | Dated shifts | Slots |
|----------|------|--------------|-------|
| OPD 08:00–14:00 | 2 Medical Officers, Mon–Thu + Sat–Sun | 13 | 26 |
| Emergency Morning 08:00–15:00 | 1 Medical Officer, daily | 15 | 15 |
| Emergency Night 15:00–08:00 | 1 Medical Officer, daily | 15 | 15 |
| **Total** | | **43** | **56** |

Fridays (2026-11-20, 2026-11-27) carry only the two emergency shifts.

## Decisions Made

| Decision | Rationale | Impact |
|----------|-----------|--------|
| Plans live on the roster, dated shifts inside the plan | A roster keeps several periods; plan-scoped ids stay unambiguous | Editor and solver address shifts as `(plan, dated_shift, slot)` |
| Template fields are copied, not referenced | History must not change when templates are edited | `template_id` is provenance only |
| Slots are materialized per seat | One decision variable per seat in Phase 4; explicit unfilled state for the UI | `employee_id: None` is meaningful, never removed |
| Context is supplied input | The domain cannot infer the previous schedule from an unrelated plan | `generate_plan` validates roster membership and date |
| Weekdays are normalized | Stable output and idempotent edits | Dedup + Monday-first sort in `set_shift_weekdays` |

## Deviations from Plan

### Summary

| Type | Count | Impact |
|------|-------|--------|
| Auto-fixed | 3 | Contained |
| Scope additions | 1 | One validation error variant |
| Deferred | 0 | — |

**Total impact:** No scope creep; the addition is required for context validation.

### Auto-fixed Issues

**1. [Compile] `Datelike` was missing for `date.weekday()`**
- **Found during:** Task 1
- **Issue:** `NaiveDate::weekday` is a `Datelike` method.
- **Fix:** Imported `chrono::Datelike` in `plan.rs`.
- **Verification:** `cargo test domain::` passed.
- **Commit:** `aa26d89`

**2. [Test] Unused `night` binding in the store fixture**
- **Found during:** Task 3 verification
- **Issue:** `cargo test` emitted an unused-variable warning; the fixture looks up the night shift by name instead.
- **Fix:** Removed the binding.
- **Verification:** `cargo test` is warning-free.
- **Commit:** `32308d7`

**3. [Clippy/Format] Final `cargo fmt` + `clippy -D warnings` pass**
- **Found during:** Task 3
- **Issue:** Test formatting and long expressions.
- **Fix:** `cargo fmt`; no behavior change.
- **Verification:** `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` clean.
- **Commit:** `aa26d89`, `32308d7`

### Scope Additions

`DomainError::ContextDateOutOfRange { date, plan_start }` was added so context dates before the plan start are enforced. The plan named the rule but not an error variant.

## Issues Encountered

| Issue | Resolution |
|-------|------------|
| Circularity risk between `Roster.plans` and `DatedShift` | `RosterPlan` does not contain `Roster`; only generation borrows it |
| Older v2 files lack `plans` and `weekdays` | `#[serde(default)]` on every new field, covered by a store test |
| Cargo warns that `lints.cargo` is unsupported | Pre-existing; Phase 4 will use it when adding `pumpkin-solver` |

## Next Phase Readiness

**Ready:**
- Phase 3 can evaluate dated assignments directly: every seat has a position and optional employee, and dated shifts carry snapshotted times.
- Rest rules can compare a context assignment with day-one dated shifts, and any two dated shifts in the plan.
- Phase 4 can encode one decision variable per `ShiftSlot`.
- Phase 5/7 editors can address `(roster, plan, dated shift, slot)` with stable ids.

**Concerns:**
- `PlanStatus::Generated`/`Finalized` transitions are manual; Phase 4 should set `Generated` after solving.
- Plan generation is O(days × templates); fine for monthly ranges.
- `Organization::validate()` is not yet called on every save; the store only serializes.

**Blockers:** None.

---
*Built with PAUL Framework v1.4*
*Phase: 02-domain-model, Plan: 03*
*Completed: 2026-11-17*

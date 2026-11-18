---
phase: 03-constraint-model
plan: 02
subsystem: eval
tags: [rust, fairness, workload, sharing-groups, validation, persistence]

requires:
  - phase: 03-constraint-model
    provides: plan 03-01 — evaluate, ViolationKind, soft penalties, dated/context assignments
  - phase: 02-domain-model
    provides: organization-owned employees, roster plans, dated shifts, seat slots

provides:
  - SharingGroup domain type on Roster with add/validate entry points
  - src/eval/fairness.rs: workload, WorkloadReport, per-employee and per-group minutes
  - Evaluation.workload — fairness measured alongside violations for live metrics and the solver
  - Roster sharing_groups in the v2 project file with an empty default

affects: [phase-4, phase-7]

tech-stack:
  added: []
  patterns:
    - "Fairness is one number: sum of |employee minutes − comparison mean|"
    - "Grouped employees compare with their group's average; ungrouped with the roster average"
    - "Only the plan's filled seats count; context and unfilled seats add no workload"

key-files:
  created:
    - src/domain/sharing.rs
    - src/eval/fairness.rs
    - tests/fairness.rs
  modified:
    - src/domain/error.rs
    - src/domain/mod.rs
    - src/domain/organization.rs
    - src/domain/roster.rs
    - src/eval/mod.rs
    - src/store/mod.rs

key-decisions:
  - "The roster average divides by every roster employee, including zero-workload staff"
  - "Fairness deviation is L1 (absolute deviations), which Phase 4 can linearize in Pumpkin"
  - "One sharing group per employee per roster keeps the comparison mean unambiguous"
  - "Previous-day context is excluded from workload because it lies outside the generated period"

patterns-established:
  - "WorkloadReport is computed from the roster's employees, never from the plan's assignments"
  - "Evaluation stays one read-only call: violations, coverage, and fairness together"

duration: ~18min
started: 2026-11-18T03:35:00Z
completed: 2026-11-18T03:52:45Z
description: "Sharing groups and a duration-weighted fairness report: per-employee minutes, group averages, and an L1 deviation attached to evaluation"
type: Summary
about: "roster"
---

# Phase 3 Plan 02: Sharing Groups and Fairness Summary

**A generated plan now yields a duration-weighted workload report — per-employee minutes, sharing-group averages, and one L1 fairness deviation — attached to the same read-only evaluation the solver and live editor will use.**

## Performance

| Metric | Value |
|--------|-------|
| Duration | ~18min |
| Started | 2026-11-18T03:35:00Z |
| Completed | 2026-11-18T03:52:45Z |
| Tasks | 3 completed |
| Files modified | 10 |

## Acceptance Criteria Results

| Criterion | Status | Notes |
|-----------|--------|-------|
| AC-1: Sharing groups are defined and isolated by roster | Pass | Add and `validate()` reject blank/duplicate names, empty membership, unknown/foreign/already-grouped members, and hand-edited duplicate membership or dangling ids |
| AC-2: Workload is measured from dated assignments | Pass | Filled seats add `DatedShift::duration_minutes()`; every roster employee appears, unfilled seats and context add zero; minutes reconcile with the roster total |
| AC-3: Sharing groups drive the fairness deviation | Pass | 720 + 1020 → mean 870, group deviation 300; a 1380/420 group → mean 900, deviation 960; ungrouped staff compare with the 540 roster mean; balanced plans score 0 |
| AC-4: Evaluation exposes workload alongside violations | Pass | `evaluation.workload` equals `workload(...)`; unfilled plans report zeros for every employee; two evaluations are identical |
| AC-5: Groups persist losslessly and remain compatible | Pass | The planned store fixture round-trips the Registrars group and its `employee_ids` JSON shape; older v2 files default to an empty list |

## Accomplishments

- Added `SharingGroup` with roster-scoped construction, `Organization::add_sharing_group`, `Roster::sharing_group_for_employee`, and full `Organization::validate` coverage for hand-edited files.
- Added `src/eval/fairness.rs` with `WorkloadReport`: per-employee minutes and shift counts, group totals/averages/deviations, roster total/average, min/max, and the overall L1 deviation.
- Attached the report to `Evaluation` without changing rule semantics or `evaluate`'s signature; dropped `Eq` (f64 fields) while keeping `PartialEq` for tests.
- Added 6 integration tests covering legacy duty lengths (360/420/1020), group vs roster means, balanced vs unbalanced plans, context exclusion, and evaluation parity.
- Extended store coverage so sharing groups round-trip and pre-sharing v2 files keep loading.

## Task Commits

Each task committed atomically:

| Task | Commit | Type | Description |
|------|--------|------|-------------|
| Plan refinement | `a44f128` | docs | Clarified roster-mean denominator and float policy |
| Task 1: Sharing groups | `22854ca` | feat | Domain type, add/validate entry points, rejection tests |
| Task 2: Workload and fairness | `cf70f22` | feat | `fairness.rs`, `Evaluation.workload`, unit tests |
| Task 3: End-to-end coverage | `77fbc20` | test | `tests/fairness.rs`, store round trip |
| Plan metadata | `acb04b4`, `8b242f1` | docs | APPLY state and the Windows pagefile note |

## Files Created/Modified

| File | Change | Purpose |
|------|--------|---------|
| `src/domain/sharing.rs` | Created | `SharingGroup` constructor rules and membership lookup |
| `src/eval/fairness.rs` | Created | `workload`, `WorkloadReport`, group and employee workload |
| `tests/fairness.rs` | Created | Public-API fairness coverage |
| `src/domain/error.rs` | Modified | `DuplicateSharingGroup`, `EmptySharingGroup`, `EmployeeAlreadyGrouped` |
| `src/domain/roster.rs` | Modified | `sharing_groups` field and lookups |
| `src/domain/organization.rs` | Modified | `add_sharing_group` and sharing-group validation |
| `src/domain/mod.rs` | Modified | Module registration and re-export |
| `src/eval/mod.rs` | Modified | Re-exports, `Evaluation.workload`, `Eq` drop |
| `src/store/mod.rs` | Modified | Group persistence assertions and older-v2 default |

## Decisions Made

| Decision | Rationale | Impact |
|----------|-----------|--------|
| Roster average divides by every roster employee | Hiring someone with no shifts must show up as unfairness, not disappear from the denominator | Zero-workload staff are visible in the fairness score |
| Fairness is an L1 deviation | Sums of absolute deviations linearize with one auxiliary variable per employee | Phase 4 can express the same measure as a Pumpkin objective |
| One group per employee | Two groups would make the comparison mean ambiguous | `add_sharing_group` and `validate()` both enforce exclusivity |
| Workload excludes context | Context is outside the generated period and was supplied by the admin | Fairness reflects only the plan being built |

## Deviations from Plan

### Summary

| Type | Count | Impact |
|------|-------|--------|
| Auto-fixed | 0 | — |
| Scope additions | 1 | Stronger integration fixture |
| Deferred | 0 | — |

**Total impact:** No scope creep; coverage is slightly broader than planned.

### Scope Additions

**1. Integration fixture uses four employees instead of three**
- **Found during:** Task 3
- **Reason:** AC-3 requires an *ungrouped* employee compared with the roster average; three employees across two groups leaves none ungrouped.
- **Change:** Added a fourth doctor with no assignments (and no group).
- **Verification:** `cargo test --test fairness` — 6 tests pass.

### Deferred Items

None.

## Issues Encountered

| Issue | Resolution |
|-------|------------|
| Parallel `cargo test` failed with Windows pagefile error 1455 while linking several test binaries | Ran `cargo test -j 1`; recorded as a deferred environment issue in STATE.md |
| `Evaluation` could not keep `Eq` after adding f64 workload fields | Dropped `Eq`, kept `Debug/Clone/PartialEq`; tests use tolerance helpers |

## Next Phase Readiness

**Ready:**
- Phase 4 has one agreed fairness target (`WorkloadReport.deviation_minutes`) and one penalty surface (`Evaluation::soft_penalty` + `ViolationKind`).
- Workload is read-only and deterministic, so solve → evaluate → display uses the same numbers.
- Sharing groups and workload survive save/load with no schema bump.

**Concerns:**
- The deviation is measured in raw minutes, so a 1020-minute night outweighs three 360-minute OPDs; Phase 4 may need per-shift weights or normalized hours.
- Group means can hide individuals: a two-person group with 0 and 1800 minutes reports zero group deviation.
- Hard rest rules can still make a plan infeasible before fairness matters; Phase 4 must report that clearly.

**Blockers:** None.

---
*Built with PAUL Framework v1.4*
*Phase: 03-constraint-model, Plan: 02*
*Completed: 2026-11-18*

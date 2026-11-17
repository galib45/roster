---
phase: 03-constraint-model
plan: 01
subsystem: eval
tags: [rust, evaluation, constraints, unavailability, leave, rest, penalties]
requires:
  - phase: 02-domain-model
    provides: plan 02-02 — unavailability, leave, recurring assignments, rest rules
  - phase: 02-domain-model
    provides: plan 02-03 — RosterPlan, DatedShift, ShiftSlot, ContextAssignment
provides:
  - src/eval module: evaluate, Evaluation, Violation, ViolationKind, ShiftRef, EvalOptions
  - recurring assignments as soft, tunable preferences suppressed by hard leave
  - context template references for rest and unavailability matching
  - overlap, unavailability/leave, rest, and multi-shift evaluation with coverage counts
affects: [phase-4, phase-7]
tech-stack:
  added: []
  patterns:
    - "Evaluation is pure: reads the project, never repairs it, and is deterministic"
    - "Severity and penalty come from stored rule data; the evaluator holds no policy"
    - "Recurring assignments are preferences: unmet ones cost a penalty, never feasibility"
key-files:
  created:
    - src/eval/mod.rs
    - src/eval/rules.rs
    - tests/constraint_eval.rs
  modified:
    - src/lib.rs
    - src/domain/constraint.rs
    - src/domain/plan.rs
    - src/domain/organization.rs
    - src/store/mod.rs
key-decisions:
  - "Recurring assignments are soft preferences with a tunable penalty (default 1)"
  - "An unmet preference is skipped when a hard leave/unavailability rule covers the date"
  - "ContextAssignment.template_id is optional so older files match only whole-day rules"
  - "Unfilled slots are reported as coverage, never as violations"
patterns-established:
  - "One evaluator for both the CP model and the live editor"
  - "Violations sorted by employee, kind, and reference for stable UI output"
duration: ~16min
started: 2026-11-17T13:38:00Z
completed: 2026-11-17T13:54:25Z
description: "Pure constraint evaluation: overlap, unavailability, leave, recurring preferences, rest, and soft penalties over generated plans"
type: Summary
about: "roster"
---

# Phase 3 Plan 01: Constraint Evaluation Summary

**A read-only evaluator now turns a generated plan into hard violations and weighted soft penalties — the single rule semantics that the Pumpkin model (Phase 4) and the live editor (Phase 7) will share.**

## Performance

| Metric | Value |
|--------|-------|
| Duration | ~16min |
| Started | 2026-11-17T13:38:00Z |
| Completed | 2026-11-17T13:54:25Z |
| Tasks | 3 completed |
| Files modified | 8 |

## Acceptance Criteria Results

| Criterion | Status | Notes |
|-----------|--------|-------|
| AC-1: Overlap and context are evaluated | Pass | Same-day OPD + Morning is one Hard overlap; touching shifts are clean; context night overlapping day one is reported |
| AC-2: Unavailability, leave, and recurring preferences are evaluated | Pass | Weekly Friday and a leave range produce Hard `Unavailable`; a soft Saturday OPD rule costs 2; empty shift lists match everything; unmet preferences are Soft and leave-suppressed |
| AC-3: Rest, multi-shift days, and penalty totals are evaluated | Pass | Night→morning at a 0-minute gap is `InsufficientRest` (penalty 3); OPD + night on one date is one Soft multi-shift; `soft_penalty()` sums only soft rules |
| AC-4: Context can be matched to templates | Pass | `template_id` round-trips through the store, resolves on generate/validate, and defaults to `None` for older files |
| AC-5: Evaluation is read-only and covers unfilled work | Pass | Two runs are equal and the project is unchanged; 56 slots / 56 unfilled on the clean November plan; unknown ids return `UnknownRoster`/`UnknownPlan` |

## Accomplishments

- Added `src/eval/` with `evaluate`, `EvalOptions`, `ShiftRef`, `ViolationKind`, `Violation`, and `Evaluation` (`hard_count`, `soft_penalty`, `is_feasible`).
- Collected filled slots and context into concrete intervals, wrapping overnight shifts onto the next day and treating touching intervals as non-overlapping.
- Implemented the rule set: overlap (including context), weekly/date/leave unavailability, unmet recurring preferences, ordered rest pairs, and multi-shift days.
- Made recurring assignments soft preferences with a tunable `penalty` (default 1); a hard leave/unavailability rule suppresses the preference so fairness outranks habit.
- Added optional `ContextAssignment.template_id`, validated on generation and load, so rest and unavailability rules can match the previous day.
- Added 14 integration tests over the November 2026 example plus unit tests for intervals, options, and penalty totals.

## Task Commits

| Task | Commit | Type | Description |
|------|--------|------|-------------|
| Task 1: Evaluation module, overlap, coverage | `53a2083` | feat | `src/eval` public API, interval collection, overlap checks |
| Task 2: Unavailability, preferences, rest, multi-shift | `3b30726` | feat | Domain additions plus the full rule set; store fixture |
| Task 3: End-to-end coverage | `ae562e4` | test | `tests/constraint_eval.rs` |

Plan metadata commit follows this SUMMARY.

## Evaluation Surface

```text
evaluate(org, roster_id, plan_id, &EvalOptions) -> Result<Evaluation, DomainError>

EvalOptions    { multi_shift_penalty: u32 }          // default 1; 0 disables
Evaluation     { violations, total_slots, unfilled_slots }
Evaluation::{hard_count, soft_penalty, is_feasible}

Violation      { severity, penalty, kind }
ViolationKind
├── Overlap { employee_id, first, second }
├── Unavailable { employee_id, rule_id, shift }
├── UnmetRecurringPreference { employee_id, assignment_id, date, template_id }
├── InsufficientRest { employee_id, rule_id, preceding, following, gap_minutes }
└── MultipleShiftsInDay { employee_id, date, shift_count }

ShiftRef { Dated { id } | Context { date, name } }
```

## Rule Semantics

| Rule | Severity | Penalty | Detail |
|------|----------|---------|--------|
| Overlapping assignments | Hard | 0 | Per employee; context included; touching intervals are clean |
| Assignment in an unavailability/leave scope | Rule's | Rule's | Weekly weekday, exact date, inclusive leave; empty shift list = every shift |
| Unmet recurring preference | Soft | `RecurringAssignment.penalty` | Never infeasible; skipped when a hard leave/unavailability covers the date |
| Rest below `min_rest_minutes` | Rule's | Rule's | Ordered pairs; gap from the earlier end to the later start; negative gaps are overlaps |
| More than one shift starting the same date | Soft | `multi_shift_penalty × (count − 1)` | Disabled at 0 |

## Domain Additions

- `RecurringAssignment.penalty: u32` with `#[serde(default = "…")]` = 1, `RecurringAssignment::with_penalty`, and `Organization::add_recurring_assignment_with_penalty`; `validate()` rejects a stored 0.
- `ContextAssignment.template_id: Option<String>` with `#[serde(default)]`; `generate_plan` and `validate()` reject templates outside the owning roster.
- `Organization::add_recurring_assignment` keeps its signature and delegates with penalty 1.

## Decisions Made

| Decision | Rationale | Impact |
|----------|-----------|--------|
| Recurring assignments are preferences | Mandating them overloads staff who take no leave when the pool shrinks | Unmet preferences cost a penalty; `is_feasible()` ignores them |
| Suppress preferences on hard leave | Leaving staff should not be effectively penalized for absence | Fairness can pick another doctor |
| Context template ids are optional | Older JSON and admin-supplied context must keep working | `None` context matches only whole-day rules |
| Coverage stays out of violations | A fair solution may legitimately leave seats empty | `unfilled_slots` is reported separately |

## Deviations from Plan

### Summary

| Type | Count | Impact |
|------|-------|--------|
| Auto-fixed | 2 | Contained |
| Scope additions | 0 | — |
| Deferred | 0 | — |

**Total impact:** No scope creep.

### Auto-fixed Issues

**1. [Compile] Task 1 could not use the future `template_id`/`date` fields**
- **Found during:** Task 1 verification
- **Issue:** Clippy's `dead_code` rejected `Assignment` fields that only Task 2 reads.
- **Fix:** Temporarily allowed the two fields, removed with the Task 2 rewrite.
- **Verification:** `cargo clippy --all-targets -- -D warnings` clean at both commits (`53a2083`, `3b30726`).

**2. [Test] Context overlap fixture only touched the next shift**
- **Found during:** Task 3
- **Issue:** A context night ending at 08:00 and a morning starting at 08:00 do not overlap, so the test expected a violation that correctly did not exist.
- **Fix:** Moved the context interval to 20:00–09:00 so it genuinely intersects day one.
- **Verification:** `cargo test --test constraint_eval` passes.

## Issues Encountered

| Issue | Resolution |
|-------|------------|
| Recurring preferences must be per date, not per assignment | Evaluated from the roster's employees against each matching dated shift |
| Overnight end times equal to the start mean 24 hours | Reused the `duration_minutes` convention: end ≤ start rolls to the next day |

## Next Phase Readiness

**Ready:**
- Phase 4 can encode one decision variable per seat and reuse `ViolationKind` as the penalty map.
- Phase 7 can call `evaluate` after each edit; results are deterministic and read-only.
- 03-02 can attach `WorkloadReport` to `Evaluation` without touching rule semantics.

**Concerns:**
- Rest rules compare every rule-matched pair; a rule whose templates overlap by design (night → night) will always report a gap, so configurations matter.
- Hard rest rules can legitimately make a generated plan infeasible before assignment review; the UI should surface them as blockers.

**Blockers:** None.

---
*Built with PAUL Framework v1.4*
*Phase: 03-constraint-model, Plan: 01*
*Completed: 2026-11-17*

---
phase: 02-domain-model
plan: 02
subsystem: domain
tags: [rust, serde, chrono, constraints, availability, rest-rules, legacy-parity]
requires:
  - phase: 02-domain-model
    provides: plan 02-01 — Organization, Employee, Roster, ShiftTemplate, schema v2
provides:
  - Severity, UnavailabilityScope, Unavailability, RecurringAssignment, RestRule
  - Employee.unavailability and Employee.recurring_assignments
  - Roster.rest_rules
  - Organization rule APIs with roster-scoped shift validation
  - Legacy doctor parity fixture
affects: [02-03, phase-3, phase-4, phase-6]
tech-stack:
  added: []
  patterns:
    - "Rule constructors validate local invariants; Organization validates cross-entity references"
    - "Internal enum tagging (`kind`) for scope variants; snake_case enum values"
key-files:
  created:
    - src/domain/constraint.rs
    - tests/legacy_parity.rs
  modified:
    - src/domain/employee.rs
    - src/domain/roster.rs
    - src/domain/organization.rs
    - src/domain/error.rs
    - src/domain/mod.rs
    - tests/phase2_domain.rs
key-decisions:
  - "Whole-day unavailability is an empty shift list; category rules list specific shift ids"
  - "Leave is an inclusive date range covering every shift"
  - "Rest rules are data only; evaluation belongs to Phase 3"
  - "Soft rules require penalty >= 1; hard rules require penalty 0"
patterns-established:
  - "Every rule id is validated unique within its owner"
  - "Cross-roster references fail with ShiftOutsideRoster"
duration: ~26min
started: 2026-11-17T12:50:00Z
completed: 2026-11-17T13:16:22Z
description: "Employee availability/leave/recurring assignments and roster-level configurable rest rules, with legacy doctor parity"
type: Summary
about: "roster"
---

# Phase 2 Plan 02: Constraint Data Summary

**The domain now carries every scheduling rule the prototype encoded — weekly/date/leave unavailability, recurring assignments, and generalized rest rules — without doctor-specific types.**

## Performance

| Metric | Value |
|--------|-------|
| Duration | ~26min |
| Started | 2026-11-17T12:50:00Z |
| Completed | 2026-11-17T13:16:22Z |
| Tasks | 3 completed |
| Files modified | 7 |

## Acceptance Criteria Results

| Criterion | Status | Notes |
|-----------|--------|-------|
| AC-1: Unavailability covers both legacy dimensions and severity | Pass | Weekly/Date/Leave scopes; empty list = whole day; soft/hard pairing enforced |
| AC-2: Recurring assignments and rest rules are validated | Pass | Employee/roster/shift references and zero rest gaps rejected |
| AC-3: Full projects with rule data round-trip | Pass | `phase2_domain` and `legacy_parity` save/load equality |
| AC-4: Legacy doctor scenario maps to the generalized model | Pass | 5 weekly, 1 date, 2 leave, 13 recurring; 360/420/1020 minutes |
| AC-5: Reference validation catches hand-edited corruption | Pass | Out-of-roster shifts, dangling refs, duplicate rule ids rejected |

## Accomplishments

- Added `constraint.rs` with `Severity`, `UnavailabilityScope` (weekly/date/leave), `Unavailability`, `RecurringAssignment`, and `RestRule`.
- Extended `Employee` with unavailability and recurring assignments, and `Roster` with rest rules, all `#[serde(default)]`.
- Added `Organization::{add_unavailability, add_recurring_assignment, add_rest_rule}` — the only path that can enforce employee ↔ roster ↔ shift membership.
- Extended `Organization::validate()` to cover rule ids and shift references inside the correct roster.
- Mirrored `legacy/utils.rs` as an executable parity fixture and kept every copied rule hard, while showing the night→morning rule can be soft with penalty 3.

## Task Commits

| Task | Commit | Type | Description |
|------|--------|------|-------------|
| Task 1: Constraint types + organization APIs | `e2771bc` | feat | Rule types, validating entry points, extended `validate()` |
| Task 2: Persistence + legacy parity | `154adba` | test | Rule-rich round trip, JSON encoding assertions, parity fixture |
| Task 3: Final verification | — | — | build/test/clippy/fmt/run all clean |

Plan metadata commit follows this SUMMARY.

## Constraint Type and API Surface

```text
Severity { Hard, Soft }

UnavailabilityScope (internally tagged by "kind")
├── Weekly { weekday: Weekday }
├── Date { date: NaiveDate }
└── Leave { start: NaiveDate, end: NaiveDate }

Unavailability {
  id, scope, shifts: Vec<String>, severity, penalty: u32
}
RecurringAssignment { id, weekday, shift_id }
RestRule { id, preceding_shift_id, following_shift_id,
           min_rest_minutes: u32, severity, penalty: u32 }
```

Organization entry points:

```text
add_unavailability(employee_id, scope, shifts: &[&str], severity, penalty) -> &Unavailability
add_recurring_assignment(employee_id, weekday, shift_id) -> &RecurringAssignment
add_rest_rule(roster_id, preceding, following, min_rest_minutes, severity, penalty) -> &RestRule
```

## JSON Encoding

| Concept | Encoding |
|---------|----------|
| `Severity` | `"hard"` / `"soft"` (snake_case) |
| `UnavailabilityScope` | `{"kind": "weekly" \| "date" \| "leave", ...}` |
| Weekday | chrono serde: `"Mon"`, `"Tue"`, … `"Sun"` |
| Date | `"YYYY-MM-DD"` |
| Whole-day | `"shifts": []` |
| Shift-scoped | `"shifts": ["<shift-id>"]` |
| Penalty | integer; 0 for hard rules |

Example:

```json
{
  "id": "6f1c…",
  "scope": { "kind": "weekly", "weekday": "Sat" },
  "shifts": ["b2e4…"],
  "severity": "soft",
  "penalty": 2
}
```

## Validation Rules and Errors

| Rule | Error |
|------|-------|
| Leave ends before it starts | `InvalidDateRange { start, end }` |
| Soft rule with penalty 0, hard rule with penalty > 0 | `InvalidPenalty { severity, penalty }` |
| Rest gap of 0 minutes | `InvalidRestGap { preceding_shift_id, following_shift_id }` |
| Shift id not in the owning roster | `ShiftOutsideRoster { shift_id, roster_id }` |
| Unknown employee/roster | `UnknownEmployee` / `UnknownRoster` |
| Duplicate rule ids in one owner | `DuplicateId { kind, id }` |
| Dangling employee roster/position | `DanglingReference { kind, id }` |

## Legacy Parity Mapping

| Legacy construct | Domain construct |
|------------------|------------------|
| `Unavailability::WholeDay` (weekly) | `UnavailabilityScope::Weekly` + empty `shifts` |
| `Unavailability::Category(shift)` (weekly) | `UnavailabilityScope::Weekly` + `shifts = [id]` |
| `add_unavailability_on(date, …)` | `UnavailabilityScope::Date` (+ optional `shifts`) |
| `add_leave_period(start, end)` | `UnavailabilityScope::Leave` |
| `add_recurring_assignment(weekday, category)` | `RecurringAssignment { weekday, shift_id }` |
| `next_em` / `prev_en` rest checks | `RestRule` between two shift ids; evaluator is Phase 3 |
| Duty hours 6/7/17 | `duration_minutes()` on 08–14 / 08–15 / 15–08 |

Parity totals: 7 employees; weekly 5, date 1, leave 2, recurring 13 (Toukir 1, everyone else 2).

## Decisions Made

| Decision | Rationale | Impact |
|----------|-----------|--------|
| Rest rules live on `Roster` | They compare two of that roster's shifts; an employee cannot own them | Phase 3 evaluates per roster |
| Rule ids must be unique within the owning employee/roster | Stable ids for edit/delete and JSON diffing | `validate()` rejects duplicates |
| `penalty` is `u32` with exact severity pairing | Removes ambiguous "soft but free" rules | Phase 3 can sum penalties directly |
| No evaluation predicates in the domain | Keeps Phase 2 pure and testable | Phase 3 owns violation/scoring semantics |

## Deviations from Plan

### Summary

| Type | Count | Impact |
|------|-------|--------|
| Auto-fixed | 2 | No behavior change |
| Scope additions | 0 | — |
| Deferred | 0 | — |

**Total impact:** None; plan executed as written.

### Auto-fixed Issues

**1. [Clippy] Collapsed a nested `if let` in `Unavailability::new`**
- **Found during:** Task 3 verification
- **Issue:** `clippy::collapsible_if` failed under `-D warnings`.
- **Fix:** Used let-chaining (`if let … && end < start`).
- **Files:** `src/domain/constraint.rs`
- **Verification:** `cargo clippy --all-targets -- -D warnings` clean.
- **Commit:** `154adba`

**2. [Formatting] `cargo fmt`**
- **Found during:** Task 3
- **Issue:** `error.rs` long match arms were not rustfmt-clean.
- **Fix:** Ran `cargo fmt`; no behavior change.
- **Verification:** `cargo fmt --check` exits 0.
- **Commit:** `154adba`

## Issues Encountered

| Issue | Resolution |
|-------|------------|
| Test helper carried an unused roster id field | Removed it instead of suppressing `dead_code` |
| `validate()` needed to reuse one id set across unavailability and recurring assignments | One `rule_ids` set per employee, as specified |

## Next Phase Readiness

**Ready:**
- Phase 3 can evaluate every rule directly: each rule has a scope, a severity/penalty, and roster-validated shift ids.
- Rest evaluation can compare any two dated shifts from the same roster.
- Recurring assignments and leave expose exactly the data the solver needs as hard demand/restrictions.

**Concerns:**
- Phase 3 still needs previous-day context and sharing groups; neither exists yet.
- 02-03 will add `Roster.plans`; keep `constraint.rs` free of plan dependencies to avoid cycles.

**Blockers:** None.

---
*Built with PAUL Framework v1.4*
*Phase: 02-domain-model, Plan: 02*
*Completed: 2026-11-17*

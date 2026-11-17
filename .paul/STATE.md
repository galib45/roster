---
description: "roster — current position and accumulated context"
type: ProjectState
about: "roster"
---

# Project State

## Project Reference

See: .paul/PROJECT.md (updated 2026-11-17)

**Core value:** Admins get fair, conflict-free duty rosters for each staff group automatically — no double-booking, no leave/rest violations, equitable workload.
**Current focus:** v0.1 Application Core — Phase 3 Constraint Model

## Current Position

Milestone: v0.1 Application Core
Phase: 3 of 8 (Constraint Model) — Planning
Plan: 03-01 and 03-02 created, awaiting approval
Status: PLAN created, ready for APPLY
Last activity: 2026-11-17T13:33:29Z — Created Phase 3 plans (03-01, 03-02)

Progress:
- Milestone: [██░░░░░░░░] 25% (2 of 8 phases)
- Phase 3: [░░░░░░░░░░] 0%

## Loop Position

Current loop state:
```
PLAN ──▶ APPLY ──▶ UNIFY
  ✓        ○        ○     [Plan created, awaiting approval]
```

## Accumulated Context

### Decisions

| Decision | Phase | Impact |
|----------|-------|--------|
| GPUI + gpui-kit as the desktop stack | Init | `gpui-kit` re-exports GPUI; it resolves `gpui-pre`, not crates.io `gpui` |
| Pumpkin solver for CP + fairness objective | Init | Constraint semantics must be reusable outside the solver for live metrics |
| JSON project files for persistence | Init | Versioned `schema_version` from the first commit |
| Rosters per staff group with isolated employee pools | Init | Domain model: org → rosters → employees/shifts |
| Shift templates declare required positions + headcount | Init | Solver coverage demand model |
| PDF/PNG export deferred out of v1 | Init | Export is Phase 8 |
| Local dev first, packaging later | Init | No CI/packaging work yet |
| Single `gpui-kit` dependency, no direct `gpui`/`gpui-pre` | Phase 1 | Avoids two incompatible GPUI versions in one binary |
| Constraint evaluation separated from search | Phase 1 planning | Enables live scoring in Phase 7 without re-solving |
| Android considered, not committed — desktop GPUI stays | Phase 1 | An Android pivot swaps only the UI layer; domain/store/solver are unaffected |
| Navigation uses `gpui_kit::component::sidebar` | Phase 1 apply | Sidebar groups, icons, and active state come from the kit instead of hand-rolled buttons; new screens only add a `Page` variant. Icons are limited to the default asset bundle (`assets::Assets`) |
| Project schema 2 with an explicit v1 migration | Phase 2 planning | Organizations gain organization-level positions/employees plus rosters; store accepts v1 (organizations only → empty collections) and writes v2. `SCHEMA_VERSION = 2`; v1 is never silently reinterpreted through v2 `serde` defaults |
| Positions and employees are organization-owned; rosters own shifts | Phase 2 planning | `Organization` owns `positions` and `employees`; `Employee.roster_id` is the staff-isolation boundary; `Roster` owns only `ShiftTemplate`s; shift requirements reference organization-level positions. Skills remain free-form employee labels in v0.1 |
| Availability is shift-reference based, not duty-category based | Phase 2 planning | Legacy whole-day/category scopes become an empty or populated `shifts` list on `Unavailability`; shift ids must resolve inside the employee's roster; rest rules live on a roster and reference any pair of that roster's shifts with configurable severity + penalty |
| A generated roster is a plan of dated shifts with per-seat slots | Phase 2 planning | `Roster.plans: Vec<RosterPlan>`; `RosterPlan` covers a date range, holds `DatedShift` occurrences and previous-day `ContextAssignment`s; each `StaffingRequirement.headcount` becomes that many `ShiftSlot`s; `employee_id: Option<String>` stays `None` when unfilled. One decision variable per seat for Phase 4 |
| Templates recur by weekday; plans snapshot times | Phase 2 planning | `ShiftTemplate.weekdays` (empty = every day) expands templates into dated shifts; `DatedShift` snapshots `name`/`start`/`end` and keeps `template_id` as provenance only, so editing a template never rewrites an existing plan |
| A roster keeps multiple generated periods | Phase 2 planning | November and December plans coexist with their own ids, ranges, and `PlanStatus` (Draft/Generated/Finalized); finalized plans reject assignment changes |
| Assignments are structurally validated, not scheduled | Phase 2 planning | Domain enforces roster membership, position match, and one employee per dated shift; overlap/leave/rest/fairness evaluation stays in Phase 3 |
| Generated plans are materialized and snapshot-based | Phase 2 apply | `RosterPlan::generate` expands weekday-filtered templates into dated shifts with one slot per seat; templates are copied, not referenced, and `template_id` is provenance only |
| Recurring assignments are soft preferences | Phase 3 planning | `RecurringAssignment` carries a tunable penalty and never makes a plan infeasible; unmet preferences are skipped when a hard leave/unavailability rule covers the date, so fairness outranks habit when staffing is tight |
| Unfilled work is explicit, never implicit | Phase 2 apply | Slots are created at generation time and remain even when unfilled (`employee_id: None`); `filled_slots()`/`total_slots()` expose coverage without scheduling logic |
| Context is admin-supplied input | Phase 2 apply | `ContextAssignment` is validated for roster membership and dates strictly before the plan start; the domain never infers it from another plan |

### Deferred Issues

| Issue | Origin | Effort | Revisit |
|-------|--------|--------|---------|
| PDF/PNG export | Init | M | Phase 8 |
| Native file dialogs for open/save | Phase 1 | S | Phase 5 (management UI) |
| Sidebar is fixed-width (no collapse toggle) | Phase 1 apply | S | Phase 5/6 if the roster grid needs more width |
| Flattening the repo's original CLI prototype | Phase 1 | S | After Phase 4 (legacy stays frozen as the solver reference until then) |
| Windows build: rust-analyzer can hold proc-macro DLLs in `target/` (LNK1104) | Phase 1 | S | Kill `rust-analyzer-proc-macro-srv.exe` or close Zed before a CLI build |

### Git State

Last phase commit: `32308d7` — test(02-03): cover plan generation, snapshots, and multi-plan persistence
Branch: main
Feature branches merged: none

### Blockers/Concerns

| Blocker | Impact | Resolution Path |
|---------|--------|-----------------|
| Fairness as a CP objective in Pumpkin | Core differentiator; may need auxiliary deviation variables | Phase 4 research spike on a small roster |
| Scope size (8 phases) | Risk of never shipping | Each phase ends with a runnable app; no phase exceeds 3 plans |
| `pumpkin-solver` pinned but unused | `unused_dependencies` lint is silenced | Remove `[lints.cargo]` allowance when Phase 4 calls the solver |
| `PlanStatus` transitions are manual | Nothing sets `Generated` after solving | Phase 4 sets it; Phase 3 only evaluates |

## Boundaries (Active)

Phase 3 (from ROADMAP.md):
- `legacy/*` frozen — do not edit or delete the prototype
- Constraint evaluation only; no solver calls yet (Phase 4)
- Evaluation must be pure over domain data: no mutation, no persistence side effects
- Hard vs soft severity comes from the rule data (`Severity` + `penalty`); do not hardcode rule policy in the evaluator
- Recurring assignments are soft preferences only — never promote them to hard demand
- Evaluate dated plans and context; avoid new schema fields unless unavoidable
- No new dependencies without stopping to ask

## Session Continuity

Last session: 2026-11-17
Stopped at: Phase 3 plans created (03-01 rules evaluation, 03-02 sharing groups/fairness)
Next action: Review and approve 03-01, then run /paul:apply .paul/phases/03-constraint-model/03-01-PLAN.md
Resume file: .paul/phases/03-constraint-model/03-01-PLAN.md

---
*STATE.md — Updated after every significant action*

---
description: "roster — milestone and phase structure"
type: Roadmap
about: "roster"
---

# Roadmap: roster

## Overview

Grow the working CLI prototype into a GPUI desktop application. Phase 1 proves the UI stack and persistence with a walking skeleton; Phase 2 ports the prototype's semantics into a real domain model; Phase 3 extracts constraint evaluation from the solver so it can be reused for live scoring; Phase 4 replaces hand-rolled backtracking with a Pumpkin CP model that maximizes fairness. From Phase 5 onward, work is delivered as **vertical slices**: each plan adds the domain logic and the UI for one feature together and ends runnable. The project file has a single schema version with **no migrations until the MVP is complete**. Phase 6 adds per-employee constraints and the generate action; Phase 7 adds live manual editing, fairness/constraint feedback, and finalization. PDF/PNG export ships last, Phase 8.

## Current Milestone

**v0.1 Application Core** (v0.1.0)
Status: In progress
Phases: 4 of 8 complete (50%); Phase 5 re-baselining as vertical slices

## Phases

| Phase | Name | Plans | Status | Completed |
|-------|------|-------|--------|-----------|
| 1 | Walking Skeleton | 1 | ✅ Complete | 2026-11-16 |
| 2 | Domain Model | 3 | ✅ Complete | 2026-11-17 |
| 3 | Constraint Model | 2 | ✅ Complete | 2026-11-18 |
| 4 | Solver Integration | 2 | ✅ Complete | 2026-11-18 |
| 5 | Management UI | 6 | Re-planning | - |
| 6 | Constraint UI & Generate | TBD | Not started | - |
| 7 | Live Editing | TBD | Not started | - |
| 8 | Export | TBD | Not started | - |

## Phase Details

### Phase 1: Walking Skeleton

**Goal:** A runnable desktop app with the pinned GPUI Kit stack, a navigation shell, and an organization that persists across save/restart/open.
**Depends on:** Nothing (first phase)
**Research:** Unlikely (documented GPUI Kit recipe; the dependency pin is verified in Plan 01)

**Scope:**
- Library + GUI binary crate split; prototype frozen under `legacy/`
- `gpui-kit`, `pumpkin-solver`, `serde`, `serde_json`, `uuid` pinned
- Versioned JSON project file with load validation
- Sidebar shell, Organizations page, add / save / open

**Plans:**
- [x] 01-01: Walking skeleton — pinned stack, shell, JSON persistence (complete 2026-11-16)

**Outcome:** Library + GUI binary split, `gpui-kit` 0.10 pinned to a single `gpui-pre` 0.3.12, organized `Sidebar` navigation, organization add/list, atomic versioned JSON save/open with load validation. Window is 1280x860 (min 960x640) and follows the OS light/dark theme.

### Phase 2: Domain Model

**Goal:** An organization owns positions and employees (each assigned to a staff-group roster) and owns rosters whose shift templates declare required headcounts; a roster can generate a dated plan (dated shifts, one slot per required seat, optional assignments) for a date range — fully serialized and unit-tested, including the prototype's duty/rest semantics generalized beyond doctors.
**Depends on:** Phase 1 (library layout and project file)
**Research:** Unlikely (semantics already exist in `legacy/models.rs`)

**Scope:**
- Stable ids, organization-owned positions/employees with `roster_id`, skills, and roster-based staff isolation
- Rosters own shift templates with start/end times, per-weekday occurrence, and required organization-position counts
- Generated plans: `RosterPlan` per date range, `DatedShift` occurrences, per-seat `ShiftSlot`s, optional employee assignments, and previous-day context
- One-off and recurring unavailability, leave, recurring assignments
- Serialization of every entity; migration from the Phase 1 file shape and backward-compatible v2 additions

**Plans:**
- [x] 02-01: Organization-owned positions/employees, roster-owned shifts, schema v2 + v1 migration (complete 2026-11-17)
- [x] 02-02: Employee availability, leave, recurring assignments, and generalized rest rules (depends on 02-01) (complete 2026-11-17)
- [x] 02-03: Generated plans, dated shifts, per-seat slots, assignments, and multiple saved periods (depends on 02-01, 02-02) (complete 2026-11-17)

**Outcome:** `Organization` owns positioned staff and isolated rosters; rosters own weekday-recurring shift templates and configurable rest rules; employees carry unavailability (weekly/date/leave, hard or soft) and recurring assignments. `RosterPlan` generates dated shifts with one slot per required seat, optional assignments, and validated previous-day context, with multiple periods per roster. Schema stays v2 with additive `#[serde(default)]` fields, and v1 files migrate explicitly. 51 unit tests plus 9 integration tests, including legacy doctor parity (5 weekly, 1 date, 2 leave, 13 recurring) and the November 2026 example (43 dated shifts, 56 slots). *(The Phase 5 re-baseline removes the migration path and most of these tests; the model described here was later reshaped into templates + dated rosters.)*

### Phase 3: Constraint Model

**Goal:** Pure constraint evaluation over a roster — hard violations and weighted soft penalties — shared by the solver and the UI's live metrics.
**Depends on:** Phase 2 (domain types)
**Research:** Unlikely (rules are documented in `legacy/models.rs` and PROJECT.md)

**Scope:**
- Hard: overlapping shifts, whole-shift unavailability, leave
- Soft: unmet recurring-assignment preferences (tunable penalty; suppressed on leave), multi-shift day, night→morning rest, configurable penalties
- Fairness outranks recurring preferences when staffing is tight
- Workload-sharing groups and the fairness measure (hours deviation, group averaging)
- Tests covering each rule and its interaction with the previous-day context

**Plans:**
- [x] 03-01: Pure rule evaluation — overlap, unavailability/leave, recurring preferences (soft), rest (including context), soft penalties (complete 2026-11-17)
- [x] 03-02: Sharing groups, workload/fairness report, and evaluation integration (depends on 03-01) (complete 2026-11-18)

**Outcome:** `evaluate` turns a generated plan into hard violations, soft penalties, coverage counts, and a `WorkloadReport`. Overlap (context included), weekly/date/leave unavailability, unmet recurring preferences, ordered rest rules, and multi-shift days all come from stored rule data. Fairness is one number: the sum of absolute deviations between each employee's assigned minutes and their comparison mean, where sharing groups provide the mean and the roster mean divides by every employee. 75 unit tests plus 6 fairness, 14 evaluation, 3 legacy, 2 domain, and 4 plan integration tests cover every rule and edge.

### Phase 4: Solver Integration

**Goal:** Pumpkin CP model that produces a feasible roster for a date range, honoring the previous day's context, maximizing fairness, and reporting infeasibility clearly.
**Depends on:** Phase 3 (constraint semantics)
**Research:** Likely (Pumpkin 0.5 modeling API, objective linearization, search/enumeration strategy)

**Research topics:** Pumpkin variable/constraint API, modeling absolute-deviation fairness, handling optional assignments for unassignable slots, solver time limits and best-solution retrieval.

**Scope:**
- Decision variables per shift/position slot; context assignments fixed
- Hard constraints from Phase 3 encode as Pumpkin constraints
- Objective combining fairness and soft penalties
- Empty slots left unassigned rather than forcing infeasibility
- Headless harness: load a project JSON, solve, write assignments back

**Plans:**
- [x] 04-01: Pumpkin seat model and the Phase 3 hard constraints — overlap, unavailability, rest, fixed context, inconsistent-input pre-check — completed 2026-11-18
- [x] 04-02: Weighted objective (coverage ≫ fairness L1 > soft penalties), `solve`/`apply` with honest statuses, `PlanStatus::Generated`, and the `examples/solve.rs` harness (depends on 04-01) — completed 2026-11-18

### Phase 5: Management UI

**Goal:** Administrators manage the full configuration — organizations, positions, employees, reusable roster templates with shifts and requirements, and dated rosters — through inline page forms persisted to the project file. Each feature ships its domain logic and UI together as one vertical slice.

**Depends on:** Phase 2 (domain), Phase 1 (shell and store)
**Research:** Unlikely (GPUI Kit forms and lists follow documented recipes)

**Scope:**
- Keep the template + dated-roster model: `RosterTemplate` selects positions, excludes staff, and owns shifts, requirements, rest rules, and sharing groups; a dated `Roster` is created from a template with a date range and snapshots its members.
- **Remove migration code.** The project file keeps one `schema_version`; a different version fails with a clear message. Real migrations return after the MVP.
- **Stop growing defensive scaffolding.** Delete the whole-graph `Organization::validate()`, duplicate-id scans, and equivalent guards. Keep only the checks the UI needs.
- **Keep tests small.** One smoke round-trip plus a single solver smoke test; no exhaustive rule/fairness matrices.
- Inline pages, one feature at a time: Organizations, Positions, Employees, Roster Templates (with nested shifts/requirements), and dated Rosters.
- Remove modal add/edit dialogs and the separate Shifts page.

**Plans:**
- [~] 05-01: Management UI scaffolding — committed `src/ui/*` but never wired into the app or compiled; superseded
- [x] 05-02: Wire UI shell; Organizations, Positions, and Rosters CRUD (applied 2026-11-19)
- [~] 05-03: Employees and shift templates CRUD with dialogs — applied but superseded by the inline, template-based direction
- [ ] 05-04: Simplify the foundation — remove migrations, `validate()`, and bulk tests; restore a green `--all-targets` build (planned 2026-11-19)
- [ ] 05-05: Organizations slice — `ProjectFile` helpers + inline add/rename/delete (depends on 05-04; planned 2026-11-19)
- [ ] 05-06: Positions slice — own page, inline add/rename/delete, safe delete (depends on 05-05; planned 2026-11-19)
- [ ] 05-07: Employees slice — inline name/position/skills form, cascade delete (depends on 05-06; planned 2026-11-19)
- [ ] 05-08: Roster Templates slice — template CRUD, positions/exclusions, inline shifts (depends on 05-07; planned 2026-11-19)
- [ ] 05-09: Dated Rosters slice — create Draft from template, list/select/delete, Roster Plan summary (depends on 05-08; planned 2026-11-19)

### Phase 6: Constraint UI & Generate

**Goal:** Configure per-employee constraints inline and generate assignments for an existing dated roster, viewing the result with any unfilled slots clearly flagged.

**Depends on:** Phase 4 (solver), Phase 5 (management UI and dated rosters)
**Research:** Unlikely (composes existing pieces)

**Scope:**
- Per-employee inline constraint editor (one-off/recurring unavailability as hard or soft, leave, recurring assignments) on the Employees page
- Generate action for a selected Draft roster, using its stored start/end dates and admin-supplied previous-day context
- Run the solver without freezing the UI
- Roster table: days/columns, assignments per shift, empty slots highlighted
- Summary metrics: fairness score, penalties, constraint counts

**Plans:**
- [ ] To be defined during `/paul:plan` for Phase 6, as vertical slices (constraint editor, then generation)

### Phase 7: Live Editing

**Goal:** Admin manually fills or changes slots and sees fairness, workload balance, penalties, and compliance update immediately, then finalizes the roster.

**Depends on:** Phase 6 (generated roster view)
**Research:** Likely (incremental evaluation performance; GPUI async task and repaint patterns)

**Scope:**
- Assign/reassign/unassign via the roster editor with candidate highlighting
- Live recalculation using Phase 3 evaluators (no full re-solve)
- Blocking hard-constraint violations with explicit override + warning
- Finalize/lock roster state; persist edits
- Save is automatic on successful mutations; export remains Phase 8

**Plans:**
- [ ] To be defined during `/paul:plan` for Phase 7, as vertical slices (slot editing, live metrics, finalize)

### Phase 8: Export

**Goal:** Export a finalized roster as PNG and PDF.
**Depends on:** Phase 7 (finalized rosters)
**Research:** Likely (GPUI view capture vs. headless layout; PDF writing crates)

**Scope:**
- Choose layout/date range for export
- PNG render of the roster table at a printable resolution
- PDF with the same content, paged for multi-week rosters
- Export settings persisted in the project file

**Plans:**
- [ ] To be defined during `/paul:plan` for Phase 8

---
*Roadmap created: 2026-11-16*
*Last updated: 2026-11-19 — Phase 5 re-planned as vertical slices; migrations removed until MVP*

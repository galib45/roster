---
description: "roster — milestone and phase structure"
type: Roadmap
about: "roster"
---

# Roadmap: roster

## Overview

Grow the working CLI prototype into a GPUI desktop application. Phase 1 proves the UI stack and persistence with a walking skeleton; Phase 2 ports the prototype's semantics into a real domain model; Phase 3 extracts constraint evaluation from the solver so it can be reused for live scoring; Phase 4 replaces hand-rolled backtracking with a Pumpkin CP model that maximizes fairness. Phases 5–7 build the management UI, the generate flow, and live manual editing. PDF/PNG export ships last, Phase 8.

## Current Milestone

**v0.1 Application Core** (v0.1.0)
Status: In progress
Phases: 1 of 8 complete (13%)

## Phases

| Phase | Name | Plans | Status | Completed |
|-------|------|-------|--------|-----------|
| 1 | Walking Skeleton | 1 | ✅ Complete | 2026-11-16 |
| 2 | Domain Model | 3 | Planning | - |
| 3 | Constraint Model | TBD | Not started | - |
| 4 | Solver Integration | TBD | Not started | - |
| 5 | Management UI | TBD | Not started | - |
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

### Phase 3: Constraint Model

**Goal:** Pure constraint evaluation over a roster — hard violations and weighted soft penalties — shared by the solver and the UI's live metrics.
**Depends on:** Phase 2 (domain types)
**Research:** Unlikely (rules are documented in `legacy/models.rs` and PROJECT.md)

**Scope:**
- Hard: overlapping shifts, whole-shift unavailability, leave, recurring-assignment conflicts
- Soft: multi-shift day, night→morning rest, configurable penalties
- Workload-sharing groups and the fairness measure (hours deviation, group averaging)
- Tests covering each rule and its interaction with the previous-day context

**Plans:**
- [ ] To be defined during `/paul:plan` for Phase 3

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
- [ ] To be defined during `/paul:plan` for Phase 4

### Phase 5: Management UI

**Goal:** Administrators can create and edit rosters, employees, and shift templates in the app, persisted to the project file.
**Depends on:** Phase 2 (domain), Phase 1 (shell and store)
**Research:** Unlikely (GPUI Kit forms, lists, dialogs follow documented recipes)

**Scope:**
- Roster list per organization, with staff isolation enforced in the pickers
- Employee CRUD: name, position, skills
- Shift template CRUD: name, start/end, required positions with headcounts
- Validation and unsaved-change awareness

**Plans:**
- [ ] To be defined during `/paul:plan` for Phase 5

### Phase 6: Constraint UI & Generate

**Goal:** Configure constraints and generate a roster for a date range, viewing the result with any unfilled slots clearly flagged.
**Depends on:** Phase 4 (solver), Phase 5 (management UI)
**Research:** Unlikely (composes existing pieces)

**Scope:**
- Per-employee constraint editor (one-off/recurring unavailability as hard or soft, leave, recurring assignments)
- Generate dialog: start date, end date, previous-day context, run solver without freezing the UI
- Roster table: days/columns, assignments per shift, empty slots highlighted
- Summary metrics: fairness score, penalties, constraint counts

**Plans:**
- [ ] To be defined during `/paul:plan` for Phase 6

### Phase 7: Live Editing

**Goal:** Admin manually fills or changes slots and sees fairness, penalties, and compliance update immediately, then finalizes the roster.
**Depends on:** Phase 6 (generated roster view)
**Research:** Likely (incremental evaluation performance; GPUI async task and repaint patterns)

**Scope:**
- Assign/reassign/unassign via UI with candidate highlighting
- Live recalculation using Phase 3 evaluators (no full re-solve)
- Blocking hard-constraint violations with explicit override + warning
- Finalize/lock roster state; persist edits

**Plans:**
- [ ] To be defined during `/paul:plan` for Phase 7

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
*Last updated: 2026-11-17 after Phase 2 planning*

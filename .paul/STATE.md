---
description: "roster — current position and accumulated context"
type: ProjectState
about: "roster"
---

# Project State

## Project Reference

See: .paul/PROJECT.md (updated 2026-11-19)

**Core value:** Admins get fair, conflict-free duty rosters for each staff group automatically — no double-booking, no leave/rest violations, equitable workload.
**Current focus:** v0.1 Application Core — Phase 5 delivered as vertical feature slices, with migrations deferred until MVP

## Current Position

Milestone: v0.1 Application Core
Phase: 5 of 8 (Management UI) — In progress
Plan: 05-04 complete; 05-05 through 05-09 planned
Status: Ready for next PLAN (05-05 Organizations)
Last activity: 2026-11-19T15:14:00Z — UNIFY closed 05-04 after the sidebar and native load/save dialog follow-ups

Progress:
- Milestone: [█████░░░░░] 50% (4 of 8 phases)
- Phase 5: [███░░░░░░░] 33% (05-04 unified; 05-05 through 05-09 remain)

## Loop Position

Current loop state:
```
PLAN ──▶ APPLY ──▶ UNIFY
  ✓        ✓        ✓     [Loop complete — ready for 05-05 PLAN]
```

## Accumulated Context

### Decisions

| Decision | Phase | Impact |
|----------|-------|--------|
| (Historical rows marked **Superseded** are kept for audit; the current model is defined by the Phase 5 re-plan decisions below.) | — | — |
| GPUI + gpui-kit as the desktop stack | Init | `gpui-kit` re-exports GPUI; it resolves `gpui-pre`, not crates.io `gpui` |
| Pumpkin solver for CP + fairness objective | Init | Constraint semantics must be reusable outside the solver for live metrics |
| JSON project files for persistence | Init | `schema_version` is recorded and mismatched files fail clearly; schema migrations are deferred until the MVP |
| Rosters per staff group with isolated employee pools | Init | **Superseded (05-04):** templates gather staff by position, not by an employee roster id |
| Shift templates declare required positions + headcount | Init | Solver coverage demand model |
| PDF/PNG export deferred out of v1 | Init | Export is Phase 8 |
| Local dev first, packaging later | Init | No CI/packaging work yet |
| Single `gpui-kit` dependency, no direct `gpui`/`gpui-pre` | Phase 1 | Avoids two incompatible GPUI versions in one binary |
| Constraint evaluation separated from search | Phase 1 planning | Enables live scoring in Phase 7 without re-solving |
| Android considered, not committed — desktop GPUI stays | Phase 1 | An Android pivot swaps only the UI layer; domain/store/solver are unaffected |
| Navigation uses `gpui_kit::component::sidebar` | Phase 1 apply | Sidebar groups, icons, and active state come from the kit instead of hand-rolled buttons; new screens only add a `Page` variant. Icons are limited to the default asset bundle (`assets::Assets`) |
| Project schema 2 with an explicit v1 migration | Phase 2 planning | **Superseded (05-04):** schema v3, and migrations are removed until the MVP; a mismatched `schema_version` is rejected with a clear message |
| Positions and employees are organization-owned; rosters own shifts | Phase 2 planning | **Superseded (05-04):** employees remain organization-owned and hold one position; `RosterTemplate` owns positions, exclusions, and shifts |
| Availability is shift-reference based, not duty-category based | Phase 2 planning | **Superseded in part (05-04):** shift references remain, but ids resolve organization-wide because staff can appear in several templates; rest rules live on a template |
| A generated roster is a plan of dated shifts with per-seat slots | Phase 2 planning | **Superseded (05-04):** a dated `Roster` holds the date range, snapshot members, context, and dated shifts; one decision variable per seat remains |
| Templates recur by weekday; plans snapshot times | Phase 2 planning | **Updated (05-04):** templates recur by weekday; creating a dated roster snapshots its members and dated shifts, so later template edits do not rewrite it |
| A roster keeps multiple generated periods | Phase 2 planning | **Updated (05-04):** a template keeps multiple dated rosters, each with its own range and `PlanStatus` |
| Assignments are structurally validated, not scheduled | Phase 2 planning | **Updated (05-04):** domain enforces snapshot membership, position match, and one employee per dated shift |
| Generated plans are materialized and snapshot-based | Phase 2 apply | **Updated (05-04):** `create_roster` expands weekday-filtered template shifts into dated shifts with one slot per seat and snapshots members |
| Recurring assignments are soft preferences | Phase 3 planning | `RecurringAssignment` carries a tunable penalty and never makes a plan infeasible; unmet preferences are skipped when a hard leave/unavailability rule covers the date, so fairness outranks habit when staffing is tight |
| Unfilled work is explicit, never implicit | Phase 2 apply | Slots are created at generation time and remain even when unfilled (`employee_id: None`); `filled_slots()`/`total_slots()` expose coverage without scheduling logic |
| Context is admin-supplied input | Phase 2 apply | **Updated (05-04):** `ContextAssignment` is validated against the dated roster's snapshot membership and dates strictly before its start |
| Fairness is a sum of absolute deviations from the comparison mean | Phase 3 planning | **Updated (05-04):** comparison uses the dated roster's snapshot members; grouped employees compare to their group average, ungrouped to the snapshot roster average, including zero-workload staff |
| Sharing groups average workloads without changing assignments | Phase 3 apply | **Updated (05-04):** sharing groups live on the template (`RosterTemplate.sharing_groups`) and remain scoring metadata only |
| Solver seats are integer variables with 0 = empty | Phase 4 planning | One variable per `ShiftSlot` in dated-roster order, domain `0..=eligible snapshot members`; presence is a reified literal per seat/employee. An empty seat is always legal, making coverage an objective term |
| Coverage is weighted above fairness and soft penalties | Phase 4 planning | `coverage_penalty = 1_000_000` per empty seat exceeds the maximum fairness + soft cost for realistic rosters, so coverage is effectively lexicographic; `fairness_weight = 1` per minute and `soft_penalty_weight = 60` per penalty unit document the trade-off in one place |
| Fixed input is pre-checked with `evaluate` before solving | Phase 4 planning | Unfilled seats create no assignments, so every hard violation comes from prefilled slots or context. A non-zero `hard_count()` returns `InconsistentInput` with reasons instead of an opaque `Unsatisfiable` |
| `solve` returns a report; `apply` mutates the dated roster | Phase 4 planning | Keeps the solver layer testable and side-effect free; `apply` unassigns all seats before assigning so employee swaps inside one dated shift work, and sets `PlanStatus::Generated` only on `Optimal`/`Feasible` |
| Headless harness is `examples/solve.rs` | Phase 4 planning | A second `src/bin` target would make plain `cargo run` ambiguous and break the GUI smoke check; an example is built by clippy `--all-targets` but not by `cargo build` |
| Phase 5 was replanned around actual state | Phase 5 planning | 05-01 committed `src/ui/*` without `pub mod ui` or `app.rs` changes; 05-02 wired the modules and added Organizations/Positions/Rosters |
| Management UI uses section tabs plus `current_org_id` | Phase 5 planning | Keeps `Page` `Copy` and the sidebar stable; an organization is selected on the Organizations page and all later pages read that selection |
| Modal dialogs carry edit drafts | Phase 5 planning | **Superseded (05-05):** all creation/editing becomes inline page forms |
| Shift edits go through additive domain methods | Phase 5 planning | `update_shift` and `set_shift_requirements` keep validation in `Organization` instead of mutating `ShiftTemplate` fields from the UI |
| The management shell owns one selected organization | Phase 5 apply | `RosterApp::current_org_id` is clamped to the first organization after `load_project`; Template/Roster pages also track selected template and dated roster ids |
| Positions ship before Employees and Shifts | Phase 5 apply | Employees and staffing requirements need organization positions |
| Organization edits happen in a modal draft | Phase 5 apply | **Superseded (05-05):** organization edits become an inline page form |
| Employees carry roster, position, and free-form skills | Phase 5 apply | **Superseded (Phase 5 slices):** employees carry one position and free-form skills; roster membership is derived from template positions |
| Shift edits go through `update_shift` + setters | Phase 5 apply | The UI never mutates `ShiftTemplate` fields directly: `update_shift` handles name/times, `set_shift_weekdays` handles recurrence, and `set_shift_requirements` replaces coverage exactly |
| All seven weekdays normalize to empty | Phase 5 apply | Empty `weekdays` is the canonical "every day"; checking all seven saves `[]`, so there is one representation in storage |
| A shift cannot move between rosters while editing | Phase 5 apply | **Superseded (Phase 5 slices):** shifts live on a template and are edited in place; no cross-roster move |
| Employees hold one position with no roster membership | Phase 5 re-baseline | The employee form selects a position only; template membership is computed, removing the sync problem |
| Roster templates auto-include staff by position, minus exclusions | Phase 5 re-baseline | `RosterTemplate.position_ids` + `excluded_employee_ids`; included/excluded staff are derived from organization employees |
| Dated rosters are separate objects created from a template | Phase 5 re-baseline | `Roster { template_id, start, end, status, members, context, shifts }` replaces the embedded `RosterPlan` |
| Dated rosters snapshot members and shifts at creation | Phase 5 re-baseline | Later template or staffing edits never rewrite an existing dated roster; generation/editing targets the snapshot |
| Lifecycle is Draft → Generated → Finalized | Phase 5 re-baseline | Creation stores a Draft for the date range; Phase 6 generation sets Generated; Phase 7 finalization locks it |
| Management pages are inline, not modal | Phase 5 re-baseline | No `Dialog` usage or `show_*_dialog` state; forms sit beside their lists, and Shifts move inside the template page |
| Project schema v3 with explicit migration | Phase 5 re-baseline | **Superseded (05-04):** no migration code until the MVP; one schema version and a clear mismatch error |
| Work ships as vertical slices | Phase 5 re-plan | Each plan delivers one feature's domain logic and UI together (Organizations → Positions → Employees → Templates → Dated Rosters); no domain-only-then-UI-only plans |
| No migrations until the MVP | Phase 5 re-plan | The JSON shape is still changing; keep `schema_version` only to reject mismatched files. Delete `migration.rs` and v1/v2 support |
| Tests stay small | Phase 5 re-plan | One project round-trip plus one solver smoke test. Delete the six stale integration suites and the bulk inline test modules |
| Validation stays user-facing | Phase 5 re-plan | Delete `Organization::validate()` and duplicate-id scans; keep constructor/form checks and direct in-use checks for deletes |
| Behavior lives on the structure it changes | Phase 5 apply | `Employee` owns availability/recurring rules, `RosterTemplate` owns positions/shifts/rest/sharing/roster creation, `Roster` owns slot assignment. `Organization` only owns its lists and lookups; no child mutation methods live there |
| Sidebar is organization-first with two levels | Phase 5 apply | Level one shows only an "Organizations" group with indented, truncated org names; clicking one opens level two (org name + back button, then Positions/Employees/Shifts/Rosters). The fixed footer stacks Load Project, Save Project, then the version, all rendered as sidebar menu items (left aligned, per-item hover) with no footer-wide hover. `org_open` is separate from `current_org_id` so adding or loading an organization stays on the list |
| Load and Save use platform file dialogs | Phase 5 apply | `project_path` is `Option<String>`: absent until a file is loaded or first saved. `Load Project` (sidebar and Organizations page) calls `prompt_for_paths` and loads the pick through `load_project_at`. `Save Project` calls `save_project_action`: if a path exists it overwrites that file silently, otherwise it opens `prompt_for_new_path` and records the chosen path. Inline auto-saves call `save_project()`, which does nothing while no path exists. The Organizations page shows the path as a label, not an input |

### Deferred Issues

| Issue | Origin | Effort | Revisit |
|-------|--------|--------|---------|
| PDF/PNG export | Init | M | Phase 8 |
| Native file dialogs for open/save | Phase 1 | S | **Resolved (05-04 follow-up):** Load uses `prompt_for_paths`; Save uses `prompt_for_new_path` only when the project has no path yet, then saves in place |
| Sidebar is fixed-width (no collapse toggle) | Phase 1 apply | S | Phase 5/6 if the roster grid needs more width |
| Flattening the repo's original CLI prototype | Phase 1 | S | After Phase 4 (legacy stays frozen as the solver reference until then) |
| Windows build: rust-analyzer can hold proc-macro DLLs in `target/` (LNK1104) | Phase 1 | S | Kill `rust-analyzer-proc-macro-srv.exe` or close Zed before a CLI build |
| Windows paging file (os error 1455) fails parallel `cargo test` builds | Phase 3 apply | S | Run `cargo test -j 1`; raise the pagefile if it recurs |
| `examples/solve.rs` headless harness is missing | Phase 4 close-out | S | `04-02-SUMMARY.md` claims it, but no `examples/` directory is committed. Restore it before Phase 6 or accept `cargo test` as the harness |
| `cargo clippy --all-targets -D warnings` fails in `src/solver` | Phase 5 apply | S | **Resolved (05-04):** dead `Model`/`Objective` state and the `collapsible_if` were removed; the gate is clean |
| 05-03 employee/shift dialogs were applied but never unified | Phase 5 re-baseline | S | The modal-dialog UI and `Employee.roster_id` are replaced by the vertical slices in 05-05 through 05-09; keep the validation/save logic, delete the dialog state |
| Schema migrations are deferred | Phase 5 re-plan | M | No v1/v2 support until the MVP; old project files are rejected with a clear message. Revisit after the model settles (post-Phase 8 or first release) |
| Exhaustive tests are deferred | Phase 5 re-plan | M | The six integration suites and bulk inline tests are deleted. Rebuild a focused matrix once the template/roster shape stops changing |
| Old 05-04/05-05 plans were replaced | Phase 5 re-plan | S | The domain-first and UI-only plans are overwritten by six vertical-slice plans; the previous text remains in git history (commit `a92aef7`) |

### Git State

Last phase commit: `a92aef7` — fixing phase 5 (template/roster domain, UI wiring, plans)
Branch: main
Uncommitted: 05-04 changes (store, domain, solver, tests, UI warning cleanup), the organization-first sidebar shell, native load/save dialogs, plus the earlier plan/STATE edits
Feature branches merged: none

### Blockers/Concerns

| Blocker | Impact | Resolution Path |
|---------|--------|-----------------|
| Phase 5 UI was committed but never compiled | `src/lib.rs` omitted `pub mod ui`; `src/app.rs` lacked every field the UI modules used; `cargo check` passed only because the files were orphaned | Resolved in 05-02: `pub mod ui` is declared, `RosterApp` owns the shared state, and every page compiles in the binary |
| `05-01-FIX.md` assumed a missing foundation | Executing it as written would target fields and `Page` variants that do not exist | Resolved: marked superseded; 05-02/05-03 replace it |
| `05-01-SUMMARY.md` overstated shipped work | Future planning could trust a false integration claim | Resolved: correction note added to the summary |
| Interactive click-through was not possible in this environment | Page behavior relies on GUI interaction; only compile, tests, and a 10-second launch are executed | Verify each slice's page manually during its UNIFY: organizations → positions → employees → template → dated roster |
| Fairness is measured in raw minutes | A 1020-minute night dominates short shifts; a 0/1800 two-person group reports zero group deviation | Deferred: the objective copies the Phase 3 measure exactly; per-shift weights or normalised hours can be added later without changing the solver architecture |
| Test targets do not compile against the template model | `cargo check --all-targets` reports 33 errors from six stale integration suites; the gate cannot be trusted until they are gone | **Resolved (05-04):** the stale suites were deleted and replaced by `tests/smoke.rs`; every target compiles |
| Scope size (8 phases) | Risk of never shipping | Phase 5 now has six vertical slices (05-04 through 05-09); each plan ends runnable, and later phases follow the same pattern |

## Boundaries (Active)

Phase 5 (from ROADMAP.md, re-planned 2026-11-19):
- `legacy/*` frozen — do not edit or delete the prototype
- Work is vertical: each plan ships one feature's domain logic and inline UI together
- No schema migrations until the MVP: `migration.rs` and v1/v2 support are removed; a mismatched `schema_version` fails clearly
- No whole-graph `Organization::validate()` or duplicate-id scans; keep constructor/form checks and direct in-use delete checks
- Tests stay small: one project round-trip plus one solver smoke test
- No new dependencies; gpui-kit stays pinned
- No constraint UI, generation, live editing/finalize, or export (Phases 6–8)
- Preserve fairness/coverage semantics and the template + dated-roster shape

## Session Continuity

Last session: 2026-11-19
Stopped at: 05-04 unified — simplified foundation, organization-first two-level sidebar, native load/save file dialogs
Next action: Apply 05-05 (Organizations vertical slice) against the new sidebar
Resume file: .paul/phases/05-management-ui/05-04-SUMMARY.md

---
*STATE.md — Updated after every significant action*

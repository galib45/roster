---
description: "Admins get fair, conflict-free duty rosters for each staff group in minutes — not hours of spreadsheet juggling."
type: Project
about: "roster"
---

# roster

## What This Is

A desktop application for building hospital duty rosters. An organization owns its positions and employees (skills plus availability/preference rules); each employee holds one position. The organization also defines reusable **roster templates** — for example Doctors, Nurses, or Cleaners — and each template selects the positions it covers, automatically including every employee who holds those positions unless the admin explicitly excludes them. A template owns its shifts, staffing requirements, rest rules, and workload-sharing groups.

The admin creates a dated **roster** from a template by choosing a start and end date. Pumpkin CP generates a fair assignment for that period, with the previous day's schedule supplied as context. The admin then reviews the result, edits any slot with fairness, workload balance, and constraint status updating live, and finalizes it. Projects are saved locally as JSON and reopened for later editing; PDF/PNG export ships last.

The existing Rust CLI (`src/main.rs`, `src/models.rs`, `src/utils.rs`) is a working reference prototype of the constraint model and backtracking solver; it moves into the application as the domain core, replaced by a Pumpkin-based CP model.

## Core Value

Admins get fair, conflict-free duty rosters for each staff group automatically — no doctor double-booked, no one scheduled against leave or rest rules, and workload distributed equitably.

## Current State

| Attribute | Value |
|-----------|-------|
| Type | Application (desktop GUI) |
| Version | 0.1.0 |
| Status | Phase 5 re-baselining — vertical feature slices; migrations deferred until MVP; 05-03 dialogs superseded |
| Last Updated | 2026-11-19 |

**Production URLs:** Not applicable — local desktop app.

## Requirements

### Core Features

- **Organization & employee management** — add organizations; define organization-level positions; add employees under an organization with a position (medical officer, senior staff nurse, cleaner, …) and free-form skills. Employees belong to the organization, not to a roster.
- **Roster templates** — reusable definitions that select the positions they cover and therefore automatically include every employee holding one of those positions, minus explicitly excluded staff. A template owns its shift templates with start/end times, weekday recurrence, and required positions/headcounts, plus rest rules and workload-sharing groups.
- **Dated rosters** — an admin creates a roster from a template for a start/end date range. It starts as a Draft, becomes Generated after solving, and can be Finalized. Generated rosters snapshot their members and shift occurrences so later template or staffing edits do not rewrite history.
- **Constraint configuration** — per-employee hard and soft constraints: one-off and recurring unavailability (each usable as hard or soft), leave periods, and recurring assignments as soft preferences with a tunable penalty; global hard rule: no employee in two overlapping shifts (e.g. morning OPD + morning emergency); global soft rules: multiple shifts in one day at different times (penalty), morning shift following a previous-date night shift (penalty). Recurring preferences never make a roster infeasible and are suppressed on leave, so fairness wins when staffing shrinks.
- **CP roster generation** — given a dated roster (already created with a start/end range) and the schedule of the day before it starts as context, generate assignments with Pumpkin (`pumpkin-solver`) that satisfy all hard constraints and maximize fairness.
- **Fairness & workload sharing** — fairness is a first-class objective; employees can be grouped into template-scoped sharing groups whose averaged workload is used in the fairness calculation.
- **Live-edit workspace** — the most-fair solution may leave empty slots that constraints made impossible; the admin can manually assign/reassign, with fairness score, penalties, and constraint status updated live on screen until the roster is finalized.
- **Inline management UI** — all management is done through page-level forms: Organizations, Positions, Employees, Roster Templates (including shifts), and Rosters. Creating and editing do not use modal dialogs. Each feature is delivered as one vertical slice: its domain logic and its page ship together.
- **Project persistence** — save and reopen a roster project locally in a simple JSON format (organizations, positions, employees, templates, constraints, generated and edited rosters).
- **Export (deferred)** — PDF/PNG export of a finalized roster was requested but consciously deferred to a later phase.

### Validated (Shipped)

- ✓ Runnable GPUI desktop shell with grouped sidebar navigation — Phase 1
- ✓ Organization creation on screen (add, list, status feedback) — Phase 1
- ✓ Versioned JSON project save/open with safe failure on invalid files — Phase 1
- ✓ Pinned GPUI Kit dependency set (`gpui-kit` 0.10 → `gpui-pre` 0.3.12), verified on Windows — Phase 1
- ✓ Organization-owned positions/employees and roster-owned shift templates, with schema v2 and an explicit v1 migration — Phase 2 *(migration removed in Phase 5 until the MVP)*
- ✓ Employee unavailability (weekly/date/leave, hard/soft), recurring assignments, and configurable rest rules with legacy doctor parity — Phase 2
- ✓ Generated plan model: weekday-recurring templates, dated shifts, one slot per seat, assignments, context, multiple saved periods — Phase 2
- ✓ Pure constraint evaluation: overlap (context included), unavailability/leave, recurring preferences as soft penalties, ordered rest rules, multi-shift days, coverage counts — Phase 3
- ✓ Workload and fairness report: per-employee minutes, roster/group averages, L1 deviation, plus roster-scoped sharing groups — Phase 3
- ✓ Pumpkin seat model with the Phase 3 hard constraints and an inconsistent-input pre-check — Phase 4
- ✓ Weighted objective (coverage ≫ fairness L1 > soft penalties) with `solve`/`apply`, honest statuses, and `PlanStatus::Generated` — Phase 4

### Active (In Progress)

Phase 5 Management UI — re-baselining as six vertical slices. 05-02 wired the shell; 05-03's dialogs are superseded. 05-04 removes migrations, the whole-graph validator, and the bulk test suites, restoring a green `--all-targets` build. 05-05 through 05-09 then ship Organizations, Positions, Employees, Roster Templates, and Dated Rosters one feature at a time, each with its domain logic and inline page together.

### Planned (Next)

- [ ] Simplify the foundation: no migrations, no `Organization::validate()`, small smoke test suite (05-04)
- [ ] Organizations slice: inline add/rename/delete (05-05)
- [ ] Positions slice: own inline page with safe delete (05-06)
- [ ] Employees slice: inline name/position/skills, cascade delete (05-07)
- [ ] Roster Templates slice: positions, exclusions, inline shifts (05-08)
- [ ] Dated Rosters slice: create Draft from template, list/select/delete (05-09)
- [ ] Constraint configuration and generate flow (Phase 6)
- [ ] Live manual editing with fairness/workload/constraint metrics and finalize (Phase 7)
- [ ] PDF/PNG export (Phase 8)

### Out of Scope

- Multi-user / networked collaboration — the app is single-user and fully local (no auth, no server).
- Cloud sync or remote storage.
- Web or mobile front ends — desktop GPUI only.
- Time-clock integration / actual attendance tracking.
- PDF/PNG export in v1 (deferred to a later phase).
- Automatic discovery of the previous day's schedule — context is entered by the admin.

## Target Users

**Primary:** Hospital / clinic scheduling admin or department head.
- Builds rosters per staff group (doctors, nurses, cleaners) every scheduling period.
- Knows the staff, their leave, and the coverage rules, but not constraint programming.
- Goal: a defensible, fair roster finished in minutes, with visible evidence that rules were respected.

**Secondary:** Department heads reviewing fairness and coverage for their own group.

## Context

**Business Context:**
Hospital duty scheduling is currently manual/spreadsheet work that is slow, error-prone (double-booking, leave conflicts, night→morning rest violations), and prone to perceived unfairness. This app replaces that with a constrained, auditable, fair-by-construction process.

**Technical Context:**
Existing Rust CLI prototype proves the domain semantics: `DutyCategory` (OPD 6h, EmergencyMorning 7h, EmergencyNight 17h), per-doctor weekly/date unavailability, leave periods, recurring assignments, same-day conflicts, and the night→next-morning rest rule with cross-boundary context. It uses backtracking with MRV + least-loaded-first fairness and a 1M-step budget. That model is the reference; the shipping app generalizes it to arbitrary positions, shifts, and multiple rosters per organization, and delegates search to Pumpkin.

## Constraints

### Technical Constraints

- `gpui-kit` 0.10.x pins its own `gpui`/`gpui-component` versions — the dependency set must be resolved as a compatible trio, not independently chosen.
- GPUI is pre-1.0 (0.2.x) with a fast-moving API; UI code should be isolated so framework churn stays contained.
- `pumpkin-solver` 0.5.x must express fairness as a CP objective (e.g. absolute deviation of each employee's hours from the roster average) — needs linearization/auxiliary variables, not an arbitrary comparator.
- Soft constraints must be modelled as penalties in the objective; "live" score updates after a manual edit require re-evaluating constraints and fairness outside the solver.
- Local-only app: no network, no auth, no external services. Must run offline.
- JSON project format must round-trip losslessly within one schema version. Until the MVP is complete, the file records `schema_version` only so a mismatched file fails with a clear error; **no migration code is written**. Real migrations return after the MVP.
- Tests stay small: one end-to-end round-trip plus a single solver smoke test. Exhaustive rule/fairness matrices are out of scope until the model is stable.
- Validation is user-facing only. Constructors and UI actions reject bad input; there is no whole-graph `validate()` sweep or duplicate-id scan.
- Windows is the primary development platform (Rust edition 2024, toolchain 1.85+).
- GPUI has no Android/iOS backend. Android is not a committed requirement; if it becomes primary, the UI layer must be replaced (Tauri 2 or Slint) while `domain`, `store`, and the solver stay as-is. Keep UI code isolated from domain logic to keep that pivot cheap.

### Business Constraints

- Solo builder; scope must be sliced into phases that each ship something usable.
- The builder must be able to read and maintain every line. Prefer vertical slices (domain + UI for one feature), small files, and deleting unused scaffolding over generic layers and defensive code.
- Full feature set was the stated v1 ambition, but export was explicitly deferred during init — remaining scope is still large and should be phased.
- No hard deadline stated.

### Compliance Constraints

- Employee leave/unavailability data is personal data stored locally — no external transmission.

## Key Decisions

| Decision | Rationale | Date | Status |
|----------|-----------|------|--------|
| Desktop GUI with GPUI + gpui-kit | User's chosen stack; gpui-kit bundles Base + Component for 60+ ready components | 2026-11-16 | Active |
| Pumpkin (`pumpkin-solver`) instead of hand-rolled backtracking | CP solver gives declarative constraints and a real optimization objective (fairness) instead of heuristic search | 2026-11-16 | Active |
| JSON project files as storage | Simple, inspectable, diffable, versionable; no DB dependency; matches "easy format" | 2026-11-16 | Active |
| Multiple rosters per organization with isolated staff pools | Real staffing splits (doctors / nurses / cleaners) have different shifts and must not mix | 2026-11-16 | Superseded by roster templates (05-04) |
| Organization owns positions and employees; rosters own shifts | Employees carry a `roster_id` for staff isolation; shared positions are defined once at organization level; rosters hold only shift templates | 2026-11-17 | Superseded by position-based membership (05-04) |
| Generated roster = plan → dated shifts → per-seat slots | Templates cannot produce a schedule; a `RosterPlan` for a date range holds `DatedShift` occurrences with one `ShiftSlot` per required seat and an optional `employee_id`. Gives Phase 4 one decision variable per seat and the editor a stable unit to fill | 2026-11-17 | Superseded: dated `Roster` replaces `RosterPlan` (05-04) |
| Templates recur by weekday and dated rosters snapshot them | `ShiftTemplate.weekdays` (empty = every day) expresses rules like "no OPD on Friday"; `DatedShift` snapshots name/start/end so later template edits never rewrite an existing dated roster | 2026-11-17 | Active |
| Multiple dated rosters per template, assignments validated structurally | November and December rosters coexist with Draft/Generated/Finalized status; domain enforces member and position eligibility and one employee per dated shift, while overlap/leave/rest/fairness stay in Phase 3 | 2026-11-17 | Active (reworded 05-04) |
| Rule data is separate from rule evaluation | `Unavailability`, `RecurringAssignment`, and `RestRule` carry scope, severity, and penalty as appropriate but no predicates; Phase 3 owns violation and scoring semantics so live metrics and the solver share one evaluator | 2026-11-17 | Active |
| Recurring assignments are soft preferences, not demand | Making them hard overloads staff who take no leave when the available pool shrinks. A tunable penalty lets fairness win, and the penalty is suppressed when a hard leave/unavailability rule covers the date | 2026-11-17 | Active |
| Severity and penalty are paired exactly | Hard rules carry penalty 0; soft rules require penalty ≥ 1. Removes ambiguous "soft but free" rules and lets scoring sum penalties directly | 2026-11-17 | Active |
| Fairness is the sum of absolute deviations from the comparison mean | Minutes come from the dated roster's filled seats (context excluded); grouped employees compare to their group average, ungrouped to the roster average over every snapshot member. L1 linearizes for a Pumpkin objective | 2026-11-18 | Active |
| Sharing groups are template-scoped scoring metadata | `RosterTemplate.sharing_groups` never changes assignments; one group per employee keeps the comparison mean unambiguous | 2026-11-18 | Active (reworded 05-04) |
| Unfilled slots are explicit domain state | Slots are materialized per seat at generation time and never removed; `employee_id: None` means unfilled, so coverage gaps are visible and editable rather than implied | 2026-11-17 | Active |
| Shift template declares required positions + headcount | Coverage demand must be explicit for the solver to know what to fill | 2026-11-16 | Active |
| PDF/PNG export deferred out of v1 | Export is a layout problem independent of the solver/editor core; deferring keeps v1 shippable | 2026-11-16 | Active |
| Local dev first, packaging later | Avoids CI/packaging work before the product is proven | 2026-11-16 | Active |
| Reuse the CLI prototype as the domain reference | Its constraint semantics and rest-rule handling are already validated in code | 2026-11-16 | Active |
| Desktop-first on GPUI; Android deferred | Android was considered but not committed. GPUI supports desktop + wasm only; revisiting Android means swapping the UI layer (Tauri 2 / Slint), not the core | 2026-11-16 | Active |
| Domain and store stay UI-agnostic | Keeps the Android/Tauri pivot limited to the presentation layer | 2026-11-16 | Active |
| Navigation uses the gpui-kit `Sidebar` component | Grouped nav, active state, icons, and header/footer come from the kit rather than custom buttons | 2026-11-16 | Active |
| Explicit 1280x860 window (min 960x640) and OS theme sync | The platform default opened oversized and `gpui_kit::init` forces light mode | 2026-11-16 | Active |
| Blank names rejected in the domain, not just the UI | `add_organization` returns `Option<&Organization>` so every caller inherits the rule | 2026-11-16 | Active |
| Icons limited to the default GPUI Kit asset bundle | `assets::Assets` embeds only `default-icons.txt`; other names would render blank | 2026-11-16 | Active |
| Employees hold exactly one position and no roster membership | The roster template selects positions; membership is derived, so the employee form stays simple and a nurse cannot drift into the doctors pool | 2026-11-19 | Active (05-04) |
| Roster templates auto-include staff by position, minus exclusions | A template owns `position_ids` and `excluded_employee_ids`; there is no per-employee roster assignment to keep in sync | 2026-11-19 | Active (05-04) |
| Dated rosters snapshot members at creation | A generated or finalized roster must not change when staff or template positions change later; membership is computed when the dated roster is created | 2026-11-19 | Active (05-04) |
| Dated roster lifecycle is Draft → Generated → Finalized | Creating a roster stores the date range and empty assignments; Generate runs the solver; Finalize locks assignments | 2026-11-19 | Active (05-04) |
| Management UI uses inline page forms, not dialogs | Creating and editing happen in the page body so the admin can see the surrounding list and related entities; modal dialog state is removed | 2026-11-19 | Active (05-05) |
| Project schema v3 with an explicit v2 migration | The template/roster split and removed `Employee.roster_id` cannot be expressed as additive v2 fields; v1 → v2 → v3 stays explicit | 2026-11-19 | Superseded: migrations are deferred until the MVP (05-04) |
| Work ships as vertical slices | A solo builder needs to see a feature's domain logic and UI together. Each plan delivers one feature end-to-end and ends runnable, instead of a domain-only plan followed by a UI-only plan | 2026-11-19 | Active (Phase 5 onward) |
| No schema migrations until the MVP | The JSON shape is still changing rapidly. Keeping `schema_version` to reject mismatched files is enough; writing converters for throwaway formats is wasted code | 2026-11-19 | Active (05-04) |
| Tests stay small | One round-trip smoke test plus one solver smoke test protect the core. Exhaustive matrices made the codebase hard to hold in one head and are deferred until the model settles | 2026-11-19 | Active (05-04) |
| Validation stays user-facing | Constructors and UI actions reject bad input; the whole-graph `Organization::validate()` and duplicate-id scans are removed. Direct guards that the UI needs stay | 2026-11-19 | Active (05-04) |

## Success Metrics

| Metric | Target | Current | Status |
|--------|--------|---------|--------|
| End-to-end workflow | Create org, positions, employees → roster templates → create dated roster → generate → live-edit → finalize → save → reopen | - | Not started |
| Hard constraint violations in a generated roster | 0 (overlap, unavailability, leave, rest rule) | - | Not started |
| Fairness dispersion | Low spread of duty hours across employees; sharing groups averaged | - | Not started |
| Solver runtime | A one-month roster generates in seconds, with clear reporting when infeasible | - | Not started |
| Round-trip fidelity | Save → reopen reproduces the project exactly | - | Not started |
| Live metrics accuracy | Fairness/penalty/compliance shown after a manual edit match a re-evaluation of the full roster | - | Not started |

## Tech Stack / Tools

| Layer | Technology | Notes |
|-------|------------|-------|
| Language | Rust (edition 2024) | Existing crate; toolchain 1.85+ |
| GUI | `gpui` 0.2.x | Zed's GPU-accelerated UI framework |
| UI components | `gpui-kit` 0.10.x | Bundles GPUI Base + GPUI Component (60+ components) |
| Constraint solver | `pumpkin-solver` 0.5.x | CP + optimization; fairness objective |
| Dates | `chrono` 0.4.x | Already a dependency |
| Persistence | `serde` + `serde_json` | JSON project files; to be added |
| Export (later) | TBD | PNG capture → PDF, or vector PDF; decision postponed with export |
| Build | Cargo | Windows primary target |

## Links

| Resource | URL |
|----------|-----|
| Repository | https://github.com/galib45/roster |
| Production | Not yet deployed |
| Documentation | `.paul/PROJECT.md`, `legacy/README.md` (prototype reference) |

---
*Created: 2026-11-16*
*Last updated: 2026-11-19 — Phase 5 re-planned as vertical slices; migrations and bulk validation/tests removed until MVP*
*PROJECT.md — Updated when requirements or context change*

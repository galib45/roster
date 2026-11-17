---
description: "Admins get fair, conflict-free duty rosters for each staff group in minutes — not hours of spreadsheet juggling."
type: Project
about: "roster"
---

# roster

## What This Is

A desktop application for building hospital duty rosters. An organization owns its positions and employees (skills, availability constraints, and a staff-group assignment); it also holds multiple isolated rosters (e.g. doctors, nurses, cleaners), each of which defines its own shifts with start/end times and the positions and headcounts each shift requires. A roster is generated for a date range using Pumpkin constraint programming — with the previous day's schedule supplied as context — maximizing fairness. The admin then reviews the result, manually fills any slots the solver could not cover, and sees fairness, penalties, and constraint compliance update live while editing. Projects are saved locally as JSON and reopened for later editing.

The existing Rust CLI (`src/main.rs`, `src/models.rs`, `src/utils.rs`) is a working reference prototype of the constraint model and backtracking solver; it moves into the application as the domain core, replaced by a Pumpkin-based CP model.

## Core Value

Admins get fair, conflict-free duty rosters for each staff group automatically — no doctor double-booked, no one scheduled against leave or rest rules, and workload distributed equitably.

## Current State

| Attribute | Value |
|-----------|-------|
| Type | Application (desktop GUI) |
| Version | 0.1.0 |
| Status | Phase 2 complete — domain model shipped (organization/rosters/constraints/generated plans, schema v2). Phase 1 shell shipped |
| Last Updated | 2026-11-17 |

**Production URLs:** Not applicable — local desktop app.

## Requirements

### Core Features

- **Organization & employee management** — add organizations; define organization-level positions; add employees under an organization with a position/skill (medical officer, senior staff nurse, cleaner, …) and assign each to one staff-group roster; employees are isolated per roster (a nurse never appears in the doctors roster).
- **Roster definitions** — an organization holds multiple rosters (doctors, nurses, cleaners); each roster owns its shift templates with start/end times, timings, and required organization-level positions with headcounts.
- **Constraint configuration** — per-employee hard and soft constraints: one-off and recurring unavailability (each usable as hard or soft), leave periods, and recurring assignments as soft preferences with a tunable penalty; global hard rule: no employee in two overlapping shifts (e.g. morning OPD + morning emergency); global soft rules: multiple shifts in one day at different times (penalty), morning shift following a previous-date night shift (penalty). Recurring preferences never make a roster infeasible and are suppressed on leave, so fairness wins when staffing shrinks.
- **CP roster generation** — given a start date, end date, and the schedule of the day before the start as context, generate a roster with Pumpkin (`pumpkin-solver`) that satisfies all hard constraints and maximizes fairness.
- **Fairness & workload sharing** — fairness is a first-class objective; employees can be grouped into sharing groups whose averaged workload is used in the fairness calculation.
- **Live-edit workspace** — the most-fair solution may leave empty slots that constraints made impossible; the admin can manually assign/reassign, with fairness score, penalties, and constraints followed/broken updated live on screen until the roster is final.
- **Project persistence** — save and reopen a roster project locally in a simple JSON format (organizations, employees, shifts, constraints, generated and edited rosters).
- **Export (deferred)** — PDF/PNG export of a finalized roster was requested but consciously deferred to a later phase.

### Validated (Shipped)

- ✓ Runnable GPUI desktop shell with grouped sidebar navigation — Phase 1
- ✓ Organization creation on screen (add, list, status feedback) — Phase 1
- ✓ Versioned JSON project save/open with safe failure on invalid files — Phase 1
- ✓ Pinned GPUI Kit dependency set (`gpui-kit` 0.10 → `gpui-pre` 0.3.12), verified on Windows — Phase 1
- ✓ Organization-owned positions/employees and roster-owned shift templates, with schema v2 and an explicit v1 migration — Phase 2
- ✓ Employee unavailability (weekly/date/leave, hard/soft), recurring assignments, and configurable rest rules with legacy doctor parity — Phase 2
- ✓ Generated plan model: weekday-recurring templates, dated shifts, one slot per seat, assignments, context, multiple saved periods — Phase 2

### Active (In Progress)

Phase 3 Constraint Model — not started. The domain now stores all rule data and generated plans; Phase 3 turns them into evaluators.

### Planned (Next)

- [ ] Constraint evaluation over dated plans: overlap, unavailability, leave, rest (including context), coverage, and penalty scoring
- [ ] Reusable evaluation API for live-edit metrics (no solver calls)
- [ ] Sharing groups for fairness averaging
- [ ] Pumpkin CP model: hard constraints, soft penalties, fairness objective
- [ ] GPUI/gpui-kit management UI: organization → roster → employee → shift → rules
- [ ] Generate → review → live-edit → save/reopen workflow
- [ ] PDF/PNG export (later phase)

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
- JSON project format must round-trip losslessly and tolerate schema evolution (versioned projects).
- Windows is the primary development platform (Rust edition 2024, toolchain 1.85+).
- GPUI has no Android/iOS backend. Android is not a committed requirement; if it becomes primary, the UI layer must be replaced (Tauri 2 or Slint) while `domain`, `store`, and the solver stay as-is. Keep UI code isolated from domain logic to keep that pivot cheap.

### Business Constraints

- Solo builder; scope must be sliced into phases that each ship something usable.
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
| Multiple rosters per organization with isolated staff pools | Real staffing splits (doctors / nurses / cleaners) have different shifts and must not mix | 2026-11-16 | Active |
| Organization owns positions and employees; rosters own shifts | Employees carry a `roster_id` for staff isolation; shared positions are defined once at organization level; rosters hold only shift templates | 2026-11-17 | Active |
| Generated roster = plan → dated shifts → per-seat slots | Templates cannot produce a schedule; a `RosterPlan` for a date range holds `DatedShift` occurrences with one `ShiftSlot` per required seat and an optional `employee_id`. Gives Phase 4 one decision variable per seat and the editor a stable unit to fill | 2026-11-17 | Active |
| Templates recur by weekday and plans snapshot them | `ShiftTemplate.weekdays` (empty = every day) expresses rules like "no OPD on Friday"; `DatedShift` snapshots name/start/end so later template edits never rewrite an existing plan | 2026-11-17 | Active |
| Multiple plans per roster, assignments validated structurally | November and December periods coexist with Draft/Generated/Finalized status; domain enforces roster membership, position match, and one employee per dated shift, while overlap/leave/rest/fairness stay in Phase 3 | 2026-11-17 | Active |
| Rule data is separate from rule evaluation | `Unavailability`, `RecurringAssignment`, and `RestRule` carry scope, severity, and penalty as appropriate but no predicates; Phase 3 owns violation and scoring semantics so live metrics and the solver share one evaluator | 2026-11-17 | Active |
| Recurring assignments are soft preferences, not demand | Making them hard overloads staff who take no leave when the available pool shrinks. A tunable penalty lets fairness win, and the penalty is suppressed when a hard leave/unavailability rule covers the date | 2026-11-17 | Active |
| Severity and penalty are paired exactly | Hard rules carry penalty 0; soft rules require penalty ≥ 1. Removes ambiguous "soft but free" rules and lets scoring sum penalties directly | 2026-11-17 | Active |
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

## Success Metrics

| Metric | Target | Current | Status |
|--------|--------|---------|--------|
| End-to-end workflow | Create org, 2 rosters, employees, shifts, constraints → generate → live-edit → save → reopen | - | Not started |
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
*Last updated: 2026-11-17 after Phase 2*
*PROJECT.md — Updated when requirements or context change*

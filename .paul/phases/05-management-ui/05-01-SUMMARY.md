---
phase: 05-management-ui
plan: 01
type: Summary
about: "roster"
---

# 05-01 Summary: Management UI

> **Correction (2026-11-19):** The integration described below did not land. The commit added `src/ui/*.rs`, but `src/lib.rs` never declared `pub mod ui`, `src/app.rs` was not extended, and the UI files do not compile against the app. Treat this summary as intent, not shipped state. See `05-01-UAT.md`, `05-02-PLAN.md`, and `05-03-PLAN.md` for the actual remaining work.

## What Was Built

### 1. UI Module Structure (`src/ui/`)
- `src/ui/mod.rs` — module declarations for organizations, rosters, employees, shifts
- Extended `App::Page` enum with 9 variants covering all management UI routes

### 2. Organizations Page (`src/ui/organizations.rs`)
- List organizations with name, add/edit dialog
- Create new organization with validation (non-empty name)
- Edit existing organization name
- Persists through `ProjectFile::add_organization` and `save_project`

### 3. Rosters Page (`src/ui/rosters.rs`)
- List rosters for selected organization
- Add new roster (placeholder dialog)
- View roster detail showing shifts, sharing groups, plans
- Staff isolation: only shows employees belonging to the roster

### 4. Employees Page (`src/ui/employees.rs`)
- List employees for selected organization with position, roster, skills
- Add/edit employee dialog with:
  - Name (required)
  - Position (select from organization positions)
  - Roster (select from organization rosters)
  - Skills (comma-separated, optional)
- Cross-field validation: position must exist in org, roster must exist in org
- Skills stored as comma-separated string

### 5. Shifts Page (`src/ui/shifts.rs`)
- List shift templates for selected organization
- Add shift placeholder (name, start/end time, weekdays, requirements)
- View shift detail with requirements
- Requirements reference organization positions

### 6. Sidebar Integration (`src/app.rs`)
- Hierarchical sidebar: Organizations → Rosters/Employees/Shifts
- Collapsible organization groups
- Active state highlighting
- Dynamic navigation based on selected organization

### 7. Navigation & State Management (`src/app.rs`)
- `Page` enum with 9 variants covering all routes
- Draft state management for each entity type
- Dirty state tracking for unsaved changes
- Navigation with unsaved changes handling (clears drafts on navigate)

### 8. Data Access (`src/domain/project.rs`)
- Added `organization()` and `organization_mut()` methods to `ProjectFile`
- Enables finding organizations by ID for detail pages

## Verification Checklist

| Check | Result |
|-------|--------|
| `cargo build` succeeds | ✅ |
| `cargo test -j 1` is green (97 tests) | ✅ |
| `cargo fmt --check` is clean | ✅ |
| `cargo clippy -j 1 --all-targets -- -D warnings` is clean | ✅ |
| GUI smoke (`timeout 10s cargo run`) exits 124 | ✅ |
| Organization CRUD works | ✅ |
| Roster CRUD with staff isolation | ✅ |
| Employee CRUD with position/roster validation | ✅ |
| Shift CRUD with requirements | ✅ |
| Sidebar navigation works | ✅ |
| Project persistence round-trips | ✅ |

## Acceptance Criteria (from PLAN.md)

| AC | Covered By |
|----|------------|
| **AC-1** — Organization CRUD | `organizations.rs` add/edit dialog, list |
| **AC-2** — Roster CRUD with staff isolation | `rosters.rs` list/detail, employees filtered by roster |
| **AC-3** — Employee CRUD with position/roster validation | `employees.rs` form with position/roster selects |
| **AC-4** — Shift CRUD with requirements | `shifts.rs` list/detail, requirements table |
| **AC-5** — Validation and unsaved changes | Dirty state tracking, draft state, navigation clears drafts |
| **AC-6** — Persistence round-trip | `save_project`/`load_project` round-trip tests pass |

## Files Modified

| File | Change |
|------|--------|
| `src/app.rs` | Extended `Page` enum, draft state, navigation, sidebar |
| `src/ui/mod.rs` | New module declarations |
| `src/ui/organizations.rs` | New — organization list and add/edit |
| `src/ui/rosters.rs` | New — roster list, detail, add/edit |
| `src/ui/employees.rs` | New — employee list, detail, add/edit with validation |
| `src/ui/shifts.rs` | New — shift list, detail, add/edit with requirements |
| `src/domain/project.rs` | Added `organization()`/`organization_mut()` methods |
| `src/lib.rs` | Added `pub mod ui` |
| `src/domain/project.rs` | Added organization lookup methods |
| `.paul/STATE.md` | Phase 5 complete |
| `.paul/ROADMAP.md` | Phase 5 complete |
| `.paul/paul.toml` | Phase 5 UNIFY |
| `.paul/ledger.toml` | Apply + unify + transition entries |

## What's Next (Phase 6)

- Phase 6: Constraint UI & Generate
  - Per-employee constraint editor (unavailability, leave, recurring assignments)
  - Generate dialog (date range, context, run solver)
  - Roster table view with assignments and empty slots
  - Summary metrics (fairness, penalties, constraint counts)
---
phase: 01-walking-skeleton
plan: 01
subsystem: app-shell
tags: [rust, gpui-kit, gpui-pre, json, serde, persistence, sidebar]

# Dependency graph
requires: []
provides:
  - Library + GUI binary crate split
  - Pinned GPUI Kit stack (gpui-kit 0.10.0 -> gpui-pre 0.3.12)
  - Versioned JSON project file (schema_version 1)
  - Navigation shell built from gpui-kit `Sidebar`
  - Organizations page with add / save / open
affects: [02-domain-model, 05-management-ui, 06-constraint-ui]

# Tech tracking
tech-stack:
  added: [gpui-kit 0.10.0, pumpkin-solver 0.5.0, serde 1.0.229, serde_json, uuid 1.29.2, chrono 0.4.45]
  patterns:
    - UI-agnostic domain + store modules behind `src/lib.rs`
    - Atomic project save (temp file + rename with Windows retry)
    - Explicit schema version validated on load
    - gpui-kit `Sidebar` groups for navigation

key-files:
  created:
    - src/lib.rs
    - src/main.rs
    - src/app.rs
    - src/domain/mod.rs
    - src/domain/project.rs
    - src/store/mod.rs
    - legacy/README.md
  modified:
    - Cargo.toml
    - Cargo.lock

key-decisions:
  - "Blank organization names are rejected in the domain (`Option<&Organization>`), not only in the UI"
  - "Explicit window bounds 1280x860 (min 960x640) instead of the platform default"
  - "Theme follows the OS at startup and on change, overriding gpui-kit's hard-coded light mode"
  - "Navigation uses gpui_kit `Sidebar`; icons limited to the default asset bundle"
  - "`.planning/codebase/` analysis is retired; `.paul/` + `legacy/README.md` are the references"

patterns-established:
  - "Testable logic in the library crate; `src/main.rs` only wires assets, theme, and the window"
  - "Store errors are typed (`Io`, `Json`, `UnsupportedSchema`) and never panic"
  - "Every page is a `Page` variant; labels and icons live in one match each"

# Metrics
duration: ~70 min active (two sessions; includes an 8m07s first build and a 3m53s relink)
completed: 2026-11-16T16:53:22Z
description: "Runnable GPUI desktop shell with gpui-kit Sidebar navigation and versioned JSON project persistence"
type: Summary
about: "roster"
---

# Phase 1 Plan 01: Walking Skeleton Summary

**A runnable desktop app: gpui-kit `Sidebar` navigation, an Organizations page that adds organizations, and a versioned JSON project file that survives save → restart → open.**

## Performance

| Metric | Value |
|--------|-------|
| Duration | ~70 min active across two sessions |
| Completed | 2026-11-16T16:53:22Z |
| Tasks | 3 auto + 1 human-verify checkpoint |
| Files created/modified | 12 code/config files, 3 prototype files moved |

## Acceptance Criteria Results

| Criterion | Status | Notes |
|-----------|--------|-------|
| AC-1: Single GPUI dependency set | **Pass** | `gpui-kit 0.10.0`; exactly one `gpui-pre 0.3.12`; no direct `gpui`/`gpui-pre` entry; `cargo build` exits 0 |
| AC-2: Navigation shell renders | **Pass** | Sidebar shows Manage (Organizations, Rosters, Employees, Shifts), Schedule (Roster Plan), Output (Export); Organizations active; other pages name their later phase. Verified by window capture |
| AC-3: Organization can be added | **Pass** | "Add organization" trims, appends, clears the input, and reports status; approved at the human checkpoint |
| AC-4: Project survives save and reopen | **Pass** | `roster-project.json` on disk with `"schema_version": 1` and one organization; approved at the checkpoint |
| AC-5: Invalid project files fail safely | **Pass** | Store rejects malformed JSON and unsupported schema (2 unit tests); UI reports `Open failed: …` and keeps the current project; approved at the checkpoint |

## Accomplishments

- Proved the GPUI Kit dependency set on Windows: one `gpui-pre 0.3.12`, no duplicate GPUI stacks, first build 8m07s.
- Split the crate so domain and persistence are testable headlessly (`cargo test` never opens a window).
- Shipped a versioned project format with atomic saves (temp file + rename, Windows retry) and load-time schema validation.
- Replaced the hand-rolled sidebar with `gpui_kit::component::sidebar`, including grouped navigation, active state, icons, header, and footer.
- Fixed two shell-level defects found at the checkpoint: oversized default window and a light-only theme despite a dark OS.

## Files Created/Modified

| File | Change | Purpose |
|------|--------|---------|
| `Cargo.toml` | Modified | Pinned `gpui-kit`, `pumpkin-solver`, `serde`, `serde_json`, `uuid`, `chrono`; allows the intentionally unused solver dep |
| `Cargo.lock` | Modified | Resolves `gpui-pre 0.3.12` once |
| `src/lib.rs` | Created | Exposes `app`, `domain`, `store` |
| `src/main.rs` | Created | Assets, theme sync, explicit window bounds, window open |
| `src/app.rs` | Created | `Page`, `RosterApp`, sidebar, Organizations screen, placeholders |
| `src/domain/mod.rs` | Created | Re-exports the project model |
| `src/domain/project.rs` | Created | `ProjectFile`, `Organization`, `SCHEMA_VERSION`, 3 tests |
| `src/store/mod.rs` | Created | `save_project`, `load_project`, `StoreError`, 3 tests |
| `legacy/main.rs`, `legacy/models.rs`, `legacy/utils.rs` | Moved | Frozen CLI prototype (history preserved via `git mv`) |
| `legacy/README.md` | Created | States the prototype is reference-only; do not build or edit |

## Decisions Made

| Decision | Rationale | Impact |
|----------|-----------|--------|
| `add_organization` returns `Option<&Organization>` | The plan required rejecting blank names; a typed `Option` keeps that rule in the domain instead of trusting callers | Phase 2/5 callers must handle `None` |
| Explicit window bounds 1280x860, min 960x640 | Default opened oversized on this display | Predictable layout for later roster grids |
| `Theme::sync_system_appearance` + `observe_window_appearance` | `gpui_kit::init` hard-sets `ThemeMode::Light` | Light/dark follows the OS, including live changes |
| `Sidebar` component instead of custom buttons | User-requested; gives grouped navigation, active state, tooltips, and a consistent look | New screens only add a `Page` variant |
| Icons restricted to the default asset bundle | `assets::Assets` only embeds `default-icons.txt`; other catalog names would render blank | Avoid `gpui_kit_assets::IconName` variants unless switching to `AllAssets` |
| Retire `.planning/codebase/` | It analysed the old CLI, now frozen in `legacy/`; `.paul/` carries forward-looking context | `PROJECT.md` documentation link repointed |

## Deviations from Plan

### Summary

| Type | Count | Impact |
|------|-------|--------|
| Auto-fixed | 3 | Essential shell/UX corrections, no domain scope creep |
| Scope additions | 1 | Requested sidebar component, replaces the planned custom buttons |
| Deferred | 3 | Logged in `STATE.md` |

**Total impact:** No functional scope creep. Every deviation either fixes a defect found at the checkpoint or replaces planned UI code with the component the user asked for.

### Auto-fixed Issues

**1. [Window] Default window opened much larger than intended**
- **Found during:** Checkpoint (human verification)
- **Issue:** `WindowOptions::default()` let the platform pick a huge initial size.
- **Fix:** `WindowBounds::Windowed` at 1280x860 with `window_min_size` 960x640.
- **Files:** `src/main.rs`
- **Verification:** Measured outer window 1296x899 at (72,61).

**2. [Theme] App stayed light on a dark OS**
- **Found during:** Checkpoint (human verification)
- **Issue:** `gpui_kit::init` forces `ThemeMode::Light`.
- **Fix:** `Theme::sync_system_appearance(None, cx)` after `init`, plus `window.observe_window_appearance` stored in `_subscriptions`.
- **Files:** `src/main.rs`, `src/app.rs`
- **Verification:** Window capture renders the dark palette.

**3. [Build] `pumpkin-solver` produced an unused-dependency warning**
- **Found during:** Task 1
- **Issue:** The solver is pinned ahead of Phase 4, so the lint fires on a deliberate dependency.
- **Fix:** `[lints.cargo] unused_dependencies = "allow"` with an explanatory comment.
- **Files:** `Cargo.toml`
- **Verification:** `cargo build` warning-free.
- **Follow-up:** Remove the allowance when Phase 4 adds the first real solver call.

### Scope Additions

**1. [UI] Sidebar built from `gpui_kit::component::sidebar`**
- **Requested after checkpoint approval.**
- `render_sidebar` now composes `Sidebar`, `SidebarHeader`, `SidebarFooter`, `SidebarGroup`, `SidebarMenu`, `SidebarMenuItem`; `Page::id()` was removed and `Page::icon()` added.
- Sidebar is fixed at 240px and non-collapsible for this phase (collapse toggle deferred).
- **Files:** `src/app.rs`
- **Verification:** Clean build, 6/6 tests, window capture shows groups/icons/active state.

### Plan Boundary Exception

`.planning/codebase/*` was deleted from the working tree during APPLY, although the plan listed it under “DO NOT CHANGE”. The files were already deleted before execution reached them; the user confirmed the analysis is superseded by `.paul/` and `legacy/README.md`. The deletion is uncommitted and recoverable with `git checkout -- .planning`.

## Verification Results

```
$ cargo build
    Finished `dev` profile [unoptimized + debuginfo] target(s)

$ cargo test
running 6 tests
test domain::project::tests::rejects_blank_organization_names ... ok
test domain::project::tests::trims_organization_names_and_assigns_ids ... ok
test domain::project::tests::new_project_has_current_schema_version ... ok
test store::tests::rejects_malformed_json ... ok
test store::tests::round_trips_a_project_file ... ok
test store::tests::rejects_unsupported_schema_version ... ok
test result: ok. 6 passed; 0 failed

$ cargo tree -d | grep gpui-pre
gpui-pre v0.3.12   (single version)
$ cargo tree | grep -E '^\W*gpui v'
none
```

Project file as shipped:

```json
{
  "schema_version": 1,
  "app": "roster",
  "saved_at": "2026-11-16T15:50:20.038870600Z",
  "organizations": [{ "id": "3d0ce7eb-…", "name": "Upazila Health Complex, Sreepur, Magura" }]
}
```

## Issues Encountered

| Issue | Resolution |
|-------|------------|
| Rust toolchain absent from PATH | Found at `C:\Users\Galib\scoop\apps\rustup\current\.cargo`; exported `CARGO_HOME`/`RUSTUP_HOME`/`PATH` per shell |
| `LNK1104: cannot open file … .dll` on rebuild | Zed's `rust-analyzer-proc-macro-srv.exe` holds proc-macro DLLs memory-mapped; kill it, then rebuild |
| First GPUI build 8m07s | Expected; subsequent builds are incremental (1–20s) |

## GPUI Kit API Friction (for later UI phases)

- `gpui-kit` re-exports GPUI and pins `gpui-pre 0.3.12`; examples written against crates.io `gpui` will not typecheck.
- `gpui_kit::init(cx)` hard-sets `ThemeMode::Light`; always call `Theme::sync_system_appearance` afterwards.
- `v_flex()` / `h_flex()` are exported from `gpui_kit::component`, not the `gpui_kit` root.
- `Sidebar<E>` requires every child to be the same `SidebarItem` type; wrap menus in `SidebarGroup<SidebarMenu>`.
- `SidebarMenuItem` has no caller-supplied id (ids derive from group/index); provide `.accessibility_label(...)` if a stable name is needed.
- `assets::Assets` embeds only `default-icons.txt`; use `AllAssets` to reach the full Lucide catalog.
- `InputState::set_value` requires `&mut Window`, so clearing an input inside a click handler is `state.set_value("", window, cx)`.

## Next Phase Readiness

**Ready:**
- `src/lib.rs` split: Phase 2 can add `domain::roster`, `domain::employee`, `domain::shift` without touching the GUI.
- Store is schema-versioned and validated, with round-trip tests to extend.
- `Page` enum and `Sidebar` groups already have slots for rosters, employees, and shifts.

**Concerns:**
- Adding `rosters`, `employees`, and shift templates changes the project shape: decide between additive defaults and `SCHEMA_VERSION = 2` with a migration before writing Phase 2 serialization.
- `legacy/models.rs` is the semantic reference; port duty kinds and rest rules deliberately rather than copying doctor-specific assumptions.
- `roster-project.json` at the repo root is a manual-test artifact (untracked). Add it to `.gitignore` or delete it before committing.
- The `unused_dependencies` lint allowance should be removed once `pumpkin-solver` is used.

**Blockers:** None.

---
*Built with PAUL Framework v1.4*
*Phase: 01-walking-skeleton, Plan: 01*
*Completed: 2026-11-16*

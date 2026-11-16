---
description: "roster — current position and accumulated context"
type: ProjectState
about: "roster"
---

# Project State

## Project Reference

See: .paul/PROJECT.md (updated 2026-11-16)

**Core value:** Admins get fair, conflict-free duty rosters for each staff group automatically — no double-booking, no leave/rest violations, equitable workload.
**Current focus:** v0.1 Application Core — Phase 2 Domain Model

## Current Position

Milestone: v0.1 Application Core
Phase: 2 of 8 (Domain Model) — Ready to plan
Plan: Not started
Status: Ready to plan
Last activity: 2026-11-16 — Phase 1 complete, transitioned to Phase 2

Progress:
- Milestone: [█░░░░░░░░░] 13% (1 of 8 phases)
- Phase 2: [░░░░░░░░░░] 0%

## Loop Position

Current loop state:
```
PLAN ──▶ APPLY ──▶ UNIFY
  ○        ○        ○     [Loop complete — ready to plan Phase 2]
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

### Deferred Issues

| Issue | Origin | Effort | Revisit |
|-------|--------|--------|---------|
| PDF/PNG export | Init | M | Phase 8 |
| Native file dialogs for open/save | Phase 1 | S | Phase 5 (management UI) |
| Sidebar is fixed-width (no collapse toggle) | Phase 1 apply | S | Phase 5/6 if the roster grid needs more width |
| Flattening the repo's original CLI prototype | Phase 1 | S | Phase 2 when semantics are ported |
| Windows build: rust-analyzer can hold proc-macro DLLs in `target/` (LNK1104) | Phase 1 | S | Kill `rust-analyzer-proc-macro-srv.exe` or close Zed before a CLI build |

### Git State

Last phase commit: `9a76ee8` — feat(walking-skeleton): GPUI shell with Sidebar navigation and JSON project persistence
Branch: main
Feature branches merged: none

### Blockers/Concerns

| Blocker | Impact | Resolution Path |
|---------|--------|-----------------|
| Fairness as a CP objective in Pumpkin | Core differentiator; may need auxiliary deviation variables | Phase 4 research spike on a small roster |
| Scope size (8 phases) | Risk of never shipping | Each phase ends with a runnable app; no phase exceeds 3 plans |
| Project shape changes in Phase 2 (rosters, employees, shifts) | Phase 1 files must not break | Decide at Phase 2 planning: additive `#[serde(default)]` fields vs `SCHEMA_VERSION = 2` + migration |
| `pumpkin-solver` pinned but unused | `unused_dependencies` lint is silenced | Remove `[lints.cargo]` allowance when Phase 4 calls the solver |

## Boundaries (Active)

Phase 2 (from ROADMAP.md):
- `legacy/*` frozen — do not edit or delete the prototype
- Domain types only; no solver calls yet (Phase 4)
- Preserve schema_version 1 compatibility or add an explicit migration
- No new dependencies without stopping to ask

## Session Continuity

Last session: 2026-11-16
Stopped at: Phase 1 complete, ready to plan Phase 2 (Domain Model)
Next action: Run /paul:plan for Phase 2
Resume file: .paul/phases/01-walking-skeleton/01-01-SUMMARY.md

---
*STATE.md — Updated after every significant action*

<!-- refreshed: 2026-11-13 -->
# Architecture

**Analysis Date:** 2026-11-13

## System Overview

```text
┌─────────────────────────────────────────────────────────────┐
│                     CLI Binary (main)                        │
│                      `src/main.rs`                           │
│   Hardcoded problem inputs: dates, context assignment        │
└──────────────────────────┬──────────────────────────────────┘
                           │
         ┌─────────────────┴──────────────────┐
         ▼                                    ▼
┌────────────────────────┐        ┌────────────────────────┐
│   Input Construction   │        │   Domain + Solver      │
│    `src/utils.rs`      │        │    `src/models.rs`     │
│  - get_doctors_list()  │──────▶│  - RosterProblem       │
│  - generate_slots()    │        │    .solve()            │
└────────────────────────┘        │  - Doctor constraints  │
                                  │  - Backtracking search │
                                  └──────────┬─────────────┘
                                             │
                                             ▼
                                  ┌────────────────────────┐
                                  │        Output           │
                                  │  Roster.print()         │
                                  │  (stdout text table)    │
                                  └────────────────────────┘
```

## Component Responsibilities

| Component | Responsibility | File |
|-----------|----------------|------|
| `main` | Wire hardcoded inputs, trigger solve, print result | `src/main.rs` |
| `get_doctors_list` | Build the 7-doctor roster staff with weekly/date unavailabilities, leave periods, and recurring assignments | `src/utils.rs:9` |
| `generate_slots` | Expand a date range into `DutySlot`s (2× OPD + EmergencyMorning + EmergencyNight per non-Friday day) | `src/utils.rs:63` |
| Domain model | `DutyCategory`, `DoctorId`, `Doctor`, `DutySlot`, `Unavailability`, `DateRange`, `ContextAssignment` | `src/models.rs:7-166` |
| `Doctor` | Constraint checks: `is_unavailable(slot)`, `is_recurring(slot)` | `src/models.rs:131-153` |
| `RosterProblem::prepare` | Precompute constraint index: allowed matrix, same-day pairs, night→morning and morning→prev-night rest neighbors | `src/models.rs:269-371` |
| `RosterProblem::viable_doctors` | Candidate filtering + fairness ordering (least-loaded first, recurring owner tiebreak) | `src/models.rs:374-412` |
| `RosterProblem::most_constrained_slot` | MRV slot selection; fail-fast on empty candidate set | `src/models.rs:414-437` |
| `RosterProblem::search` | Recursive backtracking with step budget (1M) | `src/models.rs:439-481` |
| `RosterProblem::solve` | Seed recurring assignments, initialize load vector, run search, wrap result | `src/models.rs:483-539` |
| `Roster` | Read-only solution view; prints schedule table + per-doctor hour totals | `src/models.rs:168-244` |

## Pattern Overview

**Overall:** Single-binary constraint solver — domain model + backtracking search (constraint satisfaction problem, CSP) in one flat module tree. No I/O layer, no persistence; all inputs are hardcoded.

**Key Characteristics:**
- Classic CSP: variables = slots, domains = viable doctors, constraints = availability / same-day / rest rules
- **Precomputation phase** (`prepare`) turns slot/doctor/context data into flat indexed structures (`allowed`, `same_day`, `next_em`, `prev_en`) so the search hot loop touches only arrays
- **MRV heuristic** (Minimum Remaining Values): always assign the slot with fewest viable doctors next
- **Fairness heuristic** in candidate ordering: least accumulated duty hours first, recurring-slot owner wins ties
- **Budget guard**: search aborts with an error after 1M recursive steps — prevents hangs on infeasible input
- Index-based indirection throughout: doctors and slots are referenced by `usize` index, not by ID, inside the solver

## Layers

**Entry/wiring layer:**
- Purpose: assemble the problem from hardcoded constants and run it
- Location: `src/main.rs`
- Contains: `main()`, date range, context assignment (previous night's doctor)
- Depends on: `models`, `utils`
- Used by: binary invocation only

**Input construction layer:**
- Purpose: pure factory functions producing `Doctor` and `DutySlot` vectors
- Location: `src/utils.rs`
- Contains: `get_doctors_list()`, `generate_slots(start, end)`
- Depends on: `models` types
- Used by: `main`

**Domain + solver layer:**
- Purpose: everything else — types, constraint semantics, indexing, search, output
- Location: `src/models.rs`
- Contains: all public types, `RosterProblem` solver, `Roster` printer
- Depends on: `chrono` only
- Used by: `main`, `utils`

## Data Flow

### Primary Request Path

1. `main()` builds doctors via `get_doctors_list()` (`src/main.rs:12` → `src/utils.rs:9`)
2. `generate_slots(2026-11-16, 2026-11-30)` expands the date range into slots (`src/main.rs:15` → `src/utils.rs:63`)
3. Context assignment (Nov 15 EmergencyNight doctor) is hardcoded in a `Vec<ContextAssignment>` (`src/main.rs:16-20`)
4. `RosterProblem` struct is assembled (`src/main.rs:21-25`)
5. `solve()` → `prepare()` builds constraint indexes, resolving context entries to doctor indexes (`src/models.rs:483` → `src/models.rs:269`)
6. Recurring assignments are seeded into `assigned` with conflict detection and unavailability warnings (`src/models.rs:488-512`)
7. Load vector initialized from seeded assignments (`src/models.rs:514-519`)
8. `search()` backtracks with MRV + fairness ordering until all slots assigned or budget exhausted (`src/models.rs:530` → `src/models.rs:439`)
9. `Roster` is returned with `doctor_of_slot: Vec<usize>` aligned to `problem.slots` (`src/models.rs:532-538`)
10. `roster.print()` renders a per-day schedule table and per-doctor hour totals to stdout (`src/models.rs:184`)

### Rest Rule (the core constraint)

- A doctor on `EmergencyNight` for date D must not take `EmergencyMorning` on D+1 (`next_em` map, `src/models.rs:315-331`)
- Checked from both directions: `next_em` for night slots, `prev_en` for morning/OPD slots, so it holds regardless of assignment order (`src/models.rs:389-403`)
- When the neighboring slot is outside the problem range, the neighbor resolves to `Neighbor::FixedDoctor` from the `context` list — this is how the Nov 15 night hand-off is enforced

**State Management:**
- No global or persistent state. The solver mutates two local stacks: `assigned: Vec<Option<usize>>` (one doctor index per slot) and `load: Vec<u32>` (accumulated duty hours per doctor), both undone on backtrack (`src/models.rs:464-475`)

## Key Abstractions

**`DutyCategory` (enum):**
- Purpose: kind of duty slot — `Opd` (6h), `EmergencyMorning` (7h), `EmergencyNight` (17h)
- Examples: `src/models.rs:8-22`
- Pattern: enum + match-based `hours()` accessor

**`DoctorId` (enum):**
- Purpose: closed set of the 7 staff doctors; maps to display names via `name()`
- Examples: `src/models.rs:60-83`
- Pattern: enum-as-identifier (adding a doctor requires editing this enum — see CONCERNS in STRUCTURE.md "where to add new code")

**`Unavailability` (enum):**
- Purpose: `WholeDay` or `Category(DutyCategory)` exclusion scope, attached to either a weekday (`WeeklyUnavailability`) or a specific date (`DateUnavailability`)
- Examples: `src/models.rs:24-40`

**`Neighbor` (private enum):**
- Purpose: unifies rest-rule lookups — either another slot index in the problem (`Slot`) or a pinned doctor from context (`FixedDoctor`)
- Examples: `src/models.rs:246-250`

**`Prepared` (private struct):**
- Purpose: precomputed constraint index consumed by the search loop
- Examples: `src/models.rs:252-259`

**`Roster<'a>` (borrowed solution):**
- Purpose: zero-copy solution view borrowing the problem; owns only the assignment vector
- Examples: `src/models.rs:168-244`
- Pattern: lifetime-bound view over input data

## Entry Points

**`main`:**
- Location: `src/main.rs:11`
- Triggers: `cargo run` (binary `roster`)
- Responsibilities: build doctors, generate slots for Nov 16–30 2026, hardcode the Nov 15 context assignment, solve, print. Panics via `expect` if no feasible roster exists

## Architectural Constraints

- **Threading:** single-threaded; no async, no threads
- **Global state:** none — everything flows through `RosterProblem` and its methods
- **Circular imports:** none; `utils.rs` and `main.rs` both depend on `models.rs`, one-directional
- **Solver errors as `String`:** all solver errors are `Result<_, String>` (`src/models.rs:483`), no custom error types
- **Fixed doctor set:** `DoctorId` is a compile-time enum — staff changes are source-code changes
- **Input hardcoded in code:** date ranges, unavailabilities, leave periods, and context all live in `src/main.rs` and `src/utils.rs`; there is no config file or CLI argument parsing
- **Slot generation rule:** Fridays get no OPD slots (`src/utils.rs:68`) — encoded in `generate_slots`

## Anti-Patterns

### Hardcoded domain data in source

**What happens:** doctor roster, leave dates, and problem date range are literal values in `src/utils.rs` and `src/main.rs`
**Why it's wrong here:** every scheduling period requires a code edit + recompile; dates like `2026-11-16` are baked in (`src/main.rs:13-14`, `src/utils.rs:29-48`)
**Do this instead:** externalize inputs to a config file (TOML/JSON) or CLI args, deserialized into `RosterProblem` fields which already exist as the natural boundary

### Commented-out debug/experimental code

**What happens:** blocks of dead code left commented in place (`src/models.rs:258, 277-282, 520-528`, `src/utils.rs:53-56`)
**Why it's wrong here:** obscures the real data flow; duplicated comment lines (`src/models.rs:313-314, 333-334`)
**Do this instead:** delete; rely on `eprintln!`-gated trace or git history for temporary diagnostics

## Error Handling

**Strategy:** `Result` with `String` error payloads inside the solver; `expect`/panic at the binary edge; `eprintln!` for non-fatal warnings

**Patterns:**
- Fatal at entry: `roster_problem.solve().expect("no feasible roster found")` (`src/main.rs:26`)
- Solver errors: infeasibility messages constructed with `format!` describing the slot and failure cause (`src/models.rs:477-480`, `src/models.rs:501-507`)
- Budget exhaustion: `Err("search budget exhausted (1M steps)")` (`src/models.rs:449`)
- Non-fatal notes: `eprintln!("note: ... skipping")` when a recurring owner is unavailable (`src/models.rs:494-499`)

## Cross-Cutting Concerns

**Logging:** none; only `eprintln!` notes in `solve()` (`src/models.rs:494`) and a commented-out debug block (`src/models.rs:520-528`)
**Validation:** context doctor IDs validated against the doctor list during `prepare()` (`src/models.rs:295-299`); recurring conflicts detected during seeding (`src/models.rs:500-507`)
**Authentication:** Not applicable (offline CLI)

---

*Architecture analysis: 2026-11-13*

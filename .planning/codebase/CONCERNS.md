# Codebase Concerns

**Analysis Date:** 2026-11-13

## Tech Debt

**Hardcoded problem data in source code:**
- Issue: All doctors, their unavailabilities, leave periods, and recurring assignments are hardcoded in `get_doctors_list()` (`src/utils.rs:9-61`). The roster period (Nov 16–30, 2026) and context assignment (Nov 15 night) are hardcoded `NaiveDate` literals in `main()` (`src/main.rs:13-20`). Every new roster cycle requires editing and recompiling source code.
- Files: `src/utils.rs`, `src/main.rs:13-20`
- Impact: The tool is unusable as a general roster generator; non-programmers cannot update constraints; the Nov 15 context line still contains a placeholder comment (`src/main.rs:19` — "put the real Nov 15 night doctor here"), so the committed data is known-stale.
- Fix approach: Load doctor config and period from a data file (TOML/JSON/CSV) or CLI args. `DoctorId` as a fixed enum (`src/models.rs:60-69`) also needs to become a string-backed id or be derived from the data file.

**Dead code and commented-out blocks:**
- Issue: Multiple commented-out code remnants: duplicate comment lines (`src/models.rs:313-314`, `333-334`), the unused `context` field in `Prepared` (`src/models.rs:258`, `369`), the `opd_of` map skeleton (`src/models.rs:277`, `282-283`), a debug diagnostic block in `solve()` (`src/models.rs:520-528`), and Galib's leave period (`src/utils.rs:53-56`).
- Files: `src/models.rs`, `src/utils.rs`
- Impact: Noise obscures the actual constraint logic; the `Prepared` struct carries a placeholder for a field that was removed.
- Fix approach: Delete dead blocks; if the diagnostic block is useful, move it behind a `debug_assert` or a `--verbose` flag rather than commenting it out.

**Panic-heavy error handling at the boundary:**
- Issue: `main()` uses `.expect()` on the solver result and `from_ymd_opt` unwraps (`src/main.rs:13-14,17,26`). `Roster::solve()` uses `.expect("search succeeded but left a slot empty")` (`src/models.rs:536`) — a defensive panic guarding a solver invariant rather than returning `Result`.
- Files: `src/main.rs`, `src/models.rs:532-538`
- Impact: On infeasible input the user gets a raw panic message instead of a readable explanation of which slot/date failed.
- Fix approach: Propagate the `Result<String>` error from `solve()` and print it with context (which date/category failed); keep unwraps only for compile-time-constant dates or use `const`-safe parsing.

## Known Bugs

**None confirmed at runtime** (no tests exist and the binary has no recorded failures), but the following are latent bugs / correctness risks:

**Search budget can abort a solvable roster:**
- Symptoms: `Err("search budget exhausted (1M steps)")` even when a feasible roster exists.
- Files: `src/models.rs:445-450`, `529`
- Trigger: Larger date ranges or tighter constraints — the exhaustive backtracking search explores up to 1,000,000 nodes; the budget is decremented *before* the completion check, so even the final nodes consume budget.
- Workaround: Raise the `1_000_000` constant in `src/models.rs:529`.
- Fix approach: Make budget configurable, decrement after the completion check, and on exhaustion return a partial roster or a diagnostic of the deepest unresolved slot instead of a bare error string.

**Leave period with reversed bounds silently ignored:**
- Symptoms: `add_leave_period(start, end)` with `start > end` creates a `DateRange` that matches no dates; the doctor appears available when they are not.
- Files: `src/models.rs:127-129, 43-52`
- Trigger: Any data-entry mistake (easy when dates are handwritten literals in `src/utils.rs`).
- Workaround: None — silent.
- Fix approach: Return `Result` or `debug_assert!(start <= end)` in `add_leave_period`.

**Duplicate recurring-assignment detection is order-dependent:**
- Symptoms: If two doctors are both recurring for the same slot, the error fires only after iterating in `doctors` order; the second is reported as the conflict. Correct, but if a doctor has the *same* recurring assignment pushed twice in `src/utils.rs`, the slot is silently double-assigned to the same doctor (the `assigned[s] == Some(d)` branch compares against `other` without checking `other == d`... actually it reports it as a conflict between two doctors, which mislabels a self-duplicate).
- Files: `src/models.rs:488-512`
- Trigger: Duplicate `add_recurring_assignment` calls for the same (weekday, category) on one doctor.
- Fix approach: Deduplicate `recurring_assignments` on insert in `src/models.rs:122-125`.

## Security Considerations

**Minimal attack surface (offline CLI):**
- Risk: Very low. The binary reads no network, no files, no env secrets; it only prints to stdout.
- Files: `src/main.rs`
- Current mitigation: N/A — nothing to mitigate.
- Recommendations: None required for current scope. If the planned "load from file" feature is added, validate input (dates, category names) rather than unwrapping.

**No secrets present:** No `.env` or credential files in the repo. `.gitignore` exists (covers `target/`).

## Performance Bottlenecks

**Backtracking search recomputes candidates for every slot at every node:**
- Problem: `most_constrained_slot` calls `viable_doctors` for *every* unassigned slot at *every* search node; `viable_doctors` scans all doctors and their `same_day` neighbor lists. Cost per node is O(slots² × doctors) worst case.
- Files: `src/models.rs:414-437` (`most_constrained_slot`), `374-412` (`viable_doctors`)
- Cause: No incremental constraint propagation or memoization; forward-checking is recomputed from scratch.
- Improvement path: Cache candidate lists and invalidate only slots whose constraints change on assign/undo (rest-rule and same-day neighbors are already materialized in `Prepared` — the hook points exist at `src/models.rs:303-351`). For the current 15-day / ~90-slot problem the budget absorbs this; for a full month it may not.

**`same_day` built with O(n²) pairwise loop:**
- Problem: For each date, all slot pairs are linked (`src/models.rs:303-311`). Fine at 3–4 slots/day; quadratic in slots-per-day if categories grow.
- Files: `src/models.rs:303-311`
- Improvement path: Group slots by date and link via the date's slot list without nested pair loops, or keep as-is given the small constant.

## Fragile Areas

**The solver core (`prepare` / `viable_doctors` / `search`):**
- Files: `src/models.rs:269-481`
- Why fragile: The rest rule (night → next morning) is enforced *bidirectionally* (`next_em` forward, `prev_en` backward, `src/models.rs:315-351`) and the same-day rule relies on the sibling-slot graph. A subtle change — e.g., adding a new category with different hours, or a slot appearing twice for one category on a date (note `em_of`/`en_of` are `HashMap<NaiveDate, usize>` and silently keep the *last* duplicate, `src/models.rs:278-291`) — breaks correctness with no error. `generate_slots` currently never duplicates EM/EN per date, but nothing enforces that invariant.
- Safe modification: When touching the solver, keep `Prepared` as the single source of neighbor topology; add a validation pass in `prepare()` that rejects duplicate (date, category) slots.
- Test coverage: None. Zero tests in the repo.

**`Roster::print()` assumes slots are date-ordered:**
- Files: `src/models.rs:184-243` (comment at 185-186 acknowledges this)
- Why fragile: If `generate_slots` ever emits unsorted slots, the printed table loses days and the per-date grouping silently drops assignments.
- Safe modification: Either sort a copy of indices by date in `print()` or document/enforce the invariant in `generate_slots` (`src/utils.rs:63-78`).

## Scaling Limits

**Solver budget vs. horizon length:**
- Current capacity: ~15 days (≈90 slots) with 7 doctors within a 1M-node budget.
- Limit: `src/models.rs:529` — 1M search steps; exhaustive DFS on a month-plus horizon with tight availability may blow the budget or take minutes.
- Scaling path: Incremental forward-checking (see Performance), or switch to an OR-tools/CP-style solver or greedy + local repair if horizons grow.

**Fixed doctor set:**
- Current capacity: 7 doctors, `DoctorId` enum variants in `src/models.rs:61-69`.
- Limit: Adding a doctor requires an enum variant, a `name()` arm (`src/models.rs:71-83`), and a constructor block in `src/utils.rs`.
- Scaling path: Data-driven doctor definitions (see Tech Debt #1).

## Dependencies at Risk

**None significant.** Single runtime dependency: `chrono = "0.4.45"` (`Cargo.toml:7`) — actively maintained, no known risk. Note: `edition = "2024"` (`Cargo.toml:4`) requires a recent toolchain (repo was built with rustc 1.99.0); building on older stable Rust will fail.

## Missing Critical Features

**No input/configuration mechanism:**
- Problem: No CLI args, no config file, no stdin. Period, doctors, and constraints are compiled in.
- Blocks: Reusing the tool for any week other than Nov 16–30, 2026, without a code change.

**No tests:**
- Problem: Zero test files; the constraint solver (the entire value of the project) is untested.
- Blocks: Safe refactoring of `src/models.rs`; any solver change risks silently producing rosters that violate the rest/same-day rules.

**No CI / lint / format config:**
- Problem: No `.github/` workflows, no `rustfmt.toml`, no clippy configuration; repo has no commits yet (`main` has zero history).
- Blocks: Regression detection and consistent style as the project grows.

**No partial-roster output on infeasibility:**
- Problem: When `solve()` fails, the user gets one error string with no indication of *which* constraints made it infeasible or how far the search got (a diagnostic block for this exists but is commented out, `src/models.rs:520-528`).
- Blocks: Practical debugging of over-constrained weeks by non-developers.

## Test Coverage Gaps

**Entire codebase:**
- What's not tested: Everything — `RosterProblem::solve`, the rest rule (both `next_em` and `prev_en` directions, `src/models.rs:315-351`), the same-day rule (`src/models.rs:303-311, 385-388`), recurring-assignment seeding and conflict detection (`src/models.rs:488-512`), fairness/heuristics in `viable_doctors` (`src/models.rs:407-411`), `generate_slots` Friday exclusion and 2×OPD per non-Friday day (`src/utils.rs:63-78`), `is_unavailable` scope matching (`src/models.rs:131-147`), and `print()` output (`src/models.rs:184-243`).
- Files: `src/models.rs`, `src/utils.rs`, `src/main.rs`
- Risk: Any refactor can produce constraint-violating rosters that look plausible — the worst failure mode for a scheduling tool.
- Priority: High — before any solver refactor, add unit tests for: (1) infeasible detection, (2) rest-rule across the context boundary (a Nov-15-style context assignment is the only way to test `Neighbor::FixedDoctor`), (3) least-loaded tie-breaking, (4) `generate_slots` shape (90 slots for Nov 16–30: 15 days × 2 emergency + 13 non-Friday days × 2 OPD = 56 slots total... verify against `src/utils.rs:63-78`).

---

*Concerns audit: 2026-11-13*

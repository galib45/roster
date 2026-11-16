# Legacy prototype (frozen)

These files are the original command-line roster generator, kept as a
**non-compiled reference**. Nothing here is part of the `roster` crate; Cargo
does not build them.

Why they are kept:

- `models.rs` documents the domain semantics the app must preserve: duty
  categories and hours, per-doctor weekly/date unavailability, leave periods,
  recurring assignments, the same-day overlap rule, the night → next-morning
  rest rule, and how the previous day's assignment is supplied as context.
- `utils.rs` shows the hardcoded doctor set and slot generation (including the
  no-OPD-on-Friday rule).
- `main.rs` shows how the problem is assembled and solved.

Phase 2 ports these semantics into the real domain model. Until then:

- **Do not edit, reformat, or delete these files.**
- Do not add them to `Cargo.toml`.

Git history is the source of truth if a file is ever removed.

//! Roster — domain model, project persistence, and solver.
//!
//! The GUI binary lives in `src/main.rs`; everything testable lives here so
//! `cargo test` runs the domain and storage checks without launching a window.

pub mod app;
pub mod domain;
pub mod store;

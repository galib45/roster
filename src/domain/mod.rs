//! Core domain types for a roster project.
//!
//! Phase 1 intentionally models only what the walking skeleton persists.
//! Rosters, employees, shifts, and constraints arrive in Phase 2.

pub mod project;

pub use project::{Organization, ProjectFile, SCHEMA_VERSION};

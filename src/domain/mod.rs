//! Core domain types for a roster project.
//!
//! Ownership is intentionally explicit:
//!
//! ```text
//! ProjectFile
//! └── Organization
//!     ├── positions
//!     ├── employees   (each with a roster_id)
//!     └── rosters
//!         └── shifts
//! ```
//!
//! Rosters own shifts only. Employees and positions live on the organization,
//! with `Employee.roster_id` acting as the staff-isolation boundary.

pub mod employee;
pub mod error;
pub mod migration;
pub mod organization;
pub mod position;
pub mod project;
pub mod roster;
pub mod shift;

pub use employee::Employee;
pub use error::DomainError;
pub use organization::Organization;
pub use position::Position;
pub use project::{ProjectFile, SCHEMA_VERSION};
pub use roster::Roster;
pub use shift::{ShiftTemplate, StaffingRequirement};

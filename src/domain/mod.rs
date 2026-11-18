//! Core domain types for a roster project.
//!
//! Ownership is intentionally explicit:
//!
//! ```text
//! ProjectFile
//! └── Organization
//!     ├── positions
//!     ├── employees   (each with a roster_id, unavailability, recurring assignments)
//!     └── rosters
//!         ├── shifts
//!         ├── rest_rules
//!         ├── sharing_groups
//!         └── plans
//! ```
//!
//! Rosters own shifts and rest rules. Employees and positions live on the
//! organization, with `Employee.roster_id` acting as the staff-isolation
//! boundary. Generated plans are added by 02-03.

pub mod constraint;
pub mod employee;
pub mod error;
pub mod migration;
pub mod organization;
pub mod plan;
pub mod position;
pub mod project;
pub mod roster;
pub mod sharing;
pub mod shift;

pub use constraint::{
    RecurringAssignment, RestRule, Severity, Unavailability, UnavailabilityScope,
};
pub use employee::Employee;
pub use error::DomainError;
pub use organization::Organization;
pub use plan::{ContextAssignment, DatedShift, PlanStatus, RosterPlan, ShiftSlot};
pub use position::Position;
pub use project::{ProjectFile, SCHEMA_VERSION};
pub use roster::Roster;
pub use sharing::SharingGroup;
pub use shift::{ShiftTemplate, StaffingRequirement};

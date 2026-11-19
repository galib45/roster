//! Core domain types for a roster project.
//!
//! Ownership is intentionally explicit:
//!
//! ```text
//! ProjectFile
//! └── Organization
//!     ├── positions
//!     ├── employees   (each holding one position, plus rules)
//!     └── roster_templates
//!         ├── positions (included automatically)
//!         ├── excluded_employee_ids
//!         ├── shifts
//!         ├── rest_rules
//!         ├── sharing_groups
//!         └── rosters   (dated instances with snapshot members)
//! ```
//!
//! Templates gather their staff from the positions they include and may exclude
//! individual employees. Dated rosters are created from a template for a start
//! and end date and snapshot their members and dated shifts.

pub mod constraint;
pub mod employee;
pub mod error;
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
pub use plan::{ContextAssignment, DatedShift, PlanStatus, Roster, RosterMember, ShiftSlot};
pub use position::Position;
pub use project::{ProjectFile, SCHEMA_VERSION};
pub use roster::RosterTemplate;
pub use sharing::SharingGroup;
pub use shift::{ShiftTemplate, StaffingRequirement};

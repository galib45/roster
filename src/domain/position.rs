//! Positions: organization-level labels shared by employees and shifts.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::DomainError;

/// A role or skill group such as "Medical Officer" or "Senior Staff Nurse".
///
/// Positions belong to the organization, not to a roster, so both an
/// employee and a shift's staffing requirement can reference the same one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    /// Stable identifier, independent of the display name.
    pub id: String,
    pub name: String,
}

impl Position {
    /// Create a position from a display name.
    ///
    /// The name is trimmed; a blank name is rejected rather than stored.
    pub fn new(name: &str) -> Result<Self, DomainError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(DomainError::BlankName);
        }

        Ok(Self {
            id: Uuid::new_v4().to_string(),
            name: name.to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_position_names_and_assigns_ids() {
        let position = Position::new("  Medical Officer ").unwrap();

        assert_eq!(position.name, "Medical Officer");
        assert!(!position.id.is_empty());
    }

    #[test]
    fn rejects_blank_position_names() {
        assert_eq!(Position::new("   "), Err(DomainError::BlankName));
    }
}

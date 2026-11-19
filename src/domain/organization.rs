//! Organizations: the root of the domain graph.
//!
//! An organization owns positions, employees, and roster templates, but it does
//! not implement their behavior. Each structure owns its own mutations:
//! employees add availability rules, templates manage positions/shifts/rosters,
//! and dated rosters assign their own seats. The organization provides
//! ownership, lookups, and the derived membership query.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{DomainError, Employee, Position, Roster, RosterMember, RosterTemplate, ShiftTemplate};

/// A hospital, clinic, or department that owns staff and roster templates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Organization {
    /// Stable identifier, independent of the display name.
    pub id: String,
    pub name: String,
    /// Organization-level positions referenced by employees and shifts.
    #[serde(default)]
    pub positions: Vec<Position>,
    /// Organization-level staff, each holding exactly one position.
    #[serde(default)]
    pub employees: Vec<Employee>,
    /// Reusable roster definitions, each owning shifts and dated rosters.
    #[serde(default)]
    pub roster_templates: Vec<RosterTemplate>,
}

impl Organization {
    /// Create an empty organization.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            positions: Vec::new(),
            employees: Vec::new(),
            roster_templates: Vec::new(),
        }
    }

    /// Find a position by ID.
    pub fn position(&self, id: &str) -> Option<&Position> {
        self.positions.iter().find(|position| position.id == id)
    }

    /// Find an employee by ID.
    pub fn employee(&self, id: &str) -> Option<&Employee> {
        self.employees.iter().find(|employee| employee.id == id)
    }

    /// Find an employee by ID for mutation.
    pub fn employee_mut(&mut self, id: &str) -> Option<&mut Employee> {
        self.employees.iter_mut().find(|employee| employee.id == id)
    }

    /// Find a roster template by ID.
    pub fn roster_template(&self, id: &str) -> Option<&RosterTemplate> {
        self.roster_templates
            .iter()
            .find(|template| template.id == id)
    }

    /// Find a roster template by ID for mutation.
    pub fn roster_template_mut(&mut self, id: &str) -> Option<&mut RosterTemplate> {
        self.roster_templates
            .iter_mut()
            .find(|template| template.id == id)
    }

    /// Find a shift template anywhere in the organization.
    pub fn shift_template(&self, shift_id: &str) -> Option<&ShiftTemplate> {
        self.roster_templates
            .iter()
            .find_map(|template| template.shift(shift_id))
    }

    /// The employees a template currently gathers.
    ///
    /// Membership is computed: every employee whose position the template
    /// includes, minus explicit exclusions. An unknown template yields an
    /// empty list.
    pub fn template_members(&self, template_id: &str) -> Vec<&Employee> {
        let Some(template) = self.roster_template(template_id) else {
            return Vec::new();
        };

        self.employees
            .iter()
            .filter(|employee| {
                template.includes_position(&employee.position_id)
                    && !template.excludes_employee(&employee.id)
            })
            .collect()
    }

    /// The template's current members as snapshot pairs, ready for
    /// [`RosterTemplate::create_roster`].
    pub fn template_member_snapshot(&self, template_id: &str) -> Vec<RosterMember> {
        self.template_members(template_id)
            .into_iter()
            .map(|employee| RosterMember::new(&employee.id, &employee.position_id))
            .collect()
    }

    /// Add an organization-level position.
    pub fn add_position(&mut self, name: &str) -> Result<&Position, DomainError> {
        let name = name.trim();
        if self.positions.iter().any(|position| position.name == name) {
            return Err(DomainError::DuplicatePosition {
                name: name.to_owned(),
            });
        }

        self.positions.push(Position::new(name)?);
        Ok(self.positions.last().expect("a position was just pushed"))
    }

    /// Add a roster template.
    pub fn add_roster_template(&mut self, name: &str) -> Result<&RosterTemplate, DomainError> {
        let name = name.trim();
        if self
            .roster_templates
            .iter()
            .any(|template| template.name == name)
        {
            return Err(DomainError::DuplicateRosterTemplate {
                name: name.to_owned(),
            });
        }

        self.roster_templates.push(RosterTemplate::new(name)?);
        Ok(self
            .roster_templates
            .last()
            .expect("a roster template was just pushed"))
    }

    /// Add an employee holding an organization position.
    pub fn add_employee(
        &mut self,
        name: &str,
        position_id: &str,
    ) -> Result<&Employee, DomainError> {
        if self.position(position_id).is_none() {
            return Err(DomainError::UnknownPosition {
                id: position_id.to_owned(),
            });
        }

        self.employees.push(Employee::new(name, position_id)?);
        Ok(self.employees.last().expect("an employee was just pushed"))
    }

    /// Find a dated roster by template and roster IDs.
    pub fn roster(&self, template_id: &str, roster_id: &str) -> Option<&Roster> {
        self.roster_template(template_id)?.roster(roster_id)
    }

    /// Find a dated roster by template and roster IDs for mutation.
    pub fn roster_mut(&mut self, template_id: &str, roster_id: &str) -> Option<&mut Roster> {
        self.roster_template_mut(template_id)?.roster_mut(roster_id)
    }

    /// Find a dated roster anywhere in the organization.
    pub fn dated_roster(&self, roster_id: &str) -> Option<&Roster> {
        self.roster_templates
            .iter()
            .find_map(|template| template.roster(roster_id))
    }

    /// Find a dated roster anywhere in the organization for mutation.
    pub fn dated_roster_mut(&mut self, roster_id: &str) -> Option<&mut Roster> {
        self.roster_templates
            .iter_mut()
            .find_map(|template| template.roster_mut(roster_id))
    }

    /// The template that owns a dated roster.
    pub fn template_for_roster(&self, roster_id: &str) -> Option<&RosterTemplate> {
        self.roster_templates
            .iter()
            .find(|template| template.roster(roster_id).is_some())
    }
}

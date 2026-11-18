//! Employees page: list employees for an organization, add/edit dialog.

use gpui_kit::component::{
    button::{Button, ButtonVariants},
    dialog::{Dialog, DialogButtonProps},
    form::{Field, Form},
    h_flex,
    input::{Input, InputState},
    label::Label,
    v_flex,
};
use gpui_kit::*;

use crate::app::{RosterApp, Page};
use crate::domain::DomainError;

impl RosterApp {
    pub fn render_employees(&self, cx: &mut Context<Self>, window: &Window) -> AnyElement {
        let org_id = match &self.page {
            Page::Employees { org_id } => org_id,
            _ => return div().into_any_element(),
        };

        let Some(organization) = self.project.organization(org_id) else {
            return div()
                .child(Label::new("Organization not found"))
                .into_any_element();
        };

        // Build position options
        let positions: Vec<String> = organization
            .positions
            .iter()
            .map(|p| p.id.clone())
            .collect();

        // Build roster options
        let rosters: Vec<String> = organization
            .rosters
            .iter()
            .map(|r| r.id.clone())
            .collect();

        // Add employee button
        let add_row = div()
            .flex()
            .items_center()
            .gap_2()
            .child(
                Button::new("add-employee")
                    .primary()
                    .label("Add Employee")
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.show_employee_dialog = true;
                        this.editing_employee_id = None;
                        this.employee_name_input.update(cx, |state, cx| {
                            state.set_value("", window, cx);
                        });
                        this.employee_position.update(cx, |state, cx| {
                            state.set_value("", window, cx);
                        });
                        this.employee_roster.update(cx, |state, cx| {
                            state.set_value("", window, cx);
                        });
                        this.employee_skills_input.update(cx, |state, cx| {
                            state.set_value("", window, cx);
                        });
                        this.show_employee_dialog = true;
                        cx.notify();
                    })),
            )
            .into_any_element();

        let mut employees = Vec::new();
        for employee in organization.employees_in_roster(org_id) {
            let emp_id = employee.id.clone();
            let emp_name = employee.name.clone();
            let emp_position = employee.position_id.clone();
            let emp_roster = employee.roster_id.clone();
            employees.push(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(Label::new(format!("{} ({})", emp_name, emp_position)))
                    .child(Label::new(format!("Roster: {}", emp_roster)))
                    .child(
                        Button::new(format!("edit-emp-{}", emp_id))
                            .ghost()
                            .label("Edit")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.editing_employee_id = Some(emp_id.clone());
                                this.show_employee_dialog = true;
                                cx.notify();
                            })),
                        )
                    .into_any_element(),
            );
        }

        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(Label::new(format!("Employees — {}", organization.name)))
                    .into_any_element(),
            )
            .child(add_row)
            .child(div().flex().flex_col().gap_2().children(employees))
            .child(self.render_employee_dialog(cx, window))
            .into_any_element()
    }

    fn render_employee_dialog(&self, cx: &mut Context<Self>, _window: &Window) -> AnyElement {
        if !self.show_employee_dialog {
            return div().into_any_element();
        }

        let title = if self.editing_employee_id.is_some() { "Edit Employee" } else { "Add Employee" };

        // Get org_id for fetching positions/rosters
        let org_id = self.page.org_id().unwrap_or_default();

        let dialog = Dialog::new(cx)
            .title(if self.editing_employee_id.is_some() { "Edit Employee" } else { "Add Employee" })
            .child(
                Form::new()
                    .child(
                        Field::new()
                            .label("Name")
                            .child(Input::new(&self.employee_name_input)),
                    )
                    .child(
                        Field::new()
                            .label("Position ID")
                            .child(Input::new(&self.employee_position)),
                    )
                    .child(
                        Field::new()
                            .label("Roster ID")
                            .child(Input::new(&self.employee_roster)),
                    )
                    .child(
                        Field::new()
                            .label("Skills (comma-separated)")
                            .child(Input::new(&self.employee_skills_input)),
                    )
                    .footer(
                        div()
                            .flex()
                            .gap_2()
                            .child(Button::new("cancel-employee-edit")
                                .label("Cancel")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.show_employee_dialog = false;
                                    this.editing_employee_id = None;
                                    cx.notify();
                                })),
                            )
                            .child(Button::new("save-employee-edit")
                                .primary()
                                .label("Save")
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    // Get values from InputState using value() method
                                    let name = this.employee_name_input.read(cx).value().trim().to_owned();
                                    if name.is_empty() {
                                        this.status = "Enter an employee name first".into();
                                        cx.notify();
                                        return;
                                    }

                                    let position_id = this.employee_position.read(cx).value().trim().to_owned();
                                    let roster_id = this.employee_roster.read(cx).value().trim().to_owned();
                                    let skills = this.employee_skills_input.read(cx).value()
                                        .split(',')
                                        .map(|s| s.trim().to_string())
                                        .filter(|s| !s.is_empty())
                                        .collect();

                                    if position_id.is_empty() {
                                        this.status = "Enter a position ID".into();
                                        cx.notify();
                                        return;
                                    }
                                    if roster_id.is_empty() {
                                        this.status = "Enter a roster ID".into();
                                        cx.notify();
                                        return;
                                    }

                                    let position_id = position_id;
                                    let roster_id = roster_id;
                                    let name_clone = name.clone();
                                    let name_clone2 = name.clone();

                                    if let Some(emp_id) = this.editing_employee_id.take() {
                                        // Edit existing employee
                                        if let Some(org) = this.project.organization_mut(
                                            &this.page.org_id().unwrap_or_default()
                                        ) {
                                            if let Some(emp) = org.employee_mut(&emp_id) {
                                                emp.name = name_clone;
                                                emp.position_id = position_id;
                                                emp.roster_id = roster_id;
                                                emp.skills = skills;
                                                this.status = format!("Updated employee {name_clone}").into();
                                            }
                                        }
                                    } else {
                                        // Add new employee
                                        let org_id = this.page.org_id().unwrap_or_default();
                                        let added = this.project.organization_mut(&org_id)
                                            .and_then(|org| {
                                                org.add_employee(&name, &roster_id, &position_id).ok()?;
                                                let emp_id = org.employees.last().unwrap().id.clone();
                                                if let Some(emp) = org.employee_mut(&emp_id) {
                                                    emp.skills = skills;
                                                }
                                                Some(())
                                            })
                                            .is_some();
                                        if added {
                                            this.status = format!("Added employee {name}").into();
                                        }
                                    }

                                    this.show_employee_dialog = false;
                                    this.save_project();
                                    cx.notify();
                                })),
                            ),
                    ),
            );

        dialog.into_any_element()
    }

    pub fn render_employee_detail(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let (org_id, employee_id) = match &self.page {
            Page::EmployeeDetail { org_id, employee_id } => (org_id, employee_id),
            _ => return div().into_any_element(),
        };

        let Some(organization) = self.project.organization(org_id) else {
            return div().child(Label::new("Organization not found")).into_any_element();
        };

        if let Some(emp_id) = employee_id {
            let Some(employee) = organization.employee(emp_id) else {
                return div().child(Label::new("Employee not found")).into_any_element();
            };

            div()
                .flex()
                .flex_col()
                .gap_4()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(Label::new(format!("Edit Employee: {}", employee.name)))
                        .into_any_element(),
                )
                .child(Label::new("Employee detail coming soon"))
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_col()
                .gap_4()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(Label::new("New Employee"))
                        .into_any_element(),
                )
                .child(Label::new("Employee creation form coming soon"))
                .into_any_element()
        }
    }
}
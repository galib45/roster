//! Employees page: list employees for the selected organization and add or
//! edit them with position and roster selects.

use gpui_kit::component::{
    IndexPath,
    button::{Button, ButtonVariants},
    dialog::Dialog,
    form::{Field, Form},
    input::Input,
    label::Label,
    searchable_list::SearchableVec,
    select::{Select, SelectState},
    v_flex,
};
use gpui_kit::*;

use crate::app::RosterApp;

impl RosterApp {
    pub fn render_employees(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(organization) = self.selected_org() else {
            return v_flex()
                .gap_2()
                .child(Label::new("Employees"))
                .child(Label::new("Open an organization from the sidebar."))
                .into_any_element();
        };

        let org_name = organization.name.clone();
        let mut employee_rows = Vec::new();
        if organization.employees.is_empty() {
            employee_rows.push(Label::new("No employees yet").into_any_element());
        } else {
            for employee in &organization.employees {
                let employee_id = employee.id.clone();
                let position_name = organization
                    .position(&employee.position_id)
                    .map(|position| position.name.clone())
                    .unwrap_or_else(|| "unknown position".to_owned());

                employee_rows.push(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(Label::new(employee.name.clone()))
                        .child(Label::new(position_name.to_string()))
                        .child(
                            Button::new(format!("edit-employee-{employee_id}"))
                                .ghost()
                                .label("Edit")
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.open_employee_dialog(
                                        Some(employee_id.clone()),
                                        window,
                                        cx,
                                    );
                                })),
                        )
                        .into_any_element(),
                );
            }
        }

        v_flex()
            .gap_4()
            .child(Label::new(format!("Employees — {org_name}")))
            .child(
                Button::new("add-employee")
                    .primary()
                    .label("Add Employee")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_employee_dialog(None, window, cx);
                    })),
            )
            .child(v_flex().gap_2().children(employee_rows))
            .child(self.render_employee_dialog(cx))
            .into_any_element()
    }

    pub(crate) fn open_employee_dialog(
        &mut self,
        employee_id: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(organization) = self.selected_org() else {
            self.status = "Select an organization first".into();
            cx.notify();
            return;
        };

        let position_labels: Vec<String> = organization
            .positions
            .iter()
            .map(|position| position.name.clone())
            .collect();

        let (name, position_index) = match employee_id
            .as_deref()
            .and_then(|id| organization.employee(id))
        {
            Some(employee) => (
                employee.name.clone(),
                organization
                    .positions
                    .iter()
                    .position(|position| position.id == employee.position_id),
            ),
            None => (String::new(), None),
        };

        self.employee_position_select = cx.new(|cx| {
            SelectState::new(
                SearchableVec::new(position_labels),
                position_index.map(IndexPath::new),
                window,
                cx,
            )
        });
        self.employee_name_input.update(cx, |state, cx| {
            state.set_value(name, window, cx);
        });
        self.editing_employee_id = employee_id;
        self.show_employee_dialog = true;
        cx.notify();
    }

    fn render_employee_dialog(&self, cx: &mut Context<Self>) -> AnyElement {
        if !self.show_employee_dialog {
            return div().into_any_element();
        }

        let is_editing = self.editing_employee_id.is_some();
        let title = if is_editing {
            "Edit Employee"
        } else {
            "Add Employee"
        };

        Dialog::new(cx)
            .title(title)
            .child(
                Form::new()
                    .child(
                        Field::new()
                            .label("Name")
                            .child(Input::new(&self.employee_name_input)),
                    )
                    .child(
                        Field::new()
                            .label("Position")
                            .child(Select::new(&self.employee_position_select)),
                    )
                    .child(
                        Field::new()
                            .label("Roster")
                            .child(Select::new(&self.employee_roster_select)),
                    )
                    .footer(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                Button::new("cancel-employee-edit")
                                    .label("Cancel")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.show_employee_dialog = false;
                                        this.editing_employee_id = None;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("save-employee-edit")
                                    .primary()
                                    .label("Save")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.save_employee(cx);
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn save_employee(&mut self, cx: &mut Context<Self>) {
        let name = self.employee_name_input.read(cx).value().trim().to_owned();
        if name.is_empty() {
            self.status = "Enter an employee name first".into();
            cx.notify();
            return;
        }

        let Some(position_name) = self
            .employee_position_select
            .read(cx)
            .selected_value()
            .cloned()
        else {
            self.status = "Select a position".into();
            cx.notify();
            return;
        };

        let Some(organization) = self.selected_org() else {
            self.status = "Select an organization first".into();
            cx.notify();
            return;
        };
        let position_id = organization
            .positions
            .iter()
            .find(|position| position.name == position_name)
            .map(|position| position.id.clone());

        if let Some(employee_id) = self.editing_employee_id.clone() {
            if let Some(employee) = self
                .selected_org_mut()
                .and_then(|organization| organization.employee_mut(&employee_id))
            {
                employee.name = name.clone();
                employee.position_id = position_id.unwrap_or_default();
            }
        } else {
            let added = self.selected_org_mut().map(|organization| {
                organization
                    .add_employee(&name, &position_id.unwrap_or_default())
                    .map(|employee| employee.id.clone())
            });
            match added {
                Some(Ok(_employee_id)) => {}
                Some(Err(error)) => {
                    self.status = format!("Cannot add employee: {error}").into();
                    cx.notify();
                    return;
                }
                None => {
                    self.status = "Select an organization first".into();
                    cx.notify();
                    return;
                }
            }
        }

        self.show_employee_dialog = false;
        self.editing_employee_id = None;
        self.save_project();
        self.status = format!("Saved employee {name}").into();
        cx.notify();
    }
}

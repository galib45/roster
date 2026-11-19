//! Organizations page: list, add, edit, and the selected organization's
//! positions section.

use gpui_kit::component::{
    button::{Button, ButtonVariants},
    dialog::Dialog,
    form::{Field, Form},
    input::Input,
    label::Label,
    v_flex,
};
use gpui_kit::*;

use crate::app::RosterApp;

impl RosterApp {
    pub fn render_organizations(&self, cx: &mut Context<Self>) -> AnyElement {
        let add_row = div()
            .flex()
            .items_center()
            .gap_2()
            .child(div().flex_1().child(Input::new(&self.org_name_input)))
            .child(
                Button::new("add-organization")
                    .primary()
                    .label("Add organization")
                    .on_click(cx.listener(|this, _, window, cx| {
                        let name = this.org_name_input.read(cx).value().trim().to_owned();
                        if name.is_empty() {
                            this.status = "Enter an organization name first".into();
                            cx.notify();
                            return;
                        }

                        let new_id = this
                            .project
                            .add_organization(&name)
                            .map(|organization| organization.id.clone());
                        let Some(new_id) = new_id else {
                            this.status = "Enter an organization name first".into();
                            cx.notify();
                            return;
                        };

                        this.select_org(new_id);
                        this.org_name_input.update(cx, |state, cx| {
                            state.set_value("", window, cx);
                        });
                        this.save_project();
                        this.status = format!("Added {name}").into();
                        cx.notify();
                    })),
            );

        let path_label = match &self.project_path {
            Some(path) => path.clone(),
            None => "No project loaded".to_owned(),
        };

        let file_row = div()
            .flex()
            .items_center()
            .gap_2()
            .child(div().flex_1().child(Label::new(path_label)))
            .child(
                Button::new("save-project")
                    .label("Save project")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.save_project_action(window, cx);
                    })),
            )
            .child(
                Button::new("load-project")
                    .label("Load project")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.choose_project_file(window, cx);
                    })),
            );

        let mut organization_rows = Vec::new();
        if self.project.organizations.is_empty() {
            organization_rows.push(Label::new("No organizations yet").into_any_element());
        } else {
            for organization in &self.project.organizations {
                let org_id = organization.id.clone();
                let org_name = organization.name.clone();
                let select_id = org_id.clone();
                let edit_id = org_id.clone();
                let counts = format!(
                    "{} positions", // · {} rosters",
                    organization.positions.len(),
                    // organization.rosters.len()
                );
                let is_selected = self.current_org_id.as_deref() == Some(org_id.as_str());

                let mut row = div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(Label::new(org_name))
                    .child(Label::new(counts));
                if is_selected {
                    row = row.child(Label::new("Selected"));
                }
                organization_rows.push(
                    row.child(
                        Button::new(format!("select-org-{select_id}"))
                            .ghost()
                            .label("Open")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.open_org(select_id.clone());
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new(format!("edit-org-{edit_id}"))
                            .ghost()
                            .label("Edit")
                            .on_click(cx.listener(move |this, _, window, cx| {
                                let name = this
                                    .project
                                    .organizations
                                    .iter()
                                    .find(|organization| organization.id == edit_id)
                                    .map(|organization| organization.name.clone())
                                    .unwrap_or_default();
                                this.editing_org_id = Some(edit_id.clone());
                                this.org_edit_input.update(cx, |state, cx| {
                                    state.set_value(name, window, cx);
                                });
                                this.show_org_dialog = true;
                                cx.notify();
                            })),
                    )
                    .into_any_element(),
                );
            }
        }

        v_flex()
            .gap_4()
            .child(Label::new("Organizations"))
            .child(add_row)
            .child(file_row)
            .child(v_flex().gap_2().children(organization_rows))
            .child(self.render_org_edit_dialog(cx))
            .into_any_element()
    }

    fn render_org_edit_dialog(&self, cx: &mut Context<Self>) -> AnyElement {
        if !self.show_org_dialog {
            return div().into_any_element();
        }

        Dialog::new(cx)
            .title("Edit Organization")
            .child(
                Form::new()
                    .child(
                        Field::new()
                            .label("Organization name")
                            .child(Input::new(&self.org_edit_input)),
                    )
                    .footer(
                        div()
                            .flex()
                            .gap_2()
                            .child(Button::new("cancel-org-edit").label("Cancel").on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.show_org_dialog = false;
                                    this.editing_org_id = None;
                                    cx.notify();
                                }),
                            ))
                            .child(
                                Button::new("save-org-edit")
                                    .primary()
                                    .label("Save")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        let name =
                                            this.org_edit_input.read(cx).value().trim().to_owned();
                                        if name.is_empty() {
                                            this.status = "Enter an organization name first".into();
                                            cx.notify();
                                            return;
                                        }

                                        if let Some(org_id) = this.editing_org_id.clone()
                                            && let Some(organization) = this
                                                .project
                                                .organizations
                                                .iter_mut()
                                                .find(|organization| organization.id == org_id)
                                        {
                                            organization.name = name.clone();
                                        }

                                        this.show_org_dialog = false;
                                        this.editing_org_id = None;
                                        this.save_project();
                                        this.status =
                                            format!("Updated organization to {name}").into();
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }
}

//! Organizations page: list and add/edit dialog.

use std::path::Path;

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

impl RosterApp {
    pub fn render_organizations(&self, cx: &mut Context<Self>) -> AnyElement {
        let add_row = Form::new()
            .child(
                Field::new()
                    .label("Organization name")
                    .child(Input::new(&self.org_name_input)),
            )
            .footer(
                Button::new("add-organization")
                    .primary()
                    .label("Add organization")
                    .on_click(cx.listener(|this, _, window, cx| {
                        let name = this.org_name.trim().to_owned();
                        if name.is_empty() {
                            this.status = "Enter an organization name first".into();
                            cx.notify();
                            return;
                        }

                        if this.project.add_organization(&name).is_some() {
                            this.status = format!("Added {name}").into();
                            this.org_name.clear();
                            this.org_name_input.update(cx, |state, cx| {
                                state.set_value("", window, cx);
                            });
                        }
                        cx.notify();
                    })),
            );

        let file_row = Form::new()
            .child(
                Field::new()
                    .label("Project file")
                    .child(Input::new(&self.project_path_input)),
            )
            .footer(
                div()
                    .flex()
                    .gap_2()
                    .child(Button::new("save-project").label("Save project").on_click(
                        cx.listener(|this, _, _, cx| {
                            let path = this.project_path().to_owned();
                            this.status =
                                match crate::store::save_project(&this.project, Path::new(&path)) {
                                    Ok(()) => format!("Saved to {path}").into(),
                                    Err(error) => format!("Save failed: {error}").into(),
                                };
                            cx.notify();
                        }),
                    ))
                    .child(Button::new("open-project").label("Open project").on_click(
                        cx.listener(|this, _, _, cx| {
                            let path = this.project_path().to_owned();
                            match crate::store::load_project(Path::new(&path)) {
                                Ok(project) => {
                                    this.project = project;
                                    this.status = format!("Opened {path}").into();
                                }
                                Err(error) => {
                                    this.status = format!("Open failed: {error}").into();
                                }
                            }
                            cx.notify();
                        }),
                    )),
            );

        let mut organizations = Vec::new();
        if self.project.organizations.is_empty() {
            organizations.push(
                div()
                    .child(Label::new("No organizations yet"))
                    .into_any_element(),
            );
        } else {
            for organization in &self.project.organizations {
                let org_id = organization.id.clone();
                let org_name = organization.name.clone();
                organizations.push(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(Label::new(org_name.clone()))
                        .child(
                            Button::new(format!("edit-org-{}", org_id))
                                .ghost()
                                .label("Edit")
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.editing_org_id = Some(org_id.clone());
                                    this.editing_org_name = org_name.clone();
                                    this.org_edit_input.update(cx, |state, cx| {
                                        state.set_value(&org_name, window, cx);
                                    });
                                    this.show_org_dialog = true;
                                    cx.notify();
                                })),
                        )
                        .into_any_element(),
                );
            }
        }

        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(div().child(Label::new("Organizations")))
            .child(add_row)
            .child(file_row)
            .child(div().flex().flex_col().gap_2().children(organizations))
            .child(self.render_org_edit_dialog(cx))
            .into_any_element()
    }

    fn render_org_edit_dialog(&self, cx: &mut Context<Self>) -> AnyElement {
        if !self.show_org_dialog {
            return div().into_any_element();
        }

        let dialog = Dialog::new(cx)
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
                            .child(Button::new("cancel-org-edit")
                                .label("Cancel")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.show_org_dialog = false;
                                    this.editing_org_id = None;
                                    this.editing_org_name.clear();
                                    cx.notify();
                                })),
                            )
                            .child(Button::new("save-org-edit")
                                .primary()
                                .label("Save")
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    let name = this.editing_org_name.trim().to_owned();
                                    if name.is_empty() {
                                        this.status = "Enter an organization name first".into();
                                        cx.notify();
                                        return;
                                    }

                                    if let Some(org_id) = this.editing_org_id.take() {
                                        if let Some(org) = this.project.organization_mut(&org_id) {
                                            org.name = name.clone();
                                            this.status = format!("Updated organization to {name}").into();
                                        }
                                        this.show_org_dialog = false;
                                        this.editing_org_name.clear();
                                        this.save_project();
                                    }
                                    cx.notify();
                                })),
                            ),
                    ),
            );

        dialog.into_any_element()
    }
}
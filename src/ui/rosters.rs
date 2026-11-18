//! Rosters page: list rosters for an organization, add/edit dialog.

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
    pub fn render_rosters(&self, cx: &mut Context<Self>, window: &Window) -> AnyElement {
        let org_id = match &self.page {
            Page::Rosters { org_id } => org_id,
            _ => return div().into_any_element(),
        };

        let Some(organization) = self.project.organization(org_id) else {
            return div()
                .child(Label::new("Organization not found"))
                .into_any_element();
        };

        // Add roster form with dialog
        let add_row = div()
            .flex()
            .items_center()
            .gap_2()
            .child(
                Button::new("add-roster")
                    .primary()
                    .label("Add Roster")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.show_roster_dialog = true;
                        this.new_roster_name.clear();
                        this.roster_edit_input.update(cx, |state, cx| {
                            state.set_value("", window, cx);
                        });
                        cx.notify();
                    })),
            )
            .into_any_element();

        let mut rosters = Vec::new();
        for roster in &organization.rosters {
            let roster_id = roster.id.clone();
            let roster_name = roster.name.clone();
            let org_id_owned = org_id.to_string();
            let roster_id_owned = roster_id.clone();
            rosters.push(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(Label::new(roster_name))
                    .child(
                        Button::new(format!("view-roster-{}", roster_id))
                            .ghost()
                            .label("View")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.navigate(Page::RosterDetail {
                                    org_id: org_id_owned.clone(),
                                    roster_id: roster_id_owned.clone(),
                                }, cx);
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
                    .child(Label::new(format!("Rosters — {}", organization.name)))
                    .into_any_element(),
            )
            .child(add_row)
            .child(div().flex().flex_col().gap_2().children(rosters))
            .child(self.render_roster_add_dialog(cx, window))
            .into_any_element()
    }

    fn render_roster_add_dialog(&self, cx: &mut Context<Self>, window: &Window) -> AnyElement {
        if !self.show_roster_dialog {
            return div().into_any_element();
        }

        let window = window.clone();

        let dialog = Dialog::new(cx)
            .title("Add Roster")
            .child(
                Form::new()
                    .child(
                        Field::new()
                            .label("Roster name")
                            .child(Input::new(&self.roster_edit_input)),
                    )
                    .footer(
                        div()
                            .flex()
                            .gap_2()
                            .child(Button::new("cancel-roster-add")
                                .label("Cancel")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.show_roster_dialog = false;
                                    this.new_roster_name.clear();
                                    cx.notify();
                                })),
                            )
                            .child(Button::new("save-roster-add")
                                .primary()
                                .label("Save")
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    let name = this.new_roster_name.trim().to_owned();
                                    if name.is_empty() {
                                        this.status = "Enter a roster name first".into();
                                        cx.notify();
                                        return;
                                    }

                                    if let Some(org_id) = this.page.org_id() {
                                        if this.project.organization_mut(&org_id)
                                            .and_then(|org| org.add_roster(&name).ok())
                                            .is_some()
                                        {
                                            this.status = format!("Added roster {name}").into();
                                        }
                                        this.show_roster_dialog = false;
                                        this.new_roster_name.clear();
                                        this.save_project();
                                    }
                                    cx.notify();
                                })),
                            ),
                    ),
            );

        dialog.into_any_element()
    }

    pub fn render_roster_detail(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let (org_id, roster_id) = match &self.page {
            Page::RosterDetail { org_id, roster_id } => (org_id, roster_id),
            _ => return div().into_any_element(),
        };

        let Some(organization) = self.project.organization(org_id) else {
            return div().child(Label::new("Organization not found")).into_any_element();
        };
        let Some(roster) = organization.roster(roster_id) else {
            return div().child(Label::new("Roster not found")).into_any_element();
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
                    .child(Label::new(format!("Roster: {}", roster.name)))
                    .child(Label::new(""))
                    .into_any_element(),
            )
            .child(
                div()
                    .child(Label::new("Shifts"))
                    .child(Label::new("Coming soon"))
                    .into_any_element(),
            )
            .child(
                div()
                    .child(Label::new("Sharing Groups"))
                    .child(Label::new("Coming soon"))
                    .into_any_element(),
            )
            .child(
                div()
                    .child(Label::new("Plans"))
                    .child(Label::new("Coming soon"))
                    .into_any_element(),
            )
            .into_any_element()
    }
}
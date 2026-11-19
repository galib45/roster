//! Positions section for the selected organization.

use gpui_kit::component::{button::Button, input::Input, label::Label, v_flex};
use gpui_kit::*;

use crate::app::RosterApp;

impl RosterApp {
    pub fn render_positions_section(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(organization) = self.selected_org() else {
            return v_flex()
                .gap_2()
                .child(Label::new("Positions"))
                .child(Label::new(
                    "Select an organization to manage its positions.",
                ))
                .into_any_element();
        };

        let org_name = organization.name.clone();
        let mut position_rows = Vec::new();
        if organization.positions.is_empty() {
            position_rows.push(Label::new("No positions yet").into_any_element());
        } else {
            for position in &organization.positions {
                position_rows.push(Label::new(position.name.clone()).into_any_element());
            }
        }

        v_flex()
            .gap_2()
            .child(Label::new(format!("Positions — {org_name}")))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .w(px(280.))
                            .child(Input::new(&self.position_name_input)),
                    )
                    .child(Button::new("add-position").label("Add position").on_click(
                        cx.listener(|this, _, window, cx| {
                            let name = this.position_name_input.read(cx).value().trim().to_owned();
                            if name.is_empty() {
                                this.status = "Enter a position name first".into();
                                cx.notify();
                                return;
                            }

                            let result = this
                                .selected_org_mut()
                                .map(|organization| organization.add_position(&name));
                            match result {
                                Some(Ok(_)) => {
                                    this.position_name_input.update(cx, |state, cx| {
                                        state.set_value("", window, cx);
                                    });
                                    this.save_project();
                                    this.status = format!("Added position {name}").into();
                                }
                                Some(Err(error)) => {
                                    this.status = format!("Cannot add position: {error}").into();
                                }
                                None => {
                                    this.status = "Select an organization first".into();
                                }
                            }
                            cx.notify();
                        }),
                    )),
            )
            .child(v_flex().gap_1().children(position_rows))
            .into_any_element()
    }
}

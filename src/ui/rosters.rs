//! Rosters page: list rosters for the selected organization and add new ones.

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
    pub fn render_rosters(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(organization) = self.selected_org() else {
            return v_flex()
                .gap_2()
                .child(Label::new("Rosters"))
                .child(Label::new(
                    "Select an organization on the Organizations page.",
                ))
                .into_any_element();
        };

        let org_name = organization.name.clone();
        // let mut roster_rows = Vec::new();
        //         if organization.rosters.is_empty() {
        //             roster_rows.push(Label::new("No rosters yet").into_any_element());
        //         } else {
        //             for roster in &organization.rosters {
        //                 let shift_names = roster
        //                     .shifts
        //                     .iter()
        //                     .map(|shift| shift.name.as_str())
        //                     .collect::<Vec<_>>()
        //                     .join(", ");
        //                 let plan_names = roster
        //                     .plans
        //                     .iter()
        //                     .map(|plan| {
        //                         format!("{:?} ({}–{})", plan.status, plan.start_date, plan.end_date)
        //                     })
        //                     .collect::<Vec<_>>()
        //                     .join(", ");
        //                 let group_names = roster
        //                     .sharing_groups
        //                     .iter()
        //                     .map(|group| group.name.as_str())
        //                     .collect::<Vec<_>>()
        //                     .join(", ");
        //                 let counts = format!(
        //                     "{} shifts · {} employees · {} plans · {} sharing groups",
        //                     roster.shifts.len(),
        //                     organization.employees_in_roster(&roster.id).len(),
        //                     roster.plans.len(),
        //                     roster.sharing_groups.len()
        //                 );
        //
        //                 roster_rows.push(
        //                     v_flex()
        //                         .gap_1()
        //                         .child(Label::new(roster.name.clone()))
        //                         .child(Label::new(counts))
        //                         .child(Label::new(format!(
        //                             "Shifts: {}",
        //                             display_list(&shift_names)
        //                         )))
        //                         .child(Label::new(format!("Plans: {}", display_list(&plan_names))))
        //                         .child(Label::new(format!(
        //                             "Sharing groups: {}",
        //                             display_list(&group_names)
        //                         )))
        //                         .into_any_element(),
        //                 );
        //             }
        //         }

        v_flex()
            .gap_4()
            .child(Label::new(format!("Rosters — {org_name}")))
            .child(
                Button::new("add-roster")
                    .primary()
                    .label("Add Roster")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.roster_add_input.update(cx, |state, cx| {
                            state.set_value("", window, cx);
                        });
                        this.show_roster_dialog = true;
                        cx.notify();
                    })),
            )
            // .child(v_flex().gap_3().children(roster_rows))
            .child(self.render_roster_add_dialog(cx))
            .into_any_element()
    }

    fn render_roster_add_dialog(&self, cx: &mut Context<Self>) -> AnyElement {
        if !self.show_roster_dialog {
            return div().into_any_element();
        }

        Dialog::new(cx)
            .title("Add Roster")
            .child(
                Form::new()
                    .child(
                        Field::new()
                            .label("Roster name")
                            .child(Input::new(&self.roster_add_input)),
                    )
                    .footer(
                        div()
                            .flex()
                            .gap_2()
                            .child(Button::new("cancel-roster-add").label("Cancel").on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.show_roster_dialog = false;
                                    cx.notify();
                                }),
                            ))
                            .child(
                                Button::new("save-roster-add")
                                    .primary()
                                    .label("Save")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        let name = this
                                            .roster_add_input
                                            .read(cx)
                                            .value()
                                            .trim()
                                            .to_owned();
                                        if name.is_empty() {
                                            this.status = "Enter a roster name first".into();
                                            cx.notify();
                                            return;
                                        }

                                        // let result = this
                                        //     .selected_org_mut()
                                        //     .map(|organization| organization.add_roster(&name));
                                        // match result {
                                        //     Some(Ok(_)) => {
                                        //         this.show_roster_dialog = false;
                                        //         this.save_project();
                                        //         this.status = format!("Added roster {name}").into();
                                        //     }
                                        //     Some(Err(error)) => {
                                        //         this.status =
                                        //             format!("Cannot add roster: {error}").into();
                                        //     }
                                        //     None => {
                                        //         this.status = "Select an organization first".into();
                                        //     }
                                        // }
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }
}

fn display_list(values: &str) -> &str {
    if values.is_empty() { "none" } else { values }
}

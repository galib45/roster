//! Shifts page: list shift templates for an organization, add/edit dialog with requirements.

use gpui_kit::component::{
    button::{Button, ButtonVariants},
    form::{Field, Form},
    h_flex,
    input::{Input, InputState},
    label::Label,
    select::{Select, SelectState},
    v_flex,
};
use gpui_kit::*;

use crate::app::{RosterApp, Page};

impl RosterApp {
    pub fn render_shifts(&self, cx: &mut Context<Self>) -> AnyElement {
        let org_id = match &self.page {
            Page::Shifts { org_id } => org_id,
            _ => return div().into_any_element(),
        };

        let Some(organization) = self.project.organization(org_id) else {
            return div()
                .child(Label::new("Organization not found"))
                .into_any_element();
        };

        // Build position options for requirements
        let positions: Vec<String> = organization
            .positions
            .iter()
            .map(|p| p.id.clone())
            .collect();

        // Add shift form (placeholder - full implementation would have dynamic requirements table)
        let add_row = div()
            .child(Label::new("Shift creation form coming soon"))
            .into_any_element();

        let mut shifts = Vec::new();
        for roster in &organization.rosters {
            for shift in &roster.shifts {
                let shift_id = shift.id.clone();
                let shift_name = shift.name.clone();
                let shift_start = shift.start;
                let shift_end = shift.end;
                let org_id_owned = org_id.to_string();
                let shift_id_owned = shift_id.clone();
                shifts.push(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(Label::new(format!(
                            "{} ({:?}–{:?})",
                            shift_name, shift_start, shift_end
                        )))
                        .child(
                            Button::new(format!("view-shift-{}", shift_id))
                                .ghost()
                                .label("View")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.navigate(Page::ShiftDetail {
                                        org_id: org_id_owned.clone(),
                                        shift_id: Some(shift_id_owned.clone()),
                                    }, cx);
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
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(Label::new(format!("Shifts — {}", organization.name)))
                    .into_any_element(),
            )
            .child(add_row)
            .child(div().flex().flex_col().gap_2().children(shifts))
            .into_any_element()
    }

    pub fn render_shift_detail(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let (org_id, shift_id) = match &self.page {
            Page::ShiftDetail { org_id, shift_id } => (org_id, shift_id),
            _ => return div().into_any_element(),
        };

        let Some(organization) = self.project.organization(org_id) else {
            return div().child(Label::new("Organization not found")).into_any_element();
        };

        if let Some(s_id) = shift_id {
            // Find the shift in any of the organization's rosters
            let shift = organization
                .rosters
                .iter()
                .find_map(|roster| roster.shift(s_id));

            let Some(shift) = shift else {
                return div().child(Label::new("Shift not found")).into_any_element();
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
                        .child(Label::new(format!("Shift: {}", shift.name)))
                        .into_any_element(),
                )
                .child(
                    div()
                        .child(Label::new("Requirements"))
                        .child(Label::new("Coming soon"))
                        .into_any_element(),
                )
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
                        .child(Label::new("New Shift"))
                        .into_any_element(),
                )
                .child(Label::new("Shift creation form coming soon"))
                .into_any_element()
        }
    }
}
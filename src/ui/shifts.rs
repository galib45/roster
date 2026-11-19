//! Shifts page: list shift templates for the selected organization and add or
//! edit them with times, weekday recurrence, and staffing requirements.

use chrono::{NaiveTime, Weekday};
use gpui_kit::component::{
    IndexPath,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    dialog::Dialog,
    form::{Field, Form},
    input::{Input, InputState},
    label::Label,
    searchable_list::SearchableVec,
    select::{Select, SelectState},
    time_field::TimeField,
    v_flex,
};
use gpui_kit::*;

use crate::app::{RosterApp, ShiftRequirementDraft};

const WEEKDAYS: [(Weekday, &str); 7] = [
    (Weekday::Mon, "Mon"),
    (Weekday::Tue, "Tue"),
    (Weekday::Wed, "Wed"),
    (Weekday::Thu, "Thu"),
    (Weekday::Fri, "Fri"),
    (Weekday::Sat, "Sat"),
    (Weekday::Sun, "Sun"),
];

impl RosterApp {
    pub fn render_shifts(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(organization) = self.selected_org() else {
            return v_flex()
                .gap_2()
                .child(Label::new("Shifts"))
                .child(Label::new(
                    "Select an organization on the Organizations page.",
                ))
                .into_any_element();
        };

        let org_name = organization.name.clone();
        // let mut shift_rows = Vec::new();
        //         if organization.rosters.is_empty() {
        //             shift_rows.push(
        //                 Label::new("Add a roster before creating shift templates.").into_any_element(),
        //             );
        //         }
        //         for roster in &organization.rosters {
        //             shift_rows.push(Label::new(format!("Roster: {}", roster.name)).into_any_element());
        //             if roster.shifts.is_empty() {
        //                 shift_rows.push(Label::new("No shifts yet").into_any_element());
        //             }
        //             for shift in &roster.shifts {
        //                 let roster_id = roster.id.clone();
        //                 let shift_id = shift.id.clone();
        //                 let time = format!(
        //                     "{}–{}",
        //                     shift.start.format("%H:%M"),
        //                     shift.end.format("%H:%M")
        //                 );
        //                 let weekdays = if shift.weekdays.is_empty() {
        //                     "Every day".to_owned()
        //                 } else {
        //                     shift
        //                         .weekdays
        //                         .iter()
        //                         .map(|weekday| weekday_label(*weekday))
        //                         .collect::<Vec<_>>()
        //                         .join(", ")
        //                 };
        //                 let requirements = if shift.required_positions.is_empty() {
        //                     "no requirements".to_owned()
        //                 } else {
        //                     shift
        //                         .required_positions
        //                         .iter()
        //                         .map(|requirement| {
        //                             let position = organization
        //                                 .position(&requirement.position_id)
        //                                 .map(|position| position.name.clone())
        //                                 .unwrap_or_else(|| "unknown position".to_owned());
        //                             format!("{position} × {}", requirement.headcount)
        //                         })
        //                         .collect::<Vec<_>>()
        //                         .join(", ")
        //                 };
        //
        //                 shift_rows.push(
        //                     div()
        //                         .flex()
        //                         .items_center()
        //                         .gap_3()
        //                         .child(Label::new(shift.name.clone()))
        //                         .child(Label::new(time))
        //                         .child(Label::new(weekdays))
        //                         .child(Label::new(requirements))
        //                         .child(
        //                             Button::new(format!("edit-shift-{shift_id}"))
        //                                 .ghost()
        //                                 .label("Edit")
        //                                 .on_click(cx.listener(move |this, _, window, cx| {
        //                                     this.open_shift_dialog(
        //                                         Some((roster_id.clone(), shift_id.clone())),
        //                                         window,
        //                                         cx,
        //                                     );
        //                                 })),
        //                         )
        //                         .into_any_element(),
        //                 );
        //             }
        //         }

        v_flex()
            .gap_4()
            .child(Label::new(format!("Shifts — {org_name}")))
            .child(
                Button::new("add-shift")
                    .primary()
                    .label("Add Shift")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_shift_dialog(None, window, cx);
                    })),
            )
            // .child(v_flex().gap_2().children(shift_rows))
            .child(self.render_shift_dialog(cx))
            .into_any_element()
    }

    pub(crate) fn open_shift_dialog(
        &mut self,
        target: Option<(String, String)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(organization) = self.selected_org() else {
            self.status = "Select an organization first".into();
            cx.notify();
            return;
        };

        // let roster_labels: Vec<String> = organization
        //     .rosters
        //     .iter()
        //     .map(|roster| roster.name.clone())
        //     .collect();
        let position_labels: Vec<String> = organization
            .positions
            .iter()
            .map(|position| position.name.clone())
            .collect();

        let mut name = String::new();
        let mut start = at(8, 0);
        let mut end = at(14, 0);
        let mut weekdays = [false; 7];
        // let mut roster_index = None;
        let mut requirement_specs: Vec<(Option<usize>, String)> = vec![(None, String::new())];

        //         if let Some((roster_id, shift_id)) = &target
        //             && let Some(roster) = organization.roster(roster_id)
        //             && let Some(shift) = roster.shift(shift_id)
        //         {
        //             name = shift.name.clone();
        //             start = shift.start;
        //             end = shift.end;
        //             roster_index = organization
        //                 .rosters
        //                 .iter()
        //                 .position(|roster| roster.id == *roster_id);
        //             if shift.weekdays.is_empty() {
        //                 weekdays = [true; 7];
        //             } else {
        //                 for (index, (weekday, _)) in WEEKDAYS.iter().enumerate() {
        //                     weekdays[index] = shift.weekdays.contains(weekday);
        //                 }
        //             }
        //
        //             let mut specs: Vec<(Option<usize>, String)> = shift
        //                 .required_positions
        //                 .iter()
        //                 .map(|requirement| {
        //                     (
        //                         organization
        //                             .positions
        //                             .iter()
        //                             .position(|position| position.id == requirement.position_id),
        //                         requirement.headcount.to_string(),
        //                     )
        //                 })
        //                 .collect();
        //             if specs.is_empty() {
        //                 specs.push((None, String::new()));
        //             }
        //             requirement_specs = specs;
        //         }

        // self.shift_roster_select = cx.new(|cx| {
        //     SelectState::new(
        //         SearchableVec::new(roster_labels),
        //         roster_index.map(IndexPath::new),
        //         window,
        //         cx,
        //     )
        // });
        self.shift_name_input.update(cx, |state, cx| {
            state.set_value(name, window, cx);
        });
        self.shift_start.update(cx, |state, cx| {
            state.set_time(start, window, cx);
        });
        self.shift_end.update(cx, |state, cx| {
            state.set_time(end, window, cx);
        });
        self.shift_weekdays = weekdays;
        self.shift_requirements = requirement_specs
            .into_iter()
            .map(|(index, headcount)| {
                requirement_draft(
                    position_labels.clone(),
                    index.map(IndexPath::new),
                    &headcount,
                    window,
                    cx,
                )
            })
            .collect();
        self.editing_shift_id = target.as_ref().map(|(_, shift_id)| shift_id.clone());
        self.editing_shift_roster_id = target.map(|(roster_id, _)| roster_id);
        self.show_shift_dialog = true;
        cx.notify();
    }

    fn render_shift_dialog(&self, cx: &mut Context<Self>) -> AnyElement {
        if !self.show_shift_dialog {
            return div().into_any_element();
        }

        let is_editing = self.editing_shift_id.is_some();
        let title = if is_editing {
            "Edit Shift"
        } else {
            "Add Shift"
        };

        let mut weekday_boxes = Vec::new();
        for (index, (_, label)) in WEEKDAYS.iter().enumerate() {
            weekday_boxes.push(
                Checkbox::new(format!("weekday-{index}"))
                    .label(*label)
                    .checked(self.shift_weekdays[index])
                    .on_click(cx.listener(move |this, checked, _, cx| {
                        this.shift_weekdays[index] = *checked;
                        cx.notify();
                    }))
                    .into_any_element(),
            );
        }

        let mut requirement_rows = Vec::new();
        for (index, draft) in self.shift_requirements.iter().enumerate() {
            requirement_rows.push(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().flex_1().child(Select::new(&draft.position_select)))
                    .child(div().w(px(120.)).child(Input::new(&draft.headcount_input)))
                    .child(
                        Button::new(format!("remove-requirement-{index}"))
                            .ghost()
                            .label("Remove")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if index < this.shift_requirements.len() {
                                    this.shift_requirements.remove(index);
                                }
                                cx.notify();
                            })),
                    )
                    .into_any_element(),
            );
        }

        let requirements_block = v_flex()
            .gap_2()
            .child(v_flex().gap_2().children(requirement_rows))
            .child(
                Button::new("add-requirement")
                    .label("Add Row")
                    .on_click(cx.listener(|this, _, window, cx| {
                        let labels = this
                            .selected_org()
                            .map(|organization| {
                                organization
                                    .positions
                                    .iter()
                                    .map(|position| position.name.clone())
                                    .collect::<Vec<_>>()
                            })
                            .unwrap_or_default();
                        this.shift_requirements
                            .push(requirement_draft(labels, None, "", window, cx));
                        cx.notify();
                    })),
            );

        Dialog::new(cx)
            .title(title)
            .child(
                Form::new()
                    .child(
                        Field::new()
                            .label("Roster")
                            .child(Select::new(&self.shift_roster_select).disabled(is_editing)),
                    )
                    .child(
                        Field::new()
                            .label("Shift name")
                            .child(Input::new(&self.shift_name_input)),
                    )
                    .child(
                        Field::new()
                            .label("Start")
                            .child(TimeField::new(&self.shift_start)),
                    )
                    .child(
                        Field::new()
                            .label("End")
                            .child(TimeField::new(&self.shift_end)),
                    )
                    .child(
                        Field::new()
                            .label("Weekdays")
                            .child(div().flex().gap_2().children(weekday_boxes)),
                    )
                    .child(Field::new().label("Coverage").child(requirements_block))
                    .footer(
                        div()
                            .flex()
                            .gap_2()
                            .child(Button::new("cancel-shift-edit").label("Cancel").on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.show_shift_dialog = false;
                                    this.editing_shift_id = None;
                                    this.editing_shift_roster_id = None;
                                    cx.notify();
                                }),
                            ))
                            .child(
                                Button::new("save-shift-edit")
                                    .primary()
                                    .label("Save")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.save_shift(cx);
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn save_shift(&mut self, cx: &mut Context<Self>) {
        let name = self.shift_name_input.read(cx).value().trim().to_owned();
        if name.is_empty() {
            self.status = "Enter a shift name first".into();
            cx.notify();
            return;
        }

        let start = self.shift_start.read(cx).time();
        let end = self.shift_end.read(cx).time();
        if start == end {
            self.status = "Start and end times must differ".into();
            cx.notify();
            return;
        }

        let weekdays: Vec<Weekday> = WEEKDAYS
            .iter()
            .enumerate()
            .filter(|(index, _)| self.shift_weekdays[*index])
            .map(|(_, (weekday, _))| *weekday)
            .collect();
        if weekdays.is_empty() {
            self.status = "Select at least one weekday".into();
            cx.notify();
            return;
        }

        if self.shift_requirements.is_empty() {
            self.status = "Add at least one requirement".into();
            cx.notify();
            return;
        }

        let mut draft_requirements: Vec<(String, u16)> = Vec::new();
        for draft in &self.shift_requirements {
            let Some(position_name) = draft.position_select.read(cx).selected_value().cloned()
            else {
                self.status = "Select a position for every requirement".into();
                cx.notify();
                return;
            };
            let raw = draft.headcount_input.read(cx).value().trim().to_owned();
            let Ok(headcount) = raw.parse::<u16>() else {
                self.status = "Enter a headcount of at least 1".into();
                cx.notify();
                return;
            };
            if headcount == 0 {
                self.status = "Enter a headcount of at least 1".into();
                cx.notify();
                return;
            }
            if draft_requirements
                .iter()
                .any(|(existing, _)| existing == &position_name)
            {
                self.status = "Each position may appear only once".into();
                cx.notify();
                return;
            }
            draft_requirements.push((position_name, headcount));
        }

        let Some(organization) = self.selected_org() else {
            self.status = "Select an organization first".into();
            cx.notify();
            return;
        };
        let roster_id = if let Some(roster_id) = self.editing_shift_roster_id.clone() {
            roster_id
        } else {
            let Some(roster_name) = self.shift_roster_select.read(cx).selected_value().cloned()
            else {
                self.status = "Select a roster".into();
                cx.notify();
                return;
            };
            // let Some(roster_id) = organization
            //     .rosters
            //     .iter()
            //     .find(|roster| roster.name == roster_name)
            //     .map(|roster| roster.id.clone())
            // else {
            //     self.status = "Select a roster".into();
            //     cx.notify();
            //     return;
            // };
            // roster_id
            String::new()
        };

        let mut requirements: Vec<(String, u16)> = Vec::with_capacity(draft_requirements.len());
        for (position_name, headcount) in draft_requirements {
            let Some(position) = organization
                .positions
                .iter()
                .find(|position| position.name == position_name)
            else {
                self.status = "The selected position no longer exists".into();
                cx.notify();
                return;
            };
            requirements.push((position.id.clone(), headcount));
        }

        let shift_id = if let Some(shift_id) = self.editing_shift_id.clone() {
            let updated = self.selected_org_mut().map(|organization| {
                organization.update_shift(&roster_id, &shift_id, &name, start, end)
            });
            match updated {
                Some(Ok(())) => shift_id,
                Some(Err(error)) => {
                    self.status = format!("Cannot save shift: {error}").into();
                    cx.notify();
                    return;
                }
                None => {
                    self.status = "Select an organization first".into();
                    cx.notify();
                    return;
                }
            }
        } else {
            let added = self.selected_org_mut().map(|organization| {
                organization
                    .add_shift(&roster_id, &name, start, end)
                    .map(|shift| shift.id.clone())
            });
            match added {
                Some(Ok(shift_id)) => shift_id,
                Some(Err(error)) => {
                    self.status = format!("Cannot add shift: {error}").into();
                    cx.notify();
                    return;
                }
                None => {
                    self.status = "Select an organization first".into();
                    cx.notify();
                    return;
                }
            }
        };

        // All seven days is the same as the canonical "every day" (empty list).
        let weekdays = if weekdays.len() == WEEKDAYS.len() {
            Vec::new()
        } else {
            weekdays
        };
        let applied = self.selected_org_mut().map(|organization| {
            organization
                .set_shift_weekdays(&roster_id, &shift_id, &weekdays)
                .and_then(|()| {
                    organization.set_shift_requirements(&roster_id, &shift_id, &requirements)
                })
        });
        match applied {
            Some(Ok(())) => {}
            Some(Err(error)) => {
                self.status = format!("Cannot save shift: {error}").into();
                cx.notify();
                return;
            }
            None => {
                self.status = "Select an organization first".into();
                cx.notify();
                return;
            }
        }

        self.show_shift_dialog = false;
        self.editing_shift_id = None;
        self.editing_shift_roster_id = None;
        self.save_project();
        self.status = format!("Saved shift {name}").into();
        cx.notify();
    }
}

fn requirement_draft(
    labels: Vec<String>,
    selected: Option<IndexPath>,
    headcount: &str,
    window: &mut Window,
    cx: &mut Context<RosterApp>,
) -> ShiftRequirementDraft {
    let position_select =
        cx.new(|cx| SelectState::new(SearchableVec::new(labels), selected, window, cx));
    let headcount_input = cx.new(|cx| {
        InputState::new(window, cx)
            .placeholder("e.g. 2")
            .default_value(headcount.to_owned())
    });
    ShiftRequirementDraft {
        position_select,
        headcount_input,
    }
}

fn at(hour: u32, minute: u32) -> NaiveTime {
    NaiveTime::from_hms_opt(hour, minute, 0).expect("valid wall-clock time")
}

fn weekday_label(weekday: Weekday) -> &'static str {
    match weekday {
        Weekday::Mon => "Mon",
        Weekday::Tue => "Tue",
        Weekday::Wed => "Wed",
        Weekday::Thu => "Thu",
        Weekday::Fri => "Fri",
        Weekday::Sat => "Sat",
        Weekday::Sun => "Sun",
    }
}

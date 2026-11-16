//! The application view: navigation shell plus the organizations screen.

use std::path::Path;

use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Theme,
    button::{Button, ButtonVariants},
    form::{Field, Form},
    h_flex,
    input::{Input, InputEvent, InputState},
    label::Label,
    sidebar::{Sidebar, SidebarFooter, SidebarGroup, SidebarHeader, SidebarMenu, SidebarMenuItem},
    v_flex,
};
use gpui_kit::*;

use crate::domain::ProjectFile;
use crate::store;

const DEFAULT_PROJECT_PATH: &str = "roster-project.json";

/// Top-level sections of the app. Only Organizations is functional in Phase 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Organizations,
    Rosters,
    Employees,
    Shifts,
    RosterPlan,
    Export,
}

impl Page {
    pub const ALL: [Page; 6] = [
        Page::Organizations,
        Page::Rosters,
        Page::Employees,
        Page::Shifts,
        Page::RosterPlan,
        Page::Export,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Page::Organizations => "Organizations",
            Page::Rosters => "Rosters",
            Page::Employees => "Employees",
            Page::Shifts => "Shifts",
            Page::RosterPlan => "Roster Plan",
            Page::Export => "Export",
        }
    }

    /// Icon shown beside the label in the sidebar.
    ///
    /// Only icons from the default `gpui-kit` bundle are used, because the
    /// application registers `assets::Assets`, which embeds exactly those.
    pub fn icon(self) -> IconName {
        match self {
            Page::Organizations => IconName::Building2,
            Page::Rosters => IconName::Folder,
            Page::Employees => IconName::User,
            Page::Shifts => IconName::Calendar,
            Page::RosterPlan => IconName::LayoutDashboard,
            Page::Export => IconName::FileText,
        }
    }

    /// Phase in which this section becomes functional.
    pub fn later_phase(self) -> &'static str {
        match self {
            Page::Organizations => "",
            Page::Rosters | Page::Employees | Page::Shifts => "Phase 5 (Management UI)",
            Page::RosterPlan => "Phase 6 (Constraint UI & Generate)",
            Page::Export => "Phase 8 (Export)",
        }
    }
}

pub struct RosterApp {
    page: Page,
    project: ProjectFile,
    project_path: String,
    org_name: String,
    org_name_input: Entity<InputState>,
    project_path_input: Entity<InputState>,
    status: SharedString,
    _subscriptions: Vec<Subscription>,
}

impl RosterApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let org_name_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("e.g. City Hospital"));
        let project_path_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(DEFAULT_PROJECT_PATH));

        let org_subscription =
            cx.subscribe_in(&org_name_input, window, |this, state, event, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.org_name = state.read(cx).value().to_string();
                    cx.notify();
                }
            });
        let path_subscription =
            cx.subscribe_in(&project_path_input, window, |this, state, event, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.project_path = state.read(cx).value().to_string();
                    cx.notify();
                }
            });
        // Follow OS light/dark changes for the lifetime of the window.
        let appearance_subscription = window.observe_window_appearance(|window, cx| {
            Theme::sync_system_appearance(Some(window), cx);
        });

        Self {
            page: Page::Organizations,
            project: ProjectFile::new(),
            project_path: DEFAULT_PROJECT_PATH.to_owned(),
            org_name: String::new(),
            org_name_input,
            project_path_input,
            status: "New project — add an organization to begin".into(),
            _subscriptions: vec![org_subscription, path_subscription, appearance_subscription],
        }
    }

    fn project_path(&self) -> &str {
        let path = self.project_path.trim();
        if path.is_empty() {
            DEFAULT_PROJECT_PATH
        } else {
            path
        }
    }

    fn nav_item(&self, page: Page, cx: &mut Context<Self>) -> SidebarMenuItem {
        SidebarMenuItem::new(page.label())
            .icon(page.icon())
            .active(page == self.page)
            .on_click(cx.listener(move |this, _, _, cx| {
                this.page = page;
                cx.notify();
            }))
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> AnyElement {
        let manage = SidebarGroup::new("Manage").child(SidebarMenu::new().children([
            self.nav_item(Page::Organizations, cx),
            self.nav_item(Page::Rosters, cx),
            self.nav_item(Page::Employees, cx),
            self.nav_item(Page::Shifts, cx),
        ]));
        let schedule = SidebarGroup::new("Schedule")
            .child(SidebarMenu::new().child(self.nav_item(Page::RosterPlan, cx)));
        let output = SidebarGroup::new("Output")
            .child(SidebarMenu::new().child(self.nav_item(Page::Export, cx)));

        Sidebar::new("app-sidebar")
            // A fixed sidebar keeps the roster grid's width predictable.
            .collapsible(false)
            .w(px(240.))
            .header(
                SidebarHeader::new()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .size_8()
                            .flex_shrink_0()
                            .rounded(cx.theme().radius)
                            .bg(cx.theme().sidebar_primary)
                            .text_color(cx.theme().sidebar_primary_foreground)
                            .child(Icon::new(IconName::GalleryVerticalEnd)),
                    )
                    .child(
                        v_flex().flex_1().overflow_hidden().child("roster").child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child("Hospital duty scheduling"),
                        ),
                    ),
            )
            .children([manage, schedule, output])
            .footer(
                SidebarFooter::new().child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(Icon::new(IconName::User).size_4())
                        .child(div().text_xs().child("v0.1 · Phase 1")),
                ),
            )
            .into_any_element()
    }

    fn render_organizations(&self, cx: &mut Context<Self>) -> AnyElement {
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

        let file_row =
            Form::new()
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
                                    match store::save_project(&this.project, Path::new(&path)) {
                                        Ok(()) => format!("Saved to {path}").into(),
                                        Err(error) => format!("Save failed: {error}").into(),
                                    };
                                cx.notify();
                            }),
                        ))
                        .child(Button::new("open-project").label("Open project").on_click(
                            cx.listener(|this, _, _, cx| {
                                let path = this.project_path().to_owned();
                                match store::load_project(Path::new(&path)) {
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
                organizations.push(
                    div()
                        .child(Label::new(organization.name.clone()))
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
            .into_any_element()
    }

    fn render_placeholder(&self) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(Label::new(self.page.label()))
            .child(Label::new(format!(
                "Coming in {}.",
                self.page.later_phase()
            )))
            .into_any_element()
    }
}

impl Render for RosterApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = match self.page {
            Page::Organizations => self.render_organizations(cx),
            _ => self.render_placeholder(),
        };

        div()
            .flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.render_sidebar(cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .h_full()
                    .p_6()
                    .gap_4()
                    .child(content)
                    .child(div().flex_1())
                    .child(Label::new(self.status.clone())),
            )
    }
}

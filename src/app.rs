//! The application view: navigation shell and shared application state.

use std::path::Path;

use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Theme,
    input::{InputEvent, InputState},
    label::Label,
    searchable_list::SearchableVec,
    select::SelectState,
    sidebar::{Sidebar, SidebarFooter, SidebarGroup, SidebarHeader, SidebarMenu, SidebarMenuItem},
    time_field::TimeFieldState,
    v_flex,
};
use gpui_kit::*;

use crate::domain::ProjectFile;
use crate::store;

const DEFAULT_PROJECT_PATH: &str = "roster-project.json";

/// Top-level sections of the app.
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
            Page::Organizations | Page::Rosters | Page::Employees | Page::Shifts => "",
            Page::RosterPlan => "Phase 6 (Constraint UI & Generate)",
            Page::Export => "Phase 8 (Export)",
        }
    }
}

/// One editable staffing-requirement row in the shift dialog.
pub struct ShiftRequirementDraft {
    pub(crate) position_select: Entity<SelectState<SearchableVec<String>>>,
    pub(crate) headcount_input: Entity<InputState>,
}

/// Shared application state for the navigation shell and management pages.
pub struct RosterApp {
    pub(crate) page: Page,
    pub(crate) project: ProjectFile,
    pub(crate) project_path: String,
    pub(crate) current_org_id: Option<String>,
    pub(crate) status: SharedString,

    pub(crate) org_name_input: Entity<InputState>,
    pub(crate) project_path_input: Entity<InputState>,

    pub(crate) show_org_dialog: bool,
    pub(crate) editing_org_id: Option<String>,
    pub(crate) org_edit_input: Entity<InputState>,

    pub(crate) position_name_input: Entity<InputState>,

    pub(crate) show_roster_dialog: bool,
    pub(crate) roster_add_input: Entity<InputState>,

    // Employee add/edit dialog
    pub(crate) show_employee_dialog: bool,
    pub(crate) editing_employee_id: Option<String>,
    pub(crate) employee_name_input: Entity<InputState>,
    pub(crate) employee_skills_input: Entity<InputState>,
    pub(crate) employee_position_select: Entity<SelectState<SearchableVec<String>>>,
    pub(crate) employee_roster_select: Entity<SelectState<SearchableVec<String>>>,

    // Shift add/edit dialog
    pub(crate) show_shift_dialog: bool,
    pub(crate) editing_shift_id: Option<String>,
    pub(crate) editing_shift_roster_id: Option<String>,
    pub(crate) shift_name_input: Entity<InputState>,
    pub(crate) shift_roster_select: Entity<SelectState<SearchableVec<String>>>,
    pub(crate) shift_start: Entity<TimeFieldState>,
    pub(crate) shift_end: Entity<TimeFieldState>,
    pub(crate) shift_weekdays: [bool; 7],
    pub(crate) shift_requirements: Vec<ShiftRequirementDraft>,

    _subscriptions: Vec<Subscription>,
}

impl RosterApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let org_name_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("e.g. City Hospital"));
        let project_path_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(DEFAULT_PROJECT_PATH));
        let org_edit_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Organization name"));
        let position_name_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("e.g. Medical Officer"));
        let roster_add_input = cx.new(|cx| InputState::new(window, cx).placeholder("e.g. Doctors"));

        let employee_name_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("e.g. Dr. Amit"));
        let employee_skills_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Comma-separated, e.g. cardiology, ACLS")
        });
        let employee_position_select = cx
            .new(|cx| SelectState::new(SearchableVec::new(Vec::<String>::new()), None, window, cx));
        let employee_roster_select = cx
            .new(|cx| SelectState::new(SearchableVec::new(Vec::<String>::new()), None, window, cx));

        let shift_name_input = cx.new(|cx| InputState::new(window, cx).placeholder("e.g. OPD"));
        let shift_roster_select = cx
            .new(|cx| SelectState::new(SearchableVec::new(Vec::<String>::new()), None, window, cx));
        let shift_start = cx.new(|cx| TimeFieldState::new(window, cx));
        let shift_end = cx.new(|cx| TimeFieldState::new(window, cx));

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
            current_org_id: None,
            status: "New project — add an organization to begin".into(),
            org_name_input,
            project_path_input,
            show_org_dialog: false,
            editing_org_id: None,
            org_edit_input,
            position_name_input,
            show_roster_dialog: false,
            roster_add_input,
            show_employee_dialog: false,
            editing_employee_id: None,
            employee_name_input,
            employee_skills_input,
            employee_position_select,
            employee_roster_select,
            show_shift_dialog: false,
            editing_shift_id: None,
            editing_shift_roster_id: None,
            shift_name_input,
            shift_roster_select,
            shift_start,
            shift_end,
            shift_weekdays: [false; 7],
            shift_requirements: Vec::new(),
            _subscriptions: vec![path_subscription, appearance_subscription],
        }
    }

    pub(crate) fn project_path(&self) -> &str {
        let path = self.project_path.trim();
        if path.is_empty() {
            DEFAULT_PROJECT_PATH
        } else {
            path
        }
    }

    /// The organization selected on the Organizations page, if any.
    pub(crate) fn selected_org(&self) -> Option<&crate::domain::Organization> {
        let id = self.current_org_id.as_deref()?;
        self.project
            .organizations
            .iter()
            .find(|organization| organization.id == id)
    }

    /// The selected organization for mutation.
    pub(crate) fn selected_org_mut(&mut self) -> Option<&mut crate::domain::Organization> {
        let id = self.current_org_id.clone()?;
        self.project
            .organizations
            .iter_mut()
            .find(|organization| organization.id == id)
    }

    /// Select an organization and close any open management dialog.
    pub(crate) fn select_org(&mut self, org_id: String) {
        self.current_org_id = Some(org_id);
        self.show_org_dialog = false;
        self.editing_org_id = None;
        self.show_roster_dialog = false;
    }

    /// Persist the project to the path in the path field, updating status.
    pub(crate) fn save_project(&mut self) {
        let path = self.project_path().to_owned();
        self.status = match store::save_project(&self.project, Path::new(&path)) {
            Ok(()) => format!("Saved to {path}").into(),
            Err(error) => format!("Save failed: {error}").into(),
        };
    }

    /// Load the project file and keep the selection valid.
    pub(crate) fn open_project(&mut self) {
        let path = self.project_path().to_owned();
        match store::load_project(Path::new(&path)) {
            Ok(project) => {
                self.project = project;
                self.clamp_selected_org();
                self.status = format!("Opened {path}").into();
            }
            Err(error) => {
                self.status = format!("Open failed: {error}").into();
            }
        }
    }

    /// Keep `current_org_id` pointing at a real organization after a load.
    fn clamp_selected_org(&mut self) {
        let still_exists = self.current_org_id.as_deref().is_some_and(|id| {
            self.project
                .organizations
                .iter()
                .any(|organization| organization.id == id)
        });
        if !still_exists {
            self.current_org_id = self
                .project
                .organizations
                .first()
                .map(|organization| organization.id.clone());
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
                    div()
                        .flex()
                        .gap_2()
                        .items_center()
                        .child(Icon::new(IconName::User).size_4())
                        .child(div().text_xs().child("v0.1 · Phase 5")),
                ),
            )
            .into_any_element()
    }

    /// A titled page with a short message, used for sections that are not
    /// implemented in this plan.
    pub(crate) fn render_pending(&self, message: &str) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(Label::new(self.page.label()))
            .child(Label::new(message.to_owned()))
            .into_any_element()
    }
}

impl Render for RosterApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = match self.page {
            Page::Organizations => self.render_organizations(cx),
            Page::Rosters => self.render_rosters(cx),
            Page::Employees => self.render_employees(cx),
            Page::Shifts => self.render_shifts(cx),
            Page::RosterPlan | Page::Export => {
                self.render_pending(&format!("Coming in {}.", self.page.later_phase()))
            }
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

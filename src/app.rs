//! The application view: navigation shell and shared application state.

use std::path::{Path, PathBuf};

use gpui_kit::component::{
    ActiveTheme, Icon, IconName, Theme,
    button::{Button, ButtonVariants},
    input::InputState,
    label::Label,
    searchable_list::SearchableVec,
    select::SelectState,
    sidebar::{Sidebar, SidebarGroup, SidebarHeader, SidebarItem, SidebarMenu, SidebarMenuItem},
    time_field::TimeFieldState,
    v_flex,
};
use gpui_kit::*;

use crate::domain::ProjectFile;
use crate::store;

const DEFAULT_PROJECT_FILE: &str = "roster-project.json";

/// Sections of the app.
///
/// [`Page::Organizations`] is the level-one screen. Every other variant is a
/// subsection of the open organization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Organizations,
    Positions,
    Employees,
    Shifts,
    Rosters,
    RosterPlan,
    Export,
}

impl Page {
    pub fn label(self) -> &'static str {
        match self {
            Page::Organizations => "Organizations",
            Page::Positions => "Positions",
            Page::Employees => "Employees",
            Page::Shifts => "Shifts",
            Page::Rosters => "Rosters",
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
            Page::Positions => IconName::Frame,
            Page::Employees => IconName::User,
            Page::Shifts => IconName::Calendar,
            Page::Rosters => IconName::Folder,
            Page::RosterPlan => IconName::LayoutDashboard,
            Page::Export => IconName::FileText,
        }
    }

    /// Phase in which this section becomes functional.
    pub fn later_phase(self) -> &'static str {
        match self {
            Page::Organizations
            | Page::Positions
            | Page::Employees
            | Page::Shifts
            | Page::Rosters => "",
            Page::RosterPlan => "Phase 6 (Constraint UI & Generate)",
            Page::Export => "Phase 8 (Export)",
        }
    }
}

/// Shorten a name so it fits one sidebar row.
///
/// The sidebar clips long labels; truncating here keeps the result predictable
/// and always shows the ellipsis at the end.
fn truncated_name(name: &str, max_chars: usize) -> String {
    if name.chars().count() <= max_chars {
        return name.to_owned();
    }

    let mut short: String = name.chars().take(max_chars.saturating_sub(1)).collect();
    short.push('…');
    short
}

/// One editable staffing-requirement row in the shift dialog.
pub struct ShiftRequirementDraft {
    pub(crate) position_select: Entity<SelectState<SearchableVec<String>>>,
    pub(crate) headcount_input: Entity<InputState>,
}

/// Shared application state for the navigation shell and management pages.
pub struct RosterApp {
    pub(crate) page: Page,
    /// Whether the sidebar is showing one organization's subsections.
    ///
    /// `false` keeps the sidebar at the Organizations level. It is deliberately
    /// separate from `current_org_id` so adding an organization does not
    /// navigate away from the list.
    pub(crate) org_open: bool,
    pub(crate) project: ProjectFile,
    /// Path of the last loaded or saved project. `None` until the user picks a
    /// file, which is what makes Save ask for a location the first time.
    pub(crate) project_path: Option<String>,
    pub(crate) current_org_id: Option<String>,
    pub(crate) status: SharedString,

    pub(crate) org_name_input: Entity<InputState>,

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
        let org_edit_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Organization name"));
        let position_name_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("e.g. Medical Officer"));
        let roster_add_input = cx.new(|cx| InputState::new(window, cx).placeholder("e.g. Doctors"));

        let employee_name_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("e.g. Dr. Amit"));
        let employee_position_select = cx
            .new(|cx| SelectState::new(SearchableVec::new(Vec::<String>::new()), None, window, cx));
        let employee_roster_select = cx
            .new(|cx| SelectState::new(SearchableVec::new(Vec::<String>::new()), None, window, cx));

        let shift_name_input = cx.new(|cx| InputState::new(window, cx).placeholder("e.g. OPD"));
        let shift_roster_select = cx
            .new(|cx| SelectState::new(SearchableVec::new(Vec::<String>::new()), None, window, cx));
        let shift_start = cx.new(|cx| TimeFieldState::new(window, cx));
        let shift_end = cx.new(|cx| TimeFieldState::new(window, cx));

        // Follow OS light/dark changes for the lifetime of the window.
        let appearance_subscription = window.observe_window_appearance(|window, cx| {
            Theme::sync_system_appearance(Some(window), cx);
        });

        Self {
            page: Page::Organizations,
            org_open: false,
            project: ProjectFile::new(),
            project_path: None,
            current_org_id: None,
            status: "New project — add an organization to begin".into(),
            org_name_input,
            show_org_dialog: false,
            editing_org_id: None,
            org_edit_input,
            position_name_input,
            show_roster_dialog: false,
            roster_add_input,
            show_employee_dialog: false,
            editing_employee_id: None,
            employee_name_input,
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
            _subscriptions: vec![appearance_subscription],
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

    /// Select an organization without opening its sidebar.
    ///
    /// Used after adding an organization so the new name appears in the
    /// Organizations list without navigating away.
    pub(crate) fn select_org(&mut self, org_id: String) {
        self.current_org_id = Some(org_id);
        self.close_dialogs();
    }

    /// Open an organization: show its subsections in the sidebar.
    pub(crate) fn open_org(&mut self, org_id: String) {
        self.current_org_id = Some(org_id);
        self.org_open = true;
        if self.page == Page::Organizations {
            self.page = Page::Positions;
        }
        self.close_dialogs();
    }

    /// Return to the Organizations level of the sidebar.
    pub(crate) fn close_org(&mut self) {
        self.org_open = false;
        self.page = Page::Organizations;
        self.close_dialogs();
    }

    fn close_dialogs(&mut self) {
        self.show_org_dialog = false;
        self.editing_org_id = None;
        self.show_roster_dialog = false;
    }

    /// Save to the current project path. Does nothing when no path exists yet.
    ///
    /// Used by the inline editors after an edit so the file stays current. The
    /// first save always goes through [`Self::save_project_action`], which asks
    /// the user for a location.
    pub(crate) fn save_project(&mut self) {
        if let Some(path) = self.project_path.clone() {
            self.save_project_at(Path::new(&path));
        }
    }

    /// Save button behaviour: save in place, or ask for a location on first save.
    pub(crate) fn save_project_action(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.project_path.is_some() {
            self.save_project();
            cx.notify();
            return;
        }

        let directory = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let selection = cx.prompt_for_new_path(&directory, Some(DEFAULT_PROJECT_FILE));

        cx.spawn_in(window, async move |this, cx| {
            let selection = selection.await;
            let _ = this.update_in(cx, |app, _, cx| {
                match selection {
                    Ok(Ok(Some(path))) => app.save_project_at(&path),
                    // Cancelled: keep the project unsaved.
                    Ok(Ok(None)) => {}
                    Ok(Err(error)) => {
                        app.status = format!("Save failed: {error}").into();
                    }
                    Err(_) => {}
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Write the project to `path` and remember it as the current path.
    fn save_project_at(&mut self, path: &Path) {
        match store::save_project(&self.project, path) {
            Ok(()) => {
                let display_path = path.display().to_string();
                self.project_path = Some(display_path.clone());
                self.status = format!("Saved to {display_path}").into();
            }
            Err(error) => {
                self.status = format!("Save failed: {error}").into();
            }
        }
    }

    /// Open the native file chooser and load the project the user picks.
    ///
    /// The chooser runs on the platform thread and reports back through a
    /// oneshot channel, so the app keeps rendering while it is open.
    pub(crate) fn choose_project_file(&self, window: &mut Window, cx: &mut Context<Self>) {
        let selection = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Open roster project".into()),
        });

        cx.spawn_in(window, async move |this, cx| {
            let selection = selection.await;
            let _ = this.update_in(cx, |app, _, cx| {
                match selection {
                    Ok(Ok(Some(paths))) => {
                        if let Some(path) = paths.into_iter().next() {
                            app.load_project_at(&path);
                        }
                    }
                    // Cancelled: leave everything as it was.
                    Ok(Ok(None)) => {}
                    Ok(Err(error)) => {
                        app.status = format!("Open failed: {error}").into();
                    }
                    Err(_) => {}
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Load a project from a chosen path and show it at the Organizations level.
    fn load_project_at(&mut self, path: &Path) {
        match store::load_project(path) {
            Ok(project) => {
                let display_path = path.display().to_string();
                self.project = project;
                self.clamp_selected_org();
                // A freshly loaded file starts at the Organizations level.
                self.org_open = false;
                self.page = Page::Organizations;
                self.project_path = Some(display_path.clone());
                self.status = format!("Opened {display_path}").into();
            }
            Err(error) => {
                self.status = format!("Open failed: {error}").into();
            }
        }
    }

    /// Drop the selection when the organization no longer exists.
    ///
    /// Loading a project does not auto-open the first organization: the user
    /// starts at the Organizations list and chooses one.
    fn clamp_selected_org(&mut self) {
        let still_exists = self.current_org_id.as_deref().is_some_and(|id| {
            self.project
                .organizations
                .iter()
                .any(|organization| organization.id == id)
        });
        if !still_exists {
            self.current_org_id = None;
        }
    }

    fn section_item(&self, page: Page, cx: &mut Context<Self>) -> SidebarMenuItem {
        SidebarMenuItem::new(page.label())
            .icon(page.icon())
            .active(page == self.page)
            .on_click(cx.listener(move |this, _, _, cx| {
                this.page = page;
                cx.notify();
            }))
    }

    fn brand_header(&self, cx: &mut Context<Self>) -> SidebarHeader {
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
            )
    }

    fn project_footer(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let load = SidebarMenuItem::new("Load Project")
            .icon(IconName::FolderOpen)
            .on_click(cx.listener(|this, _, window, cx| {
                this.choose_project_file(window, cx);
            }));
        let save = SidebarMenuItem::new("Save Project")
            .icon(IconName::HardDrive)
            .on_click(cx.listener(|this, _, window, cx| {
                this.save_project_action(window, cx);
            }));
        let menu = SidebarMenu::new()
            .child(load)
            .child(save)
            .render("sidebar-project-menu", window, cx)
            .into_any_element();

        let version = div()
            .pl_2()
            .pt_3()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(concat!("v", env!("CARGO_PKG_VERSION"), " · Phase 5"));

        v_flex()
            .w_full()
            .child(menu)
            .child(version)
            .into_any_element()
    }

    /// Level one: the Organizations group and one indented row per name.
    fn render_org_list_sidebar(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let organization_items = self
            .project
            .organizations
            .iter()
            .map(|organization| {
                let org_id = organization.id.clone();
                SidebarMenuItem::new(truncated_name(&organization.name, 22))
                    .label_style(StyleRefinement::default().pl_3())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.open_org(org_id.clone());
                        cx.notify();
                    }))
            })
            .collect::<Vec<_>>();

        let organizations = SidebarGroup::new("Organizations")
            .child(SidebarMenu::new().children(organization_items));

        Sidebar::new("app-sidebar")
            // A fixed sidebar keeps the roster grid's width predictable.
            .collapsible(false)
            .w(px(240.))
            .header(self.brand_header(cx))
            .children([organizations])
            .footer(self.project_footer(window, cx))
            .into_any_element()
    }

    /// Level two: the open organization and its subsections.
    fn render_org_sidebar(
        &self,
        organization: &crate::domain::Organization,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let back = Button::new("sidebar-back")
            .ghost()
            .icon(Icon::new(IconName::ArrowLeft))
            .tooltip("Back to organizations")
            .on_click(cx.listener(|this, _, _, cx| {
                this.close_org();
                cx.notify();
            }));

        let header = SidebarHeader::new().child(back).child(
            v_flex()
                .flex_1()
                .overflow_hidden()
                .child(
                    div()
                        .text_sm()
                        .font_weight(gpui_kit::FontWeight::MEDIUM)
                        .child(truncated_name(&organization.name, 18)),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child("Management"),
                ),
        );

        let setup = SidebarGroup::new("Setup").child(SidebarMenu::new().children([
            self.section_item(Page::Positions, cx),
            self.section_item(Page::Employees, cx),
            self.section_item(Page::Shifts, cx),
        ]));
        let schedule = SidebarGroup::new("Schedule").child(SidebarMenu::new().children([
            self.section_item(Page::Rosters, cx),
            self.section_item(Page::RosterPlan, cx),
        ]));
        let output = SidebarGroup::new("Output")
            .child(SidebarMenu::new().child(self.section_item(Page::Export, cx)));

        Sidebar::new("app-sidebar")
            .collapsible(false)
            .w(px(240.))
            .header(header)
            .children([setup, schedule, output])
            .footer(self.project_footer(window, cx))
            .into_any_element()
    }

    fn render_sidebar(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        match self.selected_org() {
            Some(organization) if self.org_open => {
                self.render_org_sidebar(organization, window, cx)
            }
            _ => self.render_org_list_sidebar(window, cx),
        }
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = if !self.org_open || self.selected_org().is_none() {
            self.render_organizations(cx)
        } else {
            match self.page {
                Page::Organizations => self.render_organizations(cx),
                Page::Positions => self.render_positions(cx),
                Page::Rosters => self.render_rosters(cx),
                Page::Employees => self.render_employees(cx),
                Page::Shifts => self.render_shifts(cx),
                Page::RosterPlan | Page::Export => {
                    self.render_pending(&format!("Coming in {}.", self.page.later_phase()))
                }
            }
        };

        div()
            .flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.render_sidebar(window, cx))
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

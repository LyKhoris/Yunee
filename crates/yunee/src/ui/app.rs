//! The application shell, following the GNOME HIG: a sidebar of dynamic
//! locations (dashboard + courses + settings) and a content area whose header
//! carries a view switcher for a course's sections.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use adw::prelude::*;
use gtk4 as gtk;
use gtk4::gdk;
use gtk4::gio;
use gtk4::glib;

use yunee_store as st;
use yunee_store::Store;

use crate::paths;
use crate::runtime::runtime;
use crate::state::AppState;
use crate::sync::{self, SyncReport};
use crate::ui::{connect, course, dashboard, detail, settings, widgets};

/// The app's reverse-DNS identity.
pub const APP_ID: &str = "io.github.LyKhoris.Yunee";

/// Where to send someone who wants the latest build.
pub const RELEASES_URL: &str = "https://github.com/LyKhoris/Yunee/releases/latest";

/// One position in the content navigation stack. The sidebar selects a base
/// screen; opening an assignment, page, file, or plain web item pushes a detail
/// on top of it, and the header's back button pops it.
#[derive(Clone)]
pub(crate) enum Screen {
    Dashboard,
    Settings,
    Course(String),
    Assignment {
        course: String,
        id: String,
        title: String,
    },
    Page {
        course: String,
        key: String,
        title: String,
    },
    File {
        course: String,
        id: String,
        title: String,
    },
    /// A module item Yunee cannot render offline: a quiz, an external tool, a
    /// discussion. Shows what it is and offers Canvas + a sync.
    Web {
        course: String,
        title: String,
        kind: String,
        url: Option<String>,
    },
}

/// The live UI, shared between widgets through `Rc`.
pub struct Ui {
    pub(crate) state: Arc<AppState>,
    pub(crate) window: adw::ApplicationWindow,
    pub(crate) toasts: adw::ToastOverlay,
    pub(crate) split: adw::NavigationSplitView,
    pub(crate) sidebar: gtk::ListBox,
    pub(crate) content: gtk::Stack,
    pub(crate) content_header: adw::HeaderBar,
    pub(crate) content_title: adw::WindowTitle,
    pub(crate) back_button: gtk::Button,
    pub(crate) selected: RefCell<Option<st::Course>>,
    /// Courses whose files we have already tried to load on demand.
    pub(crate) files_attempted: RefCell<HashSet<st::LocalId>>,
    /// Pages we have already tried to fetch on demand, keyed by `course|page`.
    pub(crate) pages_attempted: RefCell<HashSet<String>>,
    /// Decoded images, keyed by URL, so re-rendering a page does not refetch.
    pub(crate) image_cache: RefCell<HashMap<String, gdk::Texture>>,
    /// The content navigation stack; the last entry is what is on screen.
    pub(crate) screens: RefCell<Vec<Screen>>,
    /// The last course tab viewed, per course, so returning from a detail lands
    /// back where you left off.
    pub(crate) course_tab: RefCell<HashMap<st::LocalId, String>>,
    pub(crate) dashboard_page: gtk::Box,
    pub(crate) course_page: gtk::Box,
    pub(crate) detail_page: gtk::Box,
    pub(crate) settings_page: gtk::Box,
}

/// Build the window, wire it up, and present it.
pub fn build(app: &adw::Application) -> adw::ApplicationWindow {
    widgets::install_css();

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Yunee")
        .default_width(1120)
        .default_height(760)
        .build();

    let store = match Store::open(&paths::db_path()) {
        Ok(store) => store,
        Err(e) => return fatal_window(app, &e.to_string()),
    };
    let state = Arc::new(AppState::new(store));

    // --- sidebar: dynamic locations (dashboard, courses, settings) ---
    let sidebar_title = adw::WindowTitle::new("Yunee", "");
    let sidebar_header = adw::HeaderBar::new();
    sidebar_header.set_title_widget(Some(&sidebar_title));

    let sidebar = gtk::ListBox::new();
    sidebar.add_css_class("navigation-sidebar");
    sidebar.set_selection_mode(gtk::SelectionMode::Single);
    let sidebar_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&sidebar)
        .build();
    let sidebar_toolbar = adw::ToolbarView::new();
    sidebar_toolbar.add_top_bar(&sidebar_header);
    sidebar_toolbar.set_content(Some(&sidebar_scroll));
    let sidebar_page = adw::NavigationPage::new(&sidebar_toolbar, "Yunee");

    // --- content ---
    let content_title = adw::WindowTitle::new("Dashboard", "");
    let content_header = adw::HeaderBar::new();
    content_header.set_title_widget(Some(&content_title));

    // Shown only while a detail is pushed; pops back to the course beneath.
    let back_button = gtk::Button::from_icon_name("go-previous-symbolic");
    back_button.set_tooltip_text(Some("Back"));
    back_button.add_css_class("flat");
    back_button.set_visible(false);
    content_header.pack_start(&back_button);

    let refresh = gtk::Button::from_icon_name("view-refresh-symbolic");
    refresh.set_tooltip_text(Some("Sync with Canvas"));
    refresh.add_css_class("flat");
    content_header.pack_end(&refresh);

    let menu = gio::Menu::new();
    menu.append(Some("Sync now"), Some("app.sync"));
    menu.append(Some("Connect to Canvas…"), Some("app.connect"));
    menu.append(Some("Settings"), Some("app.settings"));
    menu.append(Some("About Yunee"), Some("app.about"));
    let menu_button = gtk::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .menu_model(&menu)
        .build();
    menu_button.add_css_class("flat");
    content_header.pack_end(&menu_button);

    let dashboard_page = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let course_page = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let detail_page = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let settings_page = gtk::Box::new(gtk::Orientation::Vertical, 8);
    for page in [&dashboard_page, &detail_page, &settings_page] {
        page.set_margin_top(18);
        page.set_margin_bottom(24);
        page.set_margin_start(20);
        page.set_margin_end(20);
    }

    let content = gtk::Stack::new();
    content.set_transition_type(gtk::StackTransitionType::Crossfade);
    content.add_titled(&scroll(&dashboard_page), Some("dashboard"), "Dashboard");
    content.add_titled(&course_page, Some("course"), "Course");
    content.add_titled(&scroll(&detail_page), Some("detail"), "Detail");
    content.add_titled(&scroll(&settings_page), Some("settings"), "Settings");

    let content_toolbar = adw::ToolbarView::new();
    content_toolbar.add_top_bar(&content_header);
    content_toolbar.set_content(Some(&content));
    let content_nav = adw::NavigationPage::new(&content_toolbar, "Content");

    let split = adw::NavigationSplitView::new();
    split.set_sidebar(Some(&sidebar_page));
    split.set_content(Some(&content_nav));
    split.set_min_sidebar_width(240.0);
    split.set_max_sidebar_width(340.0);

    let toasts = adw::ToastOverlay::new();
    toasts.set_child(Some(&split));
    window.set_content(Some(&toasts));

    let ui = Rc::new(Ui {
        state,
        window: window.clone(),
        toasts,
        split,
        sidebar: sidebar.clone(),
        content,
        content_header,
        content_title,
        back_button: back_button.clone(),
        selected: RefCell::new(None),
        files_attempted: RefCell::new(HashSet::new()),
        pages_attempted: RefCell::new(HashSet::new()),
        image_cache: RefCell::new(HashMap::new()),
        screens: RefCell::new(Vec::new()),
        course_tab: RefCell::new(HashMap::new()),
        dashboard_page,
        course_page,
        detail_page,
        settings_page,
    });

    ui.reload_sidebar();
    ui.reload_dashboard();
    ui.reload_settings();
    ui.show_dashboard();

    {
        let ui = ui.clone();
        back_button.connect_clicked(move |_| ui.go_back());
    }
    {
        let ui = ui.clone();
        refresh.connect_clicked(move |_| ui.sync_now());
    }
    for (name, handler) in [
        ("sync", {
            let ui = ui.clone();
            Rc::new(move || ui.sync_now()) as Rc<dyn Fn()>
        }),
        ("connect", {
            let ui = ui.clone();
            Rc::new(move || ui.open_connect())
        }),
        ("settings", {
            let ui = ui.clone();
            Rc::new(move || {
                ui.reload_settings();
                ui.show_settings();
            })
        }),
        ("about", {
            let ui = ui.clone();
            Rc::new(move || ui.show_about())
        }),
    ] {
        let action = gio::SimpleAction::new(name, None);
        action.connect_activate(move |_, _| (handler)());
        app.add_action(&action);
    }

    if ui.state.connection().is_some() {
        ui.sync_now();
    }

    window.present();
    window
}

fn scroll(child: &impl IsA<gtk::Widget>) -> gtk::ScrolledWindow {
    gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(child)
        .build()
}

fn sidebar_row(
    icon: &str,
    title: &str,
    subtitle: &str,
    prefix: Option<gtk::Widget>,
) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title(glib::markup_escape_text(title))
        .subtitle(glib::markup_escape_text(subtitle))
        .activatable(true)
        .build();
    if let Some(prefix) = prefix {
        row.add_prefix(&prefix);
    } else {
        row.add_prefix(&gtk::Image::from_icon_name(icon));
    }
    row
}

fn section_header(text: &str) -> gtk::ListBoxRow {
    let label = gtk::Label::new(Some(text));
    label.set_xalign(0.0);
    label.add_css_class("section");
    label.set_margin_top(10);
    label.set_margin_bottom(4);
    label.set_margin_start(12);
    let row = gtk::ListBoxRow::new();
    row.set_selectable(false);
    row.set_activatable(false);
    row.set_child(Some(&label));
    row
}

impl Ui {
    pub(crate) fn toast(&self, message: &str) {
        // Toast titles are Pango markup; escape so a path or Canvas error with
        // `&`/`<` cannot break the label.
        self.toasts
            .add_toast(adw::Toast::new(&glib::markup_escape_text(message)));
    }

    /// Put a widget in the content header bar's title slot (a view switcher for
    /// a course, a plain title otherwise).
    pub(crate) fn set_header_widget(&self, widget: &impl IsA<gtk::Widget>) {
        self.content_header.set_title_widget(Some(widget));
    }

    pub(crate) fn set_back_visible(&self, visible: bool) {
        self.back_button.set_visible(visible);
    }

    /// Show the dashboard (header + page) without touching the nav stack.
    fn display_dashboard(&self) {
        self.content.set_visible_child_name("dashboard");
        self.set_header_widget(&self.content_title);
        self.content_title.set_title("Dashboard");
        self.content_title.set_subtitle("");
        self.window.set_title(Some("Yunee"));
        self.set_back_visible(false);
        self.split.set_show_content(true);
    }

    fn display_settings(&self) {
        self.content.set_visible_child_name("settings");
        self.set_header_widget(&self.content_title);
        self.content_title.set_title("Settings");
        self.content_title.set_subtitle("");
        self.window.set_title(Some("Yunee"));
        self.set_back_visible(false);
        self.split.set_show_content(true);
    }

    /// Switch the content area to a detail view: back button on, the detail's
    /// own title in the header, and the detail page visible.
    pub(crate) fn enter_detail(&self, title: &str, subtitle: &str) {
        let window_title = adw::WindowTitle::new(title, subtitle);
        self.set_header_widget(&window_title);
        self.set_back_visible(true);
        self.content.set_visible_child_name("detail");
        self.window.set_title(Some(title));
        self.split.set_show_content(true);
    }

    pub(crate) fn show_dashboard(self: &Rc<Self>) {
        self.select(Screen::Dashboard);
    }

    pub(crate) fn show_settings(self: &Rc<Self>) {
        self.select(Screen::Settings);
    }

    pub(crate) fn show_course(self: &Rc<Self>, course: st::Course) {
        self.select(Screen::Course(course.canvas_id));
    }

    // ------------------------------------------------------------------
    // Navigation stack
    // ------------------------------------------------------------------

    /// Replace the stack with a base screen (a sidebar selection).
    pub(crate) fn select(self: &Rc<Self>, screen: Screen) {
        *self.screens.borrow_mut() = vec![screen.clone()];
        self.render_screen(&screen);
    }

    /// Push a detail on top of the current screen.
    pub(crate) fn push_detail(self: &Rc<Self>, screen: Screen) {
        if self.screens.borrow().is_empty() {
            self.screens.borrow_mut().push(Screen::Dashboard);
        }
        self.screens.borrow_mut().push(screen.clone());
        self.render_screen(&screen);
    }

    pub(crate) fn go_back(self: &Rc<Self>) {
        let top = {
            let mut stack = self.screens.borrow_mut();
            if stack.len() > 1 {
                stack.pop();
            }
            stack.last().cloned()
        };
        if let Some(top) = top {
            self.render_screen(&top);
        }
    }

    /// Render whatever is on top of the stack (after a sync or a data change).
    pub(crate) fn render_top(self: &Rc<Self>) {
        match self.screens.borrow().last().cloned() {
            Some(top) => self.render_screen(&top),
            None => self.show_dashboard(),
        }
    }

    fn render_screen(self: &Rc<Self>, screen: &Screen) {
        match screen {
            Screen::Dashboard => {
                self.reload_dashboard();
                self.display_dashboard();
            }
            Screen::Settings => {
                self.reload_settings();
                self.display_settings();
            }
            Screen::Course(canvas_id) => match self.course_by_canvas(canvas_id) {
                Some(course) => course::open(self, course),
                None => self.gone(),
            },
            Screen::Assignment { course, id, title } => {
                let Some(course) = self.course_by_canvas(course) else {
                    return self.gone();
                };
                match self
                    .state
                    .store
                    .get_assignment_by_canvas_id(id)
                    .ok()
                    .flatten()
                {
                    Some(assignment) => {
                        *self.selected.borrow_mut() = Some(course.clone());
                        detail::assignment(self, &course, &assignment);
                    }
                    None => detail::missing(
                        self,
                        &course,
                        "Assignment",
                        Some("This assignment is not saved on this machine yet."),
                        Some(title),
                        detail::Retry::Sync,
                    ),
                }
            }
            Screen::Page { course, key, title } => {
                let Some(course) = self.course_by_canvas(course) else {
                    return self.gone();
                };
                *self.selected.borrow_mut() = Some(course.clone());
                match self.state.store.get_page(course.id, key).ok().flatten() {
                    Some(page) => detail::page(self, &course, &page),
                    None => {
                        // No row at all means sync never saw this page (its
                        // course's Pages index is disabled). Try once on open.
                        let attempt_key = format!("{}|{}", course.canvas_id, key);
                        let first = self.pages_attempted.borrow_mut().insert(attempt_key);
                        if first && self.state.connection().is_some() {
                            detail::loading(self, &course, title);
                            self.load_page(course.canvas_id.clone(), key.clone());
                        } else {
                            detail::missing(
                                self,
                                &course,
                                "Page",
                                Some("This page hasn't been downloaded yet."),
                                Some(title),
                                detail::Retry::Page { key: key.clone() },
                            );
                        }
                    }
                }
            }
            Screen::File { course, id, title } => {
                let Some(course) = self.course_by_canvas(course) else {
                    return self.gone();
                };
                match self.state.store.get_file(id).ok().flatten() {
                    Some(file) => {
                        *self.selected.borrow_mut() = Some(course.clone());
                        detail::file(self, &course, &file);
                    }
                    None => detail::missing(
                        self,
                        &course,
                        "File",
                        Some("This course's files have not been loaded yet."),
                        Some(title),
                        detail::Retry::LoadFiles,
                    ),
                }
            }
            Screen::Web {
                course,
                title,
                kind,
                url,
            } => {
                let Some(course) = self.course_by_canvas(course) else {
                    return self.gone();
                };
                *self.selected.borrow_mut() = Some(course.clone());
                detail::web(self, &course, title, kind, url.as_deref());
            }
        }
    }

    fn gone(self: &Rc<Self>) {
        self.toast("That course is no longer available.");
        self.show_dashboard();
    }

    pub(crate) fn course_by_canvas(&self, canvas_id: &str) -> Option<st::Course> {
        let local = self.state.store.course_local_id(canvas_id).ok().flatten()?;
        self.state.store.get_course(local).ok().flatten()
    }

    /// Open an assignment's detail from a course row.
    pub(crate) fn open_assignment(self: &Rc<Self>, course: &st::Course, a: &st::Assignment) {
        self.push_detail(Screen::Assignment {
            course: course.canvas_id.clone(),
            id: a.canvas_id.clone(),
            title: a.name.clone(),
        });
    }

    /// Resolve a module item to the right detail for its type.
    pub(crate) fn open_module_item(self: &Rc<Self>, course: &st::Course, item: &st::ModuleItem) {
        let course_id = course.canvas_id.clone();
        match item.item_type.as_deref() {
            Some("Assignment") => match item.content_id.clone() {
                Some(id) => self.push_detail(Screen::Assignment {
                    course: course_id,
                    id,
                    title: item.title.clone(),
                }),
                None => self.push_detail(Screen::Web {
                    course: course_id,
                    title: item.title.clone(),
                    kind: "Assignment".into(),
                    url: item.html_url.clone(),
                }),
            },
            Some("Page") => {
                let key = item
                    .content_id
                    .clone()
                    .or_else(|| slug_from_url(item.html_url.as_deref()))
                    .unwrap_or_default();
                self.push_detail(Screen::Page {
                    course: course_id,
                    key,
                    title: item.title.clone(),
                });
            }
            Some("File") => self.push_detail(Screen::File {
                course: course_id,
                id: item.content_id.clone().unwrap_or_default(),
                title: item.title.clone(),
            }),
            // An external link has no offline body — hand it to the browser.
            Some("ExternalUrl") => {
                if let Some(url) = item.html_url.clone() {
                    self.open_url(&url);
                }
            }
            // A module item's text header is shown inline, not opened.
            Some("SubHeader") => {}
            other => self.push_detail(Screen::Web {
                course: course_id,
                title: item.title.clone(),
                kind: item_type_label(other).into(),
                url: item.html_url.clone(),
            }),
        }
    }

    /// Reload every synced view from the local store.
    pub(crate) fn reload_all(self: &Rc<Self>) {
        self.reload_sidebar();
        self.reload_dashboard();
        self.reload_settings();
        self.render_top();
    }

    pub(crate) fn reload_dashboard(self: &Rc<Self>) {
        dashboard::render(self);
    }

    pub(crate) fn reload_settings(self: &Rc<Self>) {
        settings::render(self);
    }

    /// Rebuild the sidebar: Dashboard, the courses, then Settings and About.
    pub(crate) fn reload_sidebar(self: &Rc<Self>) {
        widgets::clear_list(&self.sidebar);

        let dashboard = sidebar_row(
            "view-grid-symbolic",
            "Dashboard",
            "What's due and new",
            None,
        );
        {
            let ui = self.clone();
            dashboard.connect_activated(move |_| ui.show_dashboard());
        }
        self.sidebar.append(&dashboard);

        self.sidebar.append(&section_header("Courses"));

        let courses = self.state.store.list_courses().unwrap_or_default();
        if courses.is_empty() {
            let hint = gtk::Label::new(Some("Sync to add your courses"));
            hint.set_xalign(0.0);
            hint.add_css_class("dim-label");
            hint.set_margin_top(6);
            hint.set_margin_bottom(6);
            hint.set_margin_start(12);
            let row = gtk::ListBoxRow::new();
            row.set_selectable(false);
            row.set_activatable(false);
            row.set_child(Some(&hint));
            self.sidebar.append(&row);
        }
        for course in &courses {
            let subtitle = match (&course.term, &course.professor) {
                (Some(term), Some(prof)) => format!("{term}  ·  {prof}"),
                (Some(term), None) => term.clone(),
                (None, Some(prof)) => prof.clone(),
                (None, None) => String::new(),
            };
            let row = sidebar_row(
                "",
                &short_course_name(&course.name),
                &subtitle,
                Some(widgets::accent_dot(widgets::accent_index(&course.canvas_id)).upcast()),
            );

            let open = self
                .state
                .store
                .list_assignments(course.id)
                .map(|v| v.iter().filter(|a| !a.is_submitted()).count())
                .unwrap_or(0);
            let unread = self
                .state
                .store
                .list_announcements(course.id)
                .map(|v| v.iter().filter(|a| a.is_unread()).count())
                .unwrap_or(0);
            if open > 0 || unread > 0 {
                let chips = gtk::Box::new(gtk::Orientation::Horizontal, 10);
                chips.set_valign(gtk::Align::Center);
                if open > 0 {
                    chips.append(&widgets::count_chip("document-edit-symbolic", open));
                }
                if unread > 0 {
                    chips.append(&widgets::count_chip("mail-unread-symbolic", unread));
                }
                row.add_suffix(&chips);
            }

            let ui = self.clone();
            let course = course.clone();
            row.connect_activated(move |_| ui.show_course(course.clone()));
            self.sidebar.append(&row);
        }

        self.sidebar.append(&section_header(""));

        let settings_row = sidebar_row(
            "emblem-system-symbolic",
            "Settings",
            "Canvas connection and sync",
            None,
        );
        {
            let ui = self.clone();
            settings_row.connect_activated(move |_| {
                ui.reload_settings();
                ui.show_settings();
            });
        }
        self.sidebar.append(&settings_row);

        let about_row = sidebar_row("help-about-symbolic", "About", "Version and source", None);
        {
            let ui = self.clone();
            about_row.connect_activated(move |_| ui.show_about());
        }
        self.sidebar.append(&about_row);
    }

    // ------------------------------------------------------------------
    // Dialogs
    // ------------------------------------------------------------------

    pub(crate) fn open_connect(self: &Rc<Self>) {
        let existing = self.state.connection().map(|c| c.base_url);
        self.open_connect_with(existing);
    }

    pub(crate) fn open_connect_with(self: &Rc<Self>, existing: Option<String>) {
        let ui = self.clone();
        let on_connected: Rc<dyn Fn()> = Rc::new(move || {
            ui.toast("Connected to Canvas.");
            ui.reload_settings();
            ui.sync_now();
        });
        connect::present(&self.window, self.state.clone(), existing, on_connected);
    }

    /// Open a URL in the user's default browser.
    pub(crate) fn open_url(&self, url: &str) {
        let launcher = gtk::UriLauncher::new(url);
        launcher.launch(Some(&self.window), None::<&gio::Cancellable>, |_| {});
    }

    pub(crate) fn show_about(&self) {
        let about = adw::AboutDialog::builder()
            .application_name("Yunee")
            .application_icon(APP_ID)
            .version(env!("CARGO_PKG_VERSION"))
            .developer_name("LyKhoris")
            .website("https://github.com/LyKhoris/Yunee")
            .comments("A better Canvas client for GNOME. Local, single-user, offline-first.")
            .build();
        about.present(Some(&self.window));
    }

    // ------------------------------------------------------------------
    // Async work
    // ------------------------------------------------------------------

    pub(crate) fn sync_now(self: &Rc<Self>) {
        let Some(connection) = self.state.connection() else {
            self.toast("Connect to Canvas first.");
            self.open_connect();
            return;
        };
        self.toast("Syncing…");
        let state = self.state.clone();
        let ui = self.clone();
        let (tx, rx) = async_channel::bounded(1);
        std::thread::spawn(move || {
            let report = runtime().block_on(sync::sync_all(&state.store, &connection));
            let _ = tx.send_blocking(report);
        });
        glib::MainContext::default().spawn_local(async move {
            if let Ok(report) = rx.recv().await {
                ui.on_sync_done(report);
            }
        });
    }

    fn on_sync_done(self: &Rc<Self>, report: SyncReport) {
        self.reload_all();

        if !report.new_unread.is_empty() {
            let summary = if report.new_unread.len() == 1 {
                "New Canvas announcement".to_string()
            } else {
                format!("{} new Canvas announcements", report.new_unread.len())
            };
            let body = report
                .new_unread
                .iter()
                .take(3)
                .map(|(course, title)| format!("{course}: {title}"))
                .collect::<Vec<_>>()
                .join("\n");
            let _ = notify_rust::Notification::new()
                .summary(&summary)
                .body(&body)
                .icon(APP_ID)
                .show();
        }

        if report.ok() {
            let c = &report.counts;
            self.toast(&format!(
                "Synced {} course(s), {} assignment(s), {} announcement(s).",
                c.courses, c.assignments, c.announcements
            ));
        } else {
            let first = report.errors.first().cloned().unwrap_or_default();
            self.toast(&format!("Synced with a warning — {first}"));
        }
    }

    pub(crate) fn mark_announcement_read(self: &Rc<Self>, course_canvas_id: &str, canvas_id: &str) {
        if self
            .state
            .store
            .mark_announcement_read(canvas_id, &chrono::Utc::now().to_rfc3339())
            .is_err()
        {
            return;
        }
        self.reload_dashboard();
        self.reload_sidebar();
        if let Some(course) = self.selected.borrow().clone() {
            course::open(self, course);
        }

        let state = self.state.clone();
        let course_canvas_id = course_canvas_id.to_string();
        let canvas_id = canvas_id.to_string();
        std::thread::spawn(move || {
            if let Some(connection) = state.connection() {
                if let Ok(client) = connection.client() {
                    let _ =
                        runtime().block_on(client.mark_topic_read(&course_canvas_id, &canvas_id));
                }
            }
        });
    }

    /// Load one course's files on demand (the Files tab is the only caller).
    pub(crate) fn load_course_files(self: &Rc<Self>, course: st::Course) {
        let Some(connection) = self.state.connection() else {
            return;
        };
        let store = self.state.store.clone();
        let ui = self.clone();
        let canvas_id = course.canvas_id.clone();
        let local_id = course.id;
        let (tx, rx) = async_channel::bounded(1);
        std::thread::spawn(move || {
            let result = runtime().block_on(sync::sync_course_files(
                &store,
                &connection,
                &canvas_id,
                local_id,
            ));
            let _ = tx.send_blocking(result);
        });
        glib::MainContext::default().spawn_local(async move {
            match rx.recv().await {
                Ok(Ok(_n)) => ui.reload_all(),
                Ok(Err(e)) => ui.toast(&format!("Could not load files: {e}")),
                Err(_) => {}
            }
        });
    }

    /// Fetch a single wiki page on demand, then re-render (it may be a course
    /// whose Pages index is disabled, so a full sync cannot pre-fetch it).
    pub(crate) fn load_page(self: &Rc<Self>, course_canvas_id: String, key: String) {
        let Some(connection) = self.state.connection() else {
            self.toast("Connect to Canvas first.");
            return;
        };
        let store = self.state.store.clone();
        let ui = self.clone();
        let (tx, rx) = async_channel::bounded(1);
        std::thread::spawn(move || {
            let result = runtime().block_on(sync::fetch_page(
                &store,
                &connection,
                &course_canvas_id,
                &key,
            ));
            let _ = tx.send_blocking(result);
        });
        glib::MainContext::default().spawn_local(async move {
            match rx.recv().await {
                Ok(Ok(())) => ui.reload_all(),
                Ok(Err(e)) => ui.toast(&format!("Could not load page: {e}")),
                Err(_) => {}
            }
        });
    }

    /// Fetch an image referenced by a Canvas body and set it on `picture`,
    /// hiding `spinner` when done. Results are cached by URL for the session.
    pub(crate) fn load_image(
        self: &Rc<Self>,
        src: String,
        picture: gtk::Picture,
        spinner: gtk::Spinner,
    ) {
        let connection = self.state.connection();
        let resolved = self.resolve_url(&src);
        let ui = self.clone();
        let (tx, rx) = async_channel::bounded(1);
        std::thread::spawn(move || {
            let result = runtime().block_on(async {
                let client = connection
                    .ok_or_else(|| anyhow::anyhow!("not connected"))?
                    .client()?;
                let response = client.download(&resolved).await?;
                let bytes = response.bytes().await?;
                Ok::<Vec<u8>, anyhow::Error>(bytes.to_vec())
            });
            let _ = tx.send_blocking(result);
        });
        glib::MainContext::default().spawn_local(async move {
            if let Ok(Ok(bytes)) = rx.recv().await {
                let gbytes = glib::Bytes::from_owned(bytes);
                if let Ok(texture) = gdk::Texture::from_bytes(&gbytes) {
                    ui.image_cache.borrow_mut().insert(src, texture.clone());
                    picture.set_paintable(Some(&texture));
                }
            }
            spinner.stop();
            spinner.set_visible(false);
        });
    }

    /// Resolve a possibly-relative URL in a Canvas body against the server.
    fn resolve_url(&self, src: &str) -> String {
        if src.starts_with("http://") || src.starts_with("https://") {
            return src.to_string();
        }
        let base = self.state.base_url().unwrap_or_default();
        let base = base.trim_end_matches('/');
        let base = if base.contains("://") {
            base.to_string()
        } else {
            format!("https://{base}")
        };
        if src.starts_with('/') {
            format!("{base}{src}")
        } else {
            format!("{base}/{src}")
        }
    }

    pub(crate) fn download_file(self: &Rc<Self>, file: st::FileEntry) {
        let Some(connection) = self.state.connection() else {
            self.toast("Connect to Canvas first.");
            return;
        };
        let Some(url) = file.url.clone() else {
            self.toast("Canvas did not provide a download URL for this file.");
            return;
        };
        let course_name = self
            .selected
            .borrow()
            .as_ref()
            .map(|c| c.name.clone())
            .unwrap_or_else(|| "course".into());
        let filename = file
            .filename
            .clone()
            .unwrap_or_else(|| file.display_name.clone());
        let target: PathBuf = paths::downloads_dir()
            .join(sanitize(&course_name))
            .join(sanitize(&filename));

        self.toast(&format!("Downloading {}…", file.display_name));
        let ui = self.clone();
        let canvas_id = file.canvas_id.clone();
        let (tx, rx) = async_channel::bounded(1);
        std::thread::spawn(move || {
            let result = runtime().block_on(async move {
                let client = connection.client()?;
                let response = client.download(&url).await?;
                let bytes = response.bytes().await?;
                if let Some(parent) = target.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }
                tokio::fs::write(&target, &bytes).await?;
                Ok::<PathBuf, anyhow::Error>(target)
            });
            let _ = tx.send_blocking(result);
        });
        glib::MainContext::default().spawn_local(async move {
            match rx.recv().await {
                Ok(Ok(path)) => {
                    let _ = ui
                        .state
                        .store
                        .set_file_local_path(&canvas_id, &path.to_string_lossy());
                    if let Some(course) = ui.selected.borrow().clone() {
                        course::open(&ui, course);
                    }
                    ui.toast(&format!("Saved to {}", path.display()));
                }
                Ok(Err(e)) => ui.toast(&format!("Download failed: {e}")),
                Err(_) => {}
            }
        });
    }
}

/// A compact course label for the sidebar: drop the term prefix and the
/// trailing CRN, e.g. "Fall 2026 ANTH 300-01 16568" → "ANTH 300-01".
fn short_course_name(name: &str) -> String {
    const SEASONS: [&str; 4] = ["Fall", "Spring", "Summer", "Winter"];
    let mut rest = name;
    let mut parts = name.splitn(3, ' ');
    if let (Some(season), Some(year), Some(tail)) = (parts.next(), parts.next(), parts.next()) {
        if SEASONS.contains(&season) && year.len() == 4 && year.chars().all(|c| c.is_ascii_digit())
        {
            rest = tail;
        }
    }
    let mut tokens: Vec<&str> = rest.split_whitespace().collect();
    if let Some(last) = tokens.last() {
        if last.len() >= 3 && last.chars().all(|c| c.is_ascii_digit()) {
            tokens.pop();
        }
    }
    let short = tokens.join(" ");
    if short.is_empty() {
        name.to_string()
    } else {
        short
    }
}

/// The last path segment of a Canvas URL — the slug a page is addressed by.
fn slug_from_url(url: Option<&str>) -> Option<String> {
    let url = url?;
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let slug = path.trim_end_matches('/').rsplit('/').next()?;
    if slug.is_empty() {
        None
    } else {
        Some(slug.to_string())
    }
}

/// A human label for a module item's Canvas type.
fn item_type_label(item_type: Option<&str>) -> &'static str {
    match item_type.unwrap_or("") {
        "Assignment" => "Assignment",
        "Quiz" => "Quiz",
        "File" => "File",
        "Page" => "Page",
        "Discussion" => "Discussion",
        "ExternalTool" => "External tool",
        "ExternalUrl" => "Link",
        _ => "Item",
    }
}

pub(crate) fn sanitize(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '.' | '-' | '_' | ' ') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = cleaned.trim().trim_matches('.');
    if trimmed.is_empty() {
        "file".into()
    } else {
        trimmed.to_string()
    }
}

fn fatal_window(app: &adw::Application, message: &str) -> adw::ApplicationWindow {
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Yunee")
        .build();
    let page = widgets::empty_state("dialog-error-symbolic", "Yunee could not start", message);
    window.set_content(Some(&page));
    window.present();
    window
}

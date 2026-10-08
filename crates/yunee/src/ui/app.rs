//! The application shell, following the GNOME HIG: a sidebar of dynamic
//! locations (dashboard + courses + settings) and a content area whose header
//! carries a view switcher for a course's sections.

use std::cell::RefCell;
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use adw::prelude::*;
use gtk4 as gtk;
use gtk4::gio;
use gtk4::glib;

use yunee_store as st;
use yunee_store::Store;

use crate::paths;
use crate::runtime::runtime;
use crate::state::AppState;
use crate::sync::{self, SyncReport};
use crate::ui::{connect, course, dashboard, settings, widgets};

/// The app's reverse-DNS identity.
pub const APP_ID: &str = "io.github.LyKhoris.Yunee";

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
    pub(crate) selected: RefCell<Option<st::Course>>,
    /// Courses whose files we have already tried to load on demand.
    pub(crate) files_attempted: RefCell<HashSet<st::LocalId>>,
    pub(crate) dashboard_page: gtk::Box,
    pub(crate) course_page: gtk::Box,
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
    let settings_page = gtk::Box::new(gtk::Orientation::Vertical, 8);
    for page in [&dashboard_page, &settings_page] {
        page.set_margin_top(18);
        page.set_margin_bottom(24);
        page.set_margin_start(20);
        page.set_margin_end(20);
    }

    let content = gtk::Stack::new();
    content.set_transition_type(gtk::StackTransitionType::Crossfade);
    content.add_titled(&scroll(&dashboard_page), Some("dashboard"), "Dashboard");
    content.add_titled(&course_page, Some("course"), "Course");
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
        selected: RefCell::new(None),
        files_attempted: RefCell::new(HashSet::new()),
        dashboard_page,
        course_page,
        settings_page,
    });

    ui.reload_sidebar();
    ui.reload_dashboard();
    ui.reload_settings();
    ui.show_dashboard();

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
        .title(title)
        .subtitle(subtitle)
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
        self.toasts.add_toast(adw::Toast::new(message));
    }

    /// Put a widget in the content header bar's title slot (a view switcher for
    /// a course, a plain title otherwise).
    pub(crate) fn set_header_widget(&self, widget: &impl IsA<gtk::Widget>) {
        self.content_header.set_title_widget(Some(widget));
    }

    pub(crate) fn show_dashboard(&self) {
        self.content.set_visible_child_name("dashboard");
        self.set_header_widget(&self.content_title);
        self.content_title.set_title("Dashboard");
        self.content_title.set_subtitle("");
        self.window.set_title(Some("Yunee"));
        self.split.set_show_content(true);
    }

    pub(crate) fn show_settings(&self) {
        self.content.set_visible_child_name("settings");
        self.set_header_widget(&self.content_title);
        self.content_title.set_title("Settings");
        self.content_title.set_subtitle("");
        self.window.set_title(Some("Yunee"));
        self.split.set_show_content(true);
    }

    pub(crate) fn show_course(self: &Rc<Self>, course: st::Course) {
        course::open(self, course);
    }

    /// Reload every synced view from the local store.
    pub(crate) fn reload_all(self: &Rc<Self>) {
        self.reload_sidebar();
        self.reload_dashboard();
        if let Some(course) = self.selected.borrow().clone() {
            course::open(self, course);
        }
        self.reload_settings();
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
                Ok(Ok(_n)) => course::open(&ui, course),
                Ok(Err(e)) => ui.toast(&format!("Could not load files: {e}")),
                Err(_) => {}
            }
        });
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

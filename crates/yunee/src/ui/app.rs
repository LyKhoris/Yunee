//! The application window and its pages.
//!
//! One window: a `NavigationSplitView` whose sidebar lists the dashboard and
//! the synced courses, and whose content stack holds the Dashboard, Course,
//! Files, and Settings pages. Pages are rebuilt from the local store whenever
//! data changes, so the UI never blocks on the network.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use adw::prelude::*;
use gtk4 as gtk;
use gtk4::gio;
use gtk4::glib;

use yunee_store as st;
use yunee_store::{LocalId, Store};

use crate::format;
use crate::paths;
use crate::runtime::runtime;
use crate::state::AppState;
use crate::sync::{self, SyncReport};
use crate::ui::connect;
use crate::ui::widgets;

/// The app's reverse-DNS identity.
pub const APP_ID: &str = "io.github.LyKhoris.Yunee";

/// The live UI, shared between widgets through `Rc`.
pub struct Ui {
    state: Arc<AppState>,
    window: adw::ApplicationWindow,
    toasts: adw::ToastOverlay,
    split: adw::NavigationSplitView,
    stack: gtk::Stack,
    sidebar: gtk::ListBox,
    content_title: adw::WindowTitle,
    dashboard: gtk::Box,
    course: gtk::Box,
    files: gtk::Box,
    settings: gtk::Box,
    courses: RefCell<Vec<st::Course>>,
    selected: RefCell<Option<st::Course>>,
}

/// A local menu callback.
type MenuAction = Rc<dyn Fn()>;

/// Build the window, wire it up, and present it.
pub fn build(app: &adw::Application) -> adw::ApplicationWindow {
    widgets::install_css();

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Yunee")
        .default_width(980)
        .default_height(720)
        .build();

    let store = match Store::open(&paths::db_path()) {
        Ok(store) => store,
        Err(e) => return fatal_window(app, &e.to_string()),
    };
    let state = Arc::new(AppState::new(store));

    // --- sidebar ---
    let sidebar_title = adw::WindowTitle::new("Yunee", "Canvas");
    let sync_button = gtk::Button::from_icon_name("emblem-synchronizing-symbolic");
    sync_button.set_tooltip_text(Some("Sync now"));
    sync_button.add_css_class("flat");

    let menu = gio::Menu::new();
    menu.append(Some("Dashboard"), Some("app.dashboard"));
    menu.append(Some("Sync now"), Some("app.sync"));
    menu.append(Some("Connect to Canvas…"), Some("app.connect"));
    menu.append(Some("Settings"), Some("app.settings"));
    menu.append(Some("About Yunee"), Some("app.about"));
    let menu_button = gtk::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .menu_model(&menu)
        .tooltip_text("Menu")
        .build();
    menu_button.add_css_class("flat");

    let sidebar_header = adw::HeaderBar::new();
    sidebar_header.set_title_widget(Some(&sidebar_title));
    sidebar_header.pack_end(&sync_button);
    sidebar_header.pack_end(&menu_button);

    let sidebar_list = gtk::ListBox::new();
    sidebar_list.set_selection_mode(gtk::SelectionMode::Single);
    sidebar_list.set_show_separators(true);
    let sidebar_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&sidebar_list)
        .build();
    let sidebar_toolbar = adw::ToolbarView::new();
    sidebar_toolbar.add_top_bar(&sidebar_header);
    sidebar_toolbar.set_content(Some(&sidebar_scroll));
    let sidebar_page = adw::NavigationPage::new(&sidebar_toolbar, "Yunee");

    // --- content ---
    let content_title = adw::WindowTitle::new("Dashboard", "");
    let content_header = adw::HeaderBar::new();
    content_header.set_title_widget(Some(&content_title));

    let dashboard = gtk::Box::new(gtk::Orientation::Vertical, 12);
    let course = gtk::Box::new(gtk::Orientation::Vertical, 12);
    let files = gtk::Box::new(gtk::Orientation::Vertical, 12);
    let settings = gtk::Box::new(gtk::Orientation::Vertical, 12);
    for page in [&dashboard, &course, &files, &settings] {
        page.set_margin_top(18);
        page.set_margin_bottom(24);
        page.set_margin_start(18);
        page.set_margin_end(18);
    }

    let stack = gtk::Stack::new();
    stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    stack.add_titled(&scroll(&dashboard), Some("dashboard"), "Dashboard");
    stack.add_titled(&scroll(&course), Some("course"), "Course");
    stack.add_titled(&scroll(&files), Some("files"), "Files");
    stack.add_titled(&scroll(&settings), Some("settings"), "Settings");

    let content_toolbar = adw::ToolbarView::new();
    content_toolbar.add_top_bar(&content_header);
    content_toolbar.set_content(Some(&stack));
    let content_page = adw::NavigationPage::new(&content_toolbar, "Content");

    let split = adw::NavigationSplitView::new();
    split.set_sidebar(Some(&sidebar_page));
    split.set_content(Some(&content_page));
    split.set_min_sidebar_width(240.0);

    let toasts = adw::ToastOverlay::new();
    toasts.set_child(Some(&split));
    window.set_content(Some(&toasts));

    let ui = Rc::new(Ui {
        state,
        window: window.clone(),
        toasts,
        split,
        stack,
        sidebar: sidebar_list.clone(),
        content_title,
        dashboard,
        course,
        files,
        settings,
        courses: RefCell::new(Vec::new()),
        selected: RefCell::new(None),
    });

    ui.reload_sidebar();
    ui.reload_dashboard();
    ui.reload_settings();
    ui.stack.set_visible_child_name("dashboard");

    // Selection in the sidebar.
    {
        let ui = ui.clone();
        sidebar_list.connect_row_activated(move |_, row| {
            let index = row.index();
            if index <= 0 {
                ui.show_page("dashboard", "Dashboard", "");
            } else if let Some(course) = ui.courses.borrow().get((index - 1) as usize).cloned() {
                ui.open_course(course);
            }
        });
    }

    // Header buttons.
    {
        let ui = ui.clone();
        sync_button.connect_clicked(move |_| ui.sync_now());
    }

    // Actions for the menu.
    let actions: [(&str, MenuAction); 5] = [
        ("dashboard", {
            let ui = ui.clone();
            Rc::new(move || ui.show_page("dashboard", "Dashboard", ""))
        }),
        ("sync", {
            let ui = ui.clone();
            Rc::new(move || ui.sync_now())
        }),
        ("connect", {
            let ui = ui.clone();
            Rc::new(move || ui.open_connect())
        }),
        ("settings", {
            let ui = ui.clone();
            Rc::new(move || {
                ui.reload_settings();
                ui.show_page("settings", "Settings", "");
            })
        }),
        ("about", {
            let ui = ui.clone();
            Rc::new(move || ui.show_about())
        }),
    ];
    for (name, handler) in actions {
        let action = gio::SimpleAction::new(name, None);
        action.connect_activate(move |_, _| (handler)());
        app.add_action(&action);
    }

    // Startup sync when a connection already exists.
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

impl Ui {
    fn toast(&self, message: &str) {
        self.toasts.add_toast(adw::Toast::new(message));
    }

    fn show_page(&self, name: &str, title: &str, subtitle: &str) {
        self.stack.set_visible_child_name(name);
        self.content_title.set_title(title);
        self.content_title.set_subtitle(subtitle);
        self.split.set_show_content(true);
    }

    // ------------------------------------------------------------------
    // Sidebar
    // ------------------------------------------------------------------

    fn reload_sidebar(self: &Rc<Self>) {
        widgets::clear_list(&self.sidebar);

        let dashboard = adw::ActionRow::builder()
            .title("Dashboard")
            .activatable(true)
            .build();
        dashboard.add_prefix(&gtk::Image::from_icon_name("view-grid-symbolic"));
        self.sidebar.append(&dashboard);

        let courses = self.state.store.list_courses().unwrap_or_default();
        for course in &courses {
            let row = adw::ActionRow::builder()
                .title(&course.name)
                .subtitle(&course.title)
                .activatable(true)
                .build();
            row.add_prefix(&gtk::Image::from_icon_name("x-office-calendar-symbolic"));
            self.sidebar.append(&row);
        }
        *self.courses.borrow_mut() = courses;
    }

    // ------------------------------------------------------------------
    // Dashboard
    // ------------------------------------------------------------------

    fn reload_dashboard(self: &Rc<Self>) {
        widgets::clear_box(&self.dashboard);

        let title = gtk::Label::new(Some("Dashboard"));
        title.set_xalign(0.0);
        title.add_css_class("page-title");
        self.dashboard.append(&title);

        let courses: HashMap<LocalId, st::Course> = self
            .state
            .store
            .list_courses()
            .unwrap_or_default()
            .into_iter()
            .map(|c| (c.id, c))
            .collect();

        let assignments = self.state.store.list_all_assignments().unwrap_or_default();
        let upcoming: Vec<st::Assignment> = assignments
            .into_iter()
            .filter(|a| !a.is_submitted())
            .filter(|a| match &a.due_at {
                None => false,
                Some(due) => format::days_until(due).map(|d| d <= 21).unwrap_or(false),
            })
            .collect();

        if upcoming.is_empty() {
            let empty = widgets::empty_state(
                "emblem-ok-symbolic",
                "Nothing due",
                "No open assignments in the next three weeks.",
            );
            empty.set_vexpand(false);
            self.dashboard.append(&empty);
        } else {
            let group = widgets::group("Due soon");
            for a in upcoming.iter().take(30) {
                let row = widgets::assignment_row(a);
                row.set_activatable(true);
                if let Some(course) = courses.get(&a.course_id).cloned() {
                    let ui = self.clone();
                    row.connect_activated(move |_| ui.open_course(course.clone()));
                }
                group.add(&row);
            }
            self.dashboard.append(&group);
        }

        // Unread announcements across courses.
        let unread: Vec<st::Announcement> = self
            .state
            .store
            .list_all_announcements(20)
            .unwrap_or_default()
            .into_iter()
            .filter(|a| a.is_unread())
            .collect();
        if !unread.is_empty() {
            let group = widgets::group("New announcements");
            for announcement in unread.iter().take(10) {
                let course_name = courses
                    .get(&announcement.course_id)
                    .map(|c| c.name.clone())
                    .unwrap_or_default();
                let row = widgets::announcement_row(announcement, &course_name);
                group.add(&row);
            }
            self.dashboard.append(&group);
        }
    }

    // ------------------------------------------------------------------
    // Course
    // ------------------------------------------------------------------

    fn open_course(self: &Rc<Self>, course: st::Course) {
        *self.selected.borrow_mut() = Some(course.clone());
        self.reload_course();
        self.show_page("course", &course.name, "Course");
    }

    fn reload_course(self: &Rc<Self>) {
        widgets::clear_box(&self.course);
        let Some(course) = self.selected.borrow().clone() else {
            return;
        };

        let title = gtk::Label::new(Some(&course.title));
        title.set_xalign(0.0);
        title.set_wrap(true);
        title.add_css_class("page-title");
        self.course.append(&title);

        let mut bits: Vec<String> = Vec::new();
        if let Some(prof) = &course.professor {
            bits.push(prof.clone());
        }
        if let Some(term) = &course.term {
            bits.push(term.clone());
        }
        if let Some(grade) = &course.current_grade {
            let score = course
                .current_score
                .map(|s| format!(" ({:.1}%)", s))
                .unwrap_or_default();
            bits.push(format!("Grade {grade}{score}"));
        }
        if !bits.is_empty() {
            let subtitle = gtk::Label::new(Some(&bits.join(" · ")));
            subtitle.set_xalign(0.0);
            subtitle.add_css_class("dim");
            self.course.append(&subtitle);
        }

        let files_button = gtk::Button::with_label("Course files");
        files_button.add_css_class("flat");
        {
            let ui = self.clone();
            files_button.connect_clicked(move |_| {
                ui.reload_files();
                ui.show_page("files", "Files", "");
            });
        }
        self.course.append(&files_button);

        // Assignments.
        let assignments = self
            .state
            .store
            .list_assignments(course.id)
            .unwrap_or_default();
        let group = widgets::group("Assignments");
        if assignments.is_empty() {
            let label = gtk::Label::new(Some("No assignments synced yet."));
            label.set_xalign(0.0);
            label.add_css_class("dim");
            group.add(&label);
        } else {
            for a in &assignments {
                group.add(&widgets::assignment_row(a));
            }
        }
        self.course.append(&group);

        // Announcements.
        let announcements = self
            .state
            .store
            .list_announcements(course.id)
            .unwrap_or_default();
        let group = widgets::group("Announcements");
        if announcements.is_empty() {
            let label = gtk::Label::new(Some("No announcements."));
            label.set_xalign(0.0);
            label.add_css_class("dim");
            group.add(&label);
        } else {
            for announcement in &announcements {
                let row = widgets::announcement_row(announcement, &course.name);
                if announcement.is_unread() {
                    let button = gtk::Button::with_label("Mark read");
                    button.add_css_class("flat");
                    button.set_valign(gtk::Align::Center);
                    let ui = self.clone();
                    let canvas_id = announcement.canvas_id.clone();
                    let course_canvas = course.canvas_id.clone();
                    button.connect_clicked(move |_| {
                        ui.mark_announcement_read(&course_canvas, &canvas_id)
                    });
                    row.add_suffix(&button);
                }
                group.add(&row);
            }
        }
        self.course.append(&group);
    }

    // ------------------------------------------------------------------
    // Files
    // ------------------------------------------------------------------

    fn reload_files(self: &Rc<Self>) {
        widgets::clear_box(&self.files);
        let Some(course) = self.selected.borrow().clone() else {
            self.files.append(&widgets::empty_state(
                "folder-symbolic",
                "No course selected",
                "Pick a course to browse its files.",
            ));
            return;
        };

        let title = gtk::Label::new(Some(&format!("Files — {}", course.name)));
        title.set_xalign(0.0);
        title.add_css_class("page-title");
        self.files.append(&title);

        let files = self.state.store.list_files(course.id).unwrap_or_default();
        if files.is_empty() {
            self.files.append(&widgets::empty_state(
                "folder-open-symbolic",
                "No files",
                "Canvas returned no files for this course.",
            ));
            return;
        }

        let group = widgets::group("Course files");
        for file in &files {
            let (row, button) = widgets::file_row(file);
            if file.local_path.is_some() {
                row.add_suffix(&widgets::pill("saved", "ok"));
            }
            let ui = self.clone();
            let file = file.clone();
            button.connect_clicked(move |_| ui.download_file(file.clone()));
            group.add(&row);
        }
        self.files.append(&group);
    }

    // ------------------------------------------------------------------
    // Settings
    // ------------------------------------------------------------------

    fn reload_settings(self: &Rc<Self>) {
        widgets::clear_box(&self.settings);

        let title = gtk::Label::new(Some("Settings"));
        title.set_xalign(0.0);
        title.add_css_class("page-title");
        self.settings.append(&title);

        let connection = self.state.connection();

        let group = widgets::group("Canvas connection");
        match &connection {
            Some(conn) => {
                let server = adw::ActionRow::builder()
                    .title("Server")
                    .subtitle(&conn.base_url)
                    .build();
                group.add(&server);
                let token = adw::ActionRow::builder()
                    .title("Access token")
                    .subtitle("Stored on this machine (GNOME keyring when available)")
                    .build();
                group.add(&token);

                let (disconnect_row, disconnect_button) = row_with_button(
                    "Disconnect",
                    "Forget the token and stop syncing",
                    "Disconnect",
                    "destructive-action",
                );
                {
                    let ui = self.clone();
                    disconnect_button.connect_clicked(move |_| {
                        ui.state.clear_connection();
                        ui.toast("Disconnected from Canvas.");
                        ui.reload_settings();
                        ui.reload_sidebar();
                        ui.reload_dashboard();
                        ui.show_page("dashboard", "Dashboard", "");
                    });
                }
                group.add(&disconnect_row);

                let (reconnect_row, reconnect_button) = row_with_button(
                    "Reconnect",
                    "Use a new address or token",
                    "Reconnect",
                    "suggested-action",
                );
                {
                    let ui = self.clone();
                    let base = conn.base_url.clone();
                    reconnect_button
                        .connect_clicked(move |_| ui.open_connect_with(Some(base.clone())));
                }
                group.add(&reconnect_row);
            }
            None => {
                let (row, button) = row_with_button(
                    "Not connected",
                    "Add your Canvas address and an access token",
                    "Connect…",
                    "suggested-action",
                );
                {
                    let ui = self.clone();
                    button.connect_clicked(move |_| ui.open_connect());
                }
                group.add(&row);
            }
        }
        self.settings.append(&group);

        let sync_group = widgets::group("Sync");
        let last = self
            .state
            .store
            .last_sync("last")
            .ok()
            .flatten()
            .map(|iso| format::due_label(&iso))
            .unwrap_or_else(|| "Never".into());
        sync_group.add(
            &adw::ActionRow::builder()
                .title("Last synced")
                .subtitle(&last)
                .build(),
        );
        let (sync_row, sync_btn) =
            row_with_button("Sync now", "Pull the latest from Canvas", "Sync", "flat");
        {
            let ui = self.clone();
            sync_btn.connect_clicked(move |_| ui.sync_now());
        }
        sync_group.add(&sync_row);
        self.settings.append(&sync_group);

        let about = widgets::group("About");
        about.add(
            &adw::ActionRow::builder()
                .title("Yunee")
                .subtitle(concat!("version ", env!("CARGO_PKG_VERSION")))
                .build(),
        );
        about.add(
            &adw::ActionRow::builder()
                .title("Source")
                .subtitle("github.com/LyKhoris/Yunee")
                .build(),
        );
        self.settings.append(&about);
    }

    // ------------------------------------------------------------------
    // Dialogs
    // ------------------------------------------------------------------

    fn open_connect(self: &Rc<Self>) {
        let existing = self.state.connection().map(|c| c.base_url);
        self.open_connect_with(existing);
    }

    fn open_connect_with(self: &Rc<Self>, existing: Option<String>) {
        let ui = self.clone();
        let on_connected: Rc<dyn Fn()> = Rc::new(move || {
            ui.toast("Connected to Canvas.");
            ui.reload_settings();
            ui.sync_now();
        });
        connect::present(&self.window, self.state.clone(), existing, on_connected);
    }

    fn show_about(&self) {
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

    fn sync_now(self: &Rc<Self>) {
        let Some(connection) = self.state.connection() else {
            self.toast("Connect to Canvas first.");
            self.open_connect();
            return;
        };
        self.toast("Syncing…");
        let state = self.state.clone();
        let ui = self.clone();
        // The network runs on a plain thread bound to the Tokio runtime; the
        // result crosses back through an async channel and is handled on the
        // GTK main loop, so no non-`Send` `Rc` ever leaves the main thread.
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
        self.reload_sidebar();
        self.reload_dashboard();
        self.reload_settings();
        if let Some(course) = self.selected.borrow().clone() {
            self.reload_course();
            if self.stack.visible_child_name().as_deref() == Some("files") {
                self.reload_files();
            }
            let _ = course;
        }

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
                "Synced {} course(s), {} assignment(s), {} announcement(s), {} file(s).",
                c.courses, c.assignments, c.announcements, c.files
            ));
        } else {
            let first = report.errors.first().cloned().unwrap_or_default();
            self.toast(&format!("Sync finished with problems: {first}"));
        }
    }

    fn mark_announcement_read(self: &Rc<Self>, course_canvas_id: &str, canvas_id: &str) {
        if self
            .state
            .store
            .mark_announcement_read(canvas_id, &chrono::Utc::now().to_rfc3339())
            .is_err()
        {
            return;
        }
        self.reload_dashboard();
        if self.selected.borrow().is_some() {
            self.reload_course();
        }

        // Best-effort: also mark it read on Canvas, if connected.
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

    fn download_file(self: &Rc<Self>, file: st::FileEntry) {
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
            .or_else(|| Some(file.display_name.clone()))
            .unwrap_or_else(|| "file".into());
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
                    ui.reload_files();
                    ui.toast(&format!("Saved to {}", path.display()));
                }
                Ok(Err(e)) => ui.toast(&format!("Download failed: {e}")),
                Err(_) => {}
            }
        });
    }
}

fn row_with_button(
    title: &str,
    subtitle: &str,
    button_label: &str,
    class: &str,
) -> (adw::ActionRow, gtk::Button) {
    let row = adw::ActionRow::builder()
        .title(title)
        .subtitle(subtitle)
        .build();
    let button = gtk::Button::with_label(button_label);
    button.set_valign(gtk::Align::Center);
    if !class.is_empty() {
        button.add_css_class(class);
    }
    row.add_suffix(&button);
    (row, button)
}

fn sanitize(name: &str) -> String {
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

/// A window shown when the database itself cannot open.
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

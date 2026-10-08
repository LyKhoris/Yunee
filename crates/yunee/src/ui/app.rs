//! The application shell, laid out like Canvas: a dark global navigation rail
//! on the left and a content stack that holds the dashboard, the course pages,
//! and settings.

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
    pub(crate) content: gtk::Stack,
    pub(crate) content_title: adw::WindowTitle,
    pub(crate) selected: RefCell<Option<st::Course>>,
    /// Courses whose files we have already tried to load on demand.
    pub(crate) files_attempted: RefCell<HashSet<st::LocalId>>,
    pub(crate) dashboard_page: gtk::Box,
    pub(crate) courses_page: gtk::Box,
    pub(crate) course_page: gtk::Box,
    pub(crate) settings_page: gtk::Box,
}

/// Build the window, wire it up, and present it.
pub fn build(app: &adw::Application) -> adw::ApplicationWindow {
    widgets::install_css();

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Yunee")
        .default_width(1180)
        .default_height(780)
        .build();

    let store = match Store::open(&paths::db_path()) {
        Ok(store) => store,
        Err(e) => return fatal_window(app, &e.to_string()),
    };
    let state = Arc::new(AppState::new(store));

    // --- global rail (Canvas's left navigation) ---
    let rail = gtk::ListBox::new();
    rail.set_selection_mode(gtk::SelectionMode::Single);
    rail.add_css_class("navigation-sidebar");
    rail.append(&rail_mark());
    for (icon, label) in [
        ("view-grid-symbolic", "Dashboard"),
        ("x-office-calendar-symbolic", "Courses"),
        ("emblem-system-symbolic", "Settings"),
        ("help-about-symbolic", "About"),
    ] {
        rail.append(&rail_row(icon, label));
    }
    let rail_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    rail_box.set_width_request(88);
    rail_box.append(&rail);
    let rail_page = adw::NavigationPage::new(&rail_box, "Yunee");

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

    let dashboard_page = gtk::Box::new(gtk::Orientation::Vertical, 14);
    let courses_page = gtk::Box::new(gtk::Orientation::Vertical, 14);
    let course_page = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let settings_page = gtk::Box::new(gtk::Orientation::Vertical, 14);
    for page in [&dashboard_page, &courses_page, &settings_page] {
        page.set_margin_top(18);
        page.set_margin_bottom(24);
        page.set_margin_start(20);
        page.set_margin_end(20);
    }

    let content = gtk::Stack::new();
    content.set_transition_type(gtk::StackTransitionType::Crossfade);
    content.add_titled(&scroll(&dashboard_page), Some("dashboard"), "Dashboard");
    content.add_titled(&scroll(&courses_page), Some("courses"), "Courses");
    content.add_titled(&course_page, Some("course"), "Course");
    content.add_titled(&scroll(&settings_page), Some("settings"), "Settings");

    let content_toolbar = adw::ToolbarView::new();
    content_toolbar.add_top_bar(&content_header);
    content_toolbar.set_content(Some(&content));
    let content_nav = adw::NavigationPage::new(&content_toolbar, "Content");

    let split = adw::NavigationSplitView::new();
    split.set_sidebar(Some(&rail_page));
    split.set_content(Some(&content_nav));
    split.set_min_sidebar_width(88.0);
    split.set_max_sidebar_width(88.0);

    let toasts = adw::ToastOverlay::new();
    toasts.set_child(Some(&split));
    window.set_content(Some(&toasts));

    let ui = Rc::new(Ui {
        state,
        window: window.clone(),
        toasts,
        split,
        content,
        content_title,
        selected: RefCell::new(None),
        files_attempted: RefCell::new(HashSet::new()),
        dashboard_page,
        courses_page,
        course_page,
        settings_page,
    });

    // Rail navigation.
    {
        let ui = ui.clone();
        rail.connect_row_selected(move |_, row| {
            let Some(row) = row else { return };
            match row.index() {
                1 => ui.show_page("dashboard", "Dashboard", ""),
                2 => {
                    ui.reload_courses();
                    ui.show_page("courses", "Courses", "");
                }
                3 => {
                    ui.reload_settings();
                    ui.show_page("settings", "Settings", "");
                }
                4 => ui.show_about(),
                _ => {}
            }
        });
    }

    // Header refresh.
    {
        let ui = ui.clone();
        refresh.connect_clicked(move |_| ui.sync_now());
    }

    // Menu actions.
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
                ui.show_page("settings", "Settings", "");
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

    ui.reload_dashboard();
    ui.reload_settings();
    ui.content.set_visible_child_name("dashboard");

    if ui.state.connection().is_some() {
        ui.sync_now();
    }

    window.present();
    window
}

fn rail_mark() -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_selectable(false);
    row.set_activatable(false);
    let label = gtk::Label::new(Some("Yunee"));
    label.add_css_class("rail-title");
    label.set_margin_top(16);
    label.set_margin_bottom(10);
    row.set_child(Some(&label));
    row
}

fn rail_row(icon: &str, text: &str) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    let column = gtk::Box::new(gtk::Orientation::Vertical, 2);
    column.set_margin_top(9);
    column.set_margin_bottom(9);
    column.set_halign(gtk::Align::Center);
    let image = gtk::Image::from_icon_name(icon);
    image.set_pixel_size(20);
    let label = gtk::Label::new(Some(text));
    label.add_css_class("rail-label");
    column.append(&image);
    column.append(&label);
    row.set_child(Some(&column));
    row
}

fn scroll(child: &impl IsA<gtk::Widget>) -> gtk::ScrolledWindow {
    gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(child)
        .build()
}

impl Ui {
    pub(crate) fn toast(&self, message: &str) {
        self.toasts.add_toast(adw::Toast::new(message));
    }

    pub(crate) fn show_page(&self, name: &str, title: &str, subtitle: &str) {
        self.content.set_visible_child_name(name);
        self.content_title.set_title(title);
        self.content_title.set_subtitle(subtitle);
        self.split.set_show_content(true);
    }

    /// Reload every synced view from the local store.
    pub(crate) fn reload_all(self: &Rc<Self>) {
        self.reload_dashboard();
        if self.content.visible_child_name().as_deref() == Some("courses") {
            self.reload_courses();
        }
        if let Some(course) = self.selected.borrow().clone() {
            course::open(self, course);
        }
        self.reload_settings();
    }

    pub(crate) fn reload_dashboard(self: &Rc<Self>) {
        dashboard::render(self);
    }

    pub(crate) fn reload_courses(self: &Rc<Self>) {
        dashboard::render_courses(self);
    }

    pub(crate) fn reload_settings(self: &Rc<Self>) {
        settings::render(self);
    }

    pub(crate) fn open_course(self: &Rc<Self>, course: st::Course) {
        course::open(self, course);
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
                    ui.reload_all();
                    ui.toast(&format!("Saved to {}", path.display()));
                }
                Ok(Err(e)) => ui.toast(&format!("Download failed: {e}")),
                Err(_) => {}
            }
        });
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

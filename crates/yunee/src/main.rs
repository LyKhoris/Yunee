//! Yunee — a local, single-user Canvas client for GNOME.

mod format;
mod html;
mod paths;
mod runtime;
mod secrets;
mod state;
mod sync;
mod ui;

use adw::prelude::*;
use gtk4 as gtk;

fn main() -> gtk::glib::ExitCode {
    // Headless sync: `yunee --sync-once`. Pulls Canvas into the local store and
    // exits without opening a window — useful for testing and for a scheduled
    // run. `CANVAS_URL` + `CANVAS_TOKEN` override the saved connection.
    if std::env::args().any(|arg| arg == "--sync-once") {
        return sync_once();
    }

    let app = adw::Application::builder()
        // A development build can run beside an installed Flatpak, which would
        // otherwise claim the id and make this instance exit immediately.
        .application_id(std::env::var("YUNEE_APP_ID").unwrap_or_else(|_| ui::APP_ID.to_string()))
        .build();
    app.connect_activate(|app| {
        ui::build(app);
    });
    app.run()
}

fn sync_once() -> gtk::glib::ExitCode {
    use gtk::glib::ExitCode;
    use state::AppState;

    let store = match yunee_store::Store::open(&paths::db_path()) {
        Ok(store) => store,
        Err(e) => {
            eprintln!("yunee: could not open the database: {e}");
            return ExitCode::FAILURE;
        }
    };
    let state = AppState::new(store);

    // `CANVAS_URL` + `CANVAS_TOKEN` override the saved connection (handled in
    // `AppState::connection`).
    let Some(connection) = state.connection() else {
        eprintln!("yunee: no connection — set CANVAS_URL and CANVAS_TOKEN, or connect in the app");
        return ExitCode::FAILURE;
    };

    let report = runtime::runtime().block_on(sync::sync_all(&state.store, &connection));
    let c = &report.counts;
    println!(
        "synced: courses={} assignments={} announcements={} modules={} files={} pages={}",
        c.courses, c.assignments, c.announcements, c.modules, c.files, c.pages
    );
    for error in &report.errors {
        eprintln!("warning: {error}");
    }
    if report.ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

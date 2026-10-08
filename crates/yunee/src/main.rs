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
    let app = adw::Application::builder()
        .application_id(ui::APP_ID)
        .build();
    app.connect_activate(|app| {
        ui::build(app);
    });
    app.run()
}

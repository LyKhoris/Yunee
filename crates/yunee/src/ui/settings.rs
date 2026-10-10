//! Settings: the Canvas connection, sync, and about.

use std::rc::Rc;

use adw::prelude::*;
use gtk4 as gtk;

use crate::format;
use crate::ui::app::{PROJECT_URL, Ui};
use crate::ui::widgets;

/// Render the settings page into the shared container.
pub fn render(ui: &Rc<Ui>) {
    widgets::clear_box(&ui.settings_page);
    ui.settings_page.append(&widgets::page_title("Settings"));

    // --- Canvas connection ---
    let connection_group = adw::PreferencesGroup::new();
    connection_group.set_title("Canvas connection");
    match ui.state.connection() {
        Some(conn) => {
            connection_group.add(
                &adw::ActionRow::builder()
                    .title("Server")
                    .subtitle(&conn.base_url)
                    .build(),
            );
            connection_group.add(
                &adw::ActionRow::builder()
                    .title("Access token")
                    .subtitle("Stored on this machine (GNOME keyring when available)")
                    .build(),
            );

            let (row, button) = row_button(
                "Disconnect",
                "Forget the token and stop syncing",
                "Disconnect",
                "destructive-action",
            );
            {
                let ui = ui.clone();
                button.connect_clicked(move |_| {
                    ui.state.clear_connection();
                    ui.toast("Disconnected from Canvas.");
                    ui.reload_all();
                    ui.show_dashboard();
                });
            }
            connection_group.add(&row);

            let (row, button) = row_button(
                "Reconnect",
                "Use a new address or token",
                "Reconnect",
                "suggested-action",
            );
            {
                let ui = ui.clone();
                let base = conn.base_url.clone();
                button.connect_clicked(move |_| ui.open_connect_with(Some(base.clone())));
            }
            connection_group.add(&row);
        }
        None => {
            let (row, button) = row_button(
                "Not connected",
                "Choose your school and add an access token",
                "Connect",
                "suggested-action",
            );
            {
                let ui = ui.clone();
                button.connect_clicked(move |_| ui.open_connect());
            }
            connection_group.add(&row);
        }
    }
    ui.settings_page.append(&connection_group);

    // --- Sync ---
    let sync_group = adw::PreferencesGroup::new();
    sync_group.set_title("Sync");
    let last = ui
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
    let (row, button) = row_button("Sync now", "Pull the latest from Canvas", "Sync", "flat");
    {
        let ui = ui.clone();
        button.connect_clicked(move |_| ui.sync_now());
    }
    sync_group.add(&row);
    ui.settings_page.append(&sync_group);

    // --- Updates ---
    let updates = adw::PreferencesGroup::new();
    updates.set_title("Updates");
    updates.add(
        &adw::ActionRow::builder()
            .title("Version")
            .subtitle(concat!("Yunee ", env!("CARGO_PKG_VERSION")))
            .build(),
    );
    let (project_row, project_button) = row_button(
        "Project page",
        "Opens Yunee's GitHub page in your browser",
        "Open GitHub",
        "flat",
    );
    {
        let ui = ui.clone();
        project_button.connect_clicked(move |_| ui.open_url(PROJECT_URL));
    }
    updates.add(&project_row);
    ui.settings_page.append(&updates);

    // --- About ---
    let about = adw::PreferencesGroup::new();
    about.set_title("About");
    about.add(
        &adw::ActionRow::builder()
            .title("Yunee")
            .subtitle("A local, single-user Canvas client for GNOME")
            .build(),
    );
    about.add(
        &adw::ActionRow::builder()
            .title("Source")
            .subtitle("github.com/LyKhoris/Yunee")
            .build(),
    );
    ui.settings_page.append(&about);
}

fn row_button(
    title: &str,
    subtitle: &str,
    label: &str,
    class: &str,
) -> (adw::ActionRow, gtk::Button) {
    let row = adw::ActionRow::builder()
        .title(title)
        .subtitle(subtitle)
        .build();
    let button = gtk::Button::with_label(label);
    button.set_valign(gtk::Align::Center);
    if !class.is_empty() {
        button.add_css_class(class);
    }
    row.add_suffix(&button);
    (row, button)
}

//! The "Connect to Canvas" dialog.
//!
//! Canvas auth for a single-user tool is a personal access token, not OAuth.
//! The token is verified against `/users/self` *before* it is saved, so a typo
//! fails here rather than silently at the next sync.
//!
//! Threading: widgets are not `Send`, so the network runs on a worker thread
//! that only sees owned strings; the result comes back over an async channel
//! and is applied on the GTK main loop.

use std::rc::Rc;
use std::sync::Arc;

use adw::prelude::*;
use gtk4 as gtk;

use yunee_canvas::CanvasClient;

use crate::runtime::runtime;
use crate::state::AppState;

/// Show the dialog. `on_connected` fires after a successful save.
pub fn present(
    window: &adw::ApplicationWindow,
    state: Arc<AppState>,
    existing_base: Option<String>,
    on_connected: Rc<dyn Fn()>,
) {
    let dialog = adw::Dialog::builder()
        .title("Connect to Canvas")
        .content_width(460)
        .build();

    let header = adw::HeaderBar::new();
    header.set_show_end_title_buttons(false);
    let close = gtk::Button::from_icon_name("window-close-symbolic");
    close.add_css_class("flat");
    header.pack_start(&close);

    let connect = gtk::Button::with_label("Connect");
    connect.add_css_class("suggested-action");
    header.pack_end(&connect);

    let address = adw::EntryRow::new();
    address.set_title("Canvas address");
    address.set_text(existing_base.as_deref().unwrap_or(""));
    address.set_input_purpose(gtk::InputPurpose::Url);

    let token = adw::PasswordEntryRow::new();
    token.set_title("Access token");

    let fields = adw::PreferencesGroup::new();
    fields.set_title("Your Canvas");
    fields.add(&address);
    fields.add(&token);

    let hint = gtk::Label::new(Some(
        "In Canvas: Account → Settings → New Access Token. Yunee never sends this \
         token anywhere except your own Canvas server, and keeps it on this machine.",
    ));
    hint.set_xalign(0.0);
    hint.set_wrap(true);
    hint.add_css_class("dim");
    hint.set_margin_top(12);

    let error = gtk::Label::new(None);
    error.set_xalign(0.0);
    error.set_wrap(true);
    error.add_css_class("error");
    error.set_visible(false);
    error.set_margin_top(12);

    let spinner = gtk::Spinner::new();
    spinner.set_visible(false);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.set_margin_top(12);
    content.set_margin_bottom(18);
    content.set_margin_start(18);
    content.set_margin_end(18);
    content.append(&fields);
    content.append(&hint);
    content.append(&error);
    content.append(&spinner);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&content));
    dialog.set_child(Some(&toolbar));

    {
        let dialog = dialog.clone();
        close.connect_clicked(move |_| {
            dialog.close();
        });
    }

    connect.connect_clicked({
        let button = connect.clone();
        let dialog = dialog.clone();
        let state = state.clone();
        let on_connected = on_connected.clone();
        let error = error.clone();
        let spinner = spinner.clone();
        move |_| {
            let raw_base = address.text().to_string();
            let raw_token = token.text().to_string();
            error.set_visible(false);

            if raw_base.trim().is_empty() || raw_token.trim().is_empty() {
                error.set_text("Enter both the Canvas address and an access token.");
                error.set_visible(true);
                return;
            }

            let base = match yunee_canvas::normalize_base(&raw_base) {
                Ok(base) => base.as_str().trim_end_matches('/').to_string(),
                Err(e) => {
                    error.set_text(&format!("That address doesn't look right: {e}"));
                    error.set_visible(true);
                    return;
                }
            };

            button.set_sensitive(false);
            spinner.set_visible(true);
            spinner.start();

            // Owned strings cross to the worker; widgets stay on this thread.
            let (tx, rx) = async_channel::bounded(1);
            {
                let base = base.clone();
                let token = raw_token.clone();
                std::thread::spawn(move || {
                    let result = runtime().block_on(async {
                        let client = CanvasClient::new(&base, &token)?;
                        client.verify_token().await
                    });
                    let _ = tx.send_blocking(result);
                });
            }

            let dialog = dialog.clone();
            let state = state.clone();
            let on_connected = on_connected.clone();
            let error = error.clone();
            let spinner = spinner.clone();
            let button = button.clone();
            gtk::glib::MainContext::default().spawn_local(async move {
                let result = rx.recv().await;
                spinner.stop();
                spinner.set_visible(false);
                button.set_sensitive(true);

                match result {
                    Ok(Ok(_user)) => {
                        if let Err(e) = state.save_connection(&base, &raw_token) {
                            eprintln!("yunee: could not save the token: {e}");
                        }
                        dialog.close();
                        (on_connected)();
                    }
                    Ok(Err(e)) => {
                        error.set_text(&format!("Could not connect: {e}"));
                        error.set_visible(true);
                    }
                    Err(_) => {}
                }
            });
        }
    });

    dialog.present(Some(window));
}

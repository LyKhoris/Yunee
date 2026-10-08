//! The "Connect to Canvas" dialog.
//!
//! The easy path: start typing your school's name; matches appear as you type
//! (Canvas's own account lookup resolves the host); pick one and paste an
//! access token. Typing a full `…instructure.com` address still works for
//! self-hosted installs.
//!
//! Canvas auth is a personal access token, not OAuth. The token is verified
//! against `/users/self` *before* it is saved, so a typo fails here rather than
//! silently at the next sync.
//!
//! Threading: widgets are not `Send`, so only owned strings cross to the
//! worker; results come back over an async channel and are applied on the GTK
//! main loop.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use adw::prelude::*;
use gtk4 as gtk;
use gtk4::glib;

use yunee_canvas::CanvasClient;

use crate::runtime::runtime;
use crate::state::AppState;

/// How long to wait after a keystroke before searching.
const DEBOUNCE_MS: u64 = 300;

/// Show the dialog. `on_connected` fires after a successful save.
pub fn present(
    window: &adw::ApplicationWindow,
    state: Arc<AppState>,
    existing_base: Option<String>,
    on_connected: Rc<dyn Fn()>,
) {
    let dialog = adw::Dialog::builder()
        .title("Connect to Canvas")
        .content_width(500)
        .build();

    let header = adw::HeaderBar::new();
    header.set_show_end_title_buttons(false);
    let close = gtk::Button::from_icon_name("window-close-symbolic");
    close.add_css_class("flat");
    header.pack_start(&close);

    let connect = gtk::Button::with_label("Connect");
    connect.add_css_class("suggested-action");
    header.pack_end(&connect);

    // --- school ---
    let school = adw::EntryRow::new();
    school.set_title("School");
    school.set_tooltip_text(Some(
        "Start typing a school name, or paste its Canvas address",
    ));

    let school_group = adw::PreferencesGroup::new();
    school_group.set_title("Your school");
    school_group.add(&school);

    let matches = gtk::ListBox::new();
    matches.add_css_class("boxed-list");
    matches.set_selection_mode(gtk::SelectionMode::None);
    matches.set_visible(false);

    // --- token ---
    let token = adw::PasswordEntryRow::new();
    token.set_title("Access token");
    let token_group = adw::PreferencesGroup::new();
    token_group.set_title("Access");
    token_group.add(&token);

    let hint = gtk::Label::new(Some(
        "In Canvas: Account → Settings → New Access Token. Yunee keeps the token on \
         this machine and only ever sends it to your own Canvas server.",
    ));
    hint.set_xalign(0.0);
    hint.set_wrap(true);
    hint.add_css_class("muted");

    let error = gtk::Label::new(None);
    error.set_xalign(0.0);
    error.set_wrap(true);
    error.add_css_class("error");
    error.set_visible(false);

    let spinner = gtk::Spinner::new();
    spinner.set_visible(false);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.set_margin_top(12);
    content.set_margin_bottom(18);
    content.set_margin_start(18);
    content.set_margin_end(18);
    content.append(&school_group);
    content.append(&matches);
    content.append(&token_group);
    content.append(&hint);
    content.append(&error);
    content.append(&spinner);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&content));
    dialog.set_child(Some(&toolbar));

    // The resolved Canvas base URL, once a school or address is chosen.
    let selected: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
    if let Some(base) = &existing_base {
        school.set_text(base);
        *selected.borrow_mut() = Some(base.clone());
    }

    {
        let dialog = dialog.clone();
        close.connect_clicked(move |_| {
            dialog.close();
        });
    }

    // The reusable search: clear, resolve, and either set the server directly
    // (a dotted address) or ask Canvas for matching schools.
    let search: Rc<dyn Fn(String)> = {
        let school = school.clone();
        let matches = matches.clone();
        let selected = selected.clone();
        let error = error.clone();
        let spinner = spinner.clone();
        Rc::new(move |query: String| {
            let query = query.trim().to_string();
            error.set_visible(false);
            while let Some(child) = matches.first_child() {
                matches.remove(&child);
            }
            matches.set_visible(false);

            if query.is_empty() {
                return;
            }

            // A dotted string is a server address, used directly.
            if query.contains('.') {
                match yunee_canvas::normalize_base(&query) {
                    Ok(base) => {
                        let base = base.as_str().trim_end_matches('/').to_string();
                        school.set_text(&base);
                        *selected.borrow_mut() = Some(base);
                    }
                    Err(e) => {
                        error.set_text(&format!("That address doesn't look right: {e}"));
                        error.set_visible(true);
                    }
                }
                return;
            }

            spinner.start();
            spinner.set_visible(true);

            let (tx, rx) = async_channel::bounded(1);
            std::thread::spawn(move || {
                let result = runtime().block_on(yunee_canvas::search_schools(&query));
                let _ = tx.send_blocking(result);
            });

            let school = school.clone();
            let matches = matches.clone();
            let selected = selected.clone();
            let error = error.clone();
            let spinner = spinner.clone();
            glib::MainContext::default().spawn_local(async move {
                spinner.stop();
                spinner.set_visible(false);

                match rx.recv().await {
                    Ok(Ok(list)) if !list.is_empty() => {
                        for m in list {
                            let row = adw::ActionRow::builder()
                                .title(&m.name)
                                .subtitle(&m.domain)
                                .activatable(true)
                                .build();
                            row.add_prefix(&gtk::Image::from_icon_name("go-next-symbolic"));
                            let selected = selected.clone();
                            let school = school.clone();
                            let list_box = matches.clone();
                            let domain = m.domain.clone();
                            row.connect_activated(move |_| {
                                *selected.borrow_mut() = Some(domain.clone());
                                school.set_text(&domain);
                                while let Some(child) = list_box.first_child() {
                                    list_box.remove(&child);
                                }
                                list_box.set_visible(false);
                            });
                            matches.append(&row);
                        }
                        matches.set_visible(true);
                    }
                    Ok(Ok(_)) => {
                        error.set_text(
                            "No school matched. Check the spelling, or paste your Canvas \
                             address (e.g. school.instructure.com).",
                        );
                        error.set_visible(true);
                    }
                    Ok(Err(e)) => {
                        error.set_text(&format!("Could not search: {e}"));
                        error.set_visible(true);
                    }
                    Err(_) => {}
                }
            });
        })
    };

    // Search as the user types, debounced so each keystroke is not a request.
    let debounce: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
    school.connect_changed({
        let search = search.clone();
        let debounce = debounce.clone();
        move |row| {
            if let Some(id) = debounce.borrow_mut().take() {
                id.remove();
            }
            let query = row.text().trim().to_string();
            if query.is_empty() {
                search(String::new());
                return;
            }
            // A pasted address applies immediately; a name waits for a pause.
            if query.contains('.') {
                search(query);
                return;
            }
            let search = search.clone();
            let id = glib::timeout_add_local_once(Duration::from_millis(DEBOUNCE_MS), move || {
                search(query);
            });
            *debounce.borrow_mut() = Some(id);
        }
    });

    // The check button / Enter still works, for keyboard users.
    school.connect_apply({
        let search = search.clone();
        let debounce = debounce.clone();
        move |row| {
            if let Some(id) = debounce.borrow_mut().take() {
                id.remove();
            }
            search(row.text().to_string());
        }
    });

    connect.connect_clicked({
        let button = connect.clone();
        let dialog = dialog.clone();
        let state = state.clone();
        let on_connected = on_connected.clone();
        let school = school.clone();
        let token = token.clone();
        let selected = selected.clone();
        let error = error.clone();
        let spinner = spinner.clone();
        move |_| {
            error.set_visible(false);

            // Prefer an explicit selection; fall back to a typed dotted address.
            let base = selected.borrow().clone().or_else(|| {
                let text = school.text().trim().to_string();
                if text.contains('.') {
                    yunee_canvas::normalize_base(&text)
                        .ok()
                        .map(|u| u.as_str().trim_end_matches('/').to_string())
                } else {
                    None
                }
            });
            let Some(base) = base else {
                error.set_text("Pick your school first.");
                error.set_visible(true);
                return;
            };

            let raw_token = token.text().to_string();
            if raw_token.trim().is_empty() {
                error.set_text("Paste an access token.");
                error.set_visible(true);
                return;
            }

            button.set_sensitive(false);
            spinner.set_visible(true);
            spinner.start();

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
            glib::MainContext::default().spawn_local(async move {
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

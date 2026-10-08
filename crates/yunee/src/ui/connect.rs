//! The "Connect to Canvas" dialog.
//!
//! The easy path: start typing your school's name; matches appear in a floating
//! dropdown as you type (Canvas's own account lookup resolves the host); pick
//! one and paste an access token. Typing a full `…instructure.com` address
//! still works for self-hosted installs.
//!
//! Canvas auth is a personal access token, not OAuth. The token is verified
//! against `/users/self` *before* it is saved, so a typo fails here rather than
//! silently at the next sync.
//!
//! Threading: widgets are not `Send`, so only owned strings cross to the
//! worker; results come back over an async channel and are applied on the GTK
//! main loop.

use std::cell::{Cell, RefCell};
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

    // The matches live in a floating popover anchored to the row, so they never
    // push the rest of the dialog down.
    let matches = gtk::ListBox::new();
    matches.add_css_class("boxed-list");
    matches.set_selection_mode(gtk::SelectionMode::None);
    matches.set_margin_top(6);
    matches.set_margin_bottom(6);

    let popover = gtk::Popover::new();
    popover.set_parent(&school);
    popover.set_has_arrow(false);
    popover.set_autohide(true);
    popover.set_size_request(440, -1);
    popover.set_child(Some(&matches));

    // --- token ---
    let token = adw::PasswordEntryRow::new();
    token.set_title("Access token");
    let token_group = adw::PreferencesGroup::new();
    token_group.set_title("Access");
    token_group.add(&token);

    // A shortcut to the page where a student creates a token. The URL follows
    // the chosen school, so it is filled in once the school is resolved.
    let token_link = gtk::LinkButton::with_label(
        "https://canvas.instructure.com/profile/settings",
        "Get a token from Canvas settings",
    );
    token_link.set_halign(gtk::Align::Start);
    token_link.set_sensitive(false);
    token_link.set_tooltip_text(Some(
        "Opens Canvas → Account → Settings → Approved Integrations",
    ));

    // The link handles navigation; this covers the form that opens next.
    let hint = gtk::Label::new(None);
    hint.set_markup(
        "Then, on that page:\n\
         1. <b>Purpose</b> — anything, e.g. Yunee\n\
         2. <b>Expiration date</b> and <b>time</b> — up to 90 days\n\
         3. <b>Generate Token</b>, then copy it and paste it above",
    );
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
    content.append(&token_group);
    content.append(&token_link);
    content.append(&hint);
    content.append(&error);
    content.append(&spinner);

    let scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .child(&content)
        .build();
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&scroller));
    dialog.set_child(Some(&toolbar));

    // The resolved Canvas base URL, once a school or address is chosen.
    let selected: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
    // Set while we write to the entry ourselves, so the `changed` handler does
    // not treat it as typing (which would recurse).
    let suppress = Rc::new(Cell::new(false));

    if let Some(base) = &existing_base {
        suppress.set(true);
        school.set_text(base);
        suppress.set(false);
        *selected.borrow_mut() = Some(base.clone());
        token_link.set_uri(&format!("{base}/profile/settings"));
        token_link.set_sensitive(true);
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
        let popover = popover.clone();
        let selected = selected.clone();
        let suppress = suppress.clone();
        let error = error.clone();
        let spinner = spinner.clone();
        let token_link = token_link.clone();
        Rc::new(move |query: String| {
            let query = query.trim().to_string();
            error.set_visible(false);
            while let Some(child) = matches.first_child() {
                matches.remove(&child);
            }
            popover.popdown();

            if query.is_empty() {
                return;
            }

            // A dotted string is a server address, used directly.
            if query.contains('.') {
                match yunee_canvas::normalize_base(&query) {
                    Ok(base) => {
                        let base = base.as_str().trim_end_matches('/').to_string();
                        suppress.set(true);
                        school.set_text(&base);
                        suppress.set(false);
                        token_link.set_uri(&format!("{base}/profile/settings"));
                        token_link.set_sensitive(true);
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
            let popover = popover.clone();
            let selected = selected.clone();
            let suppress = suppress.clone();
            let error = error.clone();
            let spinner = spinner.clone();
            let token_link = token_link.clone();
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
                            let suppress = suppress.clone();
                            let popover = popover.clone();
                            let token_link = token_link.clone();
                            let domain = m.domain.clone();
                            row.connect_activated(move |_| {
                                suppress.set(true);
                                school.set_text(&domain);
                                suppress.set(false);
                                token_link.set_uri(&format!("https://{domain}/profile/settings"));
                                token_link.set_sensitive(true);
                                *selected.borrow_mut() = Some(domain.clone());
                                popover.popdown();
                            });
                            matches.append(&row);
                        }
                        popover.popup();
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

    // Search as the user types, debounced. A generation counter invalidates a
    // pending search when another keystroke arrives; a stale timeout simply
    // no-ops, so nothing has to be removed (removing a fired source panics).
    let generation = Rc::new(Cell::new(0u64));
    school.connect_changed({
        let search = search.clone();
        let generation = generation.clone();
        let suppress = suppress.clone();
        move |row| {
            if suppress.get() {
                return;
            }
            let query = row.text().trim().to_string();
            let current = generation.get().wrapping_add(1);
            generation.set(current);

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
            let generation = generation.clone();
            glib::timeout_add_local_once(Duration::from_millis(DEBOUNCE_MS), move || {
                if generation.get() == current {
                    search(query);
                }
            });
        }
    });

    // The check button / Enter still works, for keyboard users.
    school.connect_apply({
        let search = search.clone();
        let generation = generation.clone();
        move |row| {
            generation.set(generation.get().wrapping_add(1));
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

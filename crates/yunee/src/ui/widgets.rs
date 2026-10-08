//! Small view helpers shared by the pages: CSS, list clearing, and the row
//! builders (course, assignment, announcement, file) with their status pills.

use adw::prelude::*;
use gtk4 as gtk;
use yunee_store as st;

/// Application stylesheet. Deliberately tiny — most of the look is stock
/// libadwaita; this only adds the status pills and a few accents.
pub const CSS: &str = r#"
.pill {
    border-radius: 999px;
    padding: 1px 10px;
    font-size: 0.75rem;
    font-weight: 600;
}
.pill.ok     { background: alpha(#3584e4, 0.18); color: #1c71d8; }
.pill.warn   { background: alpha(#e5a50a, 0.22); color: #9a6a00; }
.pill.danger { background: alpha(#e01b24, 0.18); color: #c01c28; }
.pill.muted  { background: alpha(#77767b, 0.18); color: #5e5c64; }
.dim         { opacity: 0.65; }
.page-title  { font-weight: 800; font-size: 1.25rem; }
.section     { font-weight: 700; margin-top: 6px; }
"#;

/// Attach the stylesheet to the default display, once.
pub fn install_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(CSS);
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

/// Remove every child of a box.
pub fn clear_box(container: &gtk::Box) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
}

/// Remove every row of a list box.
pub fn clear_list(list: &gtk::ListBox) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
}

/// A pill label with a style class.
pub fn pill(text: &str, class: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("pill");
    label.add_css_class(class);
    label.set_valign(gtk::Align::Center);
    label
}

/// A titled preferences group for a run of rows.
pub fn group(title: &str) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::new();
    group.set_title(title);
    group
}

/// The "nothing here" panel.
pub fn empty_state(icon: &str, title: &str, description: &str) -> adw::StatusPage {
    let page = adw::StatusPage::new();
    page.set_icon_name(Some(icon));
    page.set_title(title);
    page.set_description(Some(description));
    page.set_vexpand(true);
    page
}

/// The status pill for an assignment.
pub fn assignment_pill(a: &st::Assignment) -> gtk::Label {
    if a.excused {
        return pill("excused", "muted");
    }
    if let Some(grade) = &a.grade {
        return pill(grade, "ok");
    }
    if a.is_submitted() {
        return pill("submitted", "ok");
    }
    if a.missing
        || a.due_at
            .as_deref()
            .map(crate::format::is_overdue)
            .unwrap_or(false)
    {
        return pill("missing", "danger");
    }
    pill("todo", "warn")
}

/// One assignment as an ActionRow.
pub fn assignment_row(a: &st::Assignment) -> adw::ActionRow {
    let row = adw::ActionRow::new();
    row.set_title(&gtk::glib::markup_escape_text(&a.name));

    let mut bits: Vec<String> = Vec::new();
    match &a.due_at {
        Some(due) => bits.push(crate::format::due_label(due)),
        None => bits.push("No due date".into()),
    }
    if let Some(points) = a.points_possible {
        bits.push(format!("{} pts", trim_number(points)));
    }
    row.set_subtitle(&bits.join(" · "));
    row.add_suffix(&assignment_pill(a));
    row.set_activatable(false);
    row
}

/// One announcement as an expander with the rendered body.
pub fn announcement_row(a: &st::Announcement, course_name: &str) -> adw::ExpanderRow {
    let row = adw::ExpanderRow::new();
    row.set_title(&gtk::glib::markup_escape_text(&a.title));

    let mut sub = course_name.to_string();
    if let Some(posted) = &a.posted_at {
        sub.push_str(" · ");
        sub.push_str(&crate::format::short_date(posted));
    }
    row.set_subtitle(&sub);

    if a.is_unread() {
        row.add_suffix(&pill("new", "ok"));
    }

    let body = gtk::Label::new(None);
    body.set_xalign(0.0);
    body.set_wrap(true);
    body.set_selectable(true);
    body.set_margin_top(8);
    body.set_margin_bottom(12);
    body.set_margin_start(12);
    body.set_margin_end(12);
    let markup = match &a.html {
        Some(html) if !html.trim().is_empty() => crate::html::to_pango(html),
        _ => gtk::glib::markup_escape_text(&a.body).to_string(),
    };
    body.set_markup(&markup);
    row.add_row(&body);

    row
}

/// One course file row, with a download button wired by the caller.
pub fn file_row(f: &st::FileEntry) -> (adw::ActionRow, gtk::Button) {
    let row = adw::ActionRow::new();
    row.set_title(&gtk::glib::markup_escape_text(&f.display_name));

    let mut bits: Vec<String> = Vec::new();
    if let Some(size) = f.size {
        bits.push(crate::format::bytes_label(size));
    }
    if let Some(updated) = &f.updated_at {
        let short = crate::format::short_date(updated);
        if !short.is_empty() {
            bits.push(short);
        }
    }
    row.set_subtitle(&bits.join(" · "));
    row.set_activatable(false);

    let button = gtk::Button::from_icon_name("folder-download-symbolic");
    button.set_valign(gtk::Align::Center);
    button.add_css_class("flat");
    button.set_tooltip_text(Some(if f.local_path.is_some() {
        "Downloaded — click to open the folder entry path"
    } else {
        "Download"
    }));
    row.add_suffix(&button);
    (row, button)
}

fn trim_number(value: f64) -> String {
    if (value.fract()).abs() < f64::EPSILON {
        format!("{}", value as i64)
    } else {
        format!("{value:.1}")
    }
}

//! Shared view pieces.
//!
//! Colour policy: the app follows the user's GTK/libadwaita theme. The only
//! explicit colours are the per-course accent that Canvas gives each course
//! (the professor's colour) and the semantic status pills, which use
//! libadwaita's theme-aware named colours. Nothing else is hardcoded, so light
//! and dark themes, and custom themes, all render legibly.

use adw::prelude::*;
use gtk4 as gtk;
use yunee_store as st;

/// Course accent palette — used for the card strip and the little dot.
pub const ACCENTS: [&str; 8] = [
    "#C0392B", // red
    "#8E44AD", // purple
    "#2E86C1", // blue
    "#1E8449", // green
    "#CA6F1E", // orange
    "#117A65", // teal
    "#B7950B", // olive
    "#B03A2E", // brick
];

/// The stylesheet. Deliberately small: it never overrides theme colours except
/// the course accents and the semantic pills.
pub fn css() -> String {
    let mut accents = String::new();
    for (i, color) in ACCENTS.iter().enumerate() {
        accents.push_str(&format!(".accent-strip-{i} {{ background: {color}; }}\n"));
    }
    format!(
        r#"
/* Typography only — no colours, so the theme's foreground applies. */
.page-title {{ font-size: 1.6rem; font-weight: 800; }}
.section {{ font-weight: 700; }}
.muted {{ opacity: 0.7; }}
.tiny {{ font-size: 0.8rem; }}
.railpanel-title {{ font-weight: 700; }}
.rail-label {{ font-size: 11px; font-weight: 600; }}
.rail-title {{ font-weight: 800; font-size: 1.05rem; }}

/* Course card: theme card background + border; the strip carries the colour. */
.course-card {{
    background: @card_bg_color;
    border: 1px solid @borders;
    border-radius: 8px;
}}
.card-body {{ padding: 10px 12px 12px 12px; }}
.card-title {{ font-weight: 700; font-size: 14px; }}

/* Status pills use libadwaita's semantic colours. */
.pill {{ border-radius: 10px; padding: 0 8px; font-size: 0.72rem; font-weight: 700; }}
.pill.ok     {{ background: @success_bg_color;   color: @success_fg_color; }}
.pill.warn   {{ background: @warning_bg_color;   color: @warning_fg_color; }}
.pill.danger {{ background: @error_bg_color;     color: @error_fg_color; }}
.pill.info   {{ background: @accent_bg_color;    color: @accent_fg_color; }}
.pill.muted  {{ background: alpha(currentColor, 0.12); }}

{accents}
"#
    )
}

/// Attach the stylesheet to the default display, once.
pub fn install_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(&css());
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

/// A stable accent index for a course, derived from its Canvas id.
pub fn accent_index(canvas_id: &str) -> usize {
    let sum: u32 = canvas_id.bytes().map(|b| b as u32).sum();
    (sum as usize) % ACCENTS.len()
}

pub fn clear_box(container: &gtk::Box) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
}

pub fn pill(text: &str, class: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("pill");
    label.add_css_class(class);
    label.set_valign(gtk::Align::Center);
    label
}

pub fn page_title(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.set_xalign(0.0);
    label.add_css_class("page-title");
    label
}

pub fn section(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.set_xalign(0.0);
    label.add_css_class("section");
    label.set_margin_top(8);
    label
}

pub fn empty_state(icon: &str, title: &str, description: &str) -> adw::StatusPage {
    let page = adw::StatusPage::new();
    page.set_icon_name(Some(icon));
    page.set_title(title);
    page.set_description(Some(description));
    page.set_vexpand(true);
    page
}

/// The status pill for an assignment, Canvas-style.
pub fn assignment_pill(a: &st::Assignment) -> gtk::Label {
    if a.excused {
        return pill("excused", "muted");
    }
    if a.grade.is_some() {
        return pill(a.grade.as_deref().unwrap_or("graded"), "ok");
    }
    if a.is_submitted() {
        return pill("submitted", "ok");
    }
    if a.missing {
        return pill("missing", "danger");
    }
    if a.due_at
        .as_deref()
        .map(crate::format::is_overdue)
        .unwrap_or(false)
    {
        return pill("overdue", "danger");
    }
    pill("todo", "warn")
}

/// "5 / 5" or "- / 5" — the Score column in Canvas.
pub fn score_text(a: &st::Assignment) -> String {
    let points = a
        .points_possible
        .map(trim_number)
        .unwrap_or_else(|| "-".into());
    match a.score {
        Some(score) => format!("{} / {}", trim_number(score), points),
        None => format!("- / {points}"),
    }
}

fn trim_number(value: f64) -> String {
    if (value.fract()).abs() < f64::EPSILON {
        format!("{}", value as i64)
    } else {
        format!("{value:.1}")
    }
}

/// A small "icon + count" chip for a course card.
pub fn count_chip(icon: &str, count: usize) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    let image = gtk::Image::from_icon_name(icon);
    image.set_pixel_size(14);
    image.add_css_class("muted");
    row.append(&image);
    if count > 0 {
        let label = gtk::Label::new(Some(&count.to_string()));
        label.add_css_class("tiny");
        row.append(&label);
    }
    row
}

/// A course card, as on the Canvas dashboard.
pub fn course_card(course: &st::Course, announcements: usize, assignments: usize) -> gtk::Button {
    let index = accent_index(&course.canvas_id);
    let card = gtk::Button::new();
    card.add_css_class("flat");
    card.add_css_class("course-card");
    card.set_hexpand(true);

    let outer = gtk::Box::new(gtk::Orientation::Vertical, 0);

    let strip = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    strip.set_height_request(6);
    strip.add_css_class(&format!("accent-strip-{index}"));
    outer.append(&strip);

    let body = gtk::Box::new(gtk::Orientation::Vertical, 2);
    body.add_css_class("card-body");

    let name = gtk::Label::new(Some(&course.name));
    name.set_xalign(0.0);
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);
    name.add_css_class("card-title");
    body.append(&name);

    let title = gtk::Label::new(Some(&course.title));
    title.set_xalign(0.0);
    title.set_wrap(true);
    title.set_lines(2);
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    body.append(&title);

    let term = gtk::Label::new(Some(course.term.as_deref().unwrap_or("")));
    term.set_xalign(0.0);
    term.add_css_class("tiny");
    term.add_css_class("muted");
    term.set_margin_top(4);
    body.append(&term);

    let icons = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    icons.set_margin_top(8);
    icons.append(&count_chip("chat-bubbles-symbolic", announcements));
    icons.append(&count_chip("document-edit-symbolic", assignments));
    body.append(&icons);

    outer.append(&body);
    card.set_child(Some(&outer));
    card
}

/// A "To Do" entry, as in the Canvas right rail.
pub fn todo_row(title: &str, course: &str, detail: &str) -> adw::ActionRow {
    let row = adw::ActionRow::new();
    row.set_title(&gtk::glib::markup_escape_text(title));
    row.set_subtitle(&gtk::glib::markup_escape_text(&format!(
        "{course}\n{detail}"
    )));
    row.add_prefix(&gtk::Image::from_icon_name("document-edit-symbolic"));
    row.set_activatable(false);
    row
}

/// An assignment row: title, then "status | Due … | score" beneath.
pub fn assignment_row(a: &st::Assignment) -> adw::ActionRow {
    let row = adw::ActionRow::new();
    row.set_title(&gtk::glib::markup_escape_text(&a.name));

    let mut bits: Vec<String> = Vec::new();
    if a.is_submitted() {
        bits.push(if a.grade.is_some() {
            "Graded".into()
        } else {
            "Submitted".into()
        });
    } else if a.missing {
        bits.push("Missing".into());
    } else if a.due_at.is_some() {
        bits.push("Upcoming".into());
    } else {
        bits.push("No due date".into());
    }
    if let Some(due) = &a.due_at {
        bits.push(format!("Due {}", crate::format::due_label(due)));
    }
    bits.push(score_text(a));
    row.set_subtitle(&bits.join("  |  "));

    row.add_prefix(&gtk::Image::from_icon_name("document-edit-symbolic"));
    row.add_suffix(&assignment_pill(a));
    row.set_activatable(false);
    row
}

/// An announcement as an expander with the rendered body.
pub fn announcement_row(a: &st::Announcement, course_name: &str) -> adw::ExpanderRow {
    let row = adw::ExpanderRow::new();
    row.set_title(&gtk::glib::markup_escape_text(&a.title));

    let mut sub = course_name.to_string();
    if let Some(posted) = &a.posted_at {
        sub.push_str("  ·  ");
        sub.push_str(&crate::format::short_date(posted));
    }
    if let Some(author) = &a.author {
        sub.push_str("  ·  ");
        sub.push_str(author);
    }
    row.set_subtitle(&sub);
    if a.is_unread() {
        row.add_suffix(&pill("new", "info"));
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

/// A file row with a download button.
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
    row.set_subtitle(&bits.join("  ·  "));
    row.set_activatable(false);
    row.add_prefix(&gtk::Image::from_icon_name("text-x-generic-symbolic"));

    let button = gtk::Button::from_icon_name("folder-download-symbolic");
    button.set_valign(gtk::Align::Center);
    button.add_css_class("flat");
    button.set_tooltip_text(Some(if f.local_path.is_some() {
        "Downloaded"
    } else {
        "Download"
    }));
    row.add_suffix(&button);
    (row, button)
}

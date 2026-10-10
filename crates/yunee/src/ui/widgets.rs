//! Shared view pieces.
//!
//! Colour policy: everything follows the user's GTK/libadwaita theme. The only
//! explicit colour is the small per-course dot, which reuses the colour Canvas
//! assigns each course so they stay recognisable. Status pills use
//! libadwaita's theme-aware semantic colours.

use adw::prelude::*;
use gtk4 as gtk;
use yunee_store as st;

/// Per-course accent palette (Canvas assigns each course a colour).
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

/// The stylesheet. Small on purpose: typography and the status pills, plus the
/// course dots. Nothing else overrides the theme.
pub fn css() -> String {
    let mut dots = String::new();
    for (i, color) in ACCENTS.iter().enumerate() {
        dots.push_str(&format!(".course-dot-{i} {{ background: {color}; }}\n"));
    }
    format!(
        r#"
.page-title {{ font-size: 1.6rem; font-weight: 800; }}
.section {{ font-weight: 700; }}
.muted {{ opacity: 0.7; }}
.tiny {{ font-size: 0.8rem; }}

/* Course colour dot in the course list. */
.course-dot {{ border-radius: 999px; min-width: 10px; min-height: 10px; }}

/* Status pills: a soft tint of the semantic colour with matching text, rather
   than a loud filled block. */
.pill {{ border-radius: 999px; padding: 1px 10px; font-size: 0.72rem; font-weight: 600; }}
.pill.ok     {{ background: alpha(@success_color, 0.16); color: @success_color; }}
.pill.warn   {{ background: alpha(@warning_color, 0.18); color: @warning_color; }}
.pill.danger {{ background: alpha(@error_color, 0.16);   color: @error_color; }}
.pill.info   {{ background: alpha(@accent_color, 0.16);  color: @accent_color; }}
.pill.muted  {{ background: alpha(currentColor, 0.10); }}

{dots}
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

/// The small coloured dot shown beside a course.
pub fn accent_dot(index: usize) -> gtk::Box {
    let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    dot.set_size_request(10, 10);
    dot.set_valign(gtk::Align::Center);
    dot.add_css_class("course-dot");
    dot.add_css_class(&format!("course-dot-{index}"));
    dot
}

pub fn clear_box(container: &gtk::Box) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
}

pub fn clear_list(list: &gtk::ListBox) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
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
    label.set_wrap(true);
    label.add_css_class("page-title");
    label
}

/// A dimmed, wrapping label for secondary text.
pub fn muted(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.set_xalign(0.0);
    label.set_wrap(true);
    label.add_css_class("muted");
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

/// (text, css class) for an assignment's status. Never the grade — the score
/// is shown separately, so a number never lands where a status belongs.
pub fn status_of(a: &st::Assignment) -> (&'static str, &'static str) {
    if a.excused {
        return ("excused", "muted");
    }
    if a.score.is_some() {
        return ("graded", "ok");
    }
    if a.is_submitted() {
        return ("submitted", "ok");
    }
    if a.missing {
        return ("missing", "danger");
    }
    if a.due_at
        .as_deref()
        .map(crate::format::is_overdue)
        .unwrap_or(false)
    {
        return ("overdue", "danger");
    }
    ("todo", "warn")
}

/// The status pill for an assignment.
pub fn assignment_pill(a: &st::Assignment) -> gtk::Label {
    let (text, class) = status_of(a);
    pill(text, class)
}

/// "5 / 5" or "- / 5".
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

pub fn trim_number(value: f64) -> String {
    if (value.fract()).abs() < f64::EPSILON {
        format!("{}", value as i64)
    } else {
        format!("{value:.1}")
    }
}

/// A small "icon + count" chip.
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

/// A "To Do" entry.
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

    // The status lives in the trailing pill; the subtitle carries due date and
    // score so the two don't repeat "missing / missing".
    let mut bits: Vec<String> = Vec::new();
    if let Some(due) = &a.due_at {
        bits.push(format!("Due {}", crate::format::due_label(due)));
    } else {
        bits.push("No due date".into());
    }
    bits.push(score_text(a));
    row.set_subtitle(&bits.join("  ·  "));

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
    row.set_subtitle(&gtk::glib::markup_escape_text(&sub));
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

/// Canvas HTML rendered as a selectable, wrapped label — bold/italic and line
/// breaks only, never live markup.
pub fn rich_text(html: &str) -> gtk::Label {
    let label = gtk::Label::new(None);
    label.set_xalign(0.0);
    label.set_wrap(true);
    label.set_selectable(true);
    label.set_markup(&crate::html::to_pango(html));
    label
}

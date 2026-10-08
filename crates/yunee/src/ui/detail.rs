//! Detail views: an assignment (with its submission affordances), a wiki page,
//! a file, and honest "not downloaded" / "opens on Canvas" states. Each is
//! pushed over a course and reached with the header's back button.
//!
//! Submission is **layout only** for now: the text / URL / file controls and a
//! Submit button exist, but pressing Submit does not call Canvas yet.

use std::rc::Rc;

use adw::prelude::*;
use gtk4 as gtk;
use gtk4::gio;
use yunee_store as st;

use crate::format;
use crate::html;
use crate::ui::app::Ui;
use crate::ui::widgets;

/// What the primary button on a "not downloaded" view should do.
#[derive(Clone)]
pub(crate) enum Retry {
    /// Re-run the full sync (fetches assignments, modules, and pages).
    Sync,
    /// Load just this course's folders and files.
    LoadFiles,
    /// Fetch a single wiki page by slug/id.
    Page { key: String },
}

// --------------------------------------------------------------------------
// Assignment
// --------------------------------------------------------------------------

/// An assignment: its status, facts, instructions, and the submission area.
pub(crate) fn assignment(ui: &Rc<Ui>, course: &st::Course, a: &st::Assignment) {
    widgets::clear_box(&ui.detail_page);
    ui.enter_detail(&a.name, &course.name);

    ui.detail_page.append(&widgets::page_title(&a.name));

    // Course · status pill.
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let course_label = gtk::Label::new(Some(&course.name));
    course_label.add_css_class("muted");
    line.append(&course_label);
    line.append(&widgets::assignment_pill(a));
    ui.detail_page.append(&line);

    // Facts.
    let facts = adw::PreferencesGroup::new();
    facts.set_title("Details");
    if let Some(due) = &a.due_at {
        facts.add(&fact("Due", &format::due_label(due)));
    } else {
        facts.add(&fact("Due", "No due date"));
    }
    if let Some(points) = a.points_possible {
        facts.add(&fact("Points", &trim_number(points)));
    }
    if let Some(submitted) = &a.submitted_at {
        facts.add(&fact("Submitted", &format::due_label(submitted)));
    }
    if let Some(score) = a.score {
        let points = a
            .points_possible
            .map(trim_number)
            .unwrap_or_else(|| "—".into());
        facts.add(&fact(
            "Score",
            &format!("{} / {points}", trim_number(score)),
        ));
    } else if let Some(grade) = &a.grade {
        facts.add(&fact("Grade", grade));
    }
    ui.detail_page.append(&facts);

    // Instructions.
    if let Some(desc) = a.description.as_deref().filter(|d| !d.trim().is_empty()) {
        ui.detail_page.append(&widgets::section("Instructions"));
        ui.detail_page.append(&content_view(ui, desc));
    }

    submission(ui, a);

    if let Some(url) = &a.html_url {
        ui.detail_page
            .append(&canvas_button(ui, "Open assignment on Canvas", url));
    }
}

/// The submission area: a summary when it is already turned in, and the
/// (not-yet-wired) entry controls when it is not.
fn submission(ui: &Rc<Ui>, a: &st::Assignment) {
    if a.excused {
        return;
    }
    ui.detail_page.append(&widgets::section("Your submission"));

    if a.is_submitted() {
        let text = if a.score.is_some() {
            "This has been graded. Your grade and feedback live on Canvas."
        } else {
            "You have turned this in. Yunee only reads submissions today."
        };
        ui.detail_page.append(&muted(text));
        return;
    }

    let types = submission_types(a);
    if types.is_empty() {
        ui.detail_page.append(&muted(
            "This assignment has no online submission. Check Canvas for what you need to do.",
        ));
        return;
    }

    ui.detail_page.append(&muted(
        "Draft your submission below. Sending it to Canvas is not wired up yet — \
         this is the layout, not the action.",
    ));

    let stack = adw::ViewStack::new();
    let switcher = adw::ViewSwitcher::new();
    switcher.set_policy(adw::ViewSwitcherPolicy::Narrow);
    switcher.set_stack(Some(&stack));
    for ty in &types {
        match ty.as_str() {
            "online_text_entry" => {
                stack.add_titled(&text_entry_view(), Some("text"), "Text entry");
            }
            "online_url" => {
                stack.add_titled(&url_view(), Some("url"), "Website URL");
            }
            "online_upload" => {
                stack.add_titled(&upload_view(), Some("upload"), "File upload");
            }
            other => {
                stack.add_titled(
                    &unsupported_view(other),
                    Some(other),
                    &submission_label(other),
                );
            }
        }
    }
    ui.detail_page.append(&switcher);
    ui.detail_page.append(&stack);

    let submit = gtk::Button::with_label("Submit assignment");
    submit.add_css_class("suggested-action");
    {
        let ui = ui.clone();
        submit.connect_clicked(move |_| {
            ui.toast("Submitting from Yunee isn't wired up yet.");
        });
    }
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    row.set_halign(gtk::Align::Start);
    row.set_margin_top(12);
    row.append(&submit);
    ui.detail_page.append(&row);
}

/// A multi-line text entry, for `online_text_entry`.
fn text_entry_view() -> gtk::Widget {
    let scroller = gtk::ScrolledWindow::builder()
        .min_content_height(160)
        .has_frame(true)
        .build();
    let view = gtk::TextView::new();
    view.set_wrap_mode(gtk::WrapMode::WordChar);
    view.set_top_margin(8);
    view.set_bottom_margin(8);
    view.set_left_margin(8);
    view.set_right_margin(8);
    scroller.set_child(Some(&view));
    scroller.upcast()
}

/// A single URL field, for `online_url`.
fn url_view() -> gtk::Widget {
    let group = adw::PreferencesGroup::new();
    let row = adw::EntryRow::new();
    row.set_title("Website URL");
    group.add(&row);
    group.upcast()
}

/// A file chooser, for `online_upload`. The chosen file is only remembered for
/// display — nothing is uploaded.
fn upload_view() -> gtk::Widget {
    let group = adw::PreferencesGroup::new();
    let row = adw::ActionRow::builder()
        .title("File")
        .subtitle("Choose a file to attach")
        .build();
    let button = gtk::Button::with_label("Choose…");
    button.set_valign(gtk::Align::Center);
    row.add_suffix(&button);
    group.add(&row);

    let dialog = gtk::FileDialog::new();
    dialog.set_title("Choose a file");
    button.connect_clicked(move |_| {
        let row = row.clone();
        dialog.open(
            None::<&gtk::Window>,
            None::<&gio::Cancellable>,
            move |res| {
                if let Ok(file) = res {
                    let name = file
                        .basename()
                        .map(|b| b.to_string_lossy().to_string())
                        .unwrap_or_else(|| "Selected file".into());
                    row.set_subtitle(&format!("{name} — not uploaded yet"));
                }
            },
        );
    });
    group.upcast()
}

/// Anything Canvas allows that Yunee does not draw a control for.
fn unsupported_view(kind: &str) -> gtk::Widget {
    let label = muted(&format!(
        "{} submissions aren't supported here yet. Use Canvas.",
        submission_label(kind)
    ));
    label.set_margin_top(6);
    label.upcast()
}

fn submission_label(kind: &str) -> String {
    match kind {
        "online_text_entry" => "Text entry".into(),
        "online_url" => "Website URL".into(),
        "online_upload" => "File upload".into(),
        "media_recording" => "Media recording".into(),
        "student_annotation" => "Annotated document".into(),
        other => other.replace('_', " "),
    }
}

fn submission_types(a: &st::Assignment) -> Vec<String> {
    a.submission_types
        .as_deref()
        .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok())
        .unwrap_or_default()
}

// --------------------------------------------------------------------------
// Wiki page
// --------------------------------------------------------------------------

pub(crate) fn page(ui: &Rc<Ui>, course: &st::Course, p: &st::Page) {
    widgets::clear_box(&ui.detail_page);
    ui.enter_detail(&p.title, &course.name);
    ui.detail_page.append(&widgets::page_title(&p.title));

    if p.has_body() {
        ui.detail_page
            .append(&content_view(ui, p.body.as_deref().unwrap_or_default()));
    } else {
        ui.detail_page.append(&placeholder(
            "text-x-generic-symbolic",
            "This page has no content",
            "Canvas returned an empty page. Open it on Canvas if you expected something here.",
            &[],
        ));
    }

    if let Some(slug) = &p.url {
        let path = format!("/courses/{}/pages/{slug}", course.canvas_id);
        if let Some(url) = canvas_url(ui, &path) {
            ui.detail_page
                .append(&canvas_button(ui, "Open page on Canvas", &url));
        }
    }
}

/// A page that is being fetched on demand (from a course whose Pages index is
/// disabled, so sync could not pre-fetch it).
pub(crate) fn loading(ui: &Rc<Ui>, course: &st::Course, title: &str) {
    widgets::clear_box(&ui.detail_page);
    ui.enter_detail(title, &course.name);
    ui.detail_page.append(&widgets::page_title(title));

    let column = gtk::Box::new(gtk::Orientation::Vertical, 10);
    column.set_halign(gtk::Align::Center);
    column.set_margin_top(24);
    let spinner = gtk::Spinner::new();
    spinner.start();
    column.append(&spinner);
    column.append(&muted("Loading this page from Canvas…"));
    ui.detail_page.append(&column);
}

// --------------------------------------------------------------------------
// File
// --------------------------------------------------------------------------
pub(crate) fn file(ui: &Rc<Ui>, course: &st::Course, f: &st::FileEntry) {
    widgets::clear_box(&ui.detail_page);
    ui.enter_detail(&f.display_name, &course.name);
    ui.detail_page.append(&widgets::page_title(&f.display_name));

    let facts = adw::PreferencesGroup::new();
    facts.set_title("File");
    facts.add(&fact("Course", &course.name));
    if let Some(kind) = &f.content_type {
        facts.add(&fact("Type", kind));
    }
    if let Some(size) = f.size {
        facts.add(&fact("Size", &format::bytes_label(size)));
    }
    if let Some(updated) = &f.updated_at {
        let short = format::short_date(updated);
        if !short.is_empty() {
            facts.add(&fact("Updated", &short));
        }
    }
    if let Some(path) = &f.local_path {
        facts.add(&fact("Saved to", path));
    }
    ui.detail_page.append(&facts);

    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.set_margin_top(12);
    let download = gtk::Button::with_label(if f.local_path.is_some() {
        "Download again"
    } else {
        "Download"
    });
    download.add_css_class("suggested-action");
    {
        let ui = ui.clone();
        let f = f.clone();
        download.connect_clicked(move |_| ui.download_file(f.clone()));
    }
    row.append(&download);

    if let Some(path) = &f.local_path {
        let open = gtk::Button::with_label("Open");
        let uri = gio::File::for_path(path).uri();
        let ui2 = ui.clone();
        open.connect_clicked(move |_| ui2.open_url(&uri));
        row.append(&open);
    }
    ui.detail_page.append(&row);

    if let Some(canvas) = canvas_url(
        ui,
        &format!("/courses/{}/files/{}", course.canvas_id, f.canvas_id),
    ) {
        ui.detail_page
            .append(&canvas_button(ui, "Open on Canvas", &canvas));
    }
}

// --------------------------------------------------------------------------
// Not-downloaded and web-only states
// --------------------------------------------------------------------------

/// A "nothing here yet" view with a button that fetches what is missing.
pub(crate) fn missing(
    ui: &Rc<Ui>,
    course: &st::Course,
    kind: &str,
    description: Option<&str>,
    title: Option<&str>,
    retry: Retry,
) {
    widgets::clear_box(&ui.detail_page);
    let heading = title.unwrap_or(kind);
    ui.enter_detail(heading, &course.name);
    ui.detail_page.append(&widgets::page_title(heading));

    let message = description
        .map(str::to_string)
        .unwrap_or_else(|| format!("This {kind} has not been downloaded yet."));

    let primary = match retry {
        Retry::Sync => sync_button(ui),
        Retry::LoadFiles => {
            let button = gtk::Button::with_label("Load files");
            button.add_css_class("suggested-action");
            let ui = ui.clone();
            let course = course.clone();
            button.connect_clicked(move |_| ui.load_course_files(course.clone()));
            button
        }
        Retry::Page { key } => {
            let button = gtk::Button::with_label("Download page");
            button.add_css_class("suggested-action");
            let ui = ui.clone();
            let course_id = course.canvas_id.clone();
            button.connect_clicked(move |_| ui.load_page(course_id.clone(), key.clone()));
            button
        }
    };
    ui.detail_page.append(&placeholder(
        "cloud-download-symbolic",
        "Not downloaded yet",
        &message,
        &[primary.upcast()],
    ));
}

/// A module item Yunee cannot render offline: a quiz, an external tool, a
/// discussion. Explains itself and offers Canvas.
pub(crate) fn web(ui: &Rc<Ui>, course: &st::Course, title: &str, kind: &str, url: Option<&str>) {
    widgets::clear_box(&ui.detail_page);
    ui.enter_detail(title, &course.name);
    ui.detail_page.append(&widgets::page_title(title));

    let mut buttons: Vec<gtk::Widget> = Vec::new();
    if let Some(url) = url {
        let open = gtk::Button::with_label("Open on Canvas");
        open.add_css_class("suggested-action");
        let ui = ui.clone();
        let url = url.to_string();
        open.connect_clicked(move |_| ui.open_url(&url));
        buttons.push(open.upcast());
    }
    buttons.push(sync_button(ui).upcast());
    ui.detail_page.append(&placeholder(
        "web-browser-symbolic",
        &format!("{kind} opens on Canvas"),
        &format!("Yunee cannot display a {kind} offline yet. Open it on Canvas to continue."),
        &buttons,
    ));
}

// --------------------------------------------------------------------------
// Shared bits
// --------------------------------------------------------------------------

fn fact(title: &str, subtitle: &str) -> adw::ActionRow {
    adw::ActionRow::builder()
        .title(title)
        .subtitle(subtitle)
        .build()
}

fn muted(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.set_xalign(0.0);
    label.set_wrap(true);
    label.add_css_class("muted");
    label
}

/// Render a Canvas HTML body: text as rich labels and images fetched and drawn
/// at their position in the flow.
fn content_view(ui: &Rc<Ui>, body: &str) -> gtk::Widget {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 10);
    column.set_halign(gtk::Align::Fill);
    for block in html::blocks(body) {
        match block {
            html::Block::Text(fragment) => {
                if !fragment.trim().is_empty() {
                    column.append(&widgets::rich_text(&fragment));
                }
            }
            html::Block::Image(image) => column.append(&image_widget(ui, &image)),
        }
    }
    column.upcast()
}

/// One image from a body: a picture sized from its `width`/`height`, filled in
/// asynchronously (from the cache when we already have it).
fn image_widget(ui: &Rc<Ui>, image: &html::Image) -> gtk::Widget {
    let holder = gtk::Box::new(gtk::Orientation::Vertical, 4);
    holder.set_halign(gtk::Align::Start);
    holder.set_margin_top(4);
    holder.set_margin_bottom(4);

    let picture = gtk::Picture::new();
    picture.set_can_shrink(true);
    picture.set_content_fit(gtk::ContentFit::Contain);
    let width = image.width.unwrap_or(360).clamp(48, 560);
    let height = match (image.width, image.height) {
        (Some(w), Some(h)) if w > 0 => (h as f64 * width as f64 / w as f64).round() as i32,
        _ => -1,
    };
    picture.set_size_request(width, height);
    holder.append(&picture);

    let spinner = gtk::Spinner::new();
    spinner.set_size_request(width, 40);
    spinner.start();
    holder.append(&spinner);

    if let Some(texture) = ui.image_cache.borrow().get(&image.src).cloned() {
        picture.set_paintable(Some(&texture));
        spinner.stop();
        holder.remove(&spinner);
    } else {
        ui.load_image(image.src.clone(), picture.clone(), spinner.clone());
    }

    if let Some(alt) = &image.alt {
        if !alt.trim().is_empty() {
            let caption = gtk::Label::new(Some(alt));
            caption.set_xalign(0.0);
            caption.add_css_class("tiny");
            caption.add_css_class("muted");
            holder.append(&caption);
        }
    }
    holder.upcast()
}

fn sync_button(ui: &Rc<Ui>) -> gtk::Button {
    let button = gtk::Button::with_label("Sync now");
    button.add_css_class("suggested-action");
    let ui = ui.clone();
    button.connect_clicked(move |_| ui.sync_now());
    button
}

/// A centred icon + title + description + buttons, without the `vexpand` that
/// makes a `StatusPage` push its buttons to the bottom of a scroll view.
fn placeholder(icon: &str, title: &str, description: &str, buttons: &[gtk::Widget]) -> gtk::Box {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 10);
    column.set_halign(gtk::Align::Center);
    column.set_margin_top(36);
    column.set_margin_bottom(24);

    let image = gtk::Image::from_icon_name(icon);
    image.set_pixel_size(56);
    image.add_css_class("dim-label");
    column.append(&image);

    let heading = gtk::Label::new(None);
    heading.set_markup(&format!("<b>{}</b>", gtk::glib::markup_escape_text(title)));
    heading.add_css_class("title-3");
    column.append(&heading);

    let body = gtk::Label::new(Some(description));
    body.set_wrap(true);
    body.set_justify(gtk::Justification::Center);
    body.set_max_width_chars(46);
    body.add_css_class("muted");
    column.append(&body);

    if !buttons.is_empty() {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.set_halign(gtk::Align::Center);
        row.set_margin_top(6);
        for button in buttons {
            row.append(button);
        }
        column.append(&row);
    }
    column
}

fn canvas_button(ui: &Rc<Ui>, label: &str, url: &str) -> gtk::Button {
    let button = gtk::Button::with_label(label);
    button.add_css_class("pill");
    button.set_halign(gtk::Align::Start);
    button.set_margin_top(12);
    let ui = ui.clone();
    let url = url.to_string();
    button.connect_clicked(move |_| ui.open_url(&url));
    button
}

/// Build a Canvas URL from a `/courses/...` path, when an address is configured.
/// The stored address may omit the scheme, so one is added here.
fn canvas_url(ui: &Ui, path: &str) -> Option<String> {
    let base = ui.state.base_url()?;
    let base = base.trim_end_matches('/');
    let base = if base.contains("://") {
        base.to_string()
    } else {
        format!("https://{base}")
    };
    Some(format!("{base}{path}"))
}

fn trim_number(value: f64) -> String {
    if (value.fract()).abs() < f64::EPSILON {
        format!("{}", value as i64)
    } else {
        format!("{value:.1}")
    }
}

//! A course. Its sections are peer views, so per the HIG they are a view
//! switcher in the header bar (kept to five: Overview, Assignments, Modules,
//! Files, Grades).

use std::rc::Rc;

use adw::prelude::*;
use gtk4 as gtk;
use yunee_store as st;

use crate::format;
use crate::ui::app::Ui;
use crate::ui::widgets;

/// Open a course into the content area.
pub fn open(ui: &Rc<Ui>, course: st::Course) {
    *ui.selected.borrow_mut() = Some(course.clone());
    widgets::clear_box(&ui.course_page);

    let stack = adw::ViewStack::new();
    stack.set_vexpand(true);
    add_tab(
        &stack,
        "view-grid-symbolic",
        "overview",
        "Overview",
        build_overview(ui, &course),
    );
    add_tab(
        &stack,
        "document-edit-symbolic",
        "assignments",
        "Assignments",
        build_assignments(ui, &course),
    );
    add_tab(
        &stack,
        "view-list-symbolic",
        "modules",
        "Modules",
        build_modules(ui, &course),
    );
    add_tab(
        &stack,
        "folder-symbolic",
        "files",
        "Files",
        build_files(ui, &course),
    );
    add_tab(
        &stack,
        "starred-symbolic",
        "grades",
        "Grades",
        build_grades(ui, &course),
    );

    let switcher = adw::ViewSwitcher::new();
    switcher.set_policy(adw::ViewSwitcherPolicy::Wide);
    switcher.set_stack(Some(&stack));

    ui.course_page.append(&stack);
    ui.set_header_widget(&switcher);
    ui.content.set_visible_child_name("course");
    ui.window.set_title(Some(&course.name));
    ui.split.set_show_content(true);

    let _ = course;
}

fn add_tab(stack: &adw::ViewStack, icon: &str, name: &str, title: &str, content: gtk::Box) {
    content.set_margin_top(20);
    content.set_margin_bottom(28);
    content.set_margin_start(24);
    content.set_margin_end(24);
    let scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&content)
        .build();
    let page = stack.add_titled(&scrolled, Some(name), title);
    page.set_icon_name(Some(icon));
}

fn heading(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.set_xalign(0.0);
    label.set_wrap(true);
    label.add_css_class("page-title");
    label
}

fn dim(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.set_xalign(0.0);
    label.set_wrap(true);
    label.add_css_class("muted");
    label
}

/// An announcement expander, with a "Mark read" button when unread.
fn announcement_with_read(
    ui: &Rc<Ui>,
    course: &st::Course,
    a: &st::Announcement,
) -> adw::ExpanderRow {
    let row = widgets::announcement_row(a, &course.name);
    if a.is_unread() {
        let button = gtk::Button::with_label("Mark read");
        button.add_css_class("flat");
        button.set_valign(gtk::Align::Center);
        let ui2 = ui.clone();
        let canvas_id = a.canvas_id.clone();
        let course_canvas = course.canvas_id.clone();
        button.connect_clicked(move |_| ui2.mark_announcement_read(&course_canvas, &canvas_id));
        row.add_suffix(&button);
    }
    row
}

// --------------------------------------------------------------------------
// Overview
// --------------------------------------------------------------------------

fn build_overview(ui: &Rc<Ui>, course: &st::Course) -> gtk::Box {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 8);
    page.append(&heading(&course.title));

    let mut bits: Vec<String> = Vec::new();
    if let Some(prof) = &course.professor {
        bits.push(prof.clone());
    }
    if let Some(term) = &course.term {
        bits.push(term.clone());
    }
    if let Some(grade) = &course.current_grade {
        let score = course
            .current_score
            .map(|s| format!(" ({s:.1}%)"))
            .unwrap_or_default();
        bits.push(format!("Grade {grade}{score}"));
    }
    if !bits.is_empty() {
        let sub = gtk::Label::new(Some(&bits.join("  ·  ")));
        sub.set_xalign(0.0);
        sub.add_css_class("muted");
        page.append(&sub);
    }

    let assignments = ui
        .state
        .store
        .list_assignments(course.id)
        .unwrap_or_default();
    let upcoming: Vec<&st::Assignment> = assignments
        .iter()
        .filter(|a| !a.is_submitted())
        .filter(|a| {
            a.due_at
                .as_deref()
                .and_then(format::days_until)
                .map(|n| n <= 30)
                .unwrap_or(false)
        })
        .take(6)
        .collect();
    page.append(&widgets::section("Upcoming"));
    if upcoming.is_empty() {
        page.append(&dim("Nothing due in the next month."));
    } else {
        for a in upcoming {
            page.append(&widgets::assignment_row(a));
        }
    }

    let announcements = ui
        .state
        .store
        .list_announcements(course.id)
        .unwrap_or_default();
    page.append(&widgets::section("Announcements"));
    if announcements.is_empty() {
        page.append(&dim("No announcements."));
    } else {
        for a in &announcements {
            page.append(&announcement_with_read(ui, course, a));
        }
    }
    page
}

// --------------------------------------------------------------------------
// Assignments
// --------------------------------------------------------------------------

fn build_assignments(ui: &Rc<Ui>, course: &st::Course) -> gtk::Box {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 8);
    page.append(&heading("Assignments"));

    let assignments = ui
        .state
        .store
        .list_assignments(course.id)
        .unwrap_or_default();
    if assignments.is_empty() {
        page.append(&widgets::empty_state(
            "document-edit-symbolic",
            "No assignments",
            "Nothing has been assigned in this course yet.",
        ));
        return page;
    }

    let mut overdue: Vec<&st::Assignment> = Vec::new();
    let mut upcoming: Vec<&st::Assignment> = Vec::new();
    let mut undated: Vec<&st::Assignment> = Vec::new();
    let mut past: Vec<&st::Assignment> = Vec::new();
    for a in &assignments {
        match &a.due_at {
            None => undated.push(a),
            Some(due) => {
                let days = format::days_until(due).unwrap_or(0);
                if days < 0 {
                    if a.is_submitted() {
                        past.push(a)
                    } else {
                        overdue.push(a)
                    }
                } else {
                    upcoming.push(a)
                }
            }
        }
    }

    for (label, group) in [
        ("Overdue", &overdue),
        ("Upcoming", &upcoming),
        ("Undated", &undated),
        ("Past", &past),
    ] {
        if group.is_empty() {
            continue;
        }
        page.append(&widgets::section(label));
        for a in group {
            page.append(&widgets::assignment_row(a));
        }
    }
    page
}

// --------------------------------------------------------------------------
// Modules
// --------------------------------------------------------------------------

fn build_modules(ui: &Rc<Ui>, course: &st::Course) -> gtk::Box {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 8);
    page.append(&heading("Modules"));

    let modules = ui.state.store.list_modules(course.id).unwrap_or_default();
    if modules.is_empty() {
        page.append(&widgets::empty_state(
            "view-list-symbolic",
            "No modules",
            "This course does not use modules.",
        ));
        return page;
    }
    for module in &modules {
        let items = ui
            .state
            .store
            .list_module_items(module.id)
            .unwrap_or_default();
        let row = adw::ExpanderRow::new();
        row.set_title(&gtk::glib::markup_escape_text(&module.name));
        row.set_subtitle(&format!("{} items", items.len()));
        if let Some(state) = &module.state {
            let class = if state == "completed" { "ok" } else { "muted" };
            row.add_suffix(&widgets::pill(state, class));
        }
        for item in &items {
            let item_row = adw::ActionRow::new();
            item_row.set_title(&gtk::glib::markup_escape_text(&item.title));
            item_row.add_prefix(&gtk::Image::from_icon_name(icon_for_type(
                item.item_type.as_deref(),
            )));
            if item.completed {
                item_row.add_suffix(&widgets::pill("done", "ok"));
            }
            item_row.set_activatable(false);
            row.add_row(&item_row);
        }
        page.append(&row);
    }
    page
}

fn icon_for_type(item_type: Option<&str>) -> &'static str {
    match item_type.unwrap_or("") {
        "Assignment" => "document-edit-symbolic",
        "Quiz" => "dialog-question-symbolic",
        "File" | "Page" => "text-x-generic-symbolic",
        "ExternalUrl" | "ExternalTool" => "web-browser-symbolic",
        _ => "view-list-symbolic",
    }
}

// --------------------------------------------------------------------------
// Files
// --------------------------------------------------------------------------

fn build_files(ui: &Rc<Ui>, course: &st::Course) -> gtk::Box {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 8);
    page.append(&heading("Files"));

    let files = ui.state.store.list_files(course.id).unwrap_or_default();
    if files.is_empty() {
        let first = {
            let mut attempted = ui.files_attempted.borrow_mut();
            let first = !attempted.contains(&course.id);
            attempted.insert(course.id);
            first
        };
        if first && ui.state.connection().is_some() {
            ui.load_course_files(course.clone());
            page.append(&widgets::empty_state(
                "folder-symbolic",
                "Loading files…",
                "Fetching this course's files from Canvas.",
            ));
        } else {
            page.append(&widgets::empty_state(
                "folder-open-symbolic",
                "No files",
                "Canvas returned no files for this course.",
            ));
        }
        return page;
    }

    let list = gtk::ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk::SelectionMode::None);
    for file in &files {
        let (row, button) = widgets::file_row(file);
        if file.local_path.is_some() {
            row.add_suffix(&widgets::pill("saved", "ok"));
        }
        let ui2 = ui.clone();
        let file = file.clone();
        button.connect_clicked(move |_| ui2.download_file(file.clone()));
        list.append(&row);
    }
    page.append(&list);
    page
}

// --------------------------------------------------------------------------
// Grades
// --------------------------------------------------------------------------

fn build_grades(ui: &Rc<Ui>, course: &st::Course) -> gtk::Box {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 8);
    page.append(&heading(&format!("Grades — {}", course.name)));

    if let Some(grade) = &course.current_grade {
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let total = gtk::Label::new(Some("Total:"));
        total.add_css_class("muted");
        line.append(&total);
        let score = course
            .current_score
            .map(|s| format!("{s:.1}%"))
            .unwrap_or_default();
        line.append(&widgets::pill(&format!("{grade} {score}"), "info"));
        page.append(&line);
    }

    let assignments = ui
        .state
        .store
        .list_assignments(course.id)
        .unwrap_or_default();
    if assignments.is_empty() {
        page.append(&widgets::empty_state(
            "view-list-symbolic",
            "No grades",
            "Nothing has been graded in this course yet.",
        ));
        return page;
    }

    let grid = gtk::Grid::new();
    grid.set_column_spacing(24);
    grid.set_row_spacing(10);
    for (col, title) in ["Name", "Due", "Submitted", "Status", "Score"]
        .iter()
        .enumerate()
    {
        let label = gtk::Label::new(Some(title));
        label.set_xalign(0.0);
        label.add_css_class("section");
        grid.attach(&label, col as i32, 0, 1, 1);
    }
    for (i, a) in assignments.iter().enumerate() {
        let row = i as i32 + 1;

        let name = gtk::Label::new(Some(&a.name));
        name.set_xalign(0.0);
        name.add_css_class("row-title");
        grid.attach(&name, 0, row, 1, 1);

        let due_text = a
            .due_at
            .as_deref()
            .map(format::due_label)
            .unwrap_or_else(|| "—".into());
        let due = gtk::Label::new(Some(&due_text));
        due.set_xalign(0.0);
        grid.attach(&due, 1, row, 1, 1);

        let submitted_text = a
            .submitted_at
            .as_deref()
            .map(format::short_date)
            .unwrap_or_default();
        let submitted = gtk::Label::new(Some(&submitted_text));
        submitted.set_xalign(0.0);
        grid.attach(&submitted, 2, row, 1, 1);

        grid.attach(&widgets::assignment_pill(a), 3, row, 1, 1);

        let score = gtk::Label::new(Some(&widgets::score_text(a)));
        score.set_xalign(1.0);
        grid.attach(&score, 4, row, 1, 1);
    }
    page.append(&grid);
    page
}

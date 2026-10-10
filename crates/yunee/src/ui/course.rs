//! A course. Its sections are peer views, so per the HIG they are a view
//! switcher in the header bar (kept to five: Overview, Assignments, Modules,
//! Files, Grades).

use std::rc::Rc;

use adw::prelude::*;
use gtk4 as gtk;
use gtk4::gio;
use gtk4::glib;
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
    ui.set_back_visible(false);
    ui.content.set_visible_child_name("course");
    ui.window.set_title(Some(&course.name));
    ui.split.set_show_content(true);

    // Return to the tab that was open last, and remember changes so coming back
    // from a detail lands where you left off.
    let remembered = ui
        .course_tab
        .borrow()
        .get(&course.id)
        .cloned()
        .unwrap_or_else(|| "overview".into());
    stack.set_visible_child_name(&remembered);
    {
        let ui = ui.clone();
        let id = course.id;
        stack.connect_visible_child_name_notify(move |s| {
            if let Some(name) = s.visible_child_name() {
                ui.course_tab.borrow_mut().insert(id, name.to_string());
            }
        });
    }
}

/// Make a row open an assignment's detail view.
fn make_assignment_clickable(
    ui: &Rc<Ui>,
    course: &st::Course,
    a: &st::Assignment,
    row: &adw::ActionRow,
) {
    row.set_activatable(true);
    let ui = ui.clone();
    let course = course.clone();
    let assignment = a.clone();
    row.connect_activated(move |_| ui.open_assignment(&course, &assignment));
}

/// A boxed list to hold rows: `AdwActionRow` only emits `activated` when it sits
/// inside a `GtkListBox`, so rows must not be appended to a bare box.
fn boxed_list() -> gtk::ListBox {
    let list = gtk::ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk::SelectionMode::None);
    list
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
    page.append(&widgets::page_title(&course.title));

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
        page.append(&widgets::muted("Nothing due in the next month."));
    } else {
        let list = boxed_list();
        for a in upcoming {
            let row = widgets::assignment_row(a);
            make_assignment_clickable(ui, course, a, &row);
            list.append(&row);
        }
        page.append(&list);
    }

    let announcements = ui
        .state
        .store
        .list_announcements(course.id)
        .unwrap_or_default();
    page.append(&widgets::section("Announcements"));
    if announcements.is_empty() {
        page.append(&widgets::muted("No announcements."));
    } else {
        let list = boxed_list();
        for a in &announcements {
            list.append(&announcement_with_read(ui, course, a));
        }
        page.append(&list);
    }
    page
}

// --------------------------------------------------------------------------
// Assignments
// --------------------------------------------------------------------------

fn build_assignments(ui: &Rc<Ui>, course: &st::Course) -> gtk::Box {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 8);
    page.append(&widgets::page_title("Assignments"));

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
        let list = boxed_list();
        for a in group {
            let row = widgets::assignment_row(a);
            make_assignment_clickable(ui, course, a, &row);
            list.append(&row);
        }
        page.append(&list);
    }
    page
}

// --------------------------------------------------------------------------
// Modules
// --------------------------------------------------------------------------

fn build_modules(ui: &Rc<Ui>, course: &st::Course) -> gtk::Box {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 8);
    page.append(&widgets::page_title("Modules"));

    let modules = ui.state.store.list_modules(course.id).unwrap_or_default();
    if modules.is_empty() {
        page.append(&widgets::empty_state(
            "view-list-symbolic",
            "No modules",
            "This course does not use modules.",
        ));
        return page;
    }
    let group = adw::PreferencesGroup::new();
    for module in &modules {
        let items = ui
            .state
            .store
            .list_module_items(module.id)
            .unwrap_or_default();
        let row = adw::ExpanderRow::new();
        row.set_expanded(true);
        row.set_title(&gtk::glib::markup_escape_text(&module.name));
        row.set_subtitle(&format!("{} items", items.len()));
        if let Some(state) = &module.state {
            let class = if state == "completed" { "ok" } else { "muted" };
            row.add_suffix(&widgets::pill(state, class));
        }
        for item in &items {
            // A `SubHeader` is a text divider inside the module, not a link.
            if item.item_type.as_deref() == Some("SubHeader") {
                let label = gtk::Label::new(None);
                label.set_markup(&format!(
                    "<b>{}</b>",
                    gtk::glib::markup_escape_text(&item.title)
                ));
                label.set_xalign(0.0);
                label.set_margin_top(10);
                label.set_margin_bottom(2);
                row.add_row(&label);
                continue;
            }

            let item_row = adw::ActionRow::new();
            item_row.set_title(&gtk::glib::markup_escape_text(&item.title));
            item_row.add_prefix(&gtk::Image::from_icon_name(icon_for_type(
                item.item_type.as_deref(),
            )));
            if item.completed {
                item_row.add_suffix(&widgets::pill("done", "ok"));
            }
            item_row.set_activatable(true);
            {
                let ui = ui.clone();
                let course = course.clone();
                let item = item.clone();
                item_row.connect_activated(move |_| ui.open_module_item(&course, &item));
            }
            row.add_row(&item_row);
        }
        group.add(&row);
    }
    page.append(&group);
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
    page.append(&widgets::page_title("Files"));

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
    page.append(&widgets::page_title(&format!("Grades — {}", course.name)));

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

    // A total computed from the graded assignments, so it works even when
    // Canvas does not hand us a course-level grade.
    let graded: Vec<&st::Assignment> = assignments.iter().filter(|a| a.score.is_some()).collect();
    let earned: f64 = graded.iter().filter_map(|a| a.score).sum();
    let possible: f64 = graded.iter().filter_map(|a| a.points_possible).sum();
    let percent = if possible > 0.0 {
        earned / possible * 100.0
    } else {
        0.0
    };
    let grade = course
        .current_grade
        .clone()
        .unwrap_or_else(|| format!("{percent:.1}%"));
    let summary = format!(
        "{grade}  ·  {earned:.0} / {possible:.0} pts  ·  {} of {} graded",
        graded.len(),
        assignments.len()
    );
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let total = gtk::Label::new(Some("Total:"));
    total.add_css_class("muted");
    line.append(&total);
    line.append(&widgets::pill(&summary, "info"));
    page.append(&line);

    let store = gio::ListStore::new::<glib::BoxedAnyObject>();
    for a in &assignments {
        store.append(&glib::BoxedAnyObject::new(a.clone()));
    }

    let selection = gtk::SingleSelection::new(Some(store));
    let view = gtk::ColumnView::new(Some(selection.clone()));
    view.set_show_row_separators(true);
    view.set_show_column_separators(false);
    view.set_margin_top(6);
    {
        let ui = ui.clone();
        let course = course.clone();
        let selection = selection.clone();
        view.connect_activate(move |_, position| {
            if let Some(item) = selection.item(position) {
                if let Some(a) = assignment_of(&item) {
                    ui.open_assignment(&course, &a);
                }
            }
        });
    }

    view.append_column(&text_column(
        "Name",
        0.0,
        true,
        |a| a.name.clone(),
        |a| a.name.to_lowercase(),
    ));
    view.append_column(&text_column(
        "Due",
        0.0,
        false,
        |a| {
            a.due_at
                .as_deref()
                .map(format::due_label)
                .unwrap_or_else(|| "—".into())
        },
        |a| a.due_at.clone().unwrap_or_default(),
    ));
    view.append_column(&status_column());
    view.append_column(&text_column(
        "Score",
        1.0,
        false,
        widgets::score_text,
        |a| format!("{:09.2}", a.score.unwrap_or(-1.0)),
    ));

    page.append(&view);
    page
}

/// A sortable, single-line text column for the grades table.
fn text_column(
    title: &str,
    xalign: f32,
    expand: bool,
    text: fn(&st::Assignment) -> String,
    key: fn(&st::Assignment) -> String,
) -> gtk::ColumnViewColumn {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(move |_, item| {
        let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
            return;
        };
        let label = gtk::Label::new(None);
        label.set_xalign(xalign);
        label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        item.set_child(Some(&label));
    });
    factory.connect_bind(move |_, item| {
        let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
            return;
        };
        let (Some(label), Some(object)) = (item.child().and_downcast::<gtk::Label>(), item.item())
        else {
            return;
        };
        if let Some(a) = assignment_of(&object) {
            label.set_text(&text(&a));
        }
    });
    let column = gtk::ColumnViewColumn::new(Some(title), Some(factory));
    column.set_expand(expand);
    let sorter = gtk::CustomSorter::new(move |left, right| {
        match (assignment_of(left), assignment_of(right)) {
            (Some(a), Some(b)) => cmp_order(key(&a).cmp(&key(&b))),
            _ => gtk::Ordering::Equal,
        }
    });
    column.set_sorter(Some(&sorter));
    column
}

/// The Status column — a pill per row.
fn status_column() -> gtk::ColumnViewColumn {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(|_, item| {
        let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
            return;
        };
        let label = gtk::Label::new(None);
        label.set_halign(gtk::Align::Start);
        item.set_child(Some(&label));
    });
    factory.connect_bind(|_, item| {
        let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
            return;
        };
        let (Some(label), Some(object)) = (item.child().and_downcast::<gtk::Label>(), item.item())
        else {
            return;
        };
        for class in ["pill", "ok", "warn", "danger", "info", "muted"] {
            label.remove_css_class(class);
        }
        if let Some(a) = assignment_of(&object) {
            let (text, class) = widgets::status_of(&a);
            label.set_text(text);
            label.add_css_class("pill");
            label.add_css_class(class);
        }
    });
    let column = gtk::ColumnViewColumn::new(Some("Status"), Some(factory));
    let sorter =
        gtk::CustomSorter::new(
            |left, right| match (assignment_of(left), assignment_of(right)) {
                (Some(a), Some(b)) => {
                    cmp_order(widgets::status_of(&a).0.cmp(widgets::status_of(&b).0))
                }
                _ => gtk::Ordering::Equal,
            },
        );
    column.set_sorter(Some(&sorter));
    column
}

fn assignment_of(object: &glib::Object) -> Option<st::Assignment> {
    object
        .downcast_ref::<glib::BoxedAnyObject>()
        .map(|boxed| boxed.borrow::<st::Assignment>().clone())
}

fn cmp_order(order: std::cmp::Ordering) -> gtk::Ordering {
    match order {
        std::cmp::Ordering::Less => gtk::Ordering::Smaller,
        std::cmp::Ordering::Equal => gtk::Ordering::Equal,
        std::cmp::Ordering::Greater => gtk::Ordering::Larger,
    }
}

//! The Canvas-style dashboard: a grid of course cards with a "To Do" rail, and
//! the Courses list page.

use std::collections::HashMap;
use std::rc::Rc;

use adw::prelude::*;
use gtk4 as gtk;
use yunee_store as st;
use yunee_store::LocalId;

use crate::format;
use crate::ui::app::Ui;
use crate::ui::widgets;

/// Render the dashboard.
pub fn render(ui: &Rc<Ui>) {
    widgets::clear_box(&ui.dashboard_page);
    ui.dashboard_page.append(&widgets::page_title("Dashboard"));

    let columns = gtk::Box::new(gtk::Orientation::Horizontal, 28);
    let main = gtk::Box::new(gtk::Orientation::Vertical, 14);
    main.set_hexpand(true);
    columns.append(&main);

    let courses = ui.state.store.list_courses().unwrap_or_default();
    if courses.is_empty() {
        main.append(&widgets::empty_state(
            "x-office-calendar-symbolic",
            "No courses yet",
            "Connect to Canvas and sync to pull in your courses.",
        ));
    } else {
        let flow = gtk::FlowBox::new();
        flow.set_selection_mode(gtk::SelectionMode::None);
        flow.set_homogeneous(true);
        flow.set_max_children_per_line(3);
        flow.set_min_children_per_line(1);
        flow.set_column_spacing(16);
        flow.set_row_spacing(16);
        for course in &courses {
            let unread = ui
                .state
                .store
                .list_announcements(course.id)
                .map(|v| v.iter().filter(|a| a.is_unread()).count())
                .unwrap_or(0);
            let open = ui
                .state
                .store
                .list_assignments(course.id)
                .map(|v| v.iter().filter(|a| !a.is_submitted()).count())
                .unwrap_or(0);
            let card = widgets::course_card(course, unread, open);
            let ui2 = ui.clone();
            let course = course.clone();
            card.connect_clicked(move |_| ui2.open_course(course.clone()));
            flow.insert(&card, -1);
        }
        main.append(&flow);
    }

    // --- To Do rail ---
    let todo = gtk::Box::new(gtk::Orientation::Vertical, 8);
    todo.set_size_request(330, -1);
    let title = gtk::Label::new(Some("To Do"));
    title.set_xalign(0.0);
    title.add_css_class("railpanel-title");
    todo.append(&title);

    let items = todo_items(ui);
    if items.is_empty() {
        let empty = gtk::Label::new(Some("Nothing due. You're all caught up."));
        empty.set_xalign(0.0);
        empty.set_wrap(true);
        empty.add_css_class("muted");
        todo.append(&empty);
    } else {
        for (assignment, course) in items {
            let detail = {
                let mut d = Vec::new();
                if let Some(points) = assignment.points_possible {
                    d.push(format!("{points:.0} pts"));
                }
                if let Some(due) = &assignment.due_at {
                    d.push(format::due_label(due));
                }
                d.join("  •  ")
            };
            let row = widgets::todo_row(&assignment.name, &course.name, &detail);
            row.set_activatable(true);
            let ui2 = ui.clone();
            row.connect_activated(move |_| ui2.open_course(course.clone()));
            todo.append(&row);
        }
    }
    columns.append(&todo);

    ui.dashboard_page.append(&columns);
}

/// Up to a dozen open assignments due soonest (overdue first).
fn todo_items(ui: &Rc<Ui>) -> Vec<(st::Assignment, st::Course)> {
    let courses: HashMap<LocalId, st::Course> = ui
        .state
        .store
        .list_courses()
        .unwrap_or_default()
        .into_iter()
        .map(|c| (c.id, c))
        .collect();

    let mut items: Vec<(st::Assignment, st::Course)> = ui
        .state
        .store
        .list_all_assignments()
        .unwrap_or_default()
        .into_iter()
        .filter(|a| !a.is_submitted() && !a.excused)
        .filter(|a| match &a.due_at {
            None => false,
            Some(due) => format::days_until(due).map(|d| d <= 21).unwrap_or(false),
        })
        .filter_map(|a| courses.get(&a.course_id).cloned().map(|c| (a, c)))
        .collect();
    items.truncate(12);
    items
}

/// The Courses list page.
pub fn render_courses(ui: &Rc<Ui>) {
    widgets::clear_box(&ui.courses_page);
    ui.courses_page.append(&widgets::page_title("Courses"));

    let courses = ui.state.store.list_courses().unwrap_or_default();
    if courses.is_empty() {
        ui.courses_page.append(&widgets::empty_state(
            "x-office-calendar-symbolic",
            "No courses",
            "Connect to Canvas and sync to see your courses here.",
        ));
        return;
    }

    let list = gtk::ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk::SelectionMode::None);
    for course in &courses {
        let subtitle = match &course.term {
            Some(term) => format!("{}  ·  {term}", course.title),
            None => course.title.clone(),
        };
        let row = adw::ActionRow::builder()
            .title(&course.name)
            .subtitle(&subtitle)
            .activatable(true)
            .build();
        row.add_prefix(&gtk::Image::from_icon_name("x-office-calendar-symbolic"));
        if let Some(grade) = &course.current_grade {
            row.add_suffix(&widgets::pill(grade, "info"));
        }
        let ui2 = ui.clone();
        let course = course.clone();
        row.connect_activated(move |_| ui2.open_course(course.clone()));
        list.append(&row);
    }
    ui.courses_page.append(&list);
}

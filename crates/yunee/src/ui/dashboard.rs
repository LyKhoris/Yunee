//! The dashboard: what's due (To Do) and what's new (unread announcements).
//! Courses themselves live in the sidebar.

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

    let courses: HashMap<LocalId, st::Course> = ui
        .state
        .store
        .list_courses()
        .unwrap_or_default()
        .into_iter()
        .map(|c| (c.id, c))
        .collect();

    widgets::clear_box(&ui.dashboard_columns);
    ui.dashboard_columns.set_margin_top(12);
    ui.dashboard_page.append(&widgets::muted(
        "Your upcoming work and latest course updates, in one place.",
    ));
    ui.dashboard_page.append(&ui.dashboard_columns);

    let items = todo_items(ui, &courses);
    let tasks = section_column("To Do", items.len());
    ui.dashboard_columns.append(&tasks);
    if items.is_empty() {
        tasks.append(&empty_card("Nothing due", "You're all caught up."));
    } else {
        let (overdue, upcoming): (Vec<_>, Vec<_>) = items
            .into_iter()
            .partition(|(a, _)| a.due_at.as_deref().map(format::is_overdue).unwrap_or(false));
        for (heading, group) in [("Overdue", overdue), ("Upcoming · next 3 weeks", upcoming)] {
            if group.is_empty() {
                continue;
            }
            tasks.append(&widgets::section(heading));
            let list = gtk::ListBox::new();
            list.add_css_class("boxed-list");
            list.set_selection_mode(gtk::SelectionMode::None);
            for (assignment, course) in group {
                let points = assignment
                    .points_possible
                    .map(|p| format!("{} pts", widgets::trim_number(p)))
                    .unwrap_or_default();
                let row = widgets::todo_row(
                    &assignment.name,
                    &crate::ui::app::short_course_name(&course.name),
                    &points,
                );
                row.set_title_lines(2);
                row.set_subtitle_lines(2);
                let deadline = gtk::Box::new(gtk::Orientation::Vertical, 6);
                deadline.set_valign(gtk::Align::Center);
                let due = gtk::Label::new(
                    assignment
                        .due_at
                        .as_deref()
                        .map(format::due_label)
                        .as_deref(),
                );
                due.add_css_class("tiny");
                deadline.append(&due);
                deadline.append(&widgets::assignment_pill(&assignment));
                row.add_suffix(&deadline);
                row.set_activatable(true);
                let ui2 = ui.clone();
                row.connect_activated(move |_| ui2.open_assignment(&course, &assignment));
                list.append(&row);
            }
            tasks.append(&list);
        }
    }

    let unread: Vec<st::Announcement> = ui
        .state
        .store
        .list_all_announcements(20)
        .unwrap_or_default()
        .into_iter()
        .filter(|a| a.is_unread())
        .collect();
    let updates = section_column("New announcements", unread.len());
    ui.dashboard_columns.append(&updates);
    if unread.is_empty() {
        updates.append(&empty_card(
            "All read",
            "No new announcements from your courses.",
        ));
    } else {
        let list = gtk::ListBox::new();
        list.add_css_class("boxed-list");
        list.set_selection_mode(gtk::SelectionMode::None);
        for announcement in &unread {
            let course = courses.get(&announcement.course_id);
            let course_name = course
                .map(|c| crate::ui::app::short_course_name(&c.name))
                .unwrap_or_default();
            let row = widgets::announcement_row(announcement, &course_name);
            row.set_title_lines(2);
            row.set_subtitle_lines(2);
            if let Some(course) = course {
                let button = gtk::Button::from_icon_name("emblem-ok-symbolic");
                button.set_tooltip_text(Some("Mark read"));
                button.update_property(&[gtk::accessible::Property::Label("Mark read")]);
                button.add_css_class("flat");
                button.set_valign(gtk::Align::Center);
                let ui2 = ui.clone();
                let canvas_id = announcement.canvas_id.clone();
                let course_canvas = course.canvas_id.clone();
                button.connect_clicked(move |_| {
                    ui2.mark_announcement_read(&course_canvas, &canvas_id)
                });
                row.add_suffix(&button);
            }
            list.append(&row);
        }
        updates.append(&list);
    }
}

fn section_column(title: &str, count: usize) -> gtk::Box {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 12);
    column.set_hexpand(true);
    column.set_valign(gtk::Align::Start);
    let heading = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let label = widgets::section(title);
    label.set_margin_top(0);
    label.set_hexpand(true);
    heading.append(&label);
    heading.append(&widgets::pill(&count.to_string(), "muted"));
    column.append(&heading);
    column
}

fn empty_card(title: &str, description: &str) -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 8);
    card.add_css_class("card");
    let contents = gtk::Box::new(gtk::Orientation::Vertical, 8);
    contents.set_margin_top(24);
    contents.set_margin_bottom(24);
    contents.set_margin_start(20);
    contents.set_margin_end(20);
    contents.append(&widgets::section(title));
    contents.append(&widgets::muted(description));
    card.append(&contents);
    card
}

/// Up to a dozen open assignments due soonest (overdue first).
fn todo_items(
    ui: &Rc<Ui>,
    courses: &HashMap<LocalId, st::Course>,
) -> Vec<(st::Assignment, st::Course)> {
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

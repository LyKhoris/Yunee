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

    // --- To Do ---
    ui.dashboard_page.append(&widgets::section("To Do"));
    let items = todo_items(ui);
    if items.is_empty() {
        ui.dashboard_page
            .append(&dim("Nothing due. You're all caught up."));
    } else {
        let list = gtk::ListBox::new();
        list.add_css_class("boxed-list");
        list.set_selection_mode(gtk::SelectionMode::None);
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
            row.connect_activated(move |_| ui2.show_course(course.clone()));
            list.append(&row);
        }
        ui.dashboard_page.append(&list);
    }

    // --- New announcements ---
    let unread: Vec<st::Announcement> = ui
        .state
        .store
        .list_all_announcements(20)
        .unwrap_or_default()
        .into_iter()
        .filter(|a| a.is_unread())
        .collect();
    if !unread.is_empty() {
        ui.dashboard_page
            .append(&widgets::section("New announcements"));
        for announcement in &unread {
            let course = courses.get(&announcement.course_id);
            let course_name = course.map(|c| c.name.clone()).unwrap_or_default();
            let row = widgets::announcement_row(announcement, &course_name);
            if let Some(course) = course {
                let button = gtk::Button::with_label("Mark read");
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
            ui.dashboard_page.append(&row);
        }
    }
}

fn dim(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.set_xalign(0.0);
    label.set_wrap(true);
    label.add_css_class("muted");
    label
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

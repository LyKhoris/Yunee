//! Live smoke test for the Canvas client.
//!
//! Exercises the real API against a real instance. Credentials come from the
//! environment and are never written anywhere:
//!
//! ```bash
//! CANVAS_URL=https://yourschool.instructure.com \
//! CANVAS_TOKEN=xxxx \
//!   cargo run -p yunee-canvas --example smoke
//! ```
//!
//! It is deliberately forgiving: a 404 on an endpoint that older or
//! self-hosted Canvas lacks is reported and skipped, not treated as failure.

use yunee_canvas::{CanvasClient, CanvasError};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let base = std::env::var("CANVAS_URL").map_err(|_| "set CANVAS_URL")?;
    let token = std::env::var("CANVAS_TOKEN").map_err(|_| "set CANVAS_TOKEN")?;

    let client = CanvasClient::new(&base, &token)?;
    println!("client base: {}", client.base_url());

    // 1. Prove the token.
    let me = client.verify_token().await?;
    println!(
        "verified as: {} <{}>",
        me.name.or(me.short_name).unwrap_or_default(),
        me.login_id.unwrap_or_default()
    );

    // 2. Courses.
    let courses = client.list_courses().await?;
    println!("\ncourses: {}", courses.len());
    for course in courses.iter().take(8) {
        println!(
            "  {}  {}  ({})  prof={}",
            course.id,
            course
                .course_code
                .clone()
                .or(course.name.clone())
                .unwrap_or_default(),
            course.workflow_state.clone().unwrap_or_default(),
            course
                .teachers
                .first()
                .and_then(|t| t.display_name.clone())
                .unwrap_or_default(),
        );
    }

    // 3. Per-course detail for the first course.
    if let Some(course) = courses.first() {
        println!("\n--- detail for course {} ---", course.id);

        match client.list_assignments(&course.id).await {
            Ok(list) => {
                println!("assignments: {}", list.len());
                for a in list.iter().take(5) {
                    let sub = a
                        .submission
                        .as_ref()
                        .and_then(|s| s.workflow_state.clone())
                        .unwrap_or_else(|| "?".into());
                    println!(
                        "  {}  {}  due={}  state={}  points={:?}",
                        a.id,
                        a.name.clone().unwrap_or_default(),
                        a.due_at.clone().unwrap_or_else(|| "-".into()),
                        sub,
                        a.points_possible
                    );
                }
            }
            Err(e) => println!("assignments: {e}"),
        }

        match client.list_announcements(&course.id).await {
            Ok(list) => println!("announcements: {}", list.len()),
            Err(e) => println!("announcements: {e}"),
        }
        match client.list_modules(&course.id).await {
            Ok(list) => {
                let items: usize = list.iter().map(|m| m.items.len()).sum();
                println!("modules: {} ({} items)", list.len(), items);
            }
            Err(e) => println!("modules: {e}"),
        }
        match client.list_folders(&course.id).await {
            Ok(list) => println!("folders: {}", list.len()),
            Err(e) => println!("folders: {e}"),
        }
        match client.list_files(&course.id).await {
            Ok(list) => println!("files: {}", list.len()),
            Err(e) => println!("files: {e}"),
        }
    }

    // 4. Cross-course endpoints (404-tolerant).
    report(
        "planner items",
        client.planner_items(None, None).await.map(|v| v.len()),
    );
    report("todo", client.todo().await.map(|v| v.len()));
    report(
        "missing submissions",
        client.missing_submissions().await.map(|v| v.len()),
    );
    report(
        "activity summary",
        client.activity_summary().await.map(|v| v.len()),
    );
    report(
        "unread conversations",
        client
            .unread_conversations()
            .await
            .map(|v| v.unread_count.unwrap_or(0) as usize),
    );

    println!("\nsmoke test complete.");
    Ok(())
}

fn report(label: &str, result: Result<usize, CanvasError>) {
    match result {
        Ok(n) => println!("{label}: {n}"),
        Err(e) => println!("{label}: skipped ({e})"),
    }
}

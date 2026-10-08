# Architecture

Yunee is a Cargo workspace of three crates plus a GTK4 + libadwaita application.
This document records the crate boundaries, how a sync flows end to end, what the
local store holds, and how async work crosses back into the GTK main loop.

## Crate boundaries

| Crate | Depends on | Owns |
|---|---|---|
| `yunee-canvas` | (nothing but HTTP/JSON crates) | The typed Canvas REST client: reads, student writes, pagination, throttling, error classification. |
| `yunee-store` | (SQLite and serde only) | The local SQLite schema, upserts, queries, read markers, and the FTS5 index. |
| `yunee` | `yunee-canvas`, `yunee-store` | The GTK window and pages, the sync engine, the Tokio runtime, secret storage, notifications, and XDG paths. |

The dependency arrows point inward. `yunee-canvas` and `yunee-store` are headless
and unit-tested; neither knows the other exists, and neither knows a UI exists. All
coordination — mapping wire types to local rows, deciding what to sync, showing
results — lives in the `yunee` crate. The application is built with plain gtk4-rs
and libadwaita, not Relm4.

## Sync flow

`yunee::sync::sync_all` is the whole engine. It runs off the UI thread and returns
a `SyncReport` (counts, per-target errors, and the new unread announcements it
observed). Nothing in the UI calls Canvas directly except best-effort writes.

1. **Start.** `sync_all(&store, &connection)` builds a `CanvasClient` from the
   saved server address and token. If the client cannot be built, the report
   carries the error and the run ends.
2. **Courses first.** `list_courses` pulls the student's active courses
   (`enrollment_type=student`, `enrollment_state=active`, with teachers, term,
   enrollments, and total scores included). Each course is upserted and its local
   id is used for everything below it. If the course list fails, the run ends —
   there is nothing to iterate.
3. **Per-course pull, with error isolation.** For each course, in sequence:
   - assignments (including the student's own submission via
     `include[]=submission`),
   - announcements (`discussion_topics?only_announcements=true`),
   - modules with their items (`include[]=items`),
   - folders and files.

   A failure in one course is captured as `"<course>: <error>"` and the loop moves
   on to the next course; one broken course never aborts the rest. Within a course,
   a missing endpoint (404) is tolerated quietly where it is optional — modules and
   files are skipped on older Canvas — while a genuine error is recorded.
4. **Planner.** After the courses, `planner_items` is pulled over a window from 14
   days back to 45 days ahead, and upserted by its stable `type:id:course` key.
5. **Commit the durable side.** `store.rebuild_search()` rebuilds the FTS index
   from the freshly upserted rows, and `record_sync("last", now)` stores the sync
   timestamp.
6. **Notification material.** Finally the engine scans the (up to 50 most recent)
   announcements for ones that are still unread and whose `posted_at` is newer
   than the previous sync timestamp. Those become `report.new_unread`, which the UI
   turns into a GNOME notification.

Error isolation is a deliberate property: the unit of failure is one course (or
the planner), never the whole sync. The UI reports the first error in a toast while
keeping every course that did succeed.

## Store schema overview

Schema is applied idempotently on every open (`CREATE ... IF NOT EXISTS`) in a WAL
database. Every entity carries the Canvas id it came from (`canvas_id`, unique) so
re-syncs upsert instead of duplicating, plus a local autoincrement `id` the app can
reference. There are no tenancy columns.

| Table | Purpose |
|---|---|
| `courses` | One row per enrolled course: name/code, title, term, professor, enrollment state, current/final score and grade. |
| `assignments` | Assignment fields plus the student's submission state (state, submitted/graded timestamps, score, grade, late/missing/excused). |
| `announcements` | Title, plain-text body and original HTML, posting time and author, Canvas read state, and a **local** `local_read_at` marker that survives re-sync. |
| `modules` | Course modules with position, unlock time, state, and completion time. |
| `module_items` | Items within a module, with type, content id, position, the raw completion requirement, and a completed flag. |
| `folders` | Course file folders, with parent id and full name. |
| `files` | Course files, with display name, type, size, and the token-bearing Canvas URL (never handed out as a bare link). `local_path` is set once downloaded. |
| `planner_items` | Planner entries keyed by `type:id:course`, with due date, points, completion and dismissal state. |
| `sync_state` | Small key/value bookkeeping, notably the last sync timestamp. |
| `settings` | Non-secret app settings, notably the Canvas base URL. |
| `search` | FTS5 virtual table (`unicode61`) over courses, assignments, announcements, and files. `kind` and `ref_id` are unindexed to ride along without polluting the match. |

A handful of indexes back the queries the sync and UI lean on:
`idx_assignments_course`, `idx_assignments_due`, `idx_announcements_course`,
`idx_module_items_module`, `idx_files_course`, `idx_folders_course`.

Full-text search is rebuilt wholesale after each sync rather than maintained with
per-row triggers — cheap at one student's scale, and it guarantees the index
matches the synced rows. User input is turned into quoted prefix terms so FTS
operators a user types are treated literally, never as syntax.

## Async and threading model

GTK's main loop is not async, so Yunee keeps the two worlds separate:

- **One Tokio runtime** for the whole process, created lazily on first use
  (`runtime::runtime()`): a multi-thread runtime with four worker threads, all
  named `yunee-net`.
- **Background work on a plain thread.** Sync, uploads, downloads, token
  verification, and best-effort Canvas writes each run on a `std::thread::spawn`
  thread that does `runtime().block_on(...)`. The GTK loop is never blocked on
  the network.
- **Results marshal back through `glib::MainContext::default().invoke(...)`.** The
  background thread hands its result to a closure that runs on the GTK main thread,
  where it is safe to touch widgets: reload the sidebar, dashboard, course, files,
  and settings pages, show a toast, or post a notification. The UI never touches
  widgets from a worker thread.
- **Shared store.** One `Store` lives behind an `Arc`, shared between the GTK main
  thread and any worker. The underlying `rusqlite::Connection` is `Send` but not
  `Sync`, so the store wraps it in a `Mutex`; a poisoned lock is recovered rather
  than cascading a panic into every later query.
- **Pages read locally.** The UI render functions only ever read from the store.
  Data freshness is a side effect of sync, not a precondition of drawing.

A sync therefore looks, from the user's side, like: press Sync (or start the app
with a connection configured) → a toast appears → later the pages refresh and,
if there are new unread announcements, a GNOME notification appears.

## Related documents

- `docs/canvas-sync.md` — what is synced, the three client fixes, the supported
  writes, and the token policy.
- `docs/pivot.md` — the historical decision log.
- `AGENTS.md` — the build authority.

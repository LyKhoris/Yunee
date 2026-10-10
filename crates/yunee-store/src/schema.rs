//! The schema, applied idempotently on every open.
//!
//! Yunee is single-user and local: there is no `user_id`, no tenant column, no
//! RLS. The only isolation is the filesystem. Migrations here are additive and
//! safe to re-run — `CREATE ... IF NOT EXISTS` plus the FTS index.

/// Ordered schema statements. Run inside a transaction on every open.
pub const SCHEMA: &[&str] = &[
    r#"
    CREATE TABLE IF NOT EXISTS courses (
        id               INTEGER PRIMARY KEY AUTOINCREMENT,
        canvas_id        TEXT NOT NULL UNIQUE,
        name             TEXT NOT NULL,
        title            TEXT NOT NULL,
        term             TEXT,
        professor        TEXT,
        enrollment_state TEXT,
        current_score    REAL,
        final_score      REAL,
        current_grade    TEXT,
        final_grade      TEXT,
        updated_at       TEXT NOT NULL
    )
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS assignments (
        id               INTEGER PRIMARY KEY AUTOINCREMENT,
        canvas_id        TEXT NOT NULL UNIQUE,
        course_id        INTEGER NOT NULL REFERENCES courses(id) ON DELETE CASCADE,
        name             TEXT NOT NULL,
        description      TEXT,
        due_at           TEXT,
        unlock_at        TEXT,
        lock_at          TEXT,
        points_possible  REAL,
        submission_types TEXT,
        html_url         TEXT,
        quiz_id          TEXT,
        published        INTEGER NOT NULL DEFAULT 1,
        submission_state TEXT,
        submitted_at     TEXT,
        graded_at        TEXT,
        score            REAL,
        grade            TEXT,
        late             INTEGER NOT NULL DEFAULT 0,
        missing          INTEGER NOT NULL DEFAULT 0,
        excused          INTEGER NOT NULL DEFAULT 0,
        updated_at       TEXT NOT NULL
    )
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS announcements (
        id             INTEGER PRIMARY KEY AUTOINCREMENT,
        canvas_id      TEXT NOT NULL UNIQUE,
        course_id      INTEGER NOT NULL REFERENCES courses(id) ON DELETE CASCADE,
        title          TEXT NOT NULL,
        body           TEXT NOT NULL,
        html           TEXT,
        posted_at      TEXT,
        author         TEXT,
        read_state     TEXT,
        unread_count   INTEGER NOT NULL DEFAULT 0,
        local_read_at  TEXT,
        updated_at     TEXT NOT NULL
    )
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS modules (
        id           INTEGER PRIMARY KEY AUTOINCREMENT,
        canvas_id    TEXT NOT NULL UNIQUE,
        course_id    INTEGER NOT NULL REFERENCES courses(id) ON DELETE CASCADE,
        name         TEXT NOT NULL,
        position     INTEGER,
        unlock_at    TEXT,
        state        TEXT,
        completed_at TEXT,
        updated_at   TEXT NOT NULL
    )
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS module_items (
        id                     INTEGER PRIMARY KEY AUTOINCREMENT,
        canvas_id              TEXT NOT NULL UNIQUE,
        module_id              INTEGER NOT NULL REFERENCES modules(id) ON DELETE CASCADE,
        course_id              INTEGER NOT NULL REFERENCES courses(id) ON DELETE CASCADE,
        title                  TEXT NOT NULL,
        item_type              TEXT,
        content_id             TEXT,
        html_url               TEXT,
        position               INTEGER,
        completion_requirement TEXT,
        completed              INTEGER NOT NULL DEFAULT 0,
        updated_at             TEXT NOT NULL
    )
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS folders (
        id         INTEGER PRIMARY KEY AUTOINCREMENT,
        canvas_id  TEXT NOT NULL UNIQUE,
        course_id  INTEGER NOT NULL REFERENCES courses(id) ON DELETE CASCADE,
        parent_id  TEXT,
        name       TEXT NOT NULL,
        full_name  TEXT,
        updated_at TEXT NOT NULL
    )
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS pages (
        id         INTEGER PRIMARY KEY AUTOINCREMENT,
        canvas_id  TEXT NOT NULL UNIQUE,
        course_id  INTEGER NOT NULL REFERENCES courses(id) ON DELETE CASCADE,
        page_id    TEXT,
        url        TEXT,
        title      TEXT NOT NULL,
        body       TEXT,
        updated_at TEXT NOT NULL
    )
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS files (
        id           INTEGER PRIMARY KEY AUTOINCREMENT,
        canvas_id    TEXT NOT NULL UNIQUE,
        course_id    INTEGER NOT NULL REFERENCES courses(id) ON DELETE CASCADE,
        folder_id    TEXT,
        display_name TEXT NOT NULL,
        filename     TEXT,
        content_type TEXT,
        size         INTEGER,
        url          TEXT,
        created_at   TEXT,
        updated_at   TEXT,
        local_path   TEXT
    )
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS sync_state (
        key        TEXT PRIMARY KEY,
        value      TEXT,
        updated_at TEXT NOT NULL
    )
    "#,
    r#"
    CREATE TABLE IF NOT EXISTS settings (
        key   TEXT PRIMARY KEY,
        value TEXT
    )
    "#,
    // Full-text index over everything a student might search. `ref_id` and
    // `kind` are UNINDEXED so they ride along without polluting the match.
    r#"
    CREATE VIRTUAL TABLE IF NOT EXISTS search USING fts5(
        kind UNINDEXED,
        ref_id UNINDEXED,
        title,
        body,
        tokenize = 'unicode61'
    )
    "#,
    // Indexes the sync and the UI lean on.
    "CREATE INDEX IF NOT EXISTS idx_assignments_course ON assignments (course_id)",
    "CREATE INDEX IF NOT EXISTS idx_assignments_due ON assignments (due_at)",
    "CREATE INDEX IF NOT EXISTS idx_announcements_course ON announcements (course_id)",
    "CREATE INDEX IF NOT EXISTS idx_module_items_module ON module_items (module_id)",
    "CREATE INDEX IF NOT EXISTS idx_files_course ON files (course_id)",
    "CREATE INDEX IF NOT EXISTS idx_folders_course ON folders (course_id)",
    "CREATE INDEX IF NOT EXISTS idx_pages_course ON pages (course_id)",
    // Retired: the planner was removed. Drop its table on installs that have
    // one, so nothing is left behind.
    "DROP TABLE IF EXISTS planner_items",
];

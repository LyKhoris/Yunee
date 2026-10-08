# Yunee

A better Canvas client for GNOME. Yunee is a local, single-user desktop app that
syncs one student's Canvas LMS world — courses, assignments with submission state,
announcements, modules, files, and the planner — into a local SQLite database, so
it is fast and readable offline. There are no accounts, no members, and no server:
whoever runs it on their machine has access, authenticated to Canvas with their
own personal access token. A study layer (recording → transcription → notes with
verified quotes → chat) is planned for a later milestone, not v1.

<!-- Screenshot: replace this comment with an image once the UI is stable. -->

## Build and run

Fedora + GNOME, GTK4 + libadwaita. Install the system libraries and Flatpak
tooling:

```bash
sudo dnf install gtk4-devel libadwaita-devel libsecret-devel \
    pkgconf-pkg-config cmake gcc gcc-c++ flatpak-builder
```

Rust comes from [rustup](https://rustup.rs/). Then:

```bash
cargo run -p yunee
```

The binary is named `yunee`; the app id is `io.github.LyKhoris.Yunee`. It is
packaged as a Flatpak.

### Checks

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
cargo build --workspace --release
```

## Features

### Canvas (v1)

- Courses with teachers, term, and grades.
- Assignments, including the student's submission state (submitted, graded, late,
  missing, excused).
- Announcements with read tracking.
- Modules and their items.
- Course folders and files, with download to disk.
- Planner / todo.
- Local full-text search over everything synced.
- Background sync with per-course error isolation.
- GNOME notifications for new announcements.

Student writes: assignment submission (`online_text_entry`, `online_url`, and
`online_file` via Canvas's 3-step upload flow), mark module item done/read, planner
overrides (complete / dismiss), and mark announcement read.

### Study layer (planned, not in v1)

Recording → transcription → notes with verified quotes → chat. Never invents
content; transcripts are durable while audio is temporary. This is the milestone
after the Canvas client is daily-usable.

## Documentation

- **Build authority:** [`AGENTS.md`](AGENTS.md)
- **What changed and why:** [`docs/pivot.md`](docs/pivot.md)
- **Architecture and crate boundaries:** [`docs/architecture.md`](docs/architecture.md)
- **Canvas sync and auth:** [`docs/canvas-sync.md`](docs/canvas-sync.md)

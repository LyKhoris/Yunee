# Yunee — build notes

**Yunee** is a local, single-user desktop application for one student's Canvas
world. It is a Canvas LMS client for GNOME — courses, assignments with submission
state, announcements, modules, files, and the planner — synced into a local SQLite
database, with a study layer (recording → transcription → notes with verified
quotes → chat) planned as a later milestone. It is not hosted, not shared, and not
sold. Whoever runs it on their machine has access; there is no account to create
and no one to authenticate against except Canvas itself.

This file is the authority for building Yunee. Update it whenever a decision
lands. If code or a plan contradicts this file, this file wins. The historical
decision log — including the two pivots that produced this shape — is in
`docs/pivot.md`; read it, do not mistake it for the current spec.

## What Yunee is now — and is not

It **is**:

- A GTK4 + libadwaita desktop app written in Rust, packaged as a Flatpak.
- A Canvas client first. Canvas is the spine; the study layer is additive.
- Local and offline-first: the UI reads from SQLite, the network is touched only
  by the sync engine.
- One user per install, on that person's own machine, with that person's own
  Canvas token.

It **is not**:

- A web app, a PWA, a SaaS, or anything with accounts, members, or invites.
- Multi-tenant. There is no `user_id`, no tenant column, no row-level security;
  the filesystem is the only isolation.
- Hosted by anyone. There is no server to run, no cron, no webhook, no push
  delivery.
- Billing, tiers, usage caps, or cost telemetry.
- A multi-user distribution. The Canvas API's terms would require OAuth for that;
  a personal access token is the correct and intended auth for a single-user local
  tool.

## Runtime shape & access — decided 2026-10-08

One local desktop app, one user, no auth layer of its own. The window opens onto a
`NavigationSplitView` shell: a sidebar of the dashboard and synced courses, and a
content stack holding the **Dashboard**, **Course**, **Files**, and **Settings**
pages. Pages are rebuilt from the local store whenever data changes, so the UI
never blocks on the network.

Access is simply running the app. Conversations that existed for the retired web
shape — members, `/admin`, invite tokens, session epochs, expiry, roles, payment
periods — are gone and must not return. They only ever made sense because strangers
reached a shared server.

## Product identity — decided 2026-10-08

**"A better Canvas client first."** The app's spine is Canvas: courses,
assignments with submission status, announcements with read tracking, modules and
items, folders and files, planner/todo, and grade views, synced to local SQLite
with FTS5 search.

The **study layer** — recording, transcription, notes with verified quotes, and
chat — is the milestone *after* the Canvas client is daily-usable. It is not in
v1. Foxi's algorithms for it (quote guard, quality gates, process-once, audio
hygiene) carry over as **behavioral contracts only**, never as code, and never
with a storage dependency on the old system.

## Architecture

A Cargo workspace with three crates. Each has one job, and the dependency arrows
point inward — the UI depends on the store and the Canvas client, and neither of
those depends on the UI.

### `crates/yunee-canvas` — typed Canvas REST client

Speaks only HTTP and JSON; it has no storage or UI dependency. It covers every
read Yunee needs (courses with teachers/term/grades, assignments including the
student's submission state, announcements, modules + items, folders + files,
wiki pages, and planner/todo), file downloads, and the student writes listed
below.

It deliberately fixes three weaknesses of the earlier integration:

1. **Pagination.** Every list endpoint follows the RFC 8288 `Link: rel="next"`
   header to the end, instead of silently truncating at one page.
2. **Throttling.** It watches `X-Rate-Limit-Remaining`, pauses briefly when the
   budget runs low, and backs off with `Retry-After`-aware delays on HTTP 403/429,
   retrying a bounded number of times.
3. **Feature detection.** HTTP 404 surfaces as `CanvasError::NotFound`, not a
   crash, so callers can degrade gracefully on older or self-hosted Canvas that
   lacks an endpoint.

ids are parsed tolerantly (Canvas may send numbers or strings) and normalized to
`String`. See `docs/canvas-sync.md` for the full contract.

### `crates/yunee-store` — local SQLite memory

The durable memory of one student: a single SQLite file (rusqlite, bundled, WAL)
holding courses, assignments, announcements, modules, module items, folders,
files, wiki pages, planner items, sync bookkeeping, settings, and an FTS5 index
over the searchable text. There is no tenancy of any kind.

Everything is headless and unit-tested. The UI and the sync engine are its only
callers. A `rusqlite::Connection` is `Send` but not `Sync`, so it lives behind a
`Mutex`; the app shares one `Store` (via `Arc`) between the GTK main thread and
background work. Upserts key on the Canvas id, so a re-sync updates rows instead
of duplicating them. See `docs/architecture.md` for the schema overview.

### `crates/yunee` — the GTK4 + libadwaita app

The application itself, built with plain gtk4-rs and libadwaita (no Relm4). It owns
the window and pages, the sync engine, the shared Tokio runtime, the secret store,
and desktop integration (GNOME notifications). This is the only crate that
depends on both of the others.

## Canvas integration & auth

**Auth is a Canvas personal access token**, created in Canvas under
**Account → Settings → New Access Token**. The token is verified against
`/users/self` *before* it is saved, so a typo or a wrong-instance token fails at
connect time rather than silently at the next sync. Tokens are domain-bound, so a
token for one Canvas instance will not authenticate against another.

**Policy note:** the Canvas API terms require OAuth for multi-user distribution.
Yunee is a single-user local tool, so a personal access token is the appropriate
mechanism; Yunee is not distributed as a multi-user client.

**v1 Canvas reads:** courses (with teachers, term, grades), assignments including
submission state, announcements, modules + items, folders + files, wiki pages,
planner/todo.

**v1 Canvas writes (student-only, whatever Canvas documents for students):**

- Assignment submission — `online_text_entry`, `online_url`, and `online_file`
  via the documented **3-step upload flow** (ask Canvas for an upload target, POST
  the file to it, then attach the returned file id to the submission). *Current
  status: the detail view lays out the text / URL / file controls and a Submit
  button, but the button is not wired to Canvas yet — layout only.*
- Mark a module item done / mark it read.
- Planner overrides (mark complete / dismiss).
- Mark an announcement read.

## Hard rules that survive

These grew up around the recording/study layer and still bind it when it lands.
They do not license inventing behavior in the Canvas client either.

1. **Never invent content.** Quotes, names, page numbers, and dates come from the
   transcript only — never synthesized, never approximated.
2. **Trustworthy transcripts.** Surface a thin or suspect recording; offer a
   re-upload path. Never generate notes from a transcript known to be unreliable
   without saying so.
3. **Audio is temporary, the transcript is durable.** Delete audio once its
   transcript (and note) are committed.
4. **Never lose a recording.** Buffer locally where recording happens locally.
5. **Real deletion.** Delete means delete.

*(Open: whether the consent / recording-law acknowledgment, framed for a public
product recording others' lectures, still applies for a local single-user tool.
See `docs/pivot.md`.)*

## Data locations & secrets

Single-user, XDG, no hidden cloud.

| What | Where |
|---|---|
| Database | `$XDG_DATA_HOME/yunee/yunee.db`, else `~/.local/share/yunee/yunee.db` |
| Downloaded files | `$XDG_DATA_HOME/yunee/files/`, else `~/.local/share/yunee/files/` |
| Canvas token | GNOME keyring via libsecret; fallback `~/.local/share/yunee/canvas-token` (mode `0600`) |

Keyring is the primary home. If the Secret Service is unreachable (no D-Bus, a
locked keyring, a headless session), Yunee falls back to the `0600` file so the
app stays usable, and says so. The server address is a non-secret setting; the
token is never written to the database.

## Distribution & updates — decided 2026-10-09

Yunee ships from a **signed Flatpak repository** hosted on GitHub Pages at
`https://lykhoris.github.io/Yunee/`, built and published by
`.github/workflows/release.yml` on every `v*` tag. The source repo is public,
because free GitHub Pages requires it.

Install once from the `.flatpakref`, then update normally:

```
flatpak install --from https://lykhoris.github.io/Yunee/io.github.LyKhoris.Yunee.flatpakref
flatpak update
```

GNOME Software's Updates page shows the app and offers the update button, because
the app is installed from a remote that publishes newer metadata.

**Why not bundles alone.** A `.flatpak` bundle is an installer, not an update
channel: GNOME Software shows an installed app as installed and never compares
versions, and `flatpak install` compares commits, not versions (it will silently
downgrade). A standalone bundle is still attached to each release as an offline
fallback, but it never updates in place. This supersedes the 2026-10-08
bundle-only decision in commit `615da98`.

**Signing.** The repo is signed with a GPG key: the secret half is the
`FLATPAK_GPG_KEY` Actions secret, the public half is embedded in the
`.flatpakref`. Losing the secret means every user must re-add the remote. It has
no expiry.

The release workflow stamps the pushed tag into the workspace version
(`Cargo.toml`) and the AppStream `<release>`, so every release is a distinct
version in both the Settings row (`CARGO_PKG_VERSION`) and `flatpak info`.

**Rejected:** RPM via COPR — public source would be acceptable, but Fedora's Rust
packaging wants ~200 crates as separate RPMs, it would not reuse the Flatpak
build, and it buys nothing the repo does not already do. Also rejected:
self-hosting the repo on the Umbrel, and Flathub.

## Build / run / check

Fedora workstation, GNOME. System libraries plus Flatpak tooling:

```bash
sudo dnf install gtk4-devel libadwaita-devel libsecret-devel \
    pkgconf-pkg-config cmake gcc gcc-c++ flatpak-builder
```

Rust via rustup (workspace is edition 2024, `rust-version = "1.85"`).

```bash
cargo run -p yunee            # build and launch the app
cargo test --workspace        # unit tests across all three crates
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
cargo build --workspace --release
```

CI builds and tests inside a Fedora container, because Yunee links GTK 4.22 and
libadwaita 1.9, which are newer than what Ubuntu's repositories carry.

**Development runs beside an installed Flatpak.** A dev build can run without
fighting the installed app for the single-instance id, reuse an existing profile
with no duplication, and borrow a Canvas token for the session without saving it:

```bash
YUNEE_APP_ID=io.github.LyKhoris.Yunee.Dev \
XDG_DATA_HOME=~/.var/app/io.github.LyKhoris.Yunee/data \
CANVAS_URL=https://school.instructure.com CANVAS_TOKEN=… \
    cargo run -p yunee
```

`YUNEE_DATA_DIR`/`XDG_DATA_HOME` point at the profile; `CANVAS_URL` +
`CANVAS_TOKEN` override the saved token (handled in `AppState::connection`, never
written to disk). `yunee --sync-once` uses the same two variables for a headless
sync. The installed Flatpak keeps its token in its own sandboxed keyring, which a
host process cannot read.

## What changed — retired web / SaaS assumptions, do not reintroduce

The 2026-10-08 pivot deleted the web code outright; nothing from it is reused. The
following existed only because Yunee was once a hosted multi-tenant service, and
must not leak back into the design:

- Multi-tenancy, `user_id` scoping, RLS or any tenancy column as an isolation
  boundary. The filesystem is the isolation.
- An always-on hosted server: cron jobs, webhooks, push delivery, sync endpoints.
- Billing, tiers, monthly usage caps, cost telemetry, payment periods.
- Email, email verification, password resets, or any auth provider.
- Invite links, member accounts, sessions, expiry, roles, and admin surfaces.
- Public-product identity: a store application id for a store listing, an OAuth
  URL scheme, a custom domain, provider redirect allowlists, terms/privacy pages.
- Next.js, TypeScript, Turso / libSQL Cloud, Vercel, and the `.env`-based
  deployment config. Local SQLite replaces the managed store entirely.

The app id `io.github.LyKhoris.Yunee` and the binary name `yunee` are current; see
Open questions for the parts still provisional.

## Working rules

- **Stage explicit paths. Never `git add -A`.**
- **Commit small and promptly.**
- **Secrets never enter git.** The Canvas token lives in the keyring or a `0600`
  file, both outside the tree. `*.db`, `*.db-wal`, and `canvas-token` are
  gitignored; keep it that way.
- **Write the decision down before building it.** Decisions go in this file or
  `docs/`, then become code.
- **Verify by running the app** on Fedora + GNOME for any user-facing surface.

## Open questions

1. **App id and name.** `io.github.LyKhoris.Yunee` and "Yunee" are the working
   identity, tied to the GitHub account. Both are provisional until the founder
   locks a name; the repo, Flatpak manifest, and desktop files follow the
   decision.
2. **Study-layer intake.** How a recording arrives — in-app capture, a file drop,
   or syncing from an external recorder such as Voicenotes — is undecided. It
   only matters once the Canvas client is daily-usable.
3. **Background sync cadence.** Startup and manual sync exist. Whether an
   interval timer or a smarter trigger is added, and how notifications respect it,
   is open.

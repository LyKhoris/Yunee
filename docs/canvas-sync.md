# Canvas sync

How Yunee talks to Canvas: what it pulls, how it pulls it, what it writes back,
and why a personal access token is the right credential for this app.

## What is synced

A sync pulls the student's own account for the courses they are actively enrolled
in, then everything under each course.

| Target | Canvas endpoint | Notes |
|---|---|---|
| Courses | `GET /courses` | `enrollment_type=student`, `enrollment_state=active`, `state[]=available`; includes teachers, term, enrollments, total scores. |
| Assignments | `GET /courses/:id/assignments` | `order_by=due_at`; includes the caller's `submission` and `can_submit`. |
| Announcements | `GET /courses/:id/discussion_topics` | `only_announcements=true`. |
| Modules + items | `GET /courses/:id/modules` | `include[]=items`. |
| Folders | `GET /courses/:id/folders` | |
| Files | `GET /courses/:id/files` | `sort=updated_at&order=desc`. |
| Wiki pages | `GET /courses/:id/pages` + `.../pages/:url` | The index gives each page's title and slug; its HTML `body` comes from the page's own show request, fetched only when missing or changed. A course can **disable the Pages index** (404 `"That page has been disabled for this course"`) while individual pages still load by slug, so such pages are fetched on demand when opened. |
| File downloads | the URL in a file record | Fetched with the bearer token attached. |

Reads also available to the client (used for verification and future surfaces,
not all stored today): `GET /users/self`, `/users/self/todo`,
`/users/self/missing_submissions`, `/users/self/activity_stream/summary`,
`/conversations/unread_count`.

Each complete successful list atomically upserts by Canvas id and removes
cached rows absent from that list, scoped to the course or module. Empty lists
clear that scope; failed requests, incomplete pagination, and optional 404s
preserve it. Existing local read markers and saved file paths survive upserts.
Module items use the paginated items endpoint when Canvas omits or truncates the
inline list (see [Canvas modules documentation](https://canvas.instructure.com/doc/api/modules.html)).
Files refresh on opening their tab when five minutes old and immediately
through its refresh/retry button, while cached rows remain visible. They are still excluded from the full course sweep.
A partial sync does not advance the last successful timestamp; Settings displays
the latest attempt and outcome separately. A course failing does not abort the others; its error is
collected and reported while the rest of the sync proceeds. See
`docs/architecture.md` for the full flow.

## The three fixes vs a naive Canvas client

The earlier integration (Foxi) was GET-only and pagination-blind. Yunee's client
fixes three things that made that one fragile.

### 1. Pagination

Every Canvas list endpoint paginates and advertises the next page in the `Link`
response header (RFC 8288), for example:

```
<https://school.instructure.com/api/v1/courses?page=2>; rel="next",
<...?page=1>; rel="current",
<...?page=5>; rel="last"
```

A naive client reads one page and silently truncates. Yunee parses the `rel="next"`
URL and keeps requesting until there is no next page, so a course with more than
one page of assignments or files is fully synced.

### 2. Throttling

Canvas returns rate-limit information in `X-Rate-Limit-Remaining`. Yunee:

- shares one serial request queue, HTTP client and cooldown across sync,
  page loads, downloads and read markers;
- watches `X-Rate-Limit-Remaining` and `X-Request-Cost` on responses;
- increases pacing near a reserve sized using observed request cost;
  this is a conservative heuristic, not an assumption about an instance's refill
  rate;
- retries HTTP 429 and explicitly identified rate-limit 403s with bounded
  exponential backoff, honoring the full `Retry-After` duration (seconds or an
  HTTP date). Permission-denied 403s fail immediately. A final throttled attempt
  leaves a cooldown in place for later queued jobs.

Canvas documents additional penalties for parallel requests; serial requests
are unlikely to be throttled under its default quota behavior. See
[Canvas throttling](https://developerdocs.instructure.com/services/canvas/basics/file.throttling).
The queue reduces this risk without promising that throttling cannot occur.

Automatic startup refresh also skips fresh complete snapshots: courses,
assignments and announcements use five minutes, modules/page indexes/folders
thirty minutes. Files remain on demand with a five-minute window. Explicit
refresh bypasses freshness, while still obeying queueing and cooldowns. Missing
optional endpoints are checked again after thirty minutes. Failed requests do
not mark targets fresh. Every actual refresh still fetches complete paginated
lists, so deletion reconciliation remains safe.

### 3. Feature detection (404 → `NotFound`)

Older and self-hosted Canvas instances do not implement every endpoint. Yunee maps
HTTP 404 to a distinct `CanvasError::NotFound` instead of a generic failure, so
callers can decide to skip quietly. The sync engine already does this for modules
and files: if the endpoint is absent, that section is skipped and the rest of the
sync is untouched. HTTP 401 maps to `Unauthorized` (a dead or wrong-instance
token), which the UI turns into a "reconnect" prompt.

Ids are also parsed tolerantly: Canvas may return numbers or strings, and Yunee's
deserializers accept either and normalize to `String`, so a number/string
difference never breaks a sync.

## Supported student writes

Yunee only performs writes Canvas documents for students on their own account.

> **Not wired up yet.** The assignment detail view lays out the submission
> controls (text entry, website URL, file upload) and a Submit button, but the
> button does not call Canvas — it is the layout, not the action. The client
> methods below exist and are exercised by nothing in the UI yet.

- **Assignment submission**, in all three documented forms:
  - `online_text_entry` — submit a text body;
  - `online_url` — submit a URL;
  - `online_file` — Canvas's **3-step upload flow**: (1) `POST
    /courses/:id/assignments/:id/submissions/self/files` to get an upload target,
    (2) POST the file to that (usually S3) target, (3) attach the returned file id
    via `submission[file_ids][]` on the submission endpoint.
- **Module completion** — mark a module item done (`PUT
  .../modules/:module_id/items/:item_id/done`) or satisfy a must-view requirement
  (`POST .../mark_read`), according to the item's completion requirement.
- **Announcement read** — `PUT
  /courses/:id/discussion_topics/:id/read`. Marking read locally happens first and
  is best-effort mirrored to Canvas; the local marker survives re-sync even if the
  network call does not land.

Read markers are kept locally so an announcement stays read across syncs regardless
of what Canvas reports, and so the app is coherent offline.

## Personal access token policy

Yunee authenticates with a **Canvas personal access token**, created by the user
in Canvas under **Account → Settings → New Access Token**. The Connect dialog
states this and sends the token nowhere except the user's own Canvas server.

- The token is **verified against `GET /users/self` before it is saved**, so a typo
  or a token minted for a different instance fails at connect time instead of
  failing silently at the next sync.
- Tokens are **domain-bound**: a token for one Canvas instance does not
  authenticate against another. The server address is stored as a plain setting;
  the token is stored in the GNOME keyring via libsecret, falling back to a
  `0600` file if the Secret Service is unreachable.
- **Policy note.** The Canvas API terms require OAuth for multi-user
  distribution. Yunee is a single-user local tool, so a personal access token is
  the appropriate mechanism, and Yunee is not distributed as a multi-user client.
  If that ever changed, OAuth would be required — it is not an oversight here, it
  is the documented constraint.

Disconnecting forgets the token: it is cleared from the keyring and the fallback
file, and the stored server address is emptied.

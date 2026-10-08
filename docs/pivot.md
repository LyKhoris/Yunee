# Yunee — pivot one-pager

Status: working draft. The "What this is" decision is deliberately open — it is the
output of the ideas conversation, not an input to it.

## What changed

| | Before (Foxi) | After (Yunee) |
|---|---|---|
| Business | Public subscription SaaS | Personal + a few friends |
| Tenancy | Multi-tenant (many strangers) | Single-user per install |
| Infra | Foxi-operated cloud (Supabase, Deepgram, AI Gateway, Vercel) | Each user brings their own accounts / machine |
| Intake | In-app recorder + upload (Canvas-gated signup) | The user's own recording app (e.g. Voicenotes), upload, etc. |
| Who pays | Foxi (subscription revenue) | The user (their own keys / quota) |
| Distribution | Public: store, domain, terms/privacy | Private, no public-product identity |

## Why we're pivoting

- (TODO — founder: the one or two sentences that actually drove this. Keep it
  short; it anchors every later decision.)

## What carries over — the moat is already decoupled

In the old code the transformation core has **no storage dependency**, so it moves
nearly verbatim:

`transcribe.ts` → `chunking.ts` → `notes.ts` → `quotes.ts` (never invent content) →
`chat.ts` / `model.ts`, plus read-only `canvas.ts`.

Only two config values tie it to a provider (`DEEPGRAM_API_KEY`,
`AI_GATEWAY_API_KEY`) — and those become the *user's* keys.

## Keep / Cut / Rewrite

| Bucket | Pieces | Note |
|---|---|---|
| **Keep** | `transcribe`, `chunking`, `notes`, `quotes`, `chat`, `model`, `canvas`, `consent`, `deadline-stage`, `week`, `timezone`, `format` | pure domain; swap provider config |
| **Rewrite** | `pipeline`, `retrieval`, `study`, `digest`, `notifications`, `profile`, `canvas-sync`, `usage` + all 14 migrations | same logic, new (local) storage |
| **Cut** | Supabase auth + RLS + `dal.ts` tenancy, `shares` + share page, billing/tiers/usage caps/cost guards, metrics/operator gate, terms/privacy, Vercel/cron, OAuth providers | existed only because it was a hosted multi-tenant service |
| **Rework** | the web UI (`src/app/app/*`) | design + components reusable; every server action changes |
| **Reference only** | `docs/data-model.md`, `docs/design-language.md`, `docs/ui.md`, `docs/competitors.md` | institutional memory, read from Foxi |

## Discarded assumptions — do not leak into the new design

- Tenant scoping (`user_id` everywhere, RLS as the isolation boundary).
- Yunee pays for transcription and model calls.
- An always-on hosted server with cron jobs, webhooks, push delivery.
- Billing, tiers, monthly usage caps, cost telemetry.
- Public-product identity: store application id, an OAuth URL scheme, a custom
  domain, provider redirect allowlists, terms & privacy pages.

## Non-negotiables that survive

- **Never invent content** — quotes/names/dates come from the transcript only.
- **Trustworthy transcripts** — surface thin ones; offer a re-upload path.
- **Audio temporary, transcript durable** — delete audio once the transcript (and
  note) are committed.
- **Never lose a recording** — local buffering where recording happens locally.
- **Real deletion** — delete means delete.
- *(Revisit: consent / recording-law acknowledgment — framed for a public product
  recording others' lectures; may still apply for friends' use.)*

## Open questions — the ideas conversation

1. **Runtime shape** — a per-machine local app / CLI / MCP, or one small shared
   instance for the founder + friends? *(This decides the whole architecture.)*
2. **Storage** — notes as local markdown/JSON, or each user's own cloud bucket?
3. **Intake** — Voicenotes sync (there is one already in `.hermes/`), in-app
   recording, upload, or several? What do friends without a sync script do?
4. **Whose keys, and where** — `.env` file, config file, or OS keychain? Whose
   Deepgram / LLM account pays, and with what guardrails?
5. **Identity** — is there any sign-in, or does possession of the machine/repo
   imply the data?
6. **Name** — "Yunee" is provisional; repo, package id, and scheme all wait on the
   real name.

## Decision log

- **2026-10-02** — Pivot confirmed. Audience = founder + personal friends. New repo
  (`Yunee`), seeded from Foxi's pure core. Standalone project (not inside Hermes).
- **2026-10-02** — Foxi frozen: tag `pre-pivot-saas`, GitHub repo archived.
- **2026-10-02** — `Yunee` named as the working codename; repo created.
- **2026-10-02** — Foxi's provider keys revoked: Deepgram API key, Vercel AI Gateway
  key, Supabase `foxi` publishable key, the Foxi secret key, and the Foxi personal
  access token. The Supabase project itself is **kept** (data intact); its
  auto-generated `default` publishable key and legacy JWT keys were left in place.
- **2026-10-02** — Runtime shape decided: one small instance the founder operates,
  web/PWA, invite-only. Access **in-house** (member tokens + `access_expires_at`),
  not Cloudflare Access. Payment off-system for now (Stripe deferred). Stack:
  Next.js + TypeScript + SQLite (`node:sqlite`). This resolves open questions 1, 2,
  4, and 5 above; intake (3) and the name (6) remain open.
- **2026-10-02** — **Accounts replace bearer invite links.** The invite link is now
  a *one-time* bootstrap: opening `/i/<token>` lets the member choose a password,
  then the link is spent. Sign-in afterwards is username + password at `/login`, so
  a member can use any device without asking for a new link. Usernames (lowercased,
  founder-assigned) were chosen over email because nothing here sends email, and
  email would imply resets/verification Yunee does not have. Resets are manual
  (`npm run members -- reset <id>`). This supersedes the earlier "no passwords"
  stance in `AGENTS.md`; sessions now carry a `session_epoch` so a revoke or reset
  signs every device out. Revisits open question 5 (identity); intake (3) and the
  name (6) remain open.
- **2026-10-02** — **Roles: admin vs member, and member management moves to the web.**
  `members.is_admin` is a plain flag (like `active`); the founder is the admin and
  friends are members. Everything the CLI could do — invite, extend, revoke, enable,
  promote, reset, delete — now lives at `/admin` in the browser, so the founder does
  not need a terminal outside of bootstrapping. The CLI keeps only the one-time
  bootstrap (`promote`) and remains for emergencies. Chosen as a boolean rather than
  a roles table because the role set is exactly two and stable, and because the row
  already carries `active` in the same shape. The last admin can never be revoked,
  demoted, or deleted (enforced in both the UI and the data layer) — otherwise one
  click could lock the operator out permanently. Admin pages and actions re-check
  admin server-side from the row on every request, so a demotion takes effect
  immediately, exactly like revocation.
- **2026-10-08 — The pivot pivots again: Yunee is a local desktop app, not a web
  SaaS.** It makes no money and hosts nobody. One app, one user, running on the
  founder's own machine: **Fedora + GNOME**, built **GTK4 + libadwaita in Rust
  (Relm4, tokio, rusqlite)**, packaged as a **Flatpak**. No accounts, no members,
  no sessions, no expiry, no web app — all of it is deleted, not kept anywhere.
  All of Yunee's 2026-10-02 access decisions (members, `/admin`, invite tokens,
  epochs, Turso) are thereby retired; this entry wins for anything else written
  on 2026-10-08.
- **2026-10-08 — Product identity: "a better Canvas client first".** The app's
  spine is a Canvas LMS client — courses, assignments with submission status,
  announcements with read tracking, planner/todo, grade view, course files —
  synced to a local SQLite store (FTS5 search), with background incremental sync
  and OS notifications. Foxi's Canvas layer was deliberately GET-only and
  pagination-blind; Yunee ships **student writes in v1** (assignment submission
  via the 3-step upload flow, mark module done/read, planner overrides).
  Keyboard access tokens (not OAuth) are correct for single-user use. The
  recording / transcription / notes-with-verified-quotes / chat layer is the
  milestone after the Canvas client is daily-usable; its algorithms (quote guard,
  quality gates, process-once, audio hygiene) carry over as behavioral
  contracts **only**, never as code.
- **2026-10-08 — Build environment**: development happens on the founder's Fedora
  workstation (needs `gtk4-devel`, `libadwaita-devel`, `pkgconf-pkg-config`,
  `cmake`, `gcc`; Rust via rustup). The umbrel sandbox cannot install GTK or a
  toolchain and is not a build target.
- **2026-10-08 — Open:** app id beyond the codename (`io.github.LyKhoris.Yunee`
  provisional); name; UI identity (default: stock libadwaita, no Foxi mascot).

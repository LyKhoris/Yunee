# Yunee — build notes

**Yunee** is the private successor to Foxi. Where Foxi was a public subscription
SaaS operating its own cloud, **Yunee is a personal tool for the founder and a few
friends**, where each person brings their own machine and their own accounts. The
name is a working codename and is provisional until the founder locks it.

**Status: just created (2026-10-02).** The runtime shape and the access model are
decided — see "Runtime shape and access" below. This file is the authority for
building Yunee; update it whenever a decision lands. If code or a plan contradicts
this file, this file wins.

## Provenance

Yunee is seeded from **Foxi**, which is frozen and archived:

- Repo: `LyKhoris/Foxi` (private, archived — read-only)
- Tag: `pre-pivot-saas` at the final committed state

The old repo stays as the reference for the transformation pipeline, the course
data model, and the design system. Read it, don't build on it.

## What changed (full detail in `docs/pivot.md`)

| | Foxi (before) | Yunee (now) |
|---|---|---|
| Business | Public subscription SaaS | Founder + a few friends |
| Tenancy | Multi-tenant | Single-user per install |
| Infra | Foxi-operated cloud | Each user's own machine / accounts |
| Intake | In-app recorder + upload | The user's own recording app, upload, etc. |
| Who pays | Foxi (subscription) | The user (their own keys / quota) |
| Identity | Store id, domain, terms/privacy | Private, no public-product identity |

## Runtime shape and access — decided 2026-10-02

Yunee is **one small instance the founder operates**, for himself and a few
friends. It is a **web app / PWA** (no native app for now), and it is
**invite-only**.

**Access is in-house.** No third-party identity provider, no Cloudflare Access. A
member is a row in a table, with a username and a password:

- The founder creates a member with a **username**
  (`npm run members -- add Ada --username ada`); a secret **invite token** becomes a
  one-time link (`/i/<token>`).
- Opening the invite link once lets the member **choose a password**; the link is
  then spent. From any device afterwards they sign in at `/login` with
  username + password.
- No email, no email verification, no self-signup. **Password resets are manual**:
  the founder runs `npm run members -- reset <id>` to print a fresh one-time link
  (and clear the old password). Nothing to reset by email because nothing sends
  email.
- Sessions are signed, expiring cookies. Each carries the member's `session_epoch`,
  and a **revoke or reset bumps that epoch**, so devices already signed in are cut
  off at once.
- **Two roles: admin and member.** `is_admin` is a plain flag on the row. The
  founder is the admin; friends are members. Admin surfaces live under `/admin` and
  are gated **server-side** on every page and every action — a demoted admin loses
  them on the next request, the same instant re-check that makes revocation exact.
- **Member management is a web UI, not a CLI.** `/admin` lists the roster and does
  everything: invite someone, extend a period, revoke/enable, promote/demote,
  reset a link, delete. The CLI is only for the **one-time bootstrap** — promoting
  the very first admin (`npm run members -- promote <id>`) — and for changes the UI
  cannot reach. Nothing else should require a terminal.
- **The last admin is protected.** Revoking, demoting, or deleting the only admin
  is refused (in the UI and the CLI alike) rather than allowed quietly; otherwise a
  single click could lock the operator out with no way back in.
- **Payment is off-system.** A payment buys a *period*, recorded as
  `access_expires_at`. Not renewing lets access lapse on its own. No payment
  processor yet — revisit Stripe only when the friend count makes manual renewal
  annoying.
- **Revocation** is setting the member inactive (or past expiry), checked
  server-side on every request, so it is instant and exact.
- **The founder pays** for transcription and model calls, covered by the fee.
- **Cloudflare Tunnel / Tailscale** may be added later purely as a *deployment*
  layer, to expose a home host safely. It is not part of access control.

**Stack (current choice):** Next.js (App Router) + TypeScript, with **Turso**
(libSQL / SQLite) as the store. Development uses a local SQLite file
(`file:data/yunee.db`); production points the same client at Turso Cloud via
`TURSO_DATABASE_URL` + `TURSO_AUTH_TOKEN`. One SQL dialect, no managed Postgres.

**Why Turso and not Supabase:** Yunee stores no files and needs no auth provider,
realtime, or RLS — it uses none of the bundle Supabase charges for. Turso's free
tier is always-on (no inactivity pause), includes 5 GB and 1-day point-in-time
restore, and keeps SQLite's model. Supabase's free tier pauses after 7 days of
inactivity and has **no backups**, which fails the "no file loss" requirement; its
only fixes are the parts of Pro ($25/mo) Yunee would never use.

**Backups are still on the founder.** Turso's 1-day restore window is not a
complete answer to "no file loss" — the durable safety net is a periodic dump the
founder owns (see `docs/` if it exists, otherwise a nightly export task).

**Not yet decided:** intake for friends (see Open questions); the pipeline itself.

**Live (2026-10-02):** deployed at `https://yunee.vercel.app`.
- App process: Vercel (Hobby plan — *non-commercial terms; Yunee charges, accepted
  at this scale, upgrade to Pro if it grows*).
- Database: Turso Cloud, `libsql://yunee-lykhoris.aws-us-west-2.turso.io`
  (primary `aws-us-west-2`). Nothing of the founder's is always-on.
- Env vars in Vercel (production): `SESSION_SECRET`, `APP_URL`,
  `TURSO_DATABASE_URL`, `TURSO_AUTH_TOKEN`.
- Commands: `npm run members -- …` for admin; `vercel deploy --prod` to ship;
  `turso db shell yunee` to inspect.

## What carries over — the moat is already decoupled

The transformation core has **no storage dependency** in the old code and moves
nearly verbatim:

`transcribe` → `chunking` → `notes` → `quotes` (never invent content) → `chat` /
`model`, plus read-only `canvas`.

Only two provider values tie it to a vendor (`DEEPGRAM_API_KEY`,
`AI_GATEWAY_API_KEY`) — and those become the **operator's** keys (the founder's,
covered by the fee).

## Discarded assumptions — do not reintroduce

- Multi-tenancy, `user_id` scoping, RLS as the isolation boundary.
- Yunee pays for transcription or model calls.
- An always-on hosted server: cron jobs, webhooks, push delivery.
- Billing, tiers, monthly usage caps, cost telemetry.
- Public-product identity: store application id, an OAuth URL scheme, a custom
  domain, provider redirect allowlists, terms/privacy pages.

## Hard rules that survive

1. **Never invent content.** Quotes, names, page numbers, and dates come from the
   transcript only.
2. **Trustworthy transcripts.** Surface a thin suspect recording; offer a re-upload
   path. Never generate notes from a transcript known to be unreliable without
   saying so.
3. **Audio is temporary, the transcript is durable.** Delete audio once its
   transcript (and note) are committed.
4. **Never lose a recording.** Local buffering where recording happens locally.
5. **Real deletion.** Delete means delete.

*(Open: whether the consent / recording-law acknowledgment, framed for a public
product recording others' lectures, still applies for the founder and friends. See
`docs/pivot.md`.)*

## Open questions

Resolved 2026-10-02: runtime shape, identity/access, storage, and who pays — see
"Runtime shape and access" above. Hosting is resolved too (Vercel + Turso).

Still open:

1. **Intake** — the founder uses Voicenotes (a sync script already exists in
   `.hermes/`). Do friends upload, record in-app, or bring their own app?
2. **Backups** — Turso's 1-day point-in-time restore is not a complete answer to
   "no file loss"; a periodic dump the founder owns is still owed.
3. **Payment automation** — manual periods for now; Stripe when the friend count
   makes it worth it.
4. **Name** — "Yunee" is provisional; repo, package id, and any scheme wait on the
   real name.

## Working rules

- **Stage explicit paths. Never `git add -A`.**
- **Commit small and promptly.**
- **Secrets never enter git.** `.env.local` (gitignored) only.
- **Write the decision down before building it.** Decisions go in this file or
  `docs/`, then become code.
- **Verify in the browser** for any user-facing surface, using BrowserOS.

<!-- BEGIN:nextjs-agent-rules -->

# This is NOT the Next.js you know

This version has breaking changes — APIs, conventions, and file structure may all differ from your training data. Read the relevant guide in `node_modules/next/dist/docs/` (resolved from this file's directory; in monorepos the `next` package may not be visible from the repo root) before writing any code. Heed deprecation notices.

This block is written and re-added by `next dev` — verify at `node_modules/next/dist/server/lib/generate-agent-files.js`. Removing it from a diff only re-creates the uncommitted change; committing it with your work keeps the tree clean.

<!-- END:nextjs-agent-rules -->

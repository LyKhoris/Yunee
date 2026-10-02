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
member is a row in a table, not a password account:

- No passwords, no email verification, no self-signup, nothing to reset.
- The founder creates a member; that member's secret **token** becomes an
  invitation link (`/i/<token>`). Opening it once sets a signed, long-lived session
  cookie.
- **Payment is off-system.** A payment buys a *period*, recorded as
  `access_expires_at`. Not renewing lets access lapse on its own. No payment
  processor yet — revisit Stripe only when the friend count makes manual renewal
  annoying.
- **Revocation** is setting the member inactive (or past expiry), checked
  server-side on every request, so it is instant and exact.
- **The founder pays** for transcription and model calls, covered by the fee.
- **Cloudflare Tunnel / Tailscale** may be added later purely as a *deployment*
  layer, to expose a home host safely. It is not part of access control.

**Stack (current choice):** Next.js (App Router) + TypeScript, with **SQLite**
(`node:sqlite`) as the store — one process, one file, no managed cloud.

**Not yet decided:** where the instance runs (the founder's Linux machine, the
Umbrel home server, or a small VPS), and intake for friends (see Open questions).

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
"Runtime shape and access" above.

Still open:

1. **Intake** — the founder uses Voicenotes (a sync script already exists in
   `.hermes/`). Do friends upload, record in-app, or bring their own app?
2. **Where the one instance runs** — the founder's Linux machine, the Umbrel home
   server, or a small VPS.
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

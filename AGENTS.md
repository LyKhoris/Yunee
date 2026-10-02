# Yunee — build notes

**Yunee** is the private successor to Foxi. Where Foxi was a public subscription
SaaS operating its own cloud, **Yunee is a personal tool for the founder and a few
friends**, where each person brings their own machine and their own accounts. The
name is a working codename and is provisional until the founder locks it.

**Status: just created (2026-10-02).** The runtime shape is not decided yet — see
"Open questions". This file is the authority for building Yunee; update it whenever
a decision lands. If code or a plan contradicts this file, this file wins.

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

## What carries over — the moat is already decoupled

The transformation core has **no storage dependency** in the old code and moves
nearly verbatim:

`transcribe` → `chunking` → `notes` → `quotes` (never invent content) → `chat` /
`model`, plus read-only `canvas`.

Only two provider values tie it to a vendor (`DEEPGRAM_API_KEY`,
`AI_GATEWAY_API_KEY`) — and those become the **user's** keys.

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

## Open questions — the decisions that shape everything

1. **Runtime shape** — a per-machine local app / CLI / MCP, or one small shared
   instance for the founder + friends? *(This decides the architecture.)*
2. **Storage** — notes as local markdown/JSON, or each user's own cloud bucket?
3. **Intake** — the user's own recording app (e.g. Voicenotes), in-app recording,
   upload, or several? What do friends without a sync script do?
4. **Whose keys, and where** — `.env` file, config file, or OS keychain? Whose
   transcription / model account pays, and with what guardrails?
5. **Identity** — is there any sign-in, or does possession of the machine/repo
   imply the data?
6. **Name** — "Yunee" is provisional; repo, package id, and any scheme wait on the
   real name.

## Working rules

- **Stage explicit paths. Never `git add -A`.**
- **Commit small and promptly.**
- **Secrets never enter git.** `.env.local` (gitignored) only.
- **Write the decision down before building it.** Decisions go in this file or
  `docs/`, then become code.
- **Verify in the browser** for any user-facing surface, using BrowserOS.

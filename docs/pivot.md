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

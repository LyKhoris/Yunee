# Yunee

A private lecture companion for the founder and a few friends. Each person is
invited, and access is gated by a paid period. Nothing is hosted by anyone else.

Yunee is the successor to [Foxi](../Foxi), a public subscription SaaS that is now
archived. The name is a provisional codename.

- **What changed and why:** [`docs/pivot.md`](docs/pivot.md)
- **Build authority:** [`AGENTS.md`](AGENTS.md)

## Run it

```bash
cp .env.example .env.local   # then set SESSION_SECRET (openssl rand -hex 32)
npm install
npm run dev                  # http://localhost:3000
```

Development uses a local SQLite file at `data/yunee.db` — no Turso account needed.
To use Turso Cloud, set `TURSO_DATABASE_URL` and `TURSO_AUTH_TOKEN` in
`.env.local`:

```bash
turso db show --url yunee        # -> TURSO_DATABASE_URL
turso db tokens create yunee     # -> TURSO_AUTH_TOKEN
```

## Deploy (Vercel)

The app is deployed on Vercel's Hobby plan. Two environment variables must be set
in the Vercel project (`Settings → Environment Variables`):

| Variable | Notes |
|---|---|
| `SESSION_SECRET` | `openssl rand -hex 32`. Rotating it signs everyone out. |
| `APP_URL` | The deployed URL, e.g. `https://yunee.vercel.app`. Used for invite links. |
| `TURSO_DATABASE_URL` | From `turso db show --url yunee`. |
| `TURSO_AUTH_TOKEN` | From `turso db tokens create yunee`. |

Because `TURSO_DATABASE_URL` is set, production uses Turso. Locally it stays a
file, so `npm run dev` needs no credentials.

> **Note:** Hobby is licensed for non-commercial use; Yunee charges friends. This
> is a known, accepted risk at the current scale — moving to Pro (or Cloudflare)
> is the fix if it ever grows.

## Invite and manage members

Access is invite-only and in-house — no third party, no email. **This is done in the
web app now**, not the terminal. Sign in as an admin and open **Manage members**.

From `/admin` you can invite someone (name + username, optional email and access
period), and for each member: extend by 30 days, revoke or enable, promote or
demote, issue a reset link, or delete.

The invite link is a **one-time** link that lets them set a password; after that
they sign in at `/login` from any device. Resets are manual by design: there is
nothing to send email with, so "forgot password" means opening their row and
clicking **Reset link**.

### CLI (bootstrap only)

The CLI exists for the very first setup — it is how you become an admin at all — and
as an escape hatch if the web UI is ever unreachable. It prints the database it is
about to touch before every command.

```bash
npm run members -- add Ryan --username ryan --admin        # create the first admin
npm run members -- promote 1                               # or promote an existing member
npm run members -- list                                    # id, name, username, role, status
npm run members -- reset 1                                 # (emergency) fresh one-time link
npm run members -- revoke 1                                # (emergency) cut access off
```

A payment buys a *period* (`access_expires_at`); simply not renewing lets access
lapse. Payment itself is off-system for now.

## Checks

```bash
npm test          # access + session logic
npm run typecheck
npm run build
```

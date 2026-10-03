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

## Invite a friend

Access is invite-only and in-house — no third party, no email. You create a member
with a username; the invite link they receive is a **one-time** link that lets them
set a password. After that they sign in at `/login` from any device.

```bash
npm run members -- add Ada --username ada --email ada@example.com --days 120  # create + print invite link
npm run members -- list                                                       # status + expiry
npm run members -- invite 1                                                   # reprint the invite link (only works before it's used)
npm run members -- reset 1                                                    # new one-time link; clears the password, signs them out
npm run members -- extend 1 --days 120                                        # add a paid period
npm run members -- revoke 1                                                   # cut access off now (signs them out everywhere)
npm run members -- enable 1                                                   # turn access back on
npm run members -- rm 1                                                       # delete a member
```

A payment buys a *period* (`access_expires_at`); simply not renewing lets access
lapse. `revoke` is for an immediate cutoff. Payment itself is off-system for now.

**Forgot a password?** There is no self-service reset — nothing sends email. Run
`npm run members -- reset <id>` and send the new one-time link.

## Checks

```bash
npm test          # access + session logic
npm run typecheck
npm run build
```

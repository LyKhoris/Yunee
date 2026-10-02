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

## Invite a friend

Access is invite-only and in-house — there are no passwords. You create a member
and send them the invite link.

```bash
npm run members -- add Ada --email ada@example.com --days 120  # create + print invite link
npm run members -- list                                        # status + expiry
npm run members -- extend 1 --days 120                         # add a paid period
npm run members -- revoke 1                                    # cut access off now
npm run members -- enable 1                                    # turn access back on
npm run members -- rm 1                                        # delete a member
```

A payment buys a *period* (`access_expires_at`); simply not renewing lets access
lapse. `revoke` is for an immediate cutoff. Payment itself is off-system for now.

## Checks

```bash
npm test          # access + session logic
npm run typecheck
npm run build
```

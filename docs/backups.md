## Backups

The live database is dumped nightly to a **GitHub Actions artifact** — not to the
repo. Dumps contain live member tokens, so they must never be committed.

```bash
npm run backup              # write backups/yunee-YYYY-MM-DD.sql
npm run backup -- --keep 14 # also prune, keeping the newest 14
npm run restore             # replay the newest dump into a throwaway file
npm run restore -- path.sql # verify a specific dump
```

`restore` exists so a backup can be proven, not trusted. It replays the dump as a
SQL script and reports the row counts.

### The schedule

`.github/workflows/backup.yml` runs daily at 09:00 UTC and can be triggered by
hand from the Actions tab. It needs two repository secrets:

| Secret | Value |
|---|---|
| `TURSO_DATABASE_URL` | `turso db show --url yunee` |
| `TURSO_AUTH_TOKEN` | `turso db tokens create yunee` |

GitHub keeps artifacts for **90 days**. This is deliberate: it is free, needs no
machine of ours to be on, and expires by itself.

### What this does and does not cover

- **Covers:** recovering from a bad write, a deleted member, or corruption, back
  to the most recent dump (up to 24h old).
- **Also covers:** Turso's own 1-day point-in-time restore, for anything more
  recent.
- **Does not cover:** an outage lasting longer than the retention window, or a
  lost Turso account. For true independence, periodically download an artifact and
  keep it somewhere outside GitHub.

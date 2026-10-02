import { mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { createClient, type Client, type InArgs } from "@libsql/client";

/**
 * One SQLite database, two homes:
 *
 *   dev  -> a local file (`file:data/yunee.db`), no network, no token
 *   prod -> Turso Cloud (`libsql://<db>.turso.io` + TURSO_AUTH_TOKEN)
 *
 * Both speak the same client and the same SQL, so nothing else in the app
 * needs to know which one it is talking to.
 */
const SCHEMA = [
  `CREATE TABLE IF NOT EXISTS members (
     id                INTEGER PRIMARY KEY AUTOINCREMENT,
     name              TEXT NOT NULL,
     email             TEXT,
     token             TEXT NOT NULL UNIQUE,
     created_at        TEXT NOT NULL,
     access_expires_at TEXT,
     active            INTEGER NOT NULL DEFAULT 1,
     note              TEXT
   )`,
];

let client: Client | undefined;
let ready: Promise<void> | undefined;

function connect(): Client {
  const remoteUrl = process.env.TURSO_DATABASE_URL;
  const authToken = process.env.TURSO_AUTH_TOKEN;

  // Production: Turso Cloud.
  if (remoteUrl && remoteUrl.startsWith("libsql")) {
    return createClient(authToken ? { url: remoteUrl, authToken } : { url: remoteUrl });
  }

  // Development: a local file that speaks the same SQL. The libSQL client will
  // not create parent directories for us, so do it here.
  const path = join(process.cwd(), "data", "yunee.db");
  mkdirSync(dirname(path), { recursive: true });
  return createClient({ url: `file:${path}` });
}

async function migrate(db: Client): Promise<void> {
  for (const statement of SCHEMA) await db.execute(statement);
}

export async function getDb(): Promise<Client> {
  client ??= connect();
  ready ??= migrate(client);
  await ready;
  return client;
}

/** Convenience wrapper so callers don't repeat `getDb()` before every query. */
export async function query(sql: string, args: InArgs = []): Promise<Record<string, unknown>[]> {
  const db = await getDb();
  const result = await db.execute({ sql, args });
  return result.rows as unknown as Record<string, unknown>[];
}

/** Like `query`, but also returns the insert id (libSQL reports it on the result). */
export async function execute(
  sql: string,
  args: InArgs = [],
): Promise<{ rows: Record<string, unknown>[]; lastInsertRowid: number }> {
  const db = await getDb();
  const result = await db.execute({ sql, args });
  return {
    rows: result.rows as unknown as Record<string, unknown>[],
    lastInsertRowid: Number(result.lastInsertRowid ?? 0),
  };
}

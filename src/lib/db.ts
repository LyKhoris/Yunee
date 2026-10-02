import type { InArgs } from "@tursodatabase/serverless/compat";

/**
 * One SQLite database, two homes, and two drivers.
 *
 *   dev  -> local file (data/yunee.db)        via @libsql/client  (supports file:)
 *   prod -> Turso Cloud (libsql://...)        via @tursodatabase/serverless (fetch-only)
 *
 * The serverless driver has no native dependencies and works on edge/serverless
 * runtimes, but it cannot open a local file. So the local driver is imported
 * lazily and never loaded in production.
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

/** The subset of the client API this app uses — both drivers satisfy it. */
export type Db = {
  execute(input: { sql: string; args?: InArgs }): Promise<{
    rows: unknown[];
    lastInsertRowid?: number | bigint;
  }>;
};

let client: Db | undefined;
let ready: Promise<void> | undefined;

function isRemote(url: string | undefined): url is string {
  return !!url && (url.startsWith("libsql") || url.startsWith("http"));
}

async function openClient(): Promise<Db> {
  const configured = process.env.TURSO_DATABASE_URL;
  const authToken = process.env.TURSO_AUTH_TOKEN;

  if (isRemote(configured)) {
    const { createClient } = await import("@tursodatabase/serverless/compat");
    return createClient(authToken ? { url: configured, authToken } : { url: configured }) as unknown as Db;
  }

  // Local development: a real SQLite file. Requires the filesystem, so this
  // branch is only ever taken off-platform.
  const { createClient } = await import("@libsql/client");
  const { mkdirSync } = await import("node:fs");
  const { dirname, join } = await import("node:path");
  const path = join(process.cwd(), "data", "yunee.db");
  mkdirSync(dirname(path), { recursive: true });
  return createClient({ url: `file:${path}` }) as unknown as Db;
}

async function init(): Promise<void> {
  const db = await openClient();
  for (const statement of SCHEMA) await db.execute({ sql: statement });
  client = db;
}

export async function getDb(): Promise<Db> {
  ready ??= init();
  await ready;
  return client!;
}

/** Convenience wrapper so callers don't repeat `getDb()` before every query. */
export async function query(sql: string, args: InArgs = []): Promise<Record<string, unknown>[]> {
  const db = await getDb();
  const result = await db.execute({ sql, args });
  return result.rows as unknown as Record<string, unknown>[];
}

/** Like `query`, but also returns the insert id. */
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

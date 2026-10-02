import { existsSync, mkdirSync, readdirSync, statSync, unlinkSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { createClient } from "@tursodatabase/serverless/compat";

/**
 * Dump the Turso database to a local SQL file.
 *
 *   npm run backup              -> backups/yunee-YYYY-MM-DD.sql
 *   npm run backup -- --keep 14  -> also prune, keeping the newest 14 dumps
 *
 * Turso's own 1-day point-in-time restore is not a complete answer to "no file
 * loss", so this is the durable safety net the founder owns. The output is a
 * plain, restorable SQL script (see `npm run backup -- --verify`).
 */

const OUT_DIR = join(process.cwd(), "backups");

if (existsSync(".env.local")) process.loadEnvFile(".env.local");

async function dump(): Promise<string> {
  const url = process.env.TURSO_DATABASE_URL;
  const token = process.env.TURSO_AUTH_TOKEN;
  if (!url || !token) {
    throw new Error("TURSO_DATABASE_URL and TURSO_AUTH_TOKEN must be set to back up the live database");
  }

  const db = createClient({ url, authToken: token });

  const lines: string[] = ["PRAGMA foreign_keys=OFF;", "BEGIN TRANSACTION;"];

  // Schema.
  const tables = await db.execute({
    sql: `SELECT name, sql FROM sqlite_master
          WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name`,
    args: [],
  });

  for (const table of tables.rows as unknown as { name: string; sql: string | null }[]) {
    // Make the schema idempotent so a dump can be replayed over an existing file.
    if (table.sql) {
      lines.push(`${table.sql.replace(/^CREATE TABLE\s+/i, "CREATE TABLE IF NOT EXISTS ")};`);
    }

    // Rows, one INSERT per record with explicit column names.
    const cols = await db.execute({ sql: `PRAGMA table_info("${table.name}")`, args: [] });
    const names = (cols.rows as unknown as { name: string }[]).map((c) => c.name);
    if (names.length === 0) continue;

    const rows = await db.execute({ sql: `SELECT * FROM "${table.name}"`, args: [] });
    for (const row of rows.rows as unknown as Record<string, unknown>[]) {
      const values = names.map((n) => sqlLiteral(row[n]));
      lines.push(`INSERT INTO "${table.name}" (${names.map((n) => `"${n}"`).join(", ")}) VALUES (${values.join(", ")});`);
    }
  }

  lines.push("COMMIT;", "");
  return lines.join("\n");
}

function sqlLiteral(value: unknown): string {
  if (value === null || value === undefined) return "NULL";
  if (typeof value === "number" || typeof value === "bigint") return String(value);
  if (typeof value === "boolean") return value ? "1" : "0";
  if (value instanceof Uint8Array) {
    return `X'${Buffer.from(value).toString("hex")}'`;
  }
  return `'${String(value).replace(/'/g, "''")}'`;
}

function prune(keep: number): void {
  const files = readdirSync(OUT_DIR)
    .filter((f) => f.startsWith("yunee-") && f.endsWith(".sql"))
    .map((f) => ({ f, time: statSync(join(OUT_DIR, f)).mtime.getTime() }))
    .sort((a, b) => b.time - a.time);

  for (const { f } of files.slice(keep)) {
    unlinkSync(join(OUT_DIR, f));
    console.log(`pruned ${f}`);
  }
}

async function main(): Promise<void> {
  const args = process.argv.slice(2);
  const keepIndex = args.indexOf("--keep");
  const keep = keepIndex >= 0 ? Number(args[keepIndex + 1]) : 30;

  const sql = await dump();
  mkdirSync(OUT_DIR, { recursive: true });

  const stamp = new Date().toISOString().slice(0, 10);
  const path = join(OUT_DIR, `yunee-${stamp}.sql`);
  writeFileSync(path, sql, "utf8");

  const sizeKb = (Buffer.byteLength(sql) / 1024).toFixed(1);
  console.log(`wrote ${path} (${sizeKb} KB)`);
  console.log(`rows: ${(sql.match(/^INSERT INTO/gm) ?? []).length}`);

  prune(keep);
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});

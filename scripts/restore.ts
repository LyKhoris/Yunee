import { existsSync, readFileSync, readdirSync, rmSync } from "node:fs";
import { join } from "node:path";
import { createClient } from "@libsql/client";

/**
 * Prove a dump is restorable: replay it into a throwaway local SQLite file and
 * report what came back. An untested backup is not a backup.
 *
 *   npm run restore -- <path.sql>     verify a specific dump
 *   npm run restore                   verify the newest dump in backups/
 */

const OUT_DIR = join(process.cwd(), "backups");

function newestDump(): string {
  const files = readdirSync(OUT_DIR)
    .filter((f) => f.startsWith("yunee-") && f.endsWith(".sql"))
    .sort();
  if (files.length === 0) throw new Error("no dumps found in backups/");
  return join(OUT_DIR, files[files.length - 1]);
}

async function main(): Promise<void> {
  const given = process.argv[2];
  const path = given ?? newestDump();
  if (!existsSync(path)) throw new Error(`no such dump: ${path}`);

  const sql = readFileSync(path, "utf8");
  const target = join(OUT_DIR, ".verify.db");
  rmSync(target, { force: true });

  const db = createClient({ url: `file:${target}` });
  // Replay the dump exactly as written, transaction control included, through the
  // script executor. A failure here means the dump would not restore.
  await db.executeMultiple(sql);

  const tables = await db.execute({
    sql: `SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name`,
    args: [],
  });

  console.log(`verified ${path}`);
  for (const { name } of tables.rows as unknown as { name: string }[]) {
    const count = await db.execute({ sql: `SELECT COUNT(*) AS n FROM "${name}"`, args: [] });
    console.log(`  ${name}: ${(count.rows[0] as unknown as { n: number }).n} row(s)`);
  }

  rmSync(target, { force: true });
  console.log("restore OK");
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});

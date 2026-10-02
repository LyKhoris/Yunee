import { mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { DatabaseSync } from "node:sqlite";

const DB_PATH = join(process.cwd(), "data", "yunee.db");

const SCHEMA = `
  CREATE TABLE IF NOT EXISTS members (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    name              TEXT NOT NULL,
    email             TEXT,
    token             TEXT NOT NULL UNIQUE,
    created_at        TEXT NOT NULL,
    access_expires_at TEXT,
    active            INTEGER NOT NULL DEFAULT 1,
    note              TEXT
  );
`;

function open(): DatabaseSync {
  mkdirSync(dirname(DB_PATH), { recursive: true });
  const database = new DatabaseSync(DB_PATH);
  // Wait rather than fail when another process is mid-write.
  database.exec("PRAGMA busy_timeout = 5000");
  // WAL lets readers and one writer overlap instead of colliding.
  database.exec("PRAGMA journal_mode = WAL");
  database.exec(SCHEMA);
  return database;
}

// Reuse one connection per process; open lazily so merely importing this module
// never races other build workers for the file.
const state = globalThis as unknown as { __yuneeDb?: DatabaseSync };

export function getDb(): DatabaseSync {
  return (state.__yuneeDb ??= open());
}

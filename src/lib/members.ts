import { execute, query } from "./db";
import { newToken } from "./session";

export type Member = {
  id: number;
  name: string;
  email: string | null;
  token: string;
  created_at: string;
  access_expires_at: string | null;
  active: number;
  note: string | null;
};

const DAY_MS = 86_400_000;

function rowToMember(row: Record<string, unknown> | undefined): Member | null {
  return row ? (row as unknown as Member) : null;
}

export async function createMember(input: {
  name: string;
  email?: string | null;
  days?: number | null;
  note?: string | null;
}): Promise<Member> {
  const now = new Date();
  const days = input.days ?? null;
  const expires = days && days > 0 ? new Date(now.getTime() + days * DAY_MS).toISOString() : null;
  const { lastInsertRowid } = await execute(
    `INSERT INTO members (name, email, token, created_at, access_expires_at, active, note)
     VALUES (?, ?, ?, ?, ?, 1, ?)`,
    [input.name, input.email ?? null, newToken(), now.toISOString(), expires, input.note ?? null],
  );
  const created = await getMemberById(lastInsertRowid);
  return created!;
}

export async function getMemberById(id: number): Promise<Member | null> {
  const rows = await query("SELECT * FROM members WHERE id = ?", [id]);
  return rowToMember(rows[0]);
}

export async function getMemberByToken(token: string): Promise<Member | null> {
  const rows = await query("SELECT * FROM members WHERE token = ?", [token]);
  return rowToMember(rows[0]);
}

export async function listMembers(): Promise<Member[]> {
  const rows = await query("SELECT * FROM members ORDER BY id");
  return rows as unknown as Member[];
}

export async function setActive(id: number, active: boolean): Promise<void> {
  await query("UPDATE members SET active = ? WHERE id = ?", [active ? 1 : 0, id]);
}

/** Extend from the later of now and the current expiry, and re-enable. */
export async function extendMember(id: number, days: number): Promise<Member | null> {
  const member = await getMemberById(id);
  if (!member) return null;
  const current = member.access_expires_at ? new Date(member.access_expires_at).getTime() : 0;
  const base = Math.max(current, Date.now());
  const next = new Date(base + days * DAY_MS).toISOString();
  await query("UPDATE members SET access_expires_at = ?, active = 1 WHERE id = ?", [next, id]);
  return getMemberById(id);
}

export async function deleteMember(id: number): Promise<void> {
  await query("DELETE FROM members WHERE id = ?", [id]);
}

export async function findMember(idOrToken: string): Promise<Member | null> {
  return /^\d+$/.test(idOrToken) ? getMemberById(Number(idOrToken)) : getMemberByToken(idOrToken);
}

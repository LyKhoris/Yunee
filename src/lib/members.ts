import { getDb } from "./db";
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

export function createMember(input: {
  name: string;
  email?: string | null;
  days?: number | null;
  note?: string | null;
}): Member {
  const now = new Date();
  const days = input.days ?? null;
  const expires = days && days > 0 ? new Date(now.getTime() + days * DAY_MS).toISOString() : null;
  const info = getDb()
    .prepare(
      `INSERT INTO members (name, email, token, created_at, access_expires_at, active, note)
       VALUES (?, ?, ?, ?, ?, 1, ?)`,
    )
    .run(input.name, input.email ?? null, newToken(), now.toISOString(), expires, input.note ?? null);
  return getMemberById(Number(info.lastInsertRowid))!;
}

export function getMemberById(id: number): Member | null {
  const row = getDb().prepare("SELECT * FROM members WHERE id = ?").get(id);
  return (row as unknown as Member | undefined) ?? null;
}

export function getMemberByToken(token: string): Member | null {
  const row = getDb().prepare("SELECT * FROM members WHERE token = ?").get(token);
  return (row as unknown as Member | undefined) ?? null;
}

export function listMembers(): Member[] {
  return getDb().prepare("SELECT * FROM members ORDER BY id").all() as unknown as Member[];
}

export function setActive(id: number, active: boolean): void {
  getDb().prepare("UPDATE members SET active = ? WHERE id = ?").run(active ? 1 : 0, id);
}

/** Extend from the later of now and the current expiry, and re-enable. */
export function extendMember(id: number, days: number): Member | null {
  const member = getMemberById(id);
  if (!member) return null;
  const current = member.access_expires_at ? new Date(member.access_expires_at).getTime() : 0;
  const base = Math.max(current, Date.now());
  const next = new Date(base + days * DAY_MS).toISOString();
  getDb().prepare("UPDATE members SET access_expires_at = ?, active = 1 WHERE id = ?").run(next, id);
  return getMemberById(id);
}

export function deleteMember(id: number): void {
  getDb().prepare("DELETE FROM members WHERE id = ?").run(id);
}

export function findMember(idOrToken: string): Member | null {
  return /^\d+$/.test(idOrToken) ? getMemberById(Number(idOrToken)) : getMemberByToken(idOrToken);
}

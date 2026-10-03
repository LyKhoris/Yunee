import { execute, query } from "./db";
import { newToken } from "./session";

export type Member = {
  id: number;
  name: string;
  username: string | null;
  email: string | null;
  token: string;
  password_hash: string | null;
  created_at: string;
  access_expires_at: string | null;
  active: number;
  invite_used_at: string | null;
  session_epoch: number;
  is_admin: number;
  note: string | null;
};

/**
 * The subset of a member safe to hand to a client component.
 *
 * A raw row cannot cross that boundary (Next rejects the `bigint`-ish/nullable
 * shapes it complains about), and it should not: `token` and `password_hash` are
 * server-only. Nothing in the admin UI needs either — "invite pending" is
 * derived from `has_password`, and the invite URL is only built server-side.
 */
export type MemberView = {
  id: number;
  name: string;
  username: string | null;
  email: string | null;
  active: boolean;
  is_admin: boolean;
  access_expires_at: string | null;
  has_password: boolean;
  invite_pending: boolean;
};

export function toMemberView(member: Member): MemberView {
  return {
    id: member.id,
    name: member.name,
    username: member.username,
    email: member.email,
    active: !!member.active,
    is_admin: !!member.is_admin,
    access_expires_at: member.access_expires_at,
    has_password: !!member.password_hash,
    invite_pending: !member.password_hash,
  };
}

const DAY_MS = 86_400_000;

/** Usernames are matched case-insensitively by storing one canonical form. */
export function normalizeUsername(username: string): string {
  return username.trim().toLowerCase();
}

/** Admin is a plain flag, like `active`. */
export function isAdmin(member: { is_admin: number | boolean }): boolean {
  return !!member.is_admin;
}

/** Thrown when a change would leave Yunee with no admin at all. */
export class LastAdminError extends Error {
  constructor() {
    super("That would leave Yunee with no admin. Promote someone else first.");
    this.name = "LastAdminError";
  }
}

function rowToMember(row: Record<string, unknown> | undefined): Member | null {
  return row ? (row as unknown as Member) : null;
}

export async function createMember(input: {
  name: string;
  username: string;
  email?: string | null;
  days?: number | null;
  note?: string | null;
  isAdmin?: boolean;
}): Promise<Member> {
  const username = normalizeUsername(input.username);
  if (!username) throw new Error("A username is required.");
  const clash = await getMemberByUsername(username);
  if (clash) throw new Error(`Username "${username}" is already taken (member #${clash.id}).`);

  const now = new Date();
  const days = input.days ?? null;
  const expires = days && days > 0 ? new Date(now.getTime() + days * DAY_MS).toISOString() : null;
  const { lastInsertRowid } = await execute(
    `INSERT INTO members (name, username, email, token, created_at, access_expires_at, active, is_admin, note)
     VALUES (?, ?, ?, ?, ?, ?, 1, ?, ?)`,
    [
      input.name,
      username,
      input.email ?? null,
      newToken(),
      now.toISOString(),
      expires,
      input.isAdmin ? 1 : 0,
      input.note ?? null,
    ],
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

export async function getMemberByUsername(username: string): Promise<Member | null> {
  const rows = await query("SELECT * FROM members WHERE username = ?", [normalizeUsername(username)]);
  return rowToMember(rows[0]);
}

export async function listMembers(): Promise<Member[]> {
  const rows = await query("SELECT * FROM members ORDER BY id");
  return rows as unknown as Member[];
}

/** How many admins exist. Used to refuse the change that would leave none. */
export async function countAdmins(): Promise<number> {
  const rows = await query("SELECT COUNT(*) AS n FROM members WHERE is_admin = 1");
  return Number(rows[0]?.n ?? 0);
}

/**
 * Turn access on or off. Every change bumps `session_epoch`, so a revoke signs
 * the member out immediately on every device they're already using.
 *
 * Revoking the last *active* admin is refused: it would lock the operator out of
 * the admin surfaces with no way back in.
 */
export async function setActive(id: number, active: boolean): Promise<void> {
  const member = await getMemberById(id);
  if (!member) return;
  if (!active && isAdmin(member) && (await countAdmins()) <= 1) throw new LastAdminError();
  await query(
    "UPDATE members SET active = ?, session_epoch = session_epoch + 1 WHERE id = ?",
    [active ? 1 : 0, id],
  );
}

/**
 * Grant or revoke admin. Demoting the last admin is refused, same reason as
 * revoking one. Bumps the epoch so a demoted admin loses the surfaces on the
 * next request rather than at the end of a long cookie's life.
 */
export async function setAdmin(id: number, admin: boolean): Promise<void> {
  const member = await getMemberById(id);
  if (!member) return;
  if (!admin && isAdmin(member) && (await countAdmins()) <= 1) throw new LastAdminError();
  await query("UPDATE members SET is_admin = ?, session_epoch = session_epoch + 1 WHERE id = ?", [
    admin ? 1 : 0,
    id,
  ]);
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
  const member = await getMemberById(id);
  if (!member) return;
  if (isAdmin(member) && (await countAdmins()) <= 1) throw new LastAdminError();
  await query("DELETE FROM members WHERE id = ?", [id]);
}

/**
 * Issue a fresh one-time link and clear the password, so the invite is again a
 * "set your password" step. Also bumps the epoch, signing out every device — the
 * point of a reset.
 */
export async function resetInvite(id: number): Promise<Member | null> {
  const member = await getMemberById(id);
  if (!member) return null;
  await query(
    `UPDATE members SET token = ?, invite_used_at = NULL, password_hash = NULL,
       session_epoch = session_epoch + 1 WHERE id = ?`,
    [newToken(), id],
  );
  return getMemberById(id);
}

/** Spend an invite: store the chosen password and mark the link used. */
export async function redeemInvite(id: number, passwordHash: string): Promise<void> {
  await query("UPDATE members SET password_hash = ?, invite_used_at = ? WHERE id = ?", [
    passwordHash,
    new Date().toISOString(),
    id,
  ]);
}

export async function findMember(idOrToken: string): Promise<Member | null> {
  return /^\d+$/.test(idOrToken) ? getMemberById(Number(idOrToken)) : getMemberByToken(idOrToken);
}

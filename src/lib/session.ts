import { createHmac, randomBytes, timingSafeEqual } from "node:crypto";

export const COOKIE_NAME = "yunee_session";

/** A secret invitation token. 256 bits, URL-safe. */
export function newToken(): string {
  return randomBytes(32).toString("base64url");
}

export type Session = { id: number; epoch: number };

function secret(): string {
  const s = process.env.SESSION_SECRET;
  if (!s) throw new Error("SESSION_SECRET is not set");
  return s;
}

function sign(payload: string): string {
  return createHmac("sha256", secret()).update(payload).digest("base64url");
}

/**
 * Issue a signed session cookie value for a member.
 *
 * `epoch` is the member's `session_epoch` at issue time. Bumping that value in
 * the database invalidates every cookie already out there — which is how a
 * revoke or a password reset takes effect on devices that are already signed in.
 */
export function issue(memberId: number, epoch: number, now: number = Date.now()): string {
  const payload = `${memberId}.${epoch}.${now}`;
  return `${payload}.${sign(payload)}`;
}

/** Verify a session cookie value; returns its claims, or null if invalid. */
export function verify(cookie: string | undefined | null): Session | null {
  if (!cookie) return null;
  const cut = cookie.lastIndexOf(".");
  if (cut <= 0) return null;
  const payload = cookie.slice(0, cut);
  const sig = cookie.slice(cut + 1);
  const expected = sign(payload);
  const a = Buffer.from(sig);
  const b = Buffer.from(expected);
  if (a.length !== b.length || !timingSafeEqual(a, b)) return null;

  const [idStr, epochStr] = payload.split(".");
  const id = Number(idStr);
  const epoch = Number(epochStr);
  if (!Number.isInteger(id) || id <= 0 || !Number.isInteger(epoch) || epoch < 0) return null;
  return { id, epoch };
}

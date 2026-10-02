import { createHmac, randomBytes, timingSafeEqual } from "node:crypto";

export const COOKIE_NAME = "yunee_session";

/** A secret invitation token. 256 bits, URL-safe. */
export function newToken(): string {
  return randomBytes(32).toString("base64url");
}

function secret(): string {
  const s = process.env.SESSION_SECRET;
  if (!s) throw new Error("SESSION_SECRET is not set");
  return s;
}

function sign(payload: string): string {
  return createHmac("sha256", secret()).update(payload).digest("base64url");
}

/** Issue a signed session cookie value for a member id. */
export function issue(memberId: number, now: number = Date.now()): string {
  const payload = `${memberId}.${now}`;
  return `${payload}.${sign(payload)}`;
}

/** Verify a session cookie value; returns the member id or null if invalid. */
export function verify(cookie: string | undefined | null): number | null {
  if (!cookie) return null;
  const cut = cookie.lastIndexOf(".");
  if (cut <= 0) return null;
  const payload = cookie.slice(0, cut);
  const sig = cookie.slice(cut + 1);
  const expected = sign(payload);
  const a = Buffer.from(sig);
  const b = Buffer.from(expected);
  if (a.length !== b.length || !timingSafeEqual(a, b)) return null;
  const id = Number(payload.slice(0, payload.indexOf(".")));
  return Number.isInteger(id) && id > 0 ? id : null;
}

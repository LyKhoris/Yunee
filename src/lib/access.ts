export type AccessStatus = "ok" | "revoked" | "expired";

/**
 * Pure access decision — no DB, no HTTP. A member may use Yunee only while active
 * (not revoked) and before their paid period lapses.
 */
export function evaluateAccess(
  member: { active: number | boolean; access_expires_at: string | null },
  now: Date = new Date(),
): AccessStatus {
  if (!member.active) return "revoked";
  if (member.access_expires_at && new Date(member.access_expires_at).getTime() <= now.getTime()) {
    return "expired";
  }
  return "ok";
}

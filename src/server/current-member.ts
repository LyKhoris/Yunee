import { evaluateAccess, type AccessStatus } from "@/lib/access";
import { getMemberById, type Member } from "@/lib/members";
import { getSession } from "./session";

/**
 * Resolve the current request's member, if any, along with their access status.
 * Returns null when there is no valid session for an existing member.
 *
 * Access is re-decided from the row on every request, so revocation and expiry
 * are instant. The session's `epoch` is compared against the row too, so a
 * revoke or password reset invalidates cookies that are already out there.
 */
export async function currentMember(): Promise<{ member: Member; status: AccessStatus } | null> {
  const session = await getSession();
  if (!session) return null;
  const member = await getMemberById(session.id);
  if (!member || member.session_epoch !== session.epoch) return null;
  return { member, status: evaluateAccess(member) };
}

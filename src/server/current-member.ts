import { evaluateAccess, type AccessStatus } from "@/lib/access";
import { getMemberById, type Member } from "@/lib/members";
import { getSessionMemberId } from "./session";

/**
 * Resolve the current request's member, if any, along with their access status.
 * Returns null when there is no valid session for an existing member.
 */
export async function currentMember(): Promise<{ member: Member; status: AccessStatus } | null> {
  const id = await getSessionMemberId();
  if (id == null) return null;
  const member = await getMemberById(id);
  if (!member) return null;
  return { member, status: evaluateAccess(member) };
}

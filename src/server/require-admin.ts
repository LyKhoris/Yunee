import { redirect } from "next/navigation";
import { isAdmin, type Member } from "@/lib/members";
import { currentMember } from "./current-member";

/**
 * The member making this request, but only if they are an admin with usable
 * access. Every admin page and every admin action calls this — the guard is
 * server-side, never a hidden link.
 *
 * Access is re-read from the row on each request, so a demoted admin loses these
 * surfaces on their next request, exactly like a revocation.
 */
export async function requireAdmin(): Promise<Member> {
  const access = await currentMember();
  if (!access || access.status !== "ok" || !isAdmin(access.member)) redirect("/");
  return access.member;
}

"use server";

import { revalidatePath } from "next/cache";
import { redirect } from "next/navigation";
import { isRedirectError } from "next/dist/client/components/redirect-error";
import {
  createMember,
  deleteMember,
  extendMember,
  findMember,
  LastAdminError,
  normalizeUsername,
  resetInvite,
  setActive,
  setAdmin,
} from "@/lib/members";
import { requireAdmin } from "./require-admin";

export type AdminState = { error?: string; notice?: string };

/** Read a member id out of a form post without trusting the shape. */
function idOf(formData: FormData): number | null {
  const id = Number(formData.get("id"));
  return Number.isInteger(id) && id > 0 ? id : null;
}

async function guard(): Promise<AdminState | null> {
  await requireAdmin();
  return null;
}

/**
 * Wrap every handler so a last-admin refusal becomes a message, not a crash.
 *
 * Next's `redirect()` works by *throwing* a special signal, so it must pass
 * through untouched — catching it would turn a navigation into an error message
 * (which is exactly what happened the first time this ran).
 */
async function attempt(fn: () => Promise<AdminState | void>): Promise<AdminState> {
  try {
    const result = await fn();
    return (result as AdminState) ?? {};
  } catch (error) {
    if (isRedirectError(error)) throw error;
    if (error instanceof LastAdminError) return { error: error.message };
    if (error instanceof Error) return { error: error.message };
    return { error: "Something went wrong." };
  }
}

function refresh(): void {
  revalidatePath("/admin");
}

export async function createMemberAction(
  _prev: AdminState,
  formData: FormData,
): Promise<AdminState> {
  await guard();
  return attempt(async () => {
    const name = String(formData.get("name") ?? "").trim();
    const username = normalizeUsername(String(formData.get("username") ?? ""));
    if (!name) return { error: "A name is required." };
    if (!username) return { error: "A username is required — it's what they sign in with." };
    if (!/^[a-z0-9._-]+$/.test(username)) {
      return { error: "Usernames can use letters, numbers, dots, dashes and underscores." };
    }

    const days = Number(formData.get("days"));
    const member = await createMember({
      name,
      username,
      email: String(formData.get("email") ?? "").trim() || null,
      days: Number.isFinite(days) && days > 0 ? days : null,
      note: String(formData.get("note") ?? "").trim() || null,
      isAdmin: formData.get("isAdmin") === "on",
    });

    refresh();
    redirect(`/admin/invite/${member.id}`);
  });
}

/** Extend a paid period. Defaults to the same 30 days the CLI used. */
export async function extendAction(_prev: AdminState, formData: FormData): Promise<AdminState> {
  await guard();
  return attempt(async () => {
    const id = idOf(formData);
    if (!id) return { error: "Missing member." };
    const days = Number(formData.get("days")) || 30;
    const updated = await extendMember(id, days);
    refresh();
    return { notice: `${updated?.name ?? "Member"} now has access until ${updated?.access_expires_at?.slice(0, 10)}.` };
  });
}

export async function setActiveAction(_prev: AdminState, formData: FormData): Promise<AdminState> {
  await guard();
  return attempt(async () => {
    const id = idOf(formData);
    if (!id) return { error: "Missing member." };
    const active = formData.get("active") === "1";
    await setActive(id, active);
    refresh();
    return { notice: active ? "Access turned back on." : "Revoked. They are signed out everywhere." };
  });
}

export async function setAdminAction(_prev: AdminState, formData: FormData): Promise<AdminState> {
  await guard();
  return attempt(async () => {
    const id = idOf(formData);
    if (!id) return { error: "Missing member." };
    const admin = formData.get("admin") === "1";
    await setAdmin(id, admin);
    refresh();
    return { notice: admin ? "They are now an admin." : "Admin removed." };
  });
}

export async function resetAction(_prev: AdminState, formData: FormData): Promise<AdminState> {
  await guard();
  return attempt(async () => {
    const id = idOf(formData);
    if (!id) return { error: "Missing member." };
    await resetInvite(id);
    refresh();
    redirect(`/admin/invite/${id}`);
  });
}

export async function deleteAction(_prev: AdminState, formData: FormData): Promise<AdminState> {
  await guard();
  return attempt(async () => {
    const id = idOf(formData);
    if (!id) return { error: "Missing member." };
    const member = await findMember(String(id));
    await deleteMember(id);
    refresh();
    return { notice: `Deleted ${member?.name ?? "member"}.` };
  });
}

"use server";

import { redirect } from "next/navigation";
import { hashPassword, verifyPassword } from "@/lib/password";
import { getMemberByToken, getMemberByUsername, redeemInvite } from "@/lib/members";
import { setSession } from "./session";

export type FormState = { error?: string };

const MIN_PASSWORD = 8;

/** Sign in from /login. Generic message: never reveal whether a username exists. */
export async function signIn(_prev: FormState, formData: FormData): Promise<FormState> {
  const username = String(formData.get("username") ?? "");
  const password = String(formData.get("password") ?? "");
  const member = await getMemberByUsername(username);
  if (!member || !(await verifyPassword(password, member.password_hash))) {
    return { error: "Wrong username or password." };
  }
  await setSession(member.id, member.session_epoch);
  redirect("/");
}

/** Spend an invitation: choose a password, then sign in on this device. */
export async function redeemInviteAction(_prev: FormState, formData: FormData): Promise<FormState> {
  const token = String(formData.get("token") ?? "");
  const password = String(formData.get("password") ?? "");
  const confirm = String(formData.get("confirm") ?? "");

  if (password.length < MIN_PASSWORD) {
    return { error: `Choose a password of at least ${MIN_PASSWORD} characters.` };
  }
  if (password !== confirm) {
    return { error: "The two passwords do not match." };
  }

  const member = await getMemberByToken(token);
  if (!member || member.invite_used_at) {
    return { error: "This invitation is no longer valid. Ask for a fresh link." };
  }
  await redeemInvite(member.id, await hashPassword(password));
  await setSession(member.id, member.session_epoch);
  redirect("/");
}

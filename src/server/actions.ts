"use server";

import { redirect } from "next/navigation";
import { clearSession } from "./session";

export async function signOut(): Promise<void> {
  await clearSession();
  redirect("/");
}

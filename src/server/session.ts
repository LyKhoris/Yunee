import { cookies } from "next/headers";
import { COOKIE_NAME, issue, verify, type Session } from "@/lib/session";

const ONE_YEAR = 60 * 60 * 24 * 365;

export async function setSession(memberId: number, epoch: number): Promise<void> {
  const jar = await cookies();
  jar.set(COOKIE_NAME, issue(memberId, epoch), {
    httpOnly: true,
    sameSite: "lax",
    // Only force Secure when the request actually came over HTTPS. A production
    // build served over plain http://localhost would otherwise set a cookie the
    // browser silently drops.
    secure: (process.env.APP_URL ?? "").startsWith("https://"),
    path: "/",
    maxAge: ONE_YEAR,
  });
}

export async function getSession(): Promise<Session | null> {
  const jar = await cookies();
  return verify(jar.get(COOKIE_NAME)?.value);
}

export async function clearSession(): Promise<void> {
  const jar = await cookies();
  jar.delete(COOKIE_NAME);
}

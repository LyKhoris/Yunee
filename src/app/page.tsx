import Link from "next/link";
import { signOut } from "@/server/actions";
import { currentMember } from "@/server/current-member";

export default async function Home() {
  const access = await currentMember();

  return (
    <main className="flex flex-1 flex-col items-center justify-center gap-6 px-6 py-16">
      <Link href="/" className="flex flex-col items-center gap-1 pressable">
        <span className="font-display text-3xl font-semibold tracking-tight">Yunee</span>
      </Link>

      {!access ? (
        <div className="card fx-rise w-full max-w-sm p-6">
          <h1 className="text-lg font-semibold">A private space</h1>
          <p className="mt-2 text-sm text-ink-muted">
            You need an invitation link to get in.
          </p>
          <Link href="/login" className="btn btn-ghost mt-5 w-full justify-center py-2.5">
            I already have an account
          </Link>
        </div>
      ) : access.status !== "ok" ? (
        <div className="card fx-rise w-full max-w-sm p-6">
          <h1 className="text-lg font-semibold">
            Access {access.status === "expired" ? "expired" : "revoked"}
          </h1>
          <p className="mt-2 text-sm text-ink-muted">
            {access.status === "expired"
              ? "Your access period has ended. Renew to continue."
              : "Your access has been turned off."}
          </p>
          <form action={signOut} className="mt-5">
            <button type="submit" className="btn btn-ghost w-full justify-center py-2.5">
              Sign out
            </button>
          </form>
        </div>
      ) : (
        <div className="card fx-rise w-full max-w-sm p-6">
          <h1 className="text-lg font-semibold">Hi, {access.member.name}</h1>
          <p className="mt-2 text-sm text-ink-muted">
            {access.member.access_expires_at
              ? `Access until ${access.member.access_expires_at.slice(0, 10)}.`
              : "Access does not expire."}
          </p>
          <p className="mt-2 text-sm text-ink-muted">
            Nothing here yet — the lecture pipeline comes next.
          </p>
          {access.member.is_admin ? (
            <Link href="/admin" className="btn btn-primary mt-5 w-full justify-center py-2.5">
              Manage members
            </Link>
          ) : null}
          <form action={signOut} className="mt-2">
            <button type="submit" className="btn btn-ghost w-full justify-center py-2.5">
              Sign out
            </button>
          </form>
        </div>
      )}
    </main>
  );
}

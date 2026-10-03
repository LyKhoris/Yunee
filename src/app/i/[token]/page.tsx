import Link from "next/link";
import { getMemberByToken } from "@/lib/members";
import { InviteForm } from "./invite-form";

export default async function InvitePage({ params }: { params: Promise<{ token: string }> }) {
  const { token } = await params;
  const member = await getMemberByToken(token);
  const spent = !member || member.invite_used_at;

  return (
    <main className="flex flex-1 flex-col items-center justify-center gap-6 px-6 py-16">
      <Link href="/" className="flex flex-col items-center gap-1 pressable">
        <span className="font-display text-3xl font-semibold tracking-tight">Yunee</span>
      </Link>

      <div className="card fx-rise w-full max-w-sm p-6">
        {spent ? (
          <>
            <h1 className="text-lg font-semibold">Invitation not valid</h1>
            <p className="mt-2 text-sm text-ink-muted">
              This link has already been used or has been replaced. Ask for a fresh one, or sign in
              if you already have a password.
            </p>
            <Link href="/login" className="btn btn-ghost mt-5 w-full justify-center py-2.5">
              Go to sign in
            </Link>
          </>
        ) : (
          <>
            <h1 className="text-lg font-semibold">Welcome{member.name ? `, ${member.name}` : ""}</h1>
            <p className="mt-2 text-sm text-ink-muted">
              Set a password to finish your account. You&apos;ll use it to sign in on any device —
              this link only works once.
            </p>
            <div className="mt-5">
              <InviteForm token={token} username={member.username} />
            </div>
          </>
        )}
      </div>
    </main>
  );
}

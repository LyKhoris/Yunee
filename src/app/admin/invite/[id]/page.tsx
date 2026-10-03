import Link from "next/link";
import { notFound } from "next/navigation";
import { getMemberById } from "@/lib/members";
import { requireAdmin } from "@/server/require-admin";
import { InviteLink } from "./invite-link";

/**
 * The one-time link, on screen for whoever runs Yunee. Shown after creating a
 * member and after a reset — the two moments a fresh token exists.
 */
export default async function InvitePage({ params }: { params: Promise<{ id: string }> }) {
  await requireAdmin();
  const { id } = await params;
  const member = await getMemberById(Number(id));
  if (!member) notFound();

  const appUrl = process.env.APP_URL ?? "http://localhost:3000";
  const url = `${appUrl}/i/${member.token}`;
  const spent = !!member.invite_used_at;

  return (
    <main className="mx-auto w-full max-w-xl px-6 py-12">
      <p className="section-label">Invitation</p>
      <h1 className="mt-1 text-2xl font-semibold">{member.name}</h1>
      <p className="mt-1 text-sm text-ink-muted">
        @{member.username ?? "—"}
        {member.is_admin ? " · admin" : ""}
      </p>

      <div className="card mt-6 p-6">
        {spent ? (
          <>
            <h2 className="text-lg font-semibold">No fresh link</h2>
            <p className="mt-2 text-sm text-ink-muted">
              They have already set a password, so there is nothing to send. Use{" "}
              <span className="font-semibold text-ink">Reset link</span> on the roster if they need a
              new one.
            </p>
          </>
        ) : (
          <>
            <h2 className="text-lg font-semibold">Send them this link</h2>
            <p className="mt-2 text-sm text-ink-muted">
              It works once. Opening it lets {member.name} choose a password, and then it is spent.
            </p>
            <div className="mt-4">
              <InviteLink url={url} />
            </div>
          </>
        )}
      </div>

      <p className="mt-6 text-xs text-ink-subtle">
        <Link href="/admin" className="underline underline-offset-2">
          Back to members
        </Link>
      </p>
    </main>
  );
}

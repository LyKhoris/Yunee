import Link from "next/link";
import { evaluateAccess, type AccessStatus } from "@/lib/access";
import { listMembers, toMemberView, type Member } from "@/lib/members";
import { requireAdmin } from "@/server/require-admin";
import { MemberRow } from "./member-row";
import { NewMemberDialog } from "./new-member-dialog";

function statusPill(status: AccessStatus): { label: string; className: string } {
  switch (status) {
    case "ok":
      return { label: "active", className: "pill" };
    case "expired":
      return { label: "expired", className: "pill pill-warm" };
    default:
      return { label: "revoked", className: "pill pill-danger" };
  }
}

export default async function AdminPage() {
  const admin = await requireAdmin();
  const members = await listMembers();

  const sorted = [...members].sort((a, b) => {
    const rank = (m: Member) => (m.is_admin ? 0 : 1);
    return rank(a) - rank(b) || a.id - b.id;
  });

  return (
    <main className="mx-auto w-full max-w-3xl px-6 py-12">
      <div className="flex items-center justify-between gap-4">
        <div>
          <p className="section-label">Admin</p>
          <h1 className="mt-1 text-2xl font-semibold">Members</h1>
        </div>
        <NewMemberDialog />
      </div>

      <p className="mt-3 text-sm text-ink-muted">
        You are {admin.name}. {members.length === 0
          ? "No members yet — invite the first one."
          : `${members.length} member${members.length === 1 ? "" : "s"}.`}
      </p>

      <div className="card mt-6 overflow-hidden">
        {sorted.length === 0 ? (
          <p className="p-6 text-sm text-ink-muted">
            Nobody here yet. Use <span className="font-semibold text-ink">Invite someone</span> to
            create an account and get a one-time link.
          </p>
        ) : (
          <ul className="divide-y divide-hairline">
            {sorted.map((member) => (
              <MemberRow
                key={member.id}
                member={toMemberView(member)}
                status={evaluateAccess(member)}
                pill={statusPill(evaluateAccess(member))}
                self={member.id === admin.id}
              />
            ))}
          </ul>
        )}
      </div>

      <p className="mt-6 text-xs text-ink-subtle">
        <Link href="/" className="underline underline-offset-2">
          Back to Yunee
        </Link>
      </p>
    </main>
  );
}

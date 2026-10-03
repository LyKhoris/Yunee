import Link from "next/link";
import { getMemberByToken } from "@/lib/members";
import { InviteForm } from "./invite-form";

const shell: React.CSSProperties = {
  maxWidth: 560,
  margin: "4rem auto",
  padding: "0 1rem",
  fontFamily: "system-ui, sans-serif",
  lineHeight: 1.5,
};

export default async function InvitePage({ params }: { params: Promise<{ token: string }> }) {
  const { token } = await params;
  const member = await getMemberByToken(token);

  if (!member || member.invite_used_at) {
    return (
      <main style={shell}>
        <h1>Invitation not valid</h1>
        <p>
          This link has already been used or has been replaced. Ask for a fresh one, or sign in if
          you already have a password.
        </p>
        <p>
          <Link href="/login">Go to sign in</Link>
        </p>
      </main>
    );
  }

  return (
    <main style={shell}>
      <h1>Welcome{member.name ? `, ${member.name}` : ""}</h1>
      <p>
        Set a password to finish your account. You&apos;ll use it to sign in on any device — this
        link only works once.
      </p>
      <InviteForm token={token} username={member.username} />
    </main>
  );
}

import { signOut } from "@/server/actions";
import { currentMember } from "@/server/current-member";

const shell: React.CSSProperties = {
  maxWidth: 560,
  margin: "4rem auto",
  padding: "0 1rem",
  fontFamily: "system-ui, sans-serif",
  lineHeight: 1.5,
};

export default async function Home() {
  const access = await currentMember();

  if (!access) {
    return (
      <main style={shell}>
        <h1>Yunee</h1>
        <p>This is a private space. You need an invitation link to get in.</p>
      </main>
    );
  }

  if (access.status !== "ok") {
    const expired = access.status === "expired";
    return (
      <main style={shell}>
        <h1>Access {expired ? "expired" : "revoked"}</h1>
        <p>
          {expired
            ? "Your access period has ended. Renew to continue."
            : "Your access has been turned off."}
        </p>
      </main>
    );
  }

  const { member } = access;
  return (
    <main style={shell}>
      <h1>Hi, {member.name}</h1>
      <p>
        {member.access_expires_at
          ? `Access until ${member.access_expires_at.slice(0, 10)}.`
          : "Access does not expire."}
      </p>
      <p>Nothing here yet — the lecture pipeline comes next.</p>
      <form action={signOut}>
        <button type="submit">Sign out</button>
      </form>
    </main>
  );
}

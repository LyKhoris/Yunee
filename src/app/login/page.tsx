import { redirect } from "next/navigation";
import { currentMember } from "@/server/current-member";
import { LoginForm } from "./login-form";

const shell: React.CSSProperties = {
  maxWidth: 560,
  margin: "4rem auto",
  padding: "0 1rem",
  fontFamily: "system-ui, sans-serif",
  lineHeight: 1.5,
};

export default async function LoginPage() {
  const access = await currentMember();
  if (access?.status === "ok") redirect("/");

  return (
    <main style={shell}>
      <h1>Sign in</h1>
      <p>Use the username and password you set from your invitation.</p>
      <LoginForm />
      <p style={{ marginTop: "1.5rem", color: "#555" }}>
        No account yet? You need an invitation link from the person who runs Yunee.
      </p>
    </main>
  );
}

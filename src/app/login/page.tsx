import Link from "next/link";
import { redirect } from "next/navigation";
import { currentMember } from "@/server/current-member";
import { LoginForm } from "./login-form";

export default async function LoginPage() {
  const access = await currentMember();
  if (access?.status === "ok") redirect("/");

  return (
    <main className="flex flex-1 flex-col items-center justify-center gap-6 px-6 py-16">
      <Link href="/" className="flex flex-col items-center gap-1 pressable">
        <span className="font-display text-3xl font-semibold tracking-tight">Yunee</span>
      </Link>

      <p className="max-w-xs text-center text-sm text-ink-muted">
        Your lectures, remembered. Sign in to pick up where you left off.
      </p>

      <div className="card fx-rise w-full max-w-sm p-6">
        <LoginForm />
      </div>

      <p className="max-w-xs text-center text-xs text-ink-subtle">
        No account yet? You need an invitation link from whoever runs Yunee.
      </p>
    </main>
  );
}

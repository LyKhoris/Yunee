"use client";

import { useActionState, type CSSProperties } from "react";
import { signIn, type FormState } from "@/server/account";

export function LoginForm() {
  const [state, action, pending] = useActionState<FormState, FormData>(signIn, {});
  return (
    <form action={action} style={form}>
      <label style={label}>
        Username
        <input name="username" autoComplete="username" required style={input} />
      </label>
      <label style={label}>
        Password
        <input type="password" name="password" autoComplete="current-password" required style={input} />
      </label>
      {state.error ? (
        <p role="alert" style={error}>
          {state.error}
        </p>
      ) : null}
      <button type="submit" disabled={pending}>
        {pending ? "Signing in…" : "Sign in"}
      </button>
    </form>
  );
}

const form: CSSProperties = { display: "grid", gap: "0.75rem", maxWidth: 320 };
const label: CSSProperties = { display: "grid", gap: "0.25rem" };
const input: CSSProperties = { padding: "0.5rem", font: "inherit" };
const error: CSSProperties = { color: "#b00020", margin: 0 };

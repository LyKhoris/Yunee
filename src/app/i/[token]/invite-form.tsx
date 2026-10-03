"use client";

import { useActionState, type CSSProperties } from "react";
import { redeemInviteAction, type FormState } from "@/server/account";

export function InviteForm({ token, username }: { token: string; username: string | null }) {
  const [state, action, pending] = useActionState<FormState, FormData>(redeemInviteAction, {});
  return (
    <form action={action} style={form}>
      <input type="hidden" name="token" value={token} />
      {username ? (
        <p style={{ margin: 0 }}>
          Username: <strong>{username}</strong>
        </p>
      ) : null}
      <label style={label}>
        Choose a password
        <input
          type="password"
          name="password"
          autoComplete="new-password"
          minLength={8}
          required
          style={input}
        />
      </label>
      <label style={label}>
        Repeat it
        <input
          type="password"
          name="confirm"
          autoComplete="new-password"
          minLength={8}
          required
          style={input}
        />
      </label>
      {state.error ? (
        <p role="alert" style={error}>
          {state.error}
        </p>
      ) : null}
      <button type="submit" disabled={pending}>
        {pending ? "Saving…" : "Set password and continue"}
      </button>
    </form>
  );
}

const form: CSSProperties = { display: "grid", gap: "0.75rem", maxWidth: 320 };
const label: CSSProperties = { display: "grid", gap: "0.25rem" };
const input: CSSProperties = { padding: "0.5rem", font: "inherit" };
const error: CSSProperties = { color: "#b00020", margin: 0 };

"use client";

import { useActionState } from "react";
import { signIn, type FormState } from "@/server/account";

export function LoginForm() {
  const [state, action, pending] = useActionState<FormState, FormData>(signIn, {});

  return (
    <form action={action} className="flex flex-col gap-3">
      <label className="flex flex-col gap-1 text-sm">
        <span className="font-medium text-ink-muted">Username</span>
        <input
          name="username"
          autoComplete="username"
          required
          placeholder="your username"
          className="field"
        />
      </label>

      <label className="flex flex-col gap-1 text-sm">
        <span className="font-medium text-ink-muted">Password</span>
        <input
          type="password"
          name="password"
          autoComplete="current-password"
          required
          placeholder="Your password"
          className="field"
        />
      </label>

      {state.error ? <p className="field-error">{state.error}</p> : null}

      <button type="submit" disabled={pending} className="btn btn-primary mt-1 py-2.5">
        {pending ? "One moment…" : "Sign in"}
      </button>
    </form>
  );
}

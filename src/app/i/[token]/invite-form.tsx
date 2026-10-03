"use client";

import { useActionState } from "react";
import { redeemInviteAction, type FormState } from "@/server/account";

export function InviteForm({ token, username }: { token: string; username: string | null }) {
  const [state, action, pending] = useActionState<FormState, FormData>(redeemInviteAction, {});

  return (
    <form action={action} className="flex flex-col gap-3">
      <input type="hidden" name="token" value={token} />
      {username ? (
        <p className="text-sm text-ink-muted">
          Username: <strong className="font-semibold text-ink">{username}</strong>
        </p>
      ) : null}

      <label className="flex flex-col gap-1 text-sm">
        <span className="font-medium text-ink-muted">Choose a password</span>
        <input
          type="password"
          name="password"
          autoComplete="new-password"
          minLength={8}
          required
          placeholder="At least 8 characters"
          className="field"
        />
      </label>

      <label className="flex flex-col gap-1 text-sm">
        <span className="font-medium text-ink-muted">Repeat it</span>
        <input
          type="password"
          name="confirm"
          autoComplete="new-password"
          minLength={8}
          required
          placeholder="Same password"
          className="field"
        />
      </label>

      {state.error ? <p className="field-error">{state.error}</p> : null}

      <button type="submit" disabled={pending} className="btn btn-primary mt-1 py-2.5">
        {pending ? "Saving…" : "Set password and continue"}
      </button>
    </form>
  );
}

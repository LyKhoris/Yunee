"use client";

import { useActionState, useEffect, useRef, useState } from "react";
import { createMemberAction, type AdminState } from "@/server/admin";

/**
 * Invite someone: the form the founder used to run as `members add`. On success
 * the action redirects to the invite page, so there is no success state here.
 */
export function NewMemberDialog() {
  const dialog = useRef<HTMLDialogElement>(null);
  const [open, setOpen] = useState(false);
  const [state, action, pending] = useActionState<AdminState, FormData>(createMemberAction, {});

  useEffect(() => {
    const element = dialog.current;
    if (!element) return;
    if (open && !element.open) element.showModal();
    if (!open && element.open) element.close();
  }, [open]);

  return (
    <>
      <button type="button" className="btn btn-primary" onClick={() => setOpen(true)}>
        Invite someone
      </button>

      <dialog
        ref={dialog}
        onClose={() => setOpen(false)}
        className="w-[min(26rem,92vw)] rounded-card border border-hairline bg-surface-1 p-0 text-ink backdrop:bg-black/60"
      >
        <form action={action} className="flex flex-col gap-3 p-6">
          <div className="flex items-start justify-between gap-4">
            <div>
              <h2 className="text-lg font-semibold">Invite someone</h2>
              <p className="mt-1 text-sm text-ink-muted">
                They get a one-time link and choose their own password.
              </p>
            </div>
            <button
              type="button"
              onClick={() => setOpen(false)}
              className="btn btn-quiet -mr-2 -mt-2 px-3"
              aria-label="Close"
            >
              ✕
            </button>
          </div>

          <label className="flex flex-col gap-1 text-sm">
            <span className="font-medium text-ink-muted">Name</span>
            <input name="name" required placeholder="Ada Lovelace" className="field" />
          </label>

          <label className="flex flex-col gap-1 text-sm">
            <span className="font-medium text-ink-muted">Username</span>
            <input
              name="username"
              required
              placeholder="ada"
              pattern="[A-Za-z0-9._-]+"
              autoCapitalize="none"
              autoComplete="off"
              className="field"
            />
            <span className="text-xs text-ink-subtle">
              What they sign in with. Letters, numbers, dots, dashes, underscores.
            </span>
          </label>

          <label className="flex flex-col gap-1 text-sm">
            <span className="font-medium text-ink-muted">Email (optional)</span>
            <input name="email" type="email" placeholder="ada@example.com" className="field" />
          </label>

          <label className="flex flex-col gap-1 text-sm">
            <span className="font-medium text-ink-muted">Access period in days (optional)</span>
            <input name="days" type="number" min="1" placeholder="120" className="field" />
            <span className="text-xs text-ink-subtle">Leave blank for access that never expires.</span>
          </label>

          <label className="flex items-center gap-2 text-sm">
            <input type="checkbox" name="isAdmin" className="accent-[var(--yunee-accent)]" />
            <span>Make them an admin too</span>
          </label>

          {state.error ? <p className="field-error">{state.error}</p> : null}

          <div className="mt-1 flex justify-end gap-2">
            <button type="button" className="btn btn-quiet" onClick={() => setOpen(false)}>
              Cancel
            </button>
            <button type="submit" disabled={pending} className="btn btn-primary">
              {pending ? "Creating…" : "Create + get link"}
            </button>
          </div>
        </form>
      </dialog>
    </>
  );
}

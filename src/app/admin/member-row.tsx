"use client";

import { useActionState } from "react";
import type { AccessStatus } from "@/lib/access";
import type { MemberView } from "@/lib/members";
import {
  deleteAction,
  extendAction,
  resetAction,
  setActiveAction,
  setAdminAction,
  type AdminState,
} from "@/server/admin";

type Pill = { label: string; className: string };

/**
 * One roster row: who they are, their status, and the actions the founder used
 * to run from the CLI. Every action is a form post to a server action that
 * re-checks admin — nothing here is trusted.
 *
 * Takes a `MemberView`, never a raw row: the token and password hash are
 * server-only and must not reach the browser.
 */
export function MemberRow({
  member,
  status,
  pill,
  self = false,
}: {
  member: MemberView;
  status: AccessStatus;
  pill: Pill;
  self?: boolean;
}) {
  const [activeState, submitActive, activePending] = useActionState<AdminState, FormData>(
    setActiveAction,
    {},
  );
  const [extendState, submitExtend, extendPending] = useActionState<AdminState, FormData>(
    extendAction,
    {},
  );
  const [adminState, submitAdmin, adminPending] = useActionState<AdminState, FormData>(
    setAdminAction,
    {},
  );
  const [resetState, submitReset, resetPending] = useActionState<AdminState, FormData>(
    resetAction,
    {},
  );
  const [deleteState, submitDelete, deletePending] = useActionState<AdminState, FormData>(
    deleteAction,
    {},
  );

  const busy = activePending || extendPending || adminPending || resetPending || deletePending;
  const message =
    activeState.error || extendState.error || adminState.error || resetState.error || deleteState.error;
  const notice = activeState.notice || extendState.notice || adminState.notice;

  const expires = member.access_expires_at ? member.access_expires_at.slice(0, 10) : "never";

  return (
    <li className="flex flex-col gap-3 p-4 sm:flex-row sm:items-center sm:justify-between">
      <div className="min-w-0">
        <div className="flex flex-wrap items-center gap-2">
          <span className="font-semibold">{member.name}</span>
          <span className="text-sm text-ink-subtle">@{member.username ?? "—"}</span>
          <span className={pill.className}>{pill.label}</span>
          {member.is_admin ? <span className="pill pill-quiet">admin</span> : null}
          {self ? <span className="pill pill-quiet">you</span> : null}
          {member.invite_pending ? <span className="pill pill-quiet">invite pending</span> : null}
        </div>
        <p className="mt-1 text-xs text-ink-subtle">
          {status === "ok"
            ? `until ${expires}`
            : status === "expired"
              ? `lapsed ${expires}`
              : "no access"}
          {member.email ? ` · ${member.email}` : ""}
        </p>
        {message ? <p className="field-error mt-2">{message}</p> : null}
        {notice ? <p className="mt-2 text-xs text-ink-muted">{notice}</p> : null}
      </div>

      <div className="flex flex-wrap items-center gap-2">
        <form action={submitExtend}>
          <input type="hidden" name="id" value={member.id} />
          <input type="hidden" name="days" value={30} />
          <button type="submit" disabled={busy} className="btn btn-ghost">
            +30 days
          </button>
        </form>

        <form action={submitActive}>
          <input type="hidden" name="id" value={member.id} />
          <input type="hidden" name="active" value={member.active ? "0" : "1"} />
          <button type="submit" disabled={busy} className="btn btn-ghost">
            {member.active ? "Revoke" : "Enable"}
          </button>
        </form>

        <form action={submitReset}>
          <input type="hidden" name="id" value={member.id} />
          <button type="submit" disabled={busy} className="btn btn-quiet">
            Reset link
          </button>
        </form>

        <form action={submitAdmin}>
          <input type="hidden" name="id" value={member.id} />
          <input type="hidden" name="admin" value={member.is_admin ? "0" : "1"} />
          <button type="submit" disabled={busy} className="btn btn-quiet">
            {member.is_admin ? "Remove admin" : "Make admin"}
          </button>
        </form>

        {!self ? (
          <form
            action={submitDelete}
            onSubmit={(event) => {
              if (!confirm(`Delete ${member.name}? This cannot be undone.`)) event.preventDefault();
            }}
          >
            <input type="hidden" name="id" value={member.id} />
            <button type="submit" disabled={busy} className="btn btn-danger">
              Delete
            </button>
          </form>
        ) : null}
      </div>
    </li>
  );
}

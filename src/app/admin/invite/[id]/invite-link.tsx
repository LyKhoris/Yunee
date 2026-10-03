"use client";

import { useState } from "react";

/** The invite URL, with a copy button so the founder never has to select it. */
export function InviteLink({ url }: { url: string }) {
  const [copied, setCopied] = useState(false);

  async function copy() {
    try {
      await navigator.clipboard.writeText(url);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      setCopied(false);
    }
  }

  return (
    <div className="flex flex-col gap-2 sm:flex-row sm:items-center">
      <code className="field min-w-0 flex-1 truncate font-mono text-xs">{url}</code>
      <button type="button" onClick={copy} className="btn btn-primary">
        {copied ? "Copied" : "Copy link"}
      </button>
    </div>
  );
}

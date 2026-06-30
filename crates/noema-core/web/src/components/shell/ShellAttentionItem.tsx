import { AlertTriangle } from "lucide-react";
import type { ShellAttention } from "./AppShell";

export function ShellAttentionItem({ attention }: { attention: ShellAttention }) {
  return (
    <div
      className="grid gap-1 rounded-md border border-[color-mix(in_srgb,var(--clay-600)_32%,transparent)] bg-[var(--clay-50)] px-3 py-2.5 text-[var(--red-700)]"
      role="status"
    >
      <span className="flex items-center gap-2 text-xs font-semibold">
        <AlertTriangle className="size-3.5" aria-hidden="true" />
        {attention.title}
      </span>
      <span className="text-xs leading-snug">{attention.message}</span>
    </div>
  );
}

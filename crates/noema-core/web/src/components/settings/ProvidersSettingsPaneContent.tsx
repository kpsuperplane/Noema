import { RefreshCw } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  providerStatusLabel,
  providerTechnicalRows,
  type ProviderSettingsAccount
} from "./providerMetadata";

export function ProvidersSettingsPaneContent({
  accounts,
  loading,
  error,
  onRetry
}: {
  accounts: readonly ProviderSettingsAccount[];
  loading: boolean;
  error: string | null;
  onRetry: () => void;
}) {
  const account = accounts[0] ?? null;

  if (loading) {
    return <p className="m-0 text-sm text-muted-foreground">Loading provider metadata...</p>;
  }

  if (error) {
    return (
      <div className="grid gap-3 rounded-md border border-[var(--border-subtle)] bg-white p-4">
        <p className="m-0 text-sm text-muted-foreground">{error}</p>
        <Button type="button" variant="outline" className="w-fit" onClick={onRetry}>
          <RefreshCw className="size-4" aria-hidden="true" />
          Retry
        </Button>
      </div>
    );
  }

  if (!account) {
    return (
      <div className="rounded-md border border-[var(--border-subtle)] bg-white p-4">
        <p className="m-0 text-sm text-muted-foreground">
          No provider accounts are available. Settings is open, but Noema did not return a
          connected provider account.
        </p>
      </div>
    );
  }

  const rows = providerTechnicalRows(account);

  return (
    <div className="grid gap-4">
      <div className="grid gap-2 rounded-md border border-[var(--border-subtle)] bg-white p-4">
        <p className="m-0 text-xs font-medium uppercase tracking-[0.12em] text-[var(--text-accent)]">
          Connected provider
        </p>
        <div className="flex flex-wrap items-center gap-3">
          <h2 className="m-0 font-heading text-2xl leading-tight tracking-normal text-foreground">
            {account.displayName}
          </h2>
          <Badge variant="outline">{providerStatusLabel(account.status)}</Badge>
        </div>
      </div>

      <div className="overflow-hidden rounded-md border border-[var(--border-subtle)] bg-white">
        <dl className="m-0 divide-y divide-[var(--border-subtle)]">
          {rows.map((row) => (
            <div
              key={row.label}
              className="grid grid-cols-[minmax(120px,220px)_1fr] gap-4 px-4 py-3 max-[760px]:grid-cols-1 max-[760px]:gap-1"
            >
              <dt className="text-sm font-medium text-muted-foreground">{row.label}</dt>
              <dd className="m-0 min-w-0 break-words font-mono text-sm text-foreground">
                {row.value}
              </dd>
            </div>
          ))}
        </dl>
      </div>
    </div>
  );
}

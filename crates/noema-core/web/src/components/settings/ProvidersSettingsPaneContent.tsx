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

  if (accounts.length === 0) {
    return (
      <div className="rounded-md border border-[var(--border-subtle)] bg-white p-4">
        <p className="m-0 text-sm text-muted-foreground">
          No provider accounts are available. Settings is open, but Noema did not return a
          connected provider account.
        </p>
      </div>
    );
  }

  return (
    <div className="grid gap-3">
      {accounts.map((account) => {
        const rows = providerTechnicalRows(account);
        return (
          <article
            key={`${account.providerKind}:${account.accountKey}`}
            className="grid gap-3 rounded-md border border-[var(--border-subtle)] bg-white p-4"
          >
            <div className="flex flex-wrap items-center gap-3">
              <h2 className="m-0 font-heading text-xl leading-tight tracking-normal text-foreground">
                {account.displayName}
              </h2>
              <Badge variant="outline">{providerStatusLabel(account.status)}</Badge>
            </div>
            {account.providerKind === "foundation_local" ? (
              <p className="m-0 text-sm text-muted-foreground">
                Local Apple model support is managed by this machine. Choose the model for each
                agent in Agents.
              </p>
            ) : null}
            <dl className="m-0 grid gap-2">
              {rows.map((row) => (
                <div
                  key={row.label}
                  className="grid grid-cols-[minmax(120px,220px)_1fr] gap-4 max-[760px]:grid-cols-1 max-[760px]:gap-1"
                >
                  <dt className="text-sm font-medium text-muted-foreground">{row.label}</dt>
                  <dd className="m-0 min-w-0 break-words font-mono text-sm text-foreground">
                    {row.value}
                  </dd>
                </div>
              ))}
            </dl>
          </article>
        );
      })}
    </div>
  );
}

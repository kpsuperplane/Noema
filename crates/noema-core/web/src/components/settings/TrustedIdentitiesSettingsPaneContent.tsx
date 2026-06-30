import { RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  trustedIdentityRows,
  type TrustedIdentitySelector
} from "./trustedIdentityMetadata";

export function TrustedIdentitiesSettingsPaneContent({
  selectors,
  loading,
  error,
  onRetry
}: {
  selectors: readonly TrustedIdentitySelector[];
  loading: boolean;
  error: string | null;
  onRetry: () => void;
}) {
  if (loading) {
    return <p className="m-0 text-sm text-muted-foreground">Loading trusted identities...</p>;
  }

  if (error) {
    return (
      <div className="grid gap-3 rounded-md border border-[var(--border-subtle)] bg-white p-4">
        <p className="m-0 text-sm text-muted-foreground">
          Trusted identity selectors could not be loaded.
        </p>
        <Button type="button" variant="outline" className="w-fit" onClick={onRetry}>
          <RefreshCw className="size-4" aria-hidden="true" />
          Retry
        </Button>
      </div>
    );
  }

  if (selectors.length === 0) {
    return (
      <div className="rounded-md border border-[var(--border-subtle)] bg-white p-4">
        <p className="m-0 text-sm text-muted-foreground">
          No trusted identity selectors are configured.
        </p>
      </div>
    );
  }

  return (
    <div className="overflow-hidden rounded-md border border-[var(--border-subtle)] bg-white">
      <div className="grid min-w-0 divide-y divide-[var(--border-subtle)]">
        {selectors.map((selector) => (
          <article key={selector.selectorId} className="grid gap-3 p-4">
            <div className="min-w-0">
              <h2 className="m-0 break-words font-heading text-lg leading-tight tracking-normal text-foreground">
                {selector.normalizedValue}
              </h2>
            </div>
            <dl className="m-0 grid gap-2">
              {trustedIdentityRows(selector).map((row) => (
                <div
                  key={row.label}
                  className="grid grid-cols-[minmax(110px,160px)_1fr] gap-4 max-[760px]:grid-cols-1 max-[760px]:gap-1"
                >
                  <dt className="text-sm font-medium text-muted-foreground">{row.label}</dt>
                  <dd className="m-0 min-w-0 break-words font-mono text-sm text-foreground">
                    {row.value}
                  </dd>
                </div>
              ))}
            </dl>
          </article>
        ))}
      </div>
    </div>
  );
}

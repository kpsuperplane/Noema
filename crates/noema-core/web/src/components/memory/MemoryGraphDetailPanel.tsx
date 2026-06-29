import { useQuery } from "@apollo/client/react"

import { MemoryGraphClaimDetailDocument } from "@/generated/graphql"
import type { NormalizedMemoryGraphEdge } from "@/memoryGraph"

export function MemoryGraphDetailPanel({ selectedEdge }: { selectedEdge: NormalizedMemoryGraphEdge | null }) {
  const detail = useQuery(MemoryGraphClaimDetailDocument, {
    variables: { claimId: selectedEdge?.claimId ?? "" },
    skip: !selectedEdge,
    fetchPolicy: "cache-and-network",
  })

  if (!selectedEdge) {
    return (
      <aside className="grid min-h-0 content-start gap-2 overflow-y-auto border-l border-[var(--border-subtle)] bg-white p-5">
        <h2 className="m-0 font-heading text-lg tracking-normal">Memory detail</h2>
        <p className="m-0 text-sm text-muted-foreground">Select a claim edge to inspect evidence.</p>
      </aside>
    )
  }

  const claim = detail.data?.memoryClaim
  const claimUnavailable = !detail.loading && !detail.error && detail.data && !claim

  return (
    <aside className="grid min-h-0 content-start gap-4 overflow-y-auto border-l border-[var(--border-subtle)] bg-white p-5">
      <div className="grid gap-1">
        <p className="m-0 font-mono text-[11px] tracking-[0.12em] text-[var(--text-accent)] uppercase">
          {claim?.status ?? selectedEdge.status}
        </p>
        <h2 className="m-0 font-heading text-lg leading-snug tracking-normal">
          {claim?.predicateLabel ?? selectedEdge.predicateLabel}
        </h2>
        <p className="m-0 text-sm text-muted-foreground">{claim?.fact ?? selectedEdge.fact}</p>
      </div>

      <dl className="grid grid-cols-2 gap-3 text-sm">
        <div>
          <dt className="text-xs text-muted-foreground">Sensitivity</dt>
          <dd className="m-0">{claim?.sensitivity ?? selectedEdge.sensitivity}</dd>
        </div>
        <div>
          <dt className="text-xs text-muted-foreground">Evidence</dt>
          <dd className="m-0">{claim?.evidenceCount ?? selectedEdge.evidenceCount}</dd>
        </div>
      </dl>

      {detail.loading ? <p className="m-0 text-sm text-muted-foreground">Loading evidence...</p> : null}
      {detail.error ? <p className="m-0 text-sm text-destructive">{detail.error.message}</p> : null}
      {claimUnavailable ? (
        <p className="m-0 text-sm text-muted-foreground">
          Claim detail is unavailable. The graph edge summary remains visible.
        </p>
      ) : null}

      {claim?.evidence.length ? (
        <div className="grid gap-2">
          <h3 className="m-0 text-sm font-semibold">Evidence</h3>
          {claim.evidence.map((evidence, index) => {
            const key =
              evidence.evidenceId ??
              `${evidence.sourceItemId ?? "source"}:${evidence.observedAt ?? evidence.createdAt}:${index}`

            return (
              <article key={key} className="rounded-md border border-[var(--border-subtle)] p-3">
                <strong className="block text-xs">{evidence.authority}</strong>
                {evidence.excerpt ? (
                  <p className="m-0 mt-1 text-sm text-muted-foreground">{evidence.excerpt}</p>
                ) : null}
                {evidence.sourceItemId ? (
                  <code className="mt-2 block text-[11px] text-muted-foreground">{evidence.sourceItemId}</code>
                ) : null}
              </article>
            )
          })}
        </div>
      ) : null}
    </aside>
  )
}

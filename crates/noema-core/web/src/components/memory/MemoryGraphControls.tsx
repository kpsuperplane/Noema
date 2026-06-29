import { Search } from "lucide-react"

import { Button } from "@/components/ui/button"
import { graphStatusOptions, toggleStatus } from "@/memoryGraph"

export function MemoryGraphControls({
  query,
  statuses,
  limit,
  truncated,
  onQueryChange,
  onStatusesChange,
}: {
  query: string
  statuses: string[]
  limit: number
  truncated: boolean
  onQueryChange: (value: string) => void
  onStatusesChange: (value: string[]) => void
}) {
  return (
    <section
      className="grid gap-3 border-b border-[var(--border-subtle)] bg-white px-5 py-4"
      aria-label="Memory graph filters"
    >
      <label className="flex min-h-10 items-center gap-2 rounded-md border border-[var(--border-subtle)] bg-[var(--surface-sunken)] px-3 text-sm">
        <Search className="size-4 text-muted-foreground" aria-hidden="true" />
        <input
          className="min-w-0 flex-1 bg-transparent text-sm outline-none placeholder:text-muted-foreground"
          value={query}
          placeholder="Search memories"
          onChange={(event) => onQueryChange(event.target.value)}
        />
      </label>

      <div className="flex flex-wrap items-center gap-2">
        {graphStatusOptions().map((status) => {
          const active = statuses.includes(status.value)

          return (
            <Button
              key={status.value}
              type="button"
              variant={active ? "secondary" : "outline"}
              size="sm"
              aria-pressed={active}
              onClick={() => onStatusesChange(toggleStatus(statuses, status.value))}
            >
              {status.label}
            </Button>
          )
        })}
        <span className="ml-auto text-xs text-muted-foreground">
          Limit {limit}
          {truncated ? " / truncated" : ""}
        </span>
      </div>
    </section>
  )
}

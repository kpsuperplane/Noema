import { Network } from "lucide-react";
import { Button } from "@/components/ui/button";

export function MemoryHomePage({ onOpenGraph }: { onOpenGraph: () => void }) {
  return (
    <section className="mx-auto grid min-h-0 w-[min(960px,100%)] gap-5 px-6 py-7 max-[760px]:px-5">
      <div className="grid gap-2">
        <p className="m-0 font-mono text-[11px] tracking-[0.12em] text-[var(--text-accent)] uppercase">
          Memory
        </p>
        <h1 className="m-0 font-heading text-[32px] leading-[1.1] tracking-normal text-foreground">
          Memory management
        </h1>
        <p className="m-0 max-w-[640px] text-sm text-muted-foreground">
          Inspect what Noema remembers and how those memories connect.
        </p>
      </div>

      <div className="grid gap-3 rounded-md border border-[var(--border-subtle)] bg-white p-4">
        <div className="flex items-center gap-3">
          <Network className="size-5 text-[var(--text-accent)]" aria-hidden="true" />
          <div className="min-w-0">
            <strong className="block text-sm font-semibold">Memory Graph</strong>
            <span className="block text-sm text-muted-foreground">
              View entities and claim edges from accessible memories.
            </span>
          </div>
        </div>
        <Button type="button" className="w-fit" onClick={onOpenGraph}>
          Open graph
        </Button>
      </div>
    </section>
  );
}

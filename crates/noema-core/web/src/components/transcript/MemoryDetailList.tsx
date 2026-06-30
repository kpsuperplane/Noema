import type { MemoryCardData } from "../../memoryCards";
import { memoryDetailItems } from "./markerModel";
import { MemoryDetailRow } from "./MemoryDetailRow";

export function MemoryDetailList({ memories }: { memories: MemoryCardData[] }) {
  const items = memoryDetailItems(memories);
  if (items.length === 0) {
    return null;
  }

  return (
    <div className="mt-3 grid gap-3 border-t border-[var(--border-subtle)] pt-2.5">
      {items.map((item, index) => (
        <section className="grid gap-2" key={`${item.title}:${index}`}>
          {items.length > 1 ? (
            <p className="m-0 text-[13px] font-medium text-foreground [overflow-wrap:anywhere]">{item.title}</p>
          ) : null}
          <dl className="grid gap-2">
            {item.rows.map((row) => (
              <MemoryDetailRow key={row.label} label={row.label} value={row.value} />
            ))}
          </dl>
        </section>
      ))}
    </div>
  );
}

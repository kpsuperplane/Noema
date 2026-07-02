import * as stylex from "@stylexjs/stylex";
import type { MemoryCardData } from "../../memoryCards";
import { memoryDetailItems } from "./markerModel";
import { MemoryDetailRow } from "./MemoryDetailRow";

const styles = stylex.create({
  root: {
    display: "grid",
    gap: 12,
    marginTop: 12,
    borderTopWidth: 1,
    borderTopStyle: "solid",
    borderTopColor: "var(--border-subtle)",
    paddingTop: 10
  },
  section: {
    display: "grid",
    gap: 8
  },
  title: {
    margin: 0,
    overflowWrap: "anywhere",
    color: "var(--text-primary)",
    fontSize: 13,
    fontWeight: 500
  },
  rows: {
    display: "grid",
    gap: 8
  }
});

export function MemoryDetailList({ memories }: { memories: MemoryCardData[] }) {
  const items = memoryDetailItems(memories);
  if (items.length === 0) {
    return null;
  }

  return (
    <div {...stylex.props(styles.root)}>
      {items.map((item, index) => (
        <section {...stylex.props(styles.section)} key={`${item.title}:${index}`}>
          {items.length > 1 ? <p {...stylex.props(styles.title)}>{item.title}</p> : null}
          <dl {...stylex.props(styles.rows)}>
            {item.rows.map((row) => (
              <MemoryDetailRow key={row.label} label={row.label} value={row.value} />
            ))}
          </dl>
        </section>
      ))}
    </div>
  );
}

import { Button } from "@astryxdesign/core/Button";
import { TextInput } from "@astryxdesign/core/TextInput";
import * as stylex from "@stylexjs/stylex";
import { Search } from "lucide-react";

import { graphStatusDefaults, graphStatusOptions, toggleStatus } from "@/memoryGraph";

export function MemoryGraphControls({
  query,
  statuses,
  limit,
  truncated,
  onQueryChange,
  onStatusesChange,
}: {
  query: string;
  statuses: string[];
  limit: number;
  truncated: boolean;
  onQueryChange: (value: string) => void;
  onStatusesChange: (value: string[]) => void;
}) {
  function handleStatusClick(status: string) {
    const nextStatuses = toggleStatus(statuses, status);

    if (nextStatuses.length > 0) {
      onStatusesChange(nextStatuses);
    }
  }

  function handleReset() {
    onQueryChange("");
    onStatusesChange(graphStatusDefaults());
  }

  const hasDefaultStatuses = statuses.every((status) => graphStatusDefaults().includes(status));
  const canReset = query.trim().length > 0 || statuses.length !== graphStatusDefaults().length || !hasDefaultStatuses;

  return (
    <section {...stylex.props(styles.root)} aria-label="Memory graph filters">
      <TextInput
        label="Search memories"
        isLabelHidden
        value={query}
        placeholder="Search memories"
        width="100%"
        startIcon={Search}
        hasClear
        onChange={onQueryChange}
      />

      <div {...stylex.props(styles.statusRow)}>
        {graphStatusOptions().map((status) => {
          const active = statuses.includes(status.value);

          return (
            <Button
              key={status.value}
              type="button"
              variant={active ? "secondary" : "ghost"}
              size="sm"
              label={status.label}
              aria-pressed={active}
              onClick={() => handleStatusClick(status.value)}
            >
              {status.label}
            </Button>
          );
        })}
        <Button
          type="button"
          variant="ghost"
          size="sm"
          label="Reset filters"
          isDisabled={!canReset}
          onClick={handleReset}
        >
          Reset
        </Button>
        <span {...stylex.props(styles.summary)}>
          Limit {limit}
          {truncated ? " / truncated" : ""}
        </span>
      </div>
    </section>
  );
}

const styles = stylex.create({
  root: {
    display: "grid",
    gap: 12,
    borderBottom: "1px solid var(--border-subtle)",
    backgroundColor: "white",
    padding: "16px 20px"
  },
  statusRow: {
    display: "flex",
    flexWrap: "wrap",
    alignItems: "center",
    gap: 8
  },
  summary: {
    marginLeft: "auto",
    fontSize: 12,
    color: "var(--muted-foreground)",
    "@media (max-width: 640px)": {
      flexBasis: "100%",
      marginLeft: 0
    }
  }
});

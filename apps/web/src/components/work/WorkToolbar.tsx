import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import { Plus, Search } from "lucide-react";
import { ShellSectionHeader } from "@/components/shell/ShellSectionHeader";

export function WorkToolbar({
  queryDraft,
  terminal,
  onQueryChange,
  onTerminalChange,
  onNewTask
}: {
  queryDraft: string;
  terminal: "all" | "completed" | "cancelled";
  onQueryChange: (query: string) => void;
  onTerminalChange: (value: "all" | "completed" | "cancelled") => void;
  onNewTask: () => void;
}) {
  return (
    <ShellSectionHeader
      actions={(
        <Button
          type="button"
          size="sm"
          variant="primary"
          label="New task"
          icon={<Plus aria-hidden="true" size={15} />}
          onClick={onNewTask}
        />
      )}
      navigationLabel="Tasks"
      title="Tasks"
      titleId="work-page-title"
    >
      <div role="group" aria-label="Task filters" {...stylex.props(styles.filters)}>
        <label {...stylex.props(styles.search)}>
          <Search aria-hidden="true" size={13} />
          <span {...stylex.props(styles.srOnly)}>Search history</span>
          <input type="search" value={queryDraft} placeholder="Search history" {...stylex.props(styles.searchInput)} onChange={(event) => onQueryChange(event.currentTarget.value)} />
        </label>
        <label {...stylex.props(styles.field)}>
          <span {...stylex.props(styles.srOnly)}>History status</span>
          <select value={terminal} {...stylex.props(styles.control, styles.terminal)} onChange={(event) => onTerminalChange(event.currentTarget.value as typeof terminal)}>
            <option value="all">Done and cancelled</option><option value="completed">Done</option><option value="cancelled">Cancelled</option>
          </select>
        </label>
      </div>
    </ShellSectionHeader>
  );
}

const styles = stylex.create({
  filters: {
    display: "flex",
    minWidth: 0,
    alignItems: "center",
    gap: "var(--spacing-1)",
    "@media (max-width: 480px)": {
      width: "100%"
    }
  },
  field: { minWidth: 0 },
  control: {
    minHeight: 30,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--noema-border-subtle)",
    borderRadius: 6,
    backgroundColor: "var(--noema-surface-card)",
    paddingInline: "var(--spacing-2)",
    color: "var(--noema-text-primary)",
    font: "inherit",
    fontSize: 12,
    ":focus-visible": { outlineWidth: 2, outlineStyle: "solid", outlineColor: "var(--noema-pine-500)", outlineOffset: 1 }
  },
  terminal: { maxWidth: 160 },
  search: { display: "grid", gridTemplateColumns: "auto minmax(0, 1fr)", width: 176, minHeight: 30, flexShrink: 0, alignItems: "center", gap: "var(--spacing-1)", borderWidth: 1, borderStyle: "solid", borderColor: "var(--noema-border-subtle)", borderRadius: 6, backgroundColor: "var(--noema-surface-card)", paddingInline: "var(--spacing-2)", color: "var(--noema-text-muted)", "@media (max-width: 760px)": { flexShrink: 1 } },
  searchInput: { minWidth: 0, width: "100%", borderWidth: 0, outline: "none", backgroundColor: "transparent", padding: 0, color: "var(--noema-text-primary)", font: "inherit", fontSize: 12 },
  srOnly: { position: "absolute", width: 1, height: 1, overflow: "hidden", clip: "rect(0 0 0 0)", whiteSpace: "nowrap" }
});

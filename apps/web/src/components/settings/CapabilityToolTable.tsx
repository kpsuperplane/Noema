import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import {
  Check,
  CircleOff,
  Loader2,
  Pencil,
  RefreshCcw,
  ToggleLeft,
  ToggleRight,
  TriangleAlert,
  X
} from "lucide-react";
import type { CapabilityConnectionQuery } from "@/generated/graphql";

type ManagedTool = CapabilityConnectionQuery["capabilityTools"][number];
type HintKey = "readOnly" | "idempotent" | "destructive" | "openWorld";

const hintColumns = [
  { field: "readOnly", label: "Only reads information", shortLabel: "Only reads" },
  { field: "idempotent", label: "Same effect if repeated", shortLabel: "Safe to repeat" },
  { field: "destructive", label: "Can delete or overwrite", shortLabel: "Can delete" },
  { field: "openWorld", label: "Can act outside Noema", shortLabel: "Outside Noema" }
] as const;

export function CapabilityToolTable({
  tools,
  loading,
  busy,
  onEdit,
  onReset,
  onToggle
}: {
  tools: ManagedTool[];
  loading: boolean;
  busy: boolean;
  onEdit: (tool: ManagedTool) => void;
  onReset: (tool: ManagedTool) => void;
  onToggle: (tool: ManagedTool) => void;
}) {
  if (loading && tools.length === 0) {
    return (
      <p {...stylex.props(styles.empty)}>
        <Loader2 aria-hidden="true" size={14} {...stylex.props(styles.spinner)} /> Checking…
      </p>
    );
  }
  if (tools.length === 0) {
    return <p {...stylex.props(styles.empty)}>No tools found.</p>;
  }
  const sharedPrefix = sharedToolPrefix(tools);
  return (
    <div {...stylex.props(styles.frame)}>
      <table aria-label="How Noema understands each tool" {...stylex.props(styles.table)}>
        <thead>
          <tr>
            <th scope="col" {...stylex.props(styles.heading, styles.nameHeading)}>
              Tool
            </th>
            {hintColumns.map(({ label, shortLabel }) => (
              <th
                key={label}
                scope="col"
                title={label}
                aria-label={label}
                {...stylex.props(styles.heading, styles.permissionColumn)}
              >
                {shortLabel}
              </th>
            ))}
            <th
              scope="col"
              aria-label="Actions"
              {...stylex.props(styles.heading, styles.actionsColumn)}
            />
          </tr>
        </thead>
        <tbody>
          {tools.map((tool) => (
            <tr key={tool.toolId}>
              <th
                scope="row"
                title={tool.name}
                {...stylex.props(styles.cell, styles.nameCell)}
              >
                <span {...stylex.props(styles.toolName)}>
                  <span>{displayToolName(tool.name, sharedPrefix)}</span>
                  <ToolStatus status={tool.status} />
                </span>
              </th>
              {hintColumns.map(({ field, label }) => (
                <td key={field} {...stylex.props(styles.cell, styles.permissionColumn)}>
                  <ToolHintValue label={label} hint={tool[field]} />
                </td>
              ))}
              <td {...stylex.props(styles.cell, styles.actionsColumn)}>
                <div {...stylex.props(styles.actions)}>
                  <Button
                    type="button"
                    size="sm"
                    variant="ghost"
                    label="Edit"
                    tooltip="Edit"
                    icon={<Pencil aria-hidden="true" size={16} />}
                    isIconOnly
                    onClick={() => onEdit(tool)}
                  />
                  <Button
                    type="button"
                    size="sm"
                    variant="ghost"
                    label="Reset"
                    tooltip="Reset"
                    icon={<RefreshCcw aria-hidden="true" size={16} />}
                    isIconOnly
                    isDisabled={busy}
                    onClick={() => onReset(tool)}
                  />
                  <Button
                    type="button"
                    size="sm"
                    variant="ghost"
                    label={tool.enabled ? "Turn off" : "Turn on"}
                    tooltip={tool.enabled ? "Turn off" : "Turn on"}
                    icon={tool.enabled
                      ? <ToggleLeft aria-hidden="true" size={16} />
                      : <ToggleRight aria-hidden="true" size={16} />}
                    isIconOnly
                    isDisabled={busy}
                    onClick={() => onToggle(tool)}
                  />
                </div>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function ToolStatus({ status }: { status: string }) {
  if (status === "ready") return null;
  const label = statusLabel(status);
  return (
    <span
      role="img"
      aria-label={label}
      title={label}
      {...stylex.props(
        styles.statusIcon,
        status === "defaulted"
          ? styles.statusWarning
          : status === "pending" || status === "disabled"
            ? styles.statusMuted
            : styles.statusError
      )}
    >
      {status === "pending" ? (
        <Loader2 aria-hidden="true" size={16} {...stylex.props(styles.spinner)} />
      ) : status === "defaulted" ? (
        <TriangleAlert aria-hidden="true" size={16} />
      ) : status === "disabled" ? (
        <CircleOff aria-hidden="true" size={16} />
      ) : (
        <X aria-hidden="true" size={16} />
      )}
    </span>
  );
}

function ToolHintValue({
  label,
  hint
}: {
  label: string;
  hint: ManagedTool[HintKey];
}) {
  const state = hint.value === null ? "Checking" : hint.value ? "Yes" : "No";
  const description = `${label}: ${state} · ${toolHintSourceDescription(hint.source)}`;
  return (
    <span
      role="img"
      aria-label={description}
      title={description}
      {...stylex.props(styles.hintValue)}
    >
      {hint.value === null ? (
        <Loader2 aria-hidden="true" size={14} {...stylex.props(styles.spinner)} />
      ) : hint.value ? (
        <Check aria-hidden="true" size={14} />
      ) : (
        <X aria-hidden="true" size={14} />
      )}
    </span>
  );
}

function sharedToolPrefix(tools: ManagedTool[]) {
  if (tools.length < 2) return "";
  const separator = tools[0].name.indexOf("_");
  if (separator < 1) return "";
  const prefix = tools[0].name.slice(0, separator + 1);
  return tools.every((tool) => tool.name.startsWith(prefix) && tool.name.length > prefix.length)
    ? prefix
    : "";
}

function displayToolName(name: string, sharedPrefix: string) {
  const displayName = name.slice(sharedPrefix.length).replaceAll("_", " ");
  return displayName.charAt(0).toUpperCase() + displayName.slice(1);
}

function statusLabel(status: string) {
  return ({
    pending: "Checking",
    ready: "Ready",
    defaulted: "Cautious",
    disabled: "Off"
  } as Record<string, string>)[status] ?? status;
}

export function toolHintSourceDescription(source: string | null) {
  if (source === "annotation") return "From provider";
  if (source === "model") return "Checked by Noema";
  if (source === "safe_default") return "Safety default";
  if (source === "human") return "Set by you";
  return "Still checking";
}

const styles = stylex.create({
  empty: { display: "flex", alignItems: "center", gap: "var(--spacing-2)", margin: 0, padding: "var(--spacing-4)", color: "var(--muted-foreground)", fontSize: 13 },
  frame: { width: "100%", overflowX: "auto", borderWidth: 1, borderStyle: "solid", borderColor: "var(--border-subtle)", borderRadius: 6 },
  table: { width: "100%", minWidth: 520, borderCollapse: "separate", borderSpacing: 0, tableLayout: "fixed", color: "var(--muted-foreground)" },
  heading: { height: 40, padding: "0 var(--spacing-1)", borderBottomWidth: 1, borderBottomStyle: "solid", borderBottomColor: "var(--border-subtle)", backgroundColor: "var(--surface-raised)", textAlign: "center", fontSize: 11, fontWeight: 500, lineHeight: 1.2, color: "var(--muted-foreground)" },
  cell: { height: 44, padding: "var(--spacing-1)", borderBottomWidth: 1, borderBottomStyle: "solid", borderBottomColor: "var(--border-subtle)", textAlign: "center" },
  nameHeading: { width: "auto", paddingLeft: "var(--spacing-2)", textAlign: "left" },
  nameCell: { paddingLeft: "var(--spacing-2)", textAlign: "left", fontSize: 13, fontWeight: 600, lineHeight: 1.35, overflowWrap: "anywhere", whiteSpace: "normal", color: "var(--foreground)" },
  toolName: { display: "inline-flex", alignItems: "center", gap: "var(--spacing-1)" },
  permissionColumn: { width: 76 },
  actionsColumn: { width: 116, paddingRight: "var(--spacing-2)" },
  statusIcon: { display: "inline-flex", width: 18, height: 18, flexShrink: 0, alignItems: "center", justifyContent: "center" },
  statusWarning: { color: "var(--noema-clay-600)" },
  statusMuted: { color: "var(--noema-text-faint)" },
  statusError: { color: "var(--noema-red-700)" },
  hintValue: { display: "inline-flex", width: 24, height: 20, alignItems: "center", justifyContent: "center" },
  actions: { display: "flex", alignItems: "center", justifyContent: "flex-end", gap: "var(--spacing-0-5)" },
  spinner: { animationName: stylex.keyframes({ to: { transform: "rotate(360deg)" } }), animationDuration: "900ms", animationIterationCount: "infinite", animationTimingFunction: "linear", "@media (prefers-reduced-motion: reduce)": { animationName: "none" } }
});

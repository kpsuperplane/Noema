import { HStack } from "@astryxdesign/core/HStack";
import { IconButton } from "@astryxdesign/core/IconButton";
import { VStack } from "@astryxdesign/core/VStack";
import * as stylex from "@stylexjs/stylex";
import {
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
import { settingsStatusLabel } from "./settingsStatus";

type ManagedTool = CapabilityConnectionQuery["capabilityTools"][number];

const hintFields = ["readOnly", "idempotent", "destructive", "openWorld"] as const;

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
      <HStack as="p" gap={2} vAlign="center" {...stylex.props(styles.empty)}>
        <Loader2 aria-hidden="true" size={14} {...stylex.props(styles.spinner)} /> Checking…
      </HStack>
    );
  }
  if (tools.length === 0) {
    return <p {...stylex.props(styles.empty)}>No tools found.</p>;
  }
  const sharedPrefix = sharedToolPrefix(tools);
  return (
    <VStack as="ul" gap={0} aria-label="Tool settings" {...stylex.props(styles.list)}>
      {tools.map((tool) => {
        const hasHumanOverride = hintFields.some((field) => tool[field].source === "human");
        return (
          <HStack as="li" key={tool.toolId} gap={1} vAlign="center"
            {...stylex.props(styles.row, tool.status !== "ready" && styles.unavailableRow)}>
            <VStack gap={0.5} {...stylex.props(styles.toolCopy)}>
              <HStack as="span" gap={1} vAlign="center">
                <strong {...stylex.props(styles.toolName)}>{displayToolName(tool.name, sharedPrefix)}</strong>
                <ToolStatus status={tool.status} />
              </HStack>
              <span {...stylex.props(styles.toolSummary, tool.status !== "ready" && styles.unavailableSummary)}>
                {toolSummary(tool)}
              </span>
            </VStack>
            <HStack gap={0.5} vAlign="center" hAlign="end">
              <IconButton
                size="sm"
                variant="ghost"
                label={`Edit ${tool.name}`}
                tooltip="Edit behavior"
                icon={<Pencil aria-hidden="true" size={16} />}
                onClick={() => onEdit(tool)}
              />
              {hasHumanOverride ? (
                <IconButton
                  size="sm"
                  variant="ghost"
                  label={`Reset ${tool.name}`}
                  tooltip="Reset behavior"
                  icon={<RefreshCcw aria-hidden="true" size={16} />}
                  isDisabled={busy}
                  onClick={() => onReset(tool)}
                />
              ) : null}
              <IconButton
                size="sm"
                variant="ghost"
                label={`${tool.enabled ? "Turn off" : "Turn on"} ${tool.name}`}
                tooltip={tool.enabled ? "Turn off" : "Turn on"}
                icon={tool.enabled
                  ? <ToggleRight aria-hidden="true" size={18} />
                  : <ToggleLeft aria-hidden="true" size={18} />}
                isDisabled={busy}
                onClick={() => onToggle(tool)}
              />
            </HStack>
          </HStack>
        );
      })}
    </VStack>
  );
}

function ToolStatus({ status }: { status: string }) {
  if (status === "ready") return null;
  const label = settingsStatusLabel(status);
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

function toolSummary(tool: ManagedTool) {
  if (tool.status !== "ready") return settingsStatusLabel(tool.status);
  if (tool.readOnly.value) return "Read only";
  if (tool.destructive.value) return "Can delete";
  if (tool.openWorld.value) return "Outside Noema";
  if (tool.idempotent.value) return "Safe to repeat";
  return "Can make changes";
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

export function toolHintSourceDescription(source: string | null) {
  if (source === "annotation") return "From provider";
  if (source === "model") return "Checked by Noema";
  if (source === "safe_default") return "Safety default";
  if (source === "human") return "Set by you";
  return "Still checking";
}

const styles = stylex.create({
  empty: { margin: "var(--spacing-0)", padding: "var(--spacing-3)", color: "var(--muted-foreground)", fontSize: 13 },
  list: { margin: 0, padding: 0, listStyle: "none" },
  row: { minHeight: "var(--spacing-12)", paddingInlineStart: "var(--spacing-2)", borderBlockStartWidth: "var(--border-width)", borderBlockStartStyle: "solid", borderBlockStartColor: "var(--border-subtle)" },
  unavailableRow: { backgroundColor: "var(--color-warning-muted)" },
  toolCopy: { minWidth: 0, flex: 1 },
  toolName: { color: "var(--foreground)", fontSize: 13, fontWeight: 650, lineHeight: 1.3, overflowWrap: "anywhere" },
  toolSummary: { color: "var(--muted-foreground)", fontSize: 12, lineHeight: 1.3 },
  unavailableSummary: { color: "var(--noema-clay-600)" },
  statusIcon: { display: "inline-flex", width: 18, height: 18, flexShrink: 0, alignItems: "center", justifyContent: "center" },
  statusWarning: { color: "var(--noema-clay-600)" },
  statusMuted: { color: "var(--noema-text-muted)" },
  statusError: { color: "var(--noema-red-700)" },
  spinner: { animationName: stylex.keyframes({ to: { transform: "rotate(360deg)" } }), animationDuration: "900ms", animationIterationCount: "infinite", animationTimingFunction: "linear", "@media (prefers-reduced-motion: reduce)": { animationName: "none" } }
});

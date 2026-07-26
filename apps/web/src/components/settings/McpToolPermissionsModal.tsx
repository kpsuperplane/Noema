import * as React from "react";
import { useMutation, useQuery } from "@apollo/client/react";
import {
  Activity,
  ArrowLeft,
  Check,
  Eye,
  Globe2,
  Loader2,
  Pencil,
  Power,
  PowerOff,
  RefreshCcw,
  Repeat2,
  Settings2,
  TriangleAlert,
  X
} from "lucide-react";
import { Button } from "@astryxdesign/core/Button";
import { Dialog, DialogHeader } from "@astryxdesign/core/Dialog";
import { Layout, LayoutContent, LayoutFooter } from "@astryxdesign/core/Layout";
import * as stylex from "@stylexjs/stylex";
import {
  McpToolsDocument,
  ResetMcpToolPolicyDocument,
  SaveMcpProviderPolicyDocument,
  SaveMcpToolOverrideDocument,
  SetMcpToolEnabledDocument,
  type McpToolsQuery,
  type ResetMcpToolPolicyMutation,
  type SaveMcpProviderPolicyMutation,
  type SaveMcpToolOverrideMutation,
  type SetMcpToolEnabledMutation
} from "@/generated/graphql";
import type { McpSettingsServer } from "./mcpMetadata";

export type McpTool = McpToolsQuery["mcpTools"][number];
type Step = "sharing" | "unsafe" | "advanced";
type HintDraft = { readOnly: boolean; idempotent: boolean; destructive: boolean; openWorld: boolean };

export function McpToolPermissionsModal({
  open,
  server,
  onSaved = () => {},
  onOpenChange
}: {
  open: boolean;
  server: McpSettingsServer | null;
  onSaved?: () => void;
  onOpenChange: (open: boolean) => void;
}) {
  const [step, setStep] = React.useState<Step>("sharing");
  const [sharing, setSharing] = React.useState(
    server?.dataSharingPolicy ?? "allow_automatically"
  );
  const [unsafeActions, setUnsafeActions] = React.useState(
    server?.unsafeActionPolicy ?? "reviewer_may_approve"
  );
  const [editingToolId, setEditingToolId] = React.useState<string | null>(null);
  const [draft, setDraft] = React.useState<HintDraft | null>(null);
  const [error, setError] = React.useState<string | null>(null);
  const toolsResult = useQuery<McpToolsQuery>(McpToolsDocument, {
    variables: { mcpServerId: server?.mcpServerId ?? "" },
    skip: !open || !server,
    fetchPolicy: "cache-and-network"
  });
  const [saveProviderPolicy, providerSave] =
    useMutation<SaveMcpProviderPolicyMutation>(SaveMcpProviderPolicyDocument);
  const [saveToolOverride, overrideSave] =
    useMutation<SaveMcpToolOverrideMutation>(SaveMcpToolOverrideDocument);
  const [resetToolPolicy, resetState] =
    useMutation<ResetMcpToolPolicyMutation>(ResetMcpToolPolicyDocument);
  const [setToolEnabled, enabledState] =
    useMutation<SetMcpToolEnabledMutation>(SetMcpToolEnabledDocument);
  const tools = toolsResult.data?.mcpTools ?? [];
  const { startPolling, stopPolling } = toolsResult;
  const editingTool = tools.find((tool) => tool.mcpToolId === editingToolId) ?? null;
  const pending = tools.some((tool) => tool.policy?.status === "pending");

  React.useEffect(() => {
    if (pending) startPolling(1500);
    else stopPolling();
    return () => stopPolling();
  }, [pending, startPolling, stopPolling]);

  function close() {
    onOpenChange(false);
  }

  async function savePolicy() {
    if (!server) return;
    setError(null);
    try {
      await saveProviderPolicy({
        variables: {
          input: {
            mcpServerId: server.mcpServerId,
            dataSharingPolicy: sharing,
            unsafeActionPolicy: unsafeActions
          }
        }
      });
      onSaved();
      close();
    } catch {
      setError("Noema could not save this provider policy. Review the choices and try again.");
    }
  }

  function editTool(tool: McpTool) {
    const policy = tool.policy;
    setEditingToolId(tool.mcpToolId);
    setDraft({
      readOnly: policy?.readOnly.value ?? false,
      idempotent: policy?.idempotent.value ?? false,
      destructive: policy?.destructive.value ?? true,
      openWorld: policy?.openWorld.value ?? true
    });
    setError(null);
  }

  async function saveOverride() {
    if (!editingTool || !draft) return;
    setError(null);
    try {
      await saveToolOverride({
        variables: {
          input: {
            mcpToolId: editingTool.mcpToolId,
            metadataFingerprint: editingTool.metadataFingerprint,
            ...draft
          }
        }
      });
      await toolsResult.refetch();
      setEditingToolId(null);
      setDraft(null);
      onSaved();
    } catch {
      setError("The tool changed or the override could not be saved. Reload and try again.");
    }
  }

  async function resetTool(tool: McpTool) {
    setError(null);
    try {
      await resetToolPolicy({ variables: { mcpToolId: tool.mcpToolId } });
      await toolsResult.refetch();
      onSaved();
    } catch {
      setError("Noema could not restart classification for this tool.");
    }
  }

  async function toggleTool(tool: McpTool) {
    setError(null);
    try {
      await setToolEnabled({
        variables: { mcpToolId: tool.mcpToolId, enabled: tool.policy?.status === "disabled" }
      });
      await toolsResult.refetch();
      onSaved();
    } catch {
      setError("Noema could not change this tool's availability.");
    }
  }

  const title = step === "sharing"
    ? `Can ${server?.displayName ?? "this provider"} receive personal information automatically?`
    : step === "unsafe"
      ? "How should risky calls be approved?"
      : editingTool?.name ?? "Advanced tool behavior";

  return (
    <Dialog
      isOpen={open}
      onOpenChange={onOpenChange}
      purpose="form"
      width={620}
      maxHeight="calc(100dvh - var(--spacing-8))"
      aria-label={title}
    >
      <Layout
        header={
          <DialogHeader
            title={title}
            subtitle={dialogSubtitle(step, sharing, editingTool !== null)}
            onOpenChange={onOpenChange}
            hasDivider
          />
        }
        content={
          <LayoutContent>
            {step === "sharing" ? (
              <div {...stylex.props(styles.choices)}>
                <Choice
                  selected={sharing === "allow_automatically"}
                  title="Allow automatically"
                  description="Safe calls run directly. Risky calls still follow your approval choice."
                  onClick={() => setSharing("allow_automatically")}
                />
                <Choice
                  selected={sharing === "review_every_call"}
                  title="Review every call"
                  description="Every call is treated as unsafe, including read-only and closed-world tools."
                  onClick={() => {
                    setSharing("review_every_call");
                    if (unsafeActions === "never_ask") setUnsafeActions("reviewer_may_approve");
                  }}
                />
              </div>
            ) : null}
            {step === "unsafe" ? (
              <div {...stylex.props(styles.choices)}>
                <Choice
                  selected={unsafeActions === "always_ask"}
                  title="Always ask"
                  description="Show the exact call to you without asking the reviewer first."
                  onClick={() => setUnsafeActions("always_ask")}
                />
                <Choice
                  selected={unsafeActions === "reviewer_may_approve"}
                  title="Let the reviewer decide"
                  description="The reviewer can approve lower-risk calls and sends the rest to you."
                  onClick={() => setUnsafeActions("reviewer_may_approve")}
                />
                <Choice
                  selected={unsafeActions === "never_ask"}
                  title="Never ask — Dangerous"
                  description="Unsafe calls execute automatically with ordinary credential and audit controls."
                  disabled={sharing === "review_every_call"}
                  disabledReason="Every call is unsafe under Review every call. Go back and allow automatic sharing first."
                  onClick={() => setUnsafeActions("never_ask")}
                />
              </div>
            ) : null}
            {step === "advanced" ? (
              editingTool && draft ? (
                <ToolEditor tool={editingTool} draft={draft} onChange={setDraft} />
              ) : (
                <ToolList
                  tools={tools}
                  loading={toolsResult.loading && !toolsResult.data}
                  error={toolsResult.error ? "Tool behavior could not be loaded." : null}
                  busy={resetState.loading || enabledState.loading}
                  onEdit={editTool}
                  onReset={(tool) => void resetTool(tool)}
                  onToggle={(tool) => void toggleTool(tool)}
                />
              )
            ) : null}
            {error ? <p {...stylex.props(styles.error)}>{error}</p> : null}
          </LayoutContent>
        }
        footer={
          <LayoutFooter hasDivider padding={3}>
            <div {...stylex.props(styles.footer)}>
              <div>
                {step === "sharing" ? (
                  <Button
                    type="button"
                    variant="ghost"
                    label="Advanced tool behavior"
                    icon={<Settings2 {...stylex.props(styles.icon)} aria-hidden="true" />}
                    onClick={() => setStep("advanced")}
                  />
                ) : (
                  <Button
                    type="button"
                    variant="ghost"
                    label="Back"
                    icon={<ArrowLeft {...stylex.props(styles.icon)} aria-hidden="true" />}
                    onClick={() => {
                      if (step === "unsafe") setStep("sharing");
                      else if (editingTool) { setEditingToolId(null); setDraft(null); }
                      else setStep("sharing");
                    }}
                  />
                )}
              </div>
              {step === "sharing" ? (
                <Button type="button" label="Continue to approval" onClick={() => setStep("unsafe")} />
              ) : step === "unsafe" ? (
                <Button
                  type="button"
                  label="Save policy"
                  isLoading={providerSave.loading}
                  isDisabled={sharing === "review_every_call" && unsafeActions === "never_ask"}
                  onClick={() => void savePolicy()}
                />
              ) : editingTool ? (
                <Button
                  type="button"
                  label="Save override"
                  isLoading={overrideSave.loading}
                  onClick={() => void saveOverride()}
                />
              ) : (
                <Button type="button" variant="secondary" label="Done" onClick={close} />
              )}
            </div>
          </LayoutFooter>
        }
      />
    </Dialog>
  );
}

function Choice({ selected, title, description, disabled = false, disabledReason, onClick }: {
  selected: boolean;
  title: string;
  description: string;
  disabled?: boolean;
  disabledReason?: string;
  onClick: () => void;
}) {
  return (
    <div>
      <button
        type="button"
        disabled={disabled}
        aria-pressed={selected}
        {...stylex.props(styles.choice, selected && styles.choiceSelected)}
        onClick={onClick}
      >
        <span {...stylex.props(styles.choiceTitle)}>{title}</span>
        <span {...stylex.props(styles.choiceDescription)}>{description}</span>
      </button>
      {disabled && disabledReason ? <p {...stylex.props(styles.disabledReason)}>{disabledReason}</p> : null}
    </div>
  );
}

function ToolList({ tools, loading, error, busy, onEdit, onReset, onToggle }: {
  tools: McpTool[];
  loading: boolean;
  error: string | null;
  busy: boolean;
  onEdit: (tool: McpTool) => void;
  onReset: (tool: McpTool) => void;
  onToggle: (tool: McpTool) => void;
}) {
  if (loading) return <p {...stylex.props(styles.notice)}><Loader2 {...stylex.props(styles.spinner)} /> Loading tools...</p>;
  if (error) return <p {...stylex.props(styles.error)}>{error}</p>;
  if (tools.length === 0) return <p {...stylex.props(styles.notice)}>No tools were discovered.</p>;
  const hintColumns = [
    { field: "readOnly", label: "Read only", shortLabel: "Read", icon: <Eye aria-hidden="true" size={14} /> },
    { field: "idempotent", label: "Idempotent", shortLabel: "Repeat", icon: <Repeat2 aria-hidden="true" size={14} /> },
    { field: "destructive", label: "Destructive", shortLabel: "Delete", icon: <TriangleAlert aria-hidden="true" size={14} /> },
    { field: "openWorld", label: "Open world", shortLabel: "External", icon: <Globe2 aria-hidden="true" size={14} /> }
  ] as const;
  const sharedPrefix = sharedToolPrefix(tools);
  return <div {...stylex.props(styles.toolTableFrame)}>
    <table aria-label="How Noema understands each tool" {...stylex.props(styles.toolTable)}>
      <thead>
        <tr>
          <th scope="col" {...stylex.props(styles.tableHeading, styles.toolNameHeading)}>Tool</th>
          <th scope="col" title="Status" aria-label="Status" {...stylex.props(styles.tableHeading, styles.iconColumn)}>
            <Activity aria-hidden="true" size={14} />
          </th>
          {hintColumns.map(({ label, shortLabel, icon }) => (
            <th key={label} scope="col" title={label} aria-label={label} {...stylex.props(styles.tableHeading, styles.permissionColumn)}>
              <span {...stylex.props(styles.permissionHeading)}>{icon}<span>{shortLabel}</span></span>
            </th>
          ))}
          <th scope="col" aria-label="Actions" {...stylex.props(styles.tableHeading, styles.actionsColumn)} />
        </tr>
      </thead>
      <tbody>{tools.map((tool) => {
    const status = tool.policy?.status ?? "pending";
    return (
      <tr key={tool.mcpToolId}>
        <th scope="row" title={tool.name} {...stylex.props(styles.tableCell, styles.toolNameCell)}>{displayToolName(tool.name, sharedPrefix)}</th>
        <td {...stylex.props(styles.tableCell, styles.iconColumn)}><ToolStatus status={status} /></td>
        {hintColumns.map(({ field, label }) => (
          <td key={field} {...stylex.props(styles.tableCell, styles.permissionColumn)}>
            <ToolHintValue label={label} hint={tool.policy?.[field]} />
          </td>
        ))}
        <td {...stylex.props(styles.tableCell, styles.actionsColumn)}>
          <div {...stylex.props(styles.toolActions)}>
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
          {status === "defaulted" || status === "ready" ? (
            <Button
              type="button"
              size="sm"
              variant="ghost"
              label={status === "defaulted" ? "Retry" : "Reset"}
              tooltip={status === "defaulted" ? "Retry" : "Reset"}
              icon={<RefreshCcw aria-hidden="true" size={16} />}
              isIconOnly
              isDisabled={busy}
              onClick={() => onReset(tool)}
            />
          ) : null}
          <Button
            type="button"
            size="sm"
            variant="ghost"
            label={status === "disabled" ? "Enable" : "Disable"}
            tooltip={status === "disabled" ? "Enable" : "Disable"}
            icon={status === "disabled" ? <Power aria-hidden="true" size={16} /> : <PowerOff aria-hidden="true" size={16} />}
            isIconOnly
            isDisabled={busy}
            onClick={() => onToggle(tool)}
          />
          </div>
        </td>
      </tr>
    );
  })}</tbody>
    </table>
  </div>;
}

function ToolStatus({ status }: { status: string }) {
  const label = statusLabel(status);
  return (
    <span role="img" aria-label={label} title={label} {...stylex.props(styles.statusIcon, status === "ready" ? styles.statusReady : status === "defaulted" ? styles.statusWarning : styles.statusMuted)}>
      {status === "ready" ? (
        <Check aria-hidden="true" size={16} strokeWidth={2.4} />
      ) : status === "pending" ? (
        <Loader2 aria-hidden="true" size={16} {...stylex.props(styles.spinner)} />
      ) : status === "defaulted" ? (
        <TriangleAlert aria-hidden="true" size={16} />
      ) : (
        <PowerOff aria-hidden="true" size={16} />
      )}
    </span>
  );
}

function ToolHintValue({ label, hint }: {
  label: string;
  hint: { value: boolean | null; source: string | null } | undefined;
}) {
  const state = hint?.value == null ? "Pending" : hint.value ? "Yes" : "No";
  const description = `${label}: ${state} · ${sourceLabel(hint?.source ?? null)}`;
  return (
    <span role="img" aria-label={description} title={description} {...stylex.props(styles.hintValue)}>
      {hint?.value == null ? (
        <Loader2 aria-hidden="true" size={14} {...stylex.props(styles.spinner)} />
      ) : hint.value ? (
        <Check aria-hidden="true" size={14} />
      ) : (
        <X aria-hidden="true" size={14} />
      )}
    </span>
  );
}

function sharedToolPrefix(tools: McpTool[]) {
  if (tools.length < 2) return "";
  const separator = tools[0].name.indexOf("_");
  if (separator < 1) return "";
  const prefix = tools[0].name.slice(0, separator + 1);
  return tools.every((tool) => tool.name.startsWith(prefix) && tool.name.length > prefix.length)
    ? prefix
    : "";
}

function displayToolName(name: string, sharedPrefix: string) {
  return name.slice(sharedPrefix.length).replaceAll("_", " ");
}

function ToolEditor({ tool, draft, onChange }: { tool: McpTool; draft: HintDraft; onChange: (draft: HintDraft) => void }) {
  return (
    <div {...stylex.props(styles.editor)}>
      {tool.description ? <p {...stylex.props(styles.notice)}>{tool.description}</p> : null}
      {(["readOnly", "idempotent", "destructive", "openWorld"] as const).map((field) => (
        <div key={field} {...stylex.props(styles.hintRow)}>
          <div>
            <strong>{hintLabel(field)}</strong>
            <span {...stylex.props(styles.source)}>Current source: {sourceLabel(tool.policy?.[field].source ?? null)}</span>
          </div>
          <div {...stylex.props(styles.booleanChoices)}>
            <button type="button" aria-pressed={draft[field]} {...stylex.props(styles.booleanChoice, draft[field] && styles.booleanSelected)} onClick={() => onChange({ ...draft, [field]: true })}>Yes</button>
            <button type="button" aria-pressed={!draft[field]} {...stylex.props(styles.booleanChoice, !draft[field] && styles.booleanSelected)} onClick={() => onChange({ ...draft, [field]: false })}>No</button>
          </div>
        </div>
      ))}
    </div>
  );
}

function dialogSubtitle(step: Step, sharing: string, editing: boolean) {
  if (step === "sharing") return "This controls whether otherwise-safe calls can share chat context without approval.";
  if (step === "unsafe") return sharing === "review_every_call" ? "Every call is unsafe under the sharing policy you selected." : "Risky tools are unsafe; this choice controls their approval path.";
  return editing ? "A complete human override replaces the effective behavior hints for this metadata version." : "Inspect effective hints, provenance, background state, and tool availability.";
}

function statusLabel(status: string) {
  return status === "defaulted" ? "Safe defaults" : status.charAt(0).toUpperCase() + status.slice(1);
}

function sourceLabel(source: string | null) {
  return source ? source.replaceAll("_", " ") : "not classified";
}

function hintLabel(field: keyof HintDraft) {
  return ({ readOnly: "Read only", idempotent: "Idempotent", destructive: "Destructive", openWorld: "Open world" })[field];
}

const styles = stylex.create({
  choices: { display: "grid", gap: "var(--spacing-2)" },
  choice: { display: "grid", width: "100%", gap: "var(--spacing-1)", padding: "var(--spacing-3)", textAlign: "left", borderWidth: 1, borderStyle: "solid", borderColor: "var(--border-subtle)", borderRadius: 8, color: "var(--foreground)", backgroundColor: "white", cursor: "pointer", ':disabled': { cursor: "not-allowed", opacity: 0.5 } },
  choiceSelected: { borderColor: "var(--primary)", backgroundColor: "var(--accent)" },
  choiceTitle: { fontSize: 14, fontWeight: 600, lineHeight: 1.4 },
  choiceDescription: { fontSize: 13, lineHeight: 1.45, color: "var(--muted-foreground)" },
  disabledReason: { margin: "var(--spacing-1-5) var(--spacing-1) 0", fontSize: 12, lineHeight: 1.4, color: "var(--muted-foreground)" },
  footer: { display: "flex", flexWrap: "wrap", alignItems: "center", justifyContent: "space-between", gap: "var(--spacing-3)" },
  error: { margin: "var(--spacing-3) 0 0", fontSize: 13, lineHeight: 1.45, color: "var(--destructive)" },
  notice: { display: "flex", alignItems: "center", gap: "var(--spacing-2)", margin: 0, fontSize: 13, lineHeight: 1.5, color: "var(--muted-foreground)" },
  spinner: { width: 16, height: 16, animationName: stylex.keyframes({ to: { transform: "rotate(360deg)" } }), animationDuration: "800ms", animationIterationCount: "infinite", animationTimingFunction: "linear", '@media (prefers-reduced-motion: reduce)': { animationName: "none" } },
  icon: { width: 16, height: 16 },
  toolTableFrame: { width: "100%", overflowX: "auto", borderWidth: 1, borderStyle: "solid", borderColor: "var(--border-subtle)", borderRadius: 6 },
  toolTable: { width: "100%", minWidth: 520, borderCollapse: "collapse", tableLayout: "fixed", color: "var(--muted-foreground)" },
  tableHeading: { height: 36, padding: "0 var(--spacing-1)", borderBottomWidth: 1, borderBottomStyle: "solid", borderBottomColor: "var(--border-subtle)", textAlign: "center", fontSize: 11, fontWeight: 500, color: "var(--muted-foreground)" },
  tableCell: { height: 44, padding: "var(--spacing-1)", borderBottomWidth: 1, borderBottomStyle: "solid", borderBottomColor: "var(--border-subtle)", textAlign: "center" },
  toolNameHeading: { width: "auto", paddingLeft: "var(--spacing-2)", textAlign: "left" },
  toolNameCell: { paddingLeft: "var(--spacing-2)", textAlign: "left", fontSize: 13, fontWeight: 600, lineHeight: 1.35, overflowWrap: "anywhere", whiteSpace: "normal", color: "var(--foreground)" },
  iconColumn: { width: 32 },
  permissionColumn: { width: 68 },
  permissionHeading: { display: "inline-flex", alignItems: "center", justifyContent: "center", gap: "var(--spacing-1)", whiteSpace: "nowrap" },
  actionsColumn: { width: 104 },
  statusIcon: { display: "inline-flex", width: 24, height: 24, alignItems: "center", justifyContent: "center" },
  statusReady: { color: "var(--noema-pine-600)" },
  statusWarning: { color: "var(--noema-clay-600)" },
  statusMuted: { color: "var(--noema-text-faint)" },
  hintValue: { display: "inline-flex", width: 24, height: 20, alignItems: "center", justifyContent: "center" },
  toolActions: { display: "flex", alignItems: "center", justifyContent: "flex-end", gap: "var(--spacing-0-5)" },
  editor: { display: "grid", gap: "var(--spacing-2)" },
  hintRow: { display: "flex", alignItems: "center", justifyContent: "space-between", gap: "var(--spacing-4)", padding: "var(--spacing-2) 0", borderBottomWidth: 1, borderBottomStyle: "solid", borderBottomColor: "var(--border-subtle)", fontSize: 14 },
  source: { display: "block", marginTop: "var(--spacing-0-5)", fontSize: 12, fontWeight: 400, color: "var(--muted-foreground)" },
  booleanChoices: { display: "flex", gap: "var(--spacing-1)" },
  booleanChoice: { minWidth: 48, padding: "var(--spacing-1-5) var(--spacing-2)", borderWidth: 1, borderStyle: "solid", borderColor: "var(--border-subtle)", borderRadius: 6, backgroundColor: "white", color: "var(--foreground)", cursor: "pointer" },
  booleanSelected: { borderColor: "var(--primary)", backgroundColor: "var(--accent)", fontWeight: 600 }
});

import * as React from "react";
import { useMutation, useQuery } from "@apollo/client/react";
import {
  ArrowLeft,
  ArrowRight,
  Check,
  CircleOff,
  Loader2,
  MessageSquareText,
  Pencil,
  Settings2,
  ShieldCheck,
  ToggleLeft,
  ToggleRight,
  TriangleAlert,
  UserRoundCheck,
  WandSparkles,
  Zap,
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
      setError("Couldn't save. Try again.");
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
      setError("This tool changed. Reopen it and try again.");
    }
  }

  async function resetTool(tool: McpTool) {
    setError(null);
    try {
      await resetToolPolicy({ variables: { mcpToolId: tool.mcpToolId } });
      await toolsResult.refetch();
      onSaved();
    } catch {
      setError("Couldn't check this tool. Try again.");
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
      setError("Couldn't update this tool. Try again.");
    }
  }

  const title = step === "sharing"
    ? `Share personal information with ${server?.displayName ?? "this provider"}?`
    : step === "unsafe"
      ? "Who approves risky calls?"
      : editingTool?.name ?? "Tool settings";
  const fullBleedTools = step === "advanced" && !editingTool;

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
          <LayoutContent padding={fullBleedTools ? 0 : undefined}>
            {step === "sharing" ? (
              <div {...stylex.props(styles.choices)}>
                <Choice
                  selected={sharing === "allow_automatically"}
                  title="Share when needed"
                  icon={<MessageSquareText aria-hidden="true" size={18} />}
                  steps={["Relevant details", "Tool runs"]}
                  onClick={() => setSharing("allow_automatically")}
                />
                <Choice
                  selected={sharing === "review_every_call"}
                  title="Review every time"
                  icon={<ShieldCheck aria-hidden="true" size={18} />}
                  steps={["Relevant details", "Approval check", "Tool runs"]}
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
                  title="Always me"
                  icon={<UserRoundCheck aria-hidden="true" size={18} />}
                  steps={["Risky call", "You approve", "Runs"]}
                  onClick={() => setUnsafeActions("always_ask")}
                />
                <Choice
                  selected={unsafeActions === "reviewer_may_approve"}
                  title="Noema first"
                  icon={<ShieldCheck aria-hidden="true" size={18} />}
                  steps={["Risky call", "Noema checks", "You if needed"]}
                  onClick={() => setUnsafeActions("reviewer_may_approve")}
                />
                <Choice
                  selected={unsafeActions === "never_ask"}
                  title="Run automatically"
                  note="Not recommended"
                  icon={<Zap aria-hidden="true" size={18} />}
                  steps={["Risky call", "Runs"]}
                  disabled={sharing === "review_every_call"}
                  disabledReason="Choose “Share when needed” first."
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
                  error={toolsResult.error ? "Couldn't load tools." : null}
                  busy={resetState.loading || enabledState.loading}
                  onEdit={editTool}
                  onReset={(tool) => void resetTool(tool)}
                  onToggle={(tool) => void toggleTool(tool)}
                />
              )
            ) : null}
            {error ? <p {...stylex.props(styles.error, fullBleedTools && styles.fullBleedError)}>{error}</p> : null}
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
                    label="Tool settings"
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
                <Button type="button" label="Next" onClick={() => setStep("unsafe")} />
              ) : step === "unsafe" ? (
                <Button
                  type="button"
                  label="Save"
                  isLoading={providerSave.loading}
                  isDisabled={sharing === "review_every_call" && unsafeActions === "never_ask"}
                  onClick={() => void savePolicy()}
                />
              ) : editingTool ? (
                <Button
                  type="button"
                  label="Save"
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

function Choice({ selected, title, icon, steps, note, disabled = false, disabledReason, onClick }: {
  selected: boolean;
  title: string;
  icon: React.ReactNode;
  steps: readonly string[];
  note?: string;
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
        <span {...stylex.props(styles.choiceHeader)}>
          <span {...stylex.props(styles.choiceHeading)}>
            <span {...stylex.props(styles.choiceIcon, selected && styles.choiceIconSelected)}>{icon}</span>
            <span {...stylex.props(styles.choiceTitle)}>{title}</span>
          </span>
          <span {...stylex.props(styles.choiceMeta)}>
            {note ? <span {...stylex.props(styles.choiceNote)}>{note}</span> : null}
            {selected ? <Check aria-hidden="true" {...stylex.props(styles.choiceCheck)} /> : null}
          </span>
        </span>
        <span {...stylex.props(styles.choicePath)}>
          {steps.map((step, index) => (
            <React.Fragment key={step}>
              {index > 0 ? <ArrowRight aria-hidden="true" {...stylex.props(styles.choiceArrow)} /> : null}
              <span {...stylex.props(styles.choiceStep)}>{step}</span>
            </React.Fragment>
          ))}
        </span>
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
  if (loading) return <p {...stylex.props(styles.notice, styles.toolListMessage)}><Loader2 {...stylex.props(styles.spinner)} /> Checking...</p>;
  if (error) return <p {...stylex.props(styles.error, styles.toolListMessage)}>{error}</p>;
  if (tools.length === 0) return <p {...stylex.props(styles.notice, styles.toolListMessage)}>No tools found.</p>;
  const hintColumns = [
    { field: "readOnly", label: "Only reads information", shortLabel: "Only reads" },
    { field: "idempotent", label: "Same effect if repeated", shortLabel: "Safe to repeat" },
    { field: "destructive", label: "Can delete or overwrite", shortLabel: "Can delete" },
    { field: "openWorld", label: "Can act outside Noema", shortLabel: "Outside Noema" }
  ] as const;
  const sharedPrefix = sharedToolPrefix(tools);
  return <div {...stylex.props(styles.toolTableFrame)}>
    <table aria-label="How Noema understands each tool" {...stylex.props(styles.toolTable)}>
      <thead>
        <tr>
          <th scope="col" {...stylex.props(styles.tableHeading, styles.toolNameHeading)}>Tool</th>
          {hintColumns.map(({ label, shortLabel }) => (
            <th key={label} scope="col" title={label} aria-label={label} {...stylex.props(styles.tableHeading, styles.permissionColumn)}>
              {shortLabel}
            </th>
          ))}
          <th scope="col" aria-label="Actions" {...stylex.props(styles.tableHeading, styles.actionsColumn)} />
        </tr>
      </thead>
      <tbody>{tools.map((tool) => {
    const status = tool.policy?.status ?? "pending";
    return (
      <tr key={tool.mcpToolId}>
        <th scope="row" title={tool.name} {...stylex.props(styles.tableCell, styles.toolNameCell)}>
          <span {...stylex.props(styles.toolName)}><span>{displayToolName(tool.name, sharedPrefix)}</span><ToolStatus status={status} /></span>
        </th>
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
              label={status === "defaulted" ? "Retry AI autofill" : "AI autofill"}
              tooltip={status === "defaulted" ? "Retry AI autofill" : "AI autofill"}
              icon={<WandSparkles aria-hidden="true" size={16} />}
              isIconOnly
              isDisabled={busy}
              onClick={() => onReset(tool)}
            />
          ) : null}
          <Button
            type="button"
            size="sm"
            variant="ghost"
            label={status === "disabled" ? "Turn on" : "Turn off"}
            tooltip={status === "disabled" ? "Turn on" : "Turn off"}
            icon={status === "disabled" ? <ToggleRight aria-hidden="true" size={16} /> : <ToggleLeft aria-hidden="true" size={16} />}
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
  if (status === "ready") return null;
  const label = statusLabel(status);
  return (
    <span role="img" aria-label={label} title={label} {...stylex.props(styles.statusIcon, status === "defaulted" ? styles.statusWarning : status === "pending" || status === "disabled" ? styles.statusMuted : styles.statusError)}>
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

function ToolHintValue({ label, hint }: {
  label: string;
  hint: { value: boolean | null; source: string | null } | undefined;
}) {
  const state = hint?.value == null ? "Checking" : hint.value ? "Yes" : "No";
  const description = `${label}: ${state} · ${sourceDescription(hint?.source ?? null)}`;
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
  const displayName = name.slice(sharedPrefix.length).replaceAll("_", " ");
  return displayName.charAt(0).toUpperCase() + displayName.slice(1);
}

function ToolEditor({ tool, draft, onChange }: { tool: McpTool; draft: HintDraft; onChange: (draft: HintDraft) => void }) {
  return (
    <div {...stylex.props(styles.editor)}>
      {tool.description ? <p {...stylex.props(styles.notice)}>{tool.description}</p> : null}
      {(["readOnly", "idempotent", "destructive", "openWorld"] as const).map((field) => (
        <div key={field} {...stylex.props(styles.hintRow)}>
          <div>
            <strong>{hintLabel(field)}</strong>
            <span {...stylex.props(styles.source)}>{sourceDescription(tool.policy?.[field].source ?? null)}</span>
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
  if (step === "sharing") return "Choose how Noema shares conversation details.";
  if (step === "unsafe") return sharing === "review_every_call" ? "This applies every time." : "Risky calls can change, delete, or send information.";
  return editing ? "These answers decide when approval is needed." : "Review or turn off individual tools.";
}

function statusLabel(status: string) {
  return ({
    pending: "Checking",
    ready: "Ready",
    defaulted: "Cautious",
    disabled: "Off"
  } as Record<string, string>)[status] ?? status;
}

function sourceDescription(source: string | null) {
  if (source === "annotation") return "From provider";
  if (source === "model") return "Checked by Noema";
  if (source === "safe_default") return "Safety default";
  if (source === "human") return "Set by you";
  return "Still checking";
}

function hintLabel(field: keyof HintDraft) {
  return ({
    readOnly: "Only reads?",
    idempotent: "Same result if repeated?",
    destructive: "Can delete?",
    openWorld: "Can it act outside Noema?"
  })[field];
}

const styles = stylex.create({
  choices: { display: "grid", gap: "var(--spacing-2)" },
  choice: { display: "grid", width: "100%", gap: "var(--spacing-2)", padding: "var(--spacing-3)", textAlign: "left", borderWidth: 1, borderStyle: "solid", borderColor: "var(--border-subtle)", borderRadius: 8, color: "var(--foreground)", backgroundColor: "white", cursor: "pointer", ':disabled': { cursor: "not-allowed", opacity: 0.5 } },
  choiceSelected: { borderColor: "var(--primary)", backgroundColor: "var(--accent)" },
  choiceHeader: { display: "flex", alignItems: "center", justifyContent: "space-between", gap: "var(--spacing-2)" },
  choiceHeading: { display: "inline-flex", minWidth: 0, alignItems: "center", gap: "var(--spacing-2)" },
  choiceIcon: { display: "inline-flex", width: 18, height: 18, flexShrink: 0, color: "var(--muted-foreground)" },
  choiceIconSelected: { color: "var(--primary)" },
  choiceTitle: { fontSize: 14, fontWeight: 600, lineHeight: 1.4 },
  choiceMeta: { display: "inline-flex", flexShrink: 0, alignItems: "center", gap: "var(--spacing-2)" },
  choiceNote: { fontSize: 11, fontWeight: 600, lineHeight: 1.3, color: "var(--destructive)" },
  choiceCheck: { width: 16, height: 16, color: "var(--primary)" },
  choicePath: { display: "flex", flexWrap: "wrap", alignItems: "center", gap: "var(--spacing-1)" },
  choiceStep: { fontSize: 12, fontWeight: 500, lineHeight: 1.4, color: "var(--muted-foreground)" },
  choiceArrow: { width: 12, height: 12, flexShrink: 0, color: "var(--noema-text-faint)" },
  disabledReason: { margin: "var(--spacing-1-5) var(--spacing-1) 0", fontSize: 12, lineHeight: 1.4, color: "var(--muted-foreground)" },
  footer: { display: "flex", flexWrap: "wrap", alignItems: "center", justifyContent: "space-between", gap: "var(--spacing-3)" },
  error: { margin: "var(--spacing-3) 0 0", fontSize: 13, lineHeight: 1.45, color: "var(--destructive)" },
  fullBleedError: { margin: 0, padding: "var(--spacing-3) var(--spacing-4) 0" },
  notice: { display: "flex", alignItems: "center", gap: "var(--spacing-2)", margin: 0, fontSize: 13, lineHeight: 1.5, color: "var(--muted-foreground)" },
  spinner: { width: 16, height: 16, animationName: stylex.keyframes({ to: { transform: "rotate(360deg)" } }), animationDuration: "800ms", animationIterationCount: "infinite", animationTimingFunction: "linear", '@media (prefers-reduced-motion: reduce)': { animationName: "none" } },
  icon: { width: 16, height: 16 },
  toolListMessage: { margin: 0, padding: "var(--spacing-4)" },
  toolTableFrame: { width: "100%" },
  toolTable: { width: "100%", minWidth: 520, borderCollapse: "collapse", tableLayout: "fixed", color: "var(--muted-foreground)" },
  tableHeading: { position: "sticky", top: 0, zIndex: 1, height: 40, padding: "0 var(--spacing-1)", borderBottomWidth: 1, borderBottomStyle: "solid", borderBottomColor: "var(--border-subtle)", backgroundColor: "white", textAlign: "center", fontSize: 11, fontWeight: 500, lineHeight: 1.2, color: "var(--muted-foreground)" },
  tableCell: { height: 44, padding: "var(--spacing-1)", borderBottomWidth: 1, borderBottomStyle: "solid", borderBottomColor: "var(--border-subtle)", textAlign: "center" },
  toolNameHeading: { width: "auto", paddingLeft: "var(--spacing-4)", textAlign: "left" },
  toolNameCell: { paddingLeft: "var(--spacing-4)", textAlign: "left", fontSize: 13, fontWeight: 600, lineHeight: 1.35, overflowWrap: "anywhere", whiteSpace: "normal", color: "var(--foreground)" },
  toolName: { display: "inline-flex", alignItems: "center", gap: "var(--spacing-1)" },
  permissionColumn: { width: 76 },
  actionsColumn: { width: 116, paddingRight: "var(--spacing-4)" },
  statusIcon: { display: "inline-flex", width: 18, height: 18, flexShrink: 0, alignItems: "center", justifyContent: "center" },
  statusWarning: { color: "var(--noema-clay-600)" },
  statusMuted: { color: "var(--noema-text-faint)" },
  statusError: { color: "var(--noema-red-700)" },
  hintValue: { display: "inline-flex", width: 24, height: 20, alignItems: "center", justifyContent: "center" },
  toolActions: { display: "flex", alignItems: "center", justifyContent: "flex-end", gap: "var(--spacing-0-5)" },
  editor: { display: "grid", gap: "var(--spacing-2)" },
  hintRow: { display: "flex", alignItems: "center", justifyContent: "space-between", gap: "var(--spacing-4)", padding: "var(--spacing-2) 0", borderBottomWidth: 1, borderBottomStyle: "solid", borderBottomColor: "var(--border-subtle)", fontSize: 14 },
  source: { display: "block", marginTop: "var(--spacing-0-5)", fontSize: 12, fontWeight: 400, color: "var(--muted-foreground)" },
  booleanChoices: { display: "flex", gap: "var(--spacing-1)" },
  booleanChoice: { minWidth: 48, padding: "var(--spacing-1-5) var(--spacing-2)", borderWidth: 1, borderStyle: "solid", borderColor: "var(--border-subtle)", borderRadius: 6, backgroundColor: "white", color: "var(--foreground)", cursor: "pointer" },
  booleanSelected: { borderColor: "var(--primary)", backgroundColor: "var(--accent)", fontWeight: 600 }
});

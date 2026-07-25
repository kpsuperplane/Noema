import * as React from "react";
import { useMutation, useQuery } from "@apollo/client/react";
import { ArrowLeft, Loader2, Settings2 } from "lucide-react";
import { Badge } from "@astryxdesign/core/Badge";
import { Button } from "@astryxdesign/core/Button";
import { Dialog, DialogHeader } from "@astryxdesign/core/Dialog";
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
      ? "How should unsafe calls be approved?"
      : editingTool?.name ?? "Advanced tool behavior";

  return (
    <Dialog
      isOpen={open}
      onOpenChange={onOpenChange}
      purpose="form"
      width={620}
      maxHeight="85vh"
      aria-label={title}
    >
      <div {...stylex.props(styles.dialog)}>
        <DialogHeader
          title={title}
          subtitle={dialogSubtitle(step, sharing, editingTool !== null)}
          onOpenChange={onOpenChange}
          hasDivider
        />
        <div {...stylex.props(styles.body)}>
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
        </div>
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
            <Button type="button" label="Continue" onClick={() => setStep("unsafe")} />
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
              label="Save override"
              isLoading={overrideSave.loading}
              onClick={() => void saveOverride()}
            />
          ) : (
            <Button type="button" variant="secondary" label="Done" onClick={close} />
          )}
        </div>
      </div>
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
  return <div {...stylex.props(styles.toolList)}>{tools.map((tool) => {
    const status = tool.policy?.status ?? "pending";
    return (
      <article key={tool.mcpToolId} {...stylex.props(styles.toolRow)}>
        <div {...stylex.props(styles.toolMain)}>
          <div {...stylex.props(styles.toolTitle)}>
            <strong>{tool.name}</strong>
            <Badge variant={status === "defaulted" ? "error" : "neutral"} label={statusLabel(status)} />
          </div>
          <div {...stylex.props(styles.hints)}>
            {tool.policy ? [
              ["Read only", tool.policy.readOnly],
              ["Idempotent", tool.policy.idempotent],
              ["Destructive", tool.policy.destructive],
              ["Open world", tool.policy.openWorld]
            ].map(([label, hint]) => {
              const value = hint as { value: boolean | null; source: string | null };
              return <span key={label as string}>{label as string}: {value.value == null ? "Pending" : value.value ? "Yes" : "No"} · {sourceLabel(value.source)}</span>;
            }) : null}
          </div>
        </div>
        <div {...stylex.props(styles.toolActions)}>
          <Button type="button" size="sm" variant="ghost" label="Edit" onClick={() => onEdit(tool)} />
          {status === "defaulted" || status === "ready" ? (
            <Button type="button" size="sm" variant="ghost" label={status === "defaulted" ? "Retry" : "Reset"} isDisabled={busy} onClick={() => onReset(tool)} />
          ) : null}
          <Button type="button" size="sm" variant="ghost" label={status === "disabled" ? "Enable" : "Disable"} isDisabled={busy} onClick={() => onToggle(tool)} />
        </div>
      </article>
    );
  })}</div>;
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
  dialog: { display: "grid", gridTemplateRows: "auto minmax(0, 1fr) auto", minHeight: 0, maxHeight: "85vh" },
  body: { minHeight: 280, overflowY: "auto", padding: 16 },
  choices: { display: "grid", gap: 10 },
  choice: { display: "grid", width: "100%", gap: 4, padding: 14, textAlign: "left", borderWidth: 1, borderStyle: "solid", borderColor: "var(--border-subtle)", borderRadius: 8, color: "var(--foreground)", backgroundColor: "white", cursor: "pointer", ':disabled': { cursor: "not-allowed", opacity: 0.5 } },
  choiceSelected: { borderColor: "var(--primary)", backgroundColor: "var(--accent)" },
  choiceTitle: { fontSize: 14, fontWeight: 600, lineHeight: 1.4 },
  choiceDescription: { fontSize: 13, lineHeight: 1.45, color: "var(--muted-foreground)" },
  disabledReason: { margin: "6px 4px 0", fontSize: 12, lineHeight: 1.4, color: "var(--muted-foreground)" },
  footer: { display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12, padding: 12, borderTopWidth: 1, borderTopStyle: "solid", borderTopColor: "var(--border-subtle)" },
  error: { margin: "12px 0 0", fontSize: 13, lineHeight: 1.45, color: "var(--destructive)" },
  notice: { display: "flex", alignItems: "center", gap: 8, margin: 0, fontSize: 13, lineHeight: 1.5, color: "var(--muted-foreground)" },
  spinner: { width: 16, height: 16, animationName: stylex.keyframes({ to: { transform: "rotate(360deg)" } }), animationDuration: "800ms", animationIterationCount: "infinite", animationTimingFunction: "linear" },
  icon: { width: 16, height: 16 },
  toolList: { display: "grid", gap: 8 },
  toolRow: { display: "grid", gridTemplateColumns: "minmax(0, 1fr) auto", alignItems: "center", gap: 12, padding: 12, borderWidth: 1, borderStyle: "solid", borderColor: "var(--border-subtle)", borderRadius: 6, '@media (max-width: 640px)': { gridTemplateColumns: "1fr" } },
  toolMain: { minWidth: 0 },
  toolTitle: { display: "flex", alignItems: "center", flexWrap: "wrap", gap: 8, fontSize: 14 },
  hints: { display: "flex", flexWrap: "wrap", gap: "2px 10px", marginTop: 6, fontSize: 12, lineHeight: 1.4, color: "var(--muted-foreground)" },
  toolActions: { display: "flex", alignItems: "center", flexWrap: "wrap", gap: 2 },
  editor: { display: "grid", gap: 8 },
  hintRow: { display: "flex", alignItems: "center", justifyContent: "space-between", gap: 16, padding: "10px 0", borderBottomWidth: 1, borderBottomStyle: "solid", borderBottomColor: "var(--border-subtle)", fontSize: 14 },
  source: { display: "block", marginTop: 2, fontSize: 12, fontWeight: 400, color: "var(--muted-foreground)" },
  booleanChoices: { display: "flex", gap: 4 },
  booleanChoice: { minWidth: 48, padding: "6px 10px", borderWidth: 1, borderStyle: "solid", borderColor: "var(--border-subtle)", borderRadius: 6, backgroundColor: "white", color: "var(--foreground)", cursor: "pointer" },
  booleanSelected: { borderColor: "var(--primary)", backgroundColor: "var(--accent)", fontWeight: 600 }
});

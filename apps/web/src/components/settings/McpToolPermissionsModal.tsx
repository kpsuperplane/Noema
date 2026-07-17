import * as React from "react";
import { useMutation, useQuery } from "@apollo/client/react";
import { Loader2 } from "lucide-react";
import { Dialog, DialogHeader } from "@astryxdesign/core/Dialog";
import { Selector } from "@astryxdesign/core/Selector";
import * as stylex from "@stylexjs/stylex";
import {
  AutofillToolCalibrationsDocument,
  McpToolsDocument,
  SaveToolCalibrationsDocument,
  type AutofillToolCalibrationsMutation,
  type McpToolsQuery,
  type SaveToolCalibrationsMutation
} from "@/generated/graphql";
import { McpToolPermissionRow } from "./McpToolPermissionRow";
import { ToolPermissionsFooter } from "./McpToolPermissionsFooter";
import { McpToolDescription, McpToolSchemaPreview } from "./McpToolSchemaPreview";

export type McpTool = McpToolsQuery["mcpTools"][number];

const classificationOptions = ["none", "trusted", "untrusted", "mixed"] as const;

export function McpToolPermissionsModal({
  open,
  serverId,
  serverName,
  autoAutofill = false,
  onAutoAutofillComplete = () => {},
  onOpenChange
}: {
  open: boolean;
  serverId: string | null;
  serverName: string | null;
  autoAutofill?: boolean;
  onAutoAutofillComplete?: () => void;
  onOpenChange: (open: boolean) => void;
}) {
  const result = useQuery<McpToolsQuery>(McpToolsDocument, {
    variables: { mcpServerId: serverId ?? "" },
    skip: !open || !serverId,
    fetchPolicy: "cache-and-network"
  });
  const [saveToolCalibrations, saveState] =
    useMutation<SaveToolCalibrationsMutation>(SaveToolCalibrationsDocument);
  const [autofillToolCalibrations, autofillState] =
    useMutation<AutofillToolCalibrationsMutation>(AutofillToolCalibrationsDocument);
  const [draftOverrides, setDraftOverrides] = React.useState<Record<string, ToolPermissionDraft>>(
    {}
  );
  const [editingToolId, setEditingToolId] = React.useState<string | null>(null);
  const [saveError, setSaveError] = React.useState<string | null>(null);
  const [autofillMessage, setAutofillMessage] = React.useState<string | null>(null);
  const [autofillError, setAutofillError] = React.useState<string | null>(null);
  const autoAutofillKey = React.useRef<string | null>(null);
  const tools = React.useMemo(() => result.data?.mcpTools ?? [], [result.data?.mcpTools]);
  const editingTool = tools.find((tool) => tool.mcpToolId === editingToolId) ?? null;
  const editingDraft = editingTool
    ? draftOverrides[editingTool.mcpToolId] ?? draftFromTool(editingTool)
    : null;

  function updateDraft(
    tool: McpTool,
    updater: (current: ToolPermissionDraft) => ToolPermissionDraft
  ) {
    setDraftOverrides((current) => {
      const draft = current[tool.mcpToolId] ?? draftFromTool(tool);
      return { ...current, [tool.mcpToolId]: updater(draft) };
    });
  }

  function handleOpenChange(nextOpen: boolean) {
    if (!nextOpen) {
      setDraftOverrides({});
      setEditingToolId(null);
      setSaveError(null);
      setAutofillMessage(null);
      setAutofillError(null);
    }
    onOpenChange(nextOpen);
  }

  const handleAutofill = React.useCallback(async () => {
    if (!serverId) return;
    setAutofillMessage(null);
    setAutofillError(null);
    try {
      const response = await autofillToolCalibrations({
        variables: { mcpServerId: serverId }
      });
      const suggestions = response.data?.autofillToolCalibrations.suggestions ?? [];
      setDraftOverrides((current) => {
        const next = { ...current };
        for (const suggestion of suggestions) {
          const tool = tools.find((item) => item.mcpToolId === suggestion.mcpToolId);
          if (!tool) continue;
          next[suggestion.mcpToolId] = draftFromSuggestion(
            suggestion,
            current[suggestion.mcpToolId] ?? draftFromTool(tool)
          );
        }
        return next;
      });
      setAutofillMessage("Autofill suggestions applied. Review before saving.");
    } catch {
      setAutofillError(
        "Autofill could not generate suggestions. Configure tools manually or try again."
      );
    }
  }, [autofillToolCalibrations, serverId, tools]);

  React.useEffect(() => {
    if (!open || !serverId || !autoAutofill || result.loading || result.error || tools.length === 0) {
      return;
    }
    if (autoAutofillKey.current === serverId) return;
    autoAutofillKey.current = serverId;
    void handleAutofill().finally(onAutoAutofillComplete);
  }, [
    autoAutofill,
    handleAutofill,
    onAutoAutofillComplete,
    open,
    result.error,
    result.loading,
    serverId,
    tools.length
  ]);

  async function handleSaveAll() {
    setSaveError(null);
    for (const tool of tools) {
      const draft = draftOverrides[tool.mcpToolId] ?? draftFromTool(tool);
      const validationError = validateDraft(draft);
      if (validationError) {
        setEditingToolId(tool.mcpToolId);
        setSaveError(`${tool.name}: ${validationError}`);
        return;
      }
    }
    try {
      const inputs = tools.map((tool) => {
        const draft = draftOverrides[tool.mcpToolId] ?? draftFromTool(tool);
        return calibrationInputForTool(tool, draft);
      });
      await saveToolCalibrations({ variables: { inputs } });
      await result.refetch();
      setDraftOverrides({});
      setAutofillMessage(null);
      handleOpenChange(false);
    } catch (error) {
      setSaveError(safeCalibrationSaveError(error));
    }
  }

  return (
    <Dialog
      isOpen={open}
      onOpenChange={handleOpenChange}
      purpose="form"
      width={760}
      maxHeight="85vh"
      aria-label={editingTool ? editingTool.name : "Configure tool permissions"}
    >
      <div {...stylex.props(styles.dialog)}>
        {editingTool && editingDraft ? (
          <>
            <DialogHeader
              title={editingTool.name}
              subtitle="Configure this tool. Use Save to persist all tool permission changes."
              onOpenChange={handleOpenChange}
              hasDivider
            />
            <div {...stylex.props(styles.dialogBody)}>
              {autofillState.loading ? (
                <ToolPermissionEditorGlimmer toolName={editingTool.name} />
              ) : (
                <ToolPermissionEditor
                  tool={editingTool}
                  draft={editingDraft}
                  onDraftChange={(updater) => updateDraft(editingTool, updater)}
                />
              )}
              {autofillMessage ? (
                <p {...stylex.props(styles.mutedText)}>{autofillMessage}</p>
              ) : null}
              {autofillError ? (
                <p {...stylex.props(styles.errorText)}>{autofillError}</p>
              ) : null}
              {saveError ? <p {...stylex.props(styles.errorText)}>{saveError}</p> : null}
            </div>
            <ToolPermissionsFooter
              canSave={tools.length > 0}
              saving={saveState.loading}
              loading={result.loading}
              autofilling={autofillState.loading}
              onAutofill={() => void handleAutofill()}
              onSave={() => void handleSaveAll()}
              onBack={() => setEditingToolId(null)}
            />
          </>
        ) : (
          <>
            <DialogHeader
              title="Configure tool permissions"
              subtitle={`${
                serverName ?? serverId ?? "MCP server"
              } tools stay unavailable until each ready tool has reviewed permissions.`}
              onOpenChange={handleOpenChange}
              hasDivider
            />
            <div {...stylex.props(styles.dialogBody)}>
              {result.loading && !result.data ? (
                <p {...stylex.props(styles.loadingText)}>
                  <Loader2 {...stylex.props(styles.spinner)} aria-hidden="true" />
                  Loading tools...
                </p>
              ) : null}
              {result.error ? (
                <p {...stylex.props(styles.errorText)}>Tool metadata could not be loaded.</p>
              ) : null}
              {autofillMessage ? (
                <p {...stylex.props(styles.mutedText)}>{autofillMessage}</p>
              ) : null}
              {autofillError ? (
                <p {...stylex.props(styles.errorText)}>{autofillError}</p>
              ) : null}
              {!result.loading && !result.error && tools.length === 0 ? (
                <p {...stylex.props(styles.mutedText)}>No tools were discovered.</p>
              ) : null}
              {tools.map((tool) => {
                const draft = draftOverrides[tool.mcpToolId] ?? draftFromTool(tool);
                return (
                  <McpToolPermissionRow
                    key={tool.mcpToolId}
                    tool={tool}
                    draft={draft}
                    attention={toolAttentionLabel(draft)}
                    autofilling={autofillState.loading}
                    onEdit={() => setEditingToolId(tool.mcpToolId)}
                  />
                );
              })}
              {saveError ? <p {...stylex.props(styles.errorText)}>{saveError}</p> : null}
            </div>
            <ToolPermissionsFooter
              canSave={tools.length > 0}
              saving={saveState.loading}
              loading={result.loading}
              autofilling={autofillState.loading}
              onAutofill={() => void handleAutofill()}
              onSave={() => void handleSaveAll()}
            />
          </>
        )}
      </div>
    </Dialog>
  );
}

export type ToolPermissionDraft = {
  readClassification: string;
  writeClassification: string;
  exportClassification: string;
  disabled: boolean;
};

function ToolPermissionEditorGlimmer({ toolName }: { toolName: string }) {
  return (
    <div {...stylex.props(styles.glimmerGrid)} aria-label={`Autofilling ${toolName}`}>
      <div {...stylex.props(styles.glimmerLine)} aria-hidden="true" />
      <div {...stylex.props(styles.glimmerPanel)} aria-hidden="true" />
      <div {...stylex.props(styles.classificationGrid)}>
        <div {...stylex.props(styles.glimmerControl)} aria-hidden="true" />
        <div {...stylex.props(styles.glimmerControl)} aria-hidden="true" />
        <div {...stylex.props(styles.glimmerControl)} aria-hidden="true" />
        <div {...stylex.props(styles.glimmerControl)} aria-hidden="true" />
      </div>
      <div {...stylex.props(styles.glimmerPanelSmall)} aria-hidden="true" />
    </div>
  );
}

function ToolPermissionEditor({
  tool,
  draft,
  onDraftChange
}: {
  tool: McpTool;
  draft: ToolPermissionDraft;
  onDraftChange: (updater: (current: ToolPermissionDraft) => ToolPermissionDraft) => void;
}) {
  const validationError = validateDraft(draft);
  const blocked = toolAttentionLabel(draft) === "Needs attention" && !validationError;

  return (
    <>
      {tool.description ? <McpToolDescription description={tool.description} /> : null}
      <McpToolSchemaPreview tool={tool} />
      <div {...stylex.props(styles.classificationGrid)}>
        <SelectField
          label="Read"
          value={draft.readClassification}
          options={classificationOptions}
          onChange={(readClassification) =>
            onDraftChange((current) => ({ ...current, readClassification }))
          }
        />
        <SelectField
          label="Write"
          value={draft.writeClassification}
          options={classificationOptions}
          onChange={(writeClassification) =>
            onDraftChange((current) => ({ ...current, writeClassification }))
          }
        />
        <SelectField
          label="Export"
          value={draft.exportClassification}
          options={classificationOptions}
          onChange={(exportClassification) =>
            onDraftChange((current) => ({ ...current, exportClassification }))
          }
        />
        <CheckboxField
          label="Disabled"
          checked={draft.disabled}
          onChange={(disabled) => onDraftChange((current) => ({ ...current, disabled }))}
        />
      </div>
      {validationError ? <p {...stylex.props(styles.errorText)}>{validationError}</p> : null}
      {blocked ? (
        <p {...stylex.props(styles.mutedText)}>
          Mixed ownership cannot be enforced yet, so this tool will stay blocked.
        </p>
      ) : null}
    </>
  );
}

function draftFromTool(tool: McpTool): ToolPermissionDraft {
  return {
    readClassification: tool.calibration?.readClassification ?? "mixed",
    writeClassification: tool.calibration?.writeClassification ?? "none",
    exportClassification: tool.calibration?.exportClassification ?? "none",
    disabled: tool.calibration?.status === "disabled"
  };
}

function draftFromSuggestion(
  suggestion: AutofillToolCalibrationsMutation["autofillToolCalibrations"]["suggestions"][number],
  currentDraft: ToolPermissionDraft
): ToolPermissionDraft {
  return {
    readClassification: suggestion.readClassification,
    writeClassification: suggestion.writeClassification,
    exportClassification: suggestion.exportClassification,
    disabled: suggestion.disabled ?? currentDraft.disabled
  };
}

function validateDraft(draft: ToolPermissionDraft) {
  if (draft.disabled) return null;
  const hasAnyClassification = [
    draft.readClassification,
    draft.writeClassification,
    draft.exportClassification
  ].some((classification) => classification !== "none");
  if (!hasAnyClassification) return "Enabled tools need at least one non-none permission axis.";
  return null;
}

function toolAttentionLabel(draft: ToolPermissionDraft) {
  if (draft.disabled) return null;
  if (validateDraft(draft)) return "Needs attention";
  if (hasMixedClassification(draft)) return "Needs attention";
  return null;
}

function calibrationInputForTool(tool: McpTool, draft: ToolPermissionDraft) {
  const status = derivedCalibrationStatus(draft);
  const reviewed = status === "ready" || status === "blocked_unresolved_ownership";
  return {
    calibrationId: tool.calibration?.calibrationId ?? calibrationIdForTool(tool.mcpToolId),
    mcpToolId: tool.mcpToolId,
    readClassification: draft.readClassification,
    writeClassification: draft.writeClassification,
    exportClassification: draft.exportClassification,
    status,
    reviewedBy: reviewed ? "human:local" : null,
    reviewedMetadataFingerprint: reviewed ? tool.metadataFingerprint : null
  };
}

function derivedCalibrationStatus(draft: ToolPermissionDraft) {
  if (draft.disabled) return "disabled";
  if (hasMixedClassification(draft)) {
    return "blocked_unresolved_ownership";
  }
  return "ready";
}

function hasMixedClassification(draft: ToolPermissionDraft) {
  return [
    draft.readClassification,
    draft.writeClassification,
    draft.exportClassification
  ].includes("mixed");
}

function SelectField<T extends readonly string[]>({
  label,
  value,
  options,
  onChange
}: {
  label: string;
  value: string;
  options: T;
  onChange: (value: T[number]) => void;
}) {
  return (
    <div {...stylex.props(styles.field)}>
      <span>{label}</span>
      <Selector
        isLabelHidden
        label={label}
        options={[...options]}
        value={value}
        width="100%"
        onChange={(nextValue) => onChange(nextValue as T[number])}
      />
    </div>
  );
}

function CheckboxField({
  label,
  checked,
  onChange
}: {
  label: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <label {...stylex.props(styles.checkboxField)}>
      <span>{label}</span>
      <span {...stylex.props(styles.checkboxFrame)}>
        <input
          aria-label="Disable tool"
          type="checkbox"
          checked={checked}
          onChange={(event) => onChange(event.currentTarget.checked)}
        />
      </span>
    </label>
  );
}

function safeCalibrationSaveError(error: unknown) {
  if (error instanceof Error && error.message.includes("ready mixed")) {
    return "Mixed tools need at least one owner extractor before they can be ready.";
  }
  return "Could not save tool permissions. Check the required fields and try again.";
}

function calibrationIdForTool(mcpToolId: string) {
  const fragment = mcpToolId.replace(/[^A-Za-z0-9_-]+/g, "_").replace(/^_+|_+$/g, "");
  return `tool_calibration:${fragment || "tool"}`;
}

const styles = stylex.create({
  dialog: {
    display: "grid",
    gridTemplateRows: "auto minmax(0, 1fr) auto",
    minHeight: 0,
    maxHeight: "85vh"
  },
  dialogBody: {
    display: "grid",
    gap: 12,
    minHeight: 0,
    overflowY: "auto",
    padding: 16
  },
  mutedText: {
    margin: 0,
    fontSize: 14,
    lineHeight: 1.5,
    color: "var(--muted-foreground)"
  },
  errorText: {
    margin: 0,
    fontSize: 14,
    lineHeight: 1.5,
    color: "var(--destructive)"
  },
  loadingText: {
    display: "flex",
    alignItems: "center",
    gap: 8,
    margin: 0,
    fontSize: 14,
    lineHeight: 1.5,
    color: "var(--muted-foreground)"
  },
  spinner: {
    width: 16,
    height: 16
  },
  glimmerGrid: {
    display: "grid",
    gap: 12,
    opacity: 0.7
  },
  glimmerLine: {
    width: "100%",
    maxWidth: 576,
    height: 16,
    borderRadius: 6,
    backgroundColor: "var(--muted)"
  },
  glimmerPanel: {
    height: 96,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border-subtle)",
    borderRadius: 6,
    backgroundColor: "var(--muted)"
  },
  glimmerPanelSmall: {
    height: 80,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border-subtle)",
    borderRadius: 6,
    backgroundColor: "var(--muted)"
  },
  glimmerControl: {
    height: 56,
    borderRadius: 6,
    backgroundColor: "var(--muted)"
  },
  classificationGrid: {
    display: "grid",
    gap: 8,
    gridTemplateColumns: "repeat(4, minmax(0, 1fr))",
    "@media (max-width: 760px)": {
      gridTemplateColumns: "1fr"
    }
  },
  field: {
    display: "grid",
    gap: 4,
    fontSize: 14,
    fontWeight: 500,
    lineHeight: 1.5,
    color: "var(--foreground)"
  },
  checkboxField: {
    display: "grid",
    alignContent: "start",
    gap: 8,
    fontSize: 14,
    fontWeight: 500,
    lineHeight: 1.5,
    color: "var(--foreground)"
  },
  checkboxFrame: {
    display: "flex",
    height: 36,
    alignItems: "center",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border-subtle)",
    borderRadius: 6,
    paddingInline: 12
  }
});

import * as React from "react";
import { useMutation, useQuery } from "@apollo/client/react";
import { ArrowLeft, Loader2, Pencil, Plus, Save, Trash2 } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogBody,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle
} from "@/components/ui/dialog";
import { Item, ItemActions, ItemContent, ItemTitle } from "@/components/ui/item";
import {
  McpToolsDocument,
  SaveToolCalibrationDocument,
  type McpToolsQuery,
  type SaveToolCalibrationMutation
} from "@/generated/graphql";

type McpTool = McpToolsQuery["mcpTools"][number];

const classificationOptions = ["none", "trusted", "untrusted", "mixed"] as const;
const extractorSourceOptions = [
  "arguments",
  "structured_content",
  "metadata",
  "resource_uri",
  "built_in_adapter"
] as const;
const selectorKindOptions = ["email", "phone", "domain"] as const;

export function McpToolPermissionsModal({
  open,
  serverId,
  serverName,
  onOpenChange
}: {
  open: boolean;
  serverId: string | null;
  serverName: string | null;
  onOpenChange: (open: boolean) => void;
}) {
  const result = useQuery<McpToolsQuery>(McpToolsDocument, {
    variables: { mcpServerId: serverId ?? "" },
    skip: !open || !serverId,
    fetchPolicy: "cache-and-network"
  });
  const [saveToolCalibration, saveState] =
    useMutation<SaveToolCalibrationMutation>(SaveToolCalibrationDocument);
  const [draftOverrides, setDraftOverrides] = React.useState<Record<string, ToolPermissionDraft>>(
    {}
  );
  const [editingToolId, setEditingToolId] = React.useState<string | null>(null);
  const [saveError, setSaveError] = React.useState<string | null>(null);
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
    }
    onOpenChange(nextOpen);
  }

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
      for (const tool of tools) {
        const draft = draftOverrides[tool.mcpToolId] ?? draftFromTool(tool);
        await saveToolCalibration({ variables: { input: calibrationInputForTool(tool, draft) } });
      }
      await result.refetch();
      setDraftOverrides({});
    } catch (error) {
      setSaveError(safeCalibrationSaveError(error));
    }
  }

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent className="grid-rows-[auto_minmax(0,1fr)_auto]">
        {editingTool && editingDraft ? (
          <>
            <DialogHeader>
              <DialogTitle>{editingTool.name}</DialogTitle>
              <DialogDescription>
                Configure this tool. Use Save to persist all tool permission changes.
              </DialogDescription>
            </DialogHeader>
            <DialogBody className="grid gap-3 overflow-y-auto">
              <ToolPermissionEditor
                tool={editingTool}
                draft={editingDraft}
                onDraftChange={(updater) => updateDraft(editingTool, updater)}
              />
              {saveError ? <p className="m-0 text-sm text-destructive">{saveError}</p> : null}
            </DialogBody>
            <ToolPermissionsFooter
              canSave={tools.length > 0}
              saving={saveState.loading}
              loading={result.loading}
              onSave={() => void handleSaveAll()}
              onBack={() => setEditingToolId(null)}
            />
          </>
        ) : (
          <>
            <DialogHeader>
              <DialogTitle>Configure tool permissions</DialogTitle>
              <DialogDescription>
                {serverName ?? serverId ?? "MCP server"} tools stay unavailable until each ready
                tool has reviewed permissions and owner resolution where needed.
              </DialogDescription>
            </DialogHeader>
            <DialogBody className="grid gap-3 overflow-y-auto">
              {result.loading && !result.data ? (
                <p className="m-0 flex items-center gap-2 text-sm text-muted-foreground">
                  <Loader2 className="size-4 animate-spin" aria-hidden="true" />
                  Loading tools...
                </p>
              ) : null}
              {result.error ? (
                <p className="m-0 text-sm text-destructive">Tool metadata could not be loaded.</p>
              ) : null}
              {!result.loading && !result.error && tools.length === 0 ? (
                <p className="m-0 text-sm text-muted-foreground">No tools were discovered.</p>
              ) : null}
              {tools.map((tool) => {
                const draft = draftOverrides[tool.mcpToolId] ?? draftFromTool(tool);
                return (
                  <ToolPermissionItem
                    key={tool.mcpToolId}
                    tool={tool}
                    draft={draft}
                    onEdit={() => setEditingToolId(tool.mcpToolId)}
                  />
                );
              })}
              {saveError ? <p className="m-0 text-sm text-destructive">{saveError}</p> : null}
            </DialogBody>
            <ToolPermissionsFooter
              canSave={tools.length > 0}
              saving={saveState.loading}
              loading={result.loading}
              onSave={() => void handleSaveAll()}
            />
          </>
        )}
      </DialogContent>
    </Dialog>
  );
}

type OwnerExtractorDraft = {
  id: string;
  source: string;
  selectorKind: string;
  path: string;
};

type ToolPermissionDraft = {
  readClassification: string;
  writeClassification: string;
  exportClassification: string;
  ownerExtractors: OwnerExtractorDraft[];
  disabled: boolean;
};

function ToolPermissionItem({
  tool,
  draft,
  onEdit
}: {
  tool: McpTool;
  draft: ToolPermissionDraft;
  onEdit: () => void;
}) {
  const attention = toolAttentionLabel(draft);
  return (
    <Item>
      <ItemContent>
        <div className="flex min-w-0 flex-wrap items-center gap-2">
          <ItemTitle>{tool.name}</ItemTitle>
          <ToolStatusBadge disabled={draft.disabled} />
          {attention ? <Badge variant="destructive">{attention}</Badge> : null}
        </div>
        <ToolPermissionSummary draft={draft} />
      </ItemContent>
      <ItemActions>
        <Button
          type="button"
          variant="ghost"
          size="icon-sm"
          aria-label={`Edit ${tool.name}`}
          onClick={onEdit}
        >
          <Pencil className="size-4" aria-hidden="true" />
        </Button>
      </ItemActions>
    </Item>
  );
}

function ToolPermissionSummary({ draft }: { draft: ToolPermissionDraft }) {
  return (
    <div
      className="mt-1 flex min-w-0 flex-wrap items-center gap-1.5"
      aria-label="Configured tool permissions"
    >
      <ClassificationBadge label="Read" value={draft.readClassification} />
      <ClassificationBadge label="Write" value={draft.writeClassification} />
      <ClassificationBadge label="Export" value={draft.exportClassification} />
    </div>
  );
}

function ToolStatusBadge({ disabled }: { disabled: boolean }) {
  const style = disabled
    ? {
        backgroundColor: "var(--red-100)",
        borderColor: "var(--red-700)",
        color: "var(--red-700)"
      }
    : {
        backgroundColor: "var(--pine-50)",
        borderColor: "var(--pine-500)",
        color: "var(--pine-700)"
      };

  return (
    <Badge variant="outline" style={style}>
      {disabled ? "Disabled" : "Enabled"}
    </Badge>
  );
}

function ClassificationBadge({ label, value }: { label: string; value: string }) {
  return (
    <Badge
      variant={value === "none" ? "outline" : "secondary"}
      className="font-mono text-xs"
    >
      {label}: {value}
    </Badge>
  );
}

function ToolPermissionsFooter({
  canSave,
  saving,
  loading,
  onSave,
  onBack
}: {
  canSave: boolean;
  saving: boolean;
  loading: boolean;
  onSave: () => void;
  onBack?: () => void;
}) {
  return (
    <div className="flex items-center justify-between gap-3 border-t border-[var(--border-subtle)] px-6 py-4">
      {onBack ? (
        <Button type="button" variant="ghost" className="w-fit" onClick={onBack}>
          <ArrowLeft className="size-4" aria-hidden="true" />
          Back
        </Button>
      ) : (
        <span aria-hidden="true" />
      )}
      <Button
        type="button"
        className="w-fit"
        disabled={saving || loading || !canSave}
        onClick={onSave}
      >
        {saving ? (
          <Loader2 className="size-4 animate-spin" aria-hidden="true" />
        ) : (
          <Save className="size-4" aria-hidden="true" />
        )}
        Save
      </Button>
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
      {tool.description ? <ToolDescription description={tool.description} /> : null}
      <details className="rounded-md border border-[var(--border-subtle)] p-3">
        <summary className="cursor-pointer text-sm font-medium">Review discovered schema</summary>
        <pre className="mt-3 max-h-56 overflow-auto whitespace-pre-wrap rounded-md bg-muted p-3 text-xs">
          {formatSchemaPreview(tool)}
        </pre>
      </details>
      <div className="grid gap-2 sm:grid-cols-4">
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
      <OwnerExtractorsEditor draft={draft} onChange={onDraftChange} />
      {validationError ? <p className="m-0 text-sm text-destructive">{validationError}</p> : null}
      {blocked ? (
        <p className="m-0 text-sm text-muted-foreground">
          This mixed tool will stay blocked until an owner extractor is added.
        </p>
      ) : null}
    </>
  );
}

function ToolDescription({ description }: { description: string }) {
  const [expanded, setExpanded] = React.useState(false);

  return (
    <div className="min-w-0 text-sm text-muted-foreground">
      <div className={expanded ? "grid gap-1" : "flex min-w-0 items-baseline gap-2"}>
        <p className={expanded ? "m-0 whitespace-pre-wrap" : "m-0 min-w-0 flex-1 truncate"}>
          {description}
        </p>
        <Button
          type="button"
          variant="link"
          className="h-auto w-fit shrink-0 p-0 text-xs"
          onClick={() => setExpanded((current) => !current)}
        >
          {expanded ? "Show less" : "Show more..."}
        </Button>
      </div>
    </div>
  );
}

function OwnerExtractorsEditor({
  draft,
  onChange
}: {
  draft: ToolPermissionDraft;
  onChange: (updater: (current: ToolPermissionDraft) => ToolPermissionDraft) => void;
}) {
  return (
    <section className="grid gap-2">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="grid gap-1">
          <h4 className="m-0 text-sm font-medium">Owner resolution</h4>
          <p className="m-0 text-sm text-muted-foreground">
            Mixed tools need a deterministic field that resolves an email, phone, or domain owner.
          </p>
        </div>
        <Button
          type="button"
          variant="outline"
          className="w-fit"
          onClick={() =>
            onChange((current) => ({
              ...current,
              ownerExtractors: [
                ...current.ownerExtractors,
                {
                  id: `extractor:${Date.now()}`,
                  source: "arguments",
                  selectorKind: "email",
                  path: ""
                }
              ]
            }))
          }
        >
          <Plus className="size-4" aria-hidden="true" />
          Add extractor
        </Button>
      </div>
      {draft.ownerExtractors.length === 0 ? (
        <p className="m-0 text-sm text-muted-foreground">
          No owner extractor configured. Mixed tools stay blocked until one is added.
        </p>
      ) : null}
      {draft.ownerExtractors.map((extractor) => (
        <div
          key={extractor.id}
          className="grid gap-2 rounded-md border border-[var(--border-subtle)] p-3 sm:grid-cols-[1fr_1fr_2fr_auto]"
        >
          <SelectField
            label="Source"
            value={extractor.source}
            options={extractorSourceOptions}
            onChange={(source) => updateExtractor(onChange, extractor.id, { source })}
          />
          <SelectField
            label="Identity"
            value={extractor.selectorKind}
            options={selectorKindOptions}
            onChange={(selectorKind) => updateExtractor(onChange, extractor.id, { selectorKind })}
          />
          <TextField
            label="Path"
            value={extractor.path}
            onChange={(path) => updateExtractor(onChange, extractor.id, { path })}
          />
          <Button
            type="button"
            variant="ghost"
            className="self-end"
            aria-label="Remove owner extractor"
            onClick={() =>
              onChange((current) => ({
                ...current,
                ownerExtractors: current.ownerExtractors.filter((item) => item.id !== extractor.id)
              }))
            }
          >
            <Trash2 className="size-4" aria-hidden="true" />
          </Button>
        </div>
      ))}
    </section>
  );
}

function updateExtractor(
  onChange: (updater: (current: ToolPermissionDraft) => ToolPermissionDraft) => void,
  id: string,
  patch: Partial<Omit<OwnerExtractorDraft, "id">>
) {
  onChange((current) => ({
    ...current,
    ownerExtractors: current.ownerExtractors.map((extractor) =>
      extractor.id === id ? { ...extractor, ...patch } : extractor
    )
  }));
}

function draftFromTool(tool: McpTool): ToolPermissionDraft {
  return {
    readClassification: tool.calibration?.readClassification ?? "mixed",
    writeClassification: tool.calibration?.writeClassification ?? "none",
    exportClassification: tool.calibration?.exportClassification ?? "none",
    ownerExtractors: (tool.calibration?.ownerExtractors ?? []).map((extractor, index) => ({
      id: `${tool.mcpToolId}:extractor:${index}`,
      source: extractor.source,
      selectorKind: extractor.selectorKind,
      path: extractor.path
    })),
    disabled: tool.calibration?.status === "disabled"
  };
}

function validateDraft(draft: ToolPermissionDraft) {
  if (draft.disabled) return null;
  const hasAnyClassification = [
    draft.readClassification,
    draft.writeClassification,
    draft.exportClassification
  ].some((classification) => classification !== "none");
  const hasBlankExtractorPath = draft.ownerExtractors.some((extractor) => !extractor.path.trim());

  if (hasBlankExtractorPath) return "Owner extractor paths cannot be empty.";
  if (!hasAnyClassification) return "Enabled tools need at least one non-none permission axis.";
  return null;
}

function toolAttentionLabel(draft: ToolPermissionDraft) {
  if (draft.disabled) return null;
  if (validateDraft(draft)) return "Needs attention";
  if (hasMixedClassification(draft) && draft.ownerExtractors.length === 0) return "Needs attention";
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
    ownerExtractors: draft.ownerExtractors
      .filter((extractor) => !draft.disabled || extractor.path.trim())
      .map(({ source, selectorKind, path }) => ({
        source,
        selectorKind,
        path: path.trim()
      })),
    status,
    reviewedBy: reviewed ? "human:local" : null,
    reviewedMetadataFingerprint: reviewed ? tool.metadataFingerprint : null
  };
}

function derivedCalibrationStatus(draft: ToolPermissionDraft) {
  if (draft.disabled) return "disabled";
  if (hasMixedClassification(draft) && draft.ownerExtractors.length === 0) {
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
    <label className="grid gap-1 text-sm font-medium">
      {label}
      <select
        className="h-9 rounded-md border border-[var(--border-subtle)] bg-white px-2 text-sm font-normal"
        value={value}
        onChange={(event) => onChange(event.currentTarget.value as T[number])}
      >
        {options.map((option) => (
          <option key={option} value={option}>
            {option}
          </option>
        ))}
      </select>
    </label>
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
    <label className="grid content-start gap-2 text-sm font-medium">
      {label}
      <span className="flex h-9 items-center rounded-md border border-[var(--border-subtle)] px-3">
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

function TextField({
  label,
  value,
  onChange
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <label className="grid gap-1 text-sm font-medium">
      {label}
      <input
        className="h-9 rounded-md border border-[var(--border-subtle)] px-3 text-sm font-normal"
        value={value}
        onChange={(event) => onChange(event.currentTarget.value)}
      />
    </label>
  );
}

function formatSchemaPreview(tool: McpTool) {
  return JSON.stringify(
    {
      inputSchema: tool.inputSchema,
      outputSchema: tool.outputSchema,
      annotations: tool.annotations
    },
    null,
    2
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

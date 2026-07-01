import * as React from "react";
import { useMutation, useQuery } from "@apollo/client/react";
import { Loader2, Plus, Save, ShieldCheck, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogBody,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle
} from "@/components/ui/dialog";
import { Textarea } from "@/components/ui/textarea";
import {
  AgentsDocument,
  McpToolsDocument,
  SaveToolCalibrationDocument,
  type AgentsQuery,
  type McpToolsQuery,
  type SaveToolCalibrationMutation
} from "@/generated/graphql";

type McpTool = McpToolsQuery["mcpTools"][number];
type Agent = AgentsQuery["agents"][number];

const classificationOptions = ["none", "trusted", "untrusted", "mixed"] as const;
const statusOptions = ["needs_review", "blocked_unresolved_ownership", "ready", "disabled"] as const;
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
  const agentsResult = useQuery<AgentsQuery>(AgentsDocument, {
    skip: !open,
    fetchPolicy: "cache-and-network"
  });
  const [saveToolCalibration, saveState] =
    useMutation<SaveToolCalibrationMutation>(SaveToolCalibrationDocument);
  const [saveError, setSaveError] = React.useState<string | null>(null);

  async function handleSave(tool: McpTool, draft: ToolPermissionDraft) {
    setSaveError(null);
    try {
      await saveToolCalibration({
        variables: {
          input: {
            calibrationId: tool.calibration?.calibrationId ?? calibrationIdForTool(tool.mcpToolId),
            mcpToolId: tool.mcpToolId,
            readClassification: draft.readClassification,
            writeClassification: draft.writeClassification,
            exportClassification: draft.exportClassification,
            ownerExtractors: draft.ownerExtractors.map(({ source, selectorKind, path }) => ({
              source,
              selectorKind,
              path
            })),
            enabledAgentIds: draft.enabledAgentIds,
            enabledScopeIds: draft.enabledScopeIds,
            status: draft.status,
            reviewedBy: draft.status === "needs_review" ? null : "human:local",
            reviewedMetadataFingerprint:
              draft.status === "needs_review" ? null : tool.metadataFingerprint
          }
        }
      });
      await result.refetch();
    } catch (error) {
      setSaveError(safeCalibrationSaveError(error));
    }
  }

  const tools = result.data?.mcpTools ?? [];
  const agents = agentsResult.data?.agents ?? [];

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="grid-rows-[auto_minmax(0,1fr)]">
        <DialogHeader>
          <DialogTitle>Configure tool permissions</DialogTitle>
          <DialogDescription>
            {serverName ?? serverId ?? "MCP server"} tools stay unavailable until each ready tool has
            owner resolution, agent visibility, and scope visibility.
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
          {tools.map((tool) => (
            <ToolPermissionEditor
              key={toolPermissionEditorKey(tool)}
              tool={tool}
              agents={agents}
              saving={saveState.loading}
              onSave={(draft) => void handleSave(tool, draft)}
            />
          ))}
          {saveError ? <p className="m-0 text-sm text-destructive">{saveError}</p> : null}
        </DialogBody>
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
  enabledAgentIds: string[];
  enabledScopeIds: string[];
  status: string;
};

function ToolPermissionEditor({
  tool,
  agents,
  saving,
  onSave
}: {
  tool: McpTool;
  agents: readonly Agent[];
  saving: boolean;
  onSave: (draft: ToolPermissionDraft) => void;
}) {
  const [draft, setDraft] = React.useState<ToolPermissionDraft>(() => ({
    readClassification: tool.calibration?.readClassification ?? "mixed",
    writeClassification: tool.calibration?.writeClassification ?? "none",
    exportClassification: tool.calibration?.exportClassification ?? "none",
    ownerExtractors: (tool.calibration?.ownerExtractors ?? []).map((extractor, index) => ({
      id: `${tool.mcpToolId}:extractor:${index}`,
      source: extractor.source,
      selectorKind: extractor.selectorKind,
      path: extractor.path
    })),
    enabledAgentIds: tool.calibration?.enabledAgentIds.length
      ? [...tool.calibration.enabledAgentIds]
      : ["agent:primary"],
    enabledScopeIds: tool.calibration?.enabledScopeIds.length
      ? [...tool.calibration.enabledScopeIds]
      : ["human:local"],
    status: tool.calibration?.status ?? "needs_review"
  }));
  const [localError, setLocalError] = React.useState<string | null>(null);

  function save() {
    const validationError = validateReadyDraft(draft);
    if (validationError) {
      setLocalError(validationError);
      return;
    }
    setLocalError(null);
    onSave(draft);
  }

  return (
    <article className="grid gap-3 rounded-md border border-[var(--border-subtle)] p-4">
      <div className="flex min-w-0 items-start gap-2">
        <ShieldCheck className="mt-0.5 size-4 text-[var(--pine-700)]" aria-hidden="true" />
        <div className="grid min-w-0 gap-1">
          <h3 className="m-0 break-words font-heading text-base leading-tight tracking-normal">
            {tool.name}
          </h3>
          {tool.description ? <ToolDescription description={tool.description} /> : null}
        </div>
      </div>

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
          onChange={(readClassification) => setDraft((current) => ({ ...current, readClassification }))}
        />
        <SelectField
          label="Write"
          value={draft.writeClassification}
          options={classificationOptions}
          onChange={(writeClassification) =>
            setDraft((current) => ({ ...current, writeClassification }))
          }
        />
        <SelectField
          label="Export"
          value={draft.exportClassification}
          options={classificationOptions}
          onChange={(exportClassification) =>
            setDraft((current) => ({ ...current, exportClassification }))
          }
        />
        <SelectField
          label="Status"
          value={draft.status}
          options={statusOptions}
          onChange={(status) => setDraft((current) => ({ ...current, status }))}
        />
      </div>

      <OwnerExtractorsEditor draft={draft} onChange={setDraft} />
      <AgentVisibilityEditor agents={agents} draft={draft} onChange={setDraft} />
      <ScopeVisibilityEditor draft={draft} onChange={setDraft} />

      {localError ? <p className="m-0 text-sm text-destructive">{localError}</p> : null}
      <Button type="button" className="w-fit" disabled={saving} onClick={save}>
        <Save className="size-4" aria-hidden="true" />
        Save
      </Button>
    </article>
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
  onChange: React.Dispatch<React.SetStateAction<ToolPermissionDraft>>;
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
          No owner extractor configured. Save as blocked or add one before marking the tool ready.
        </p>
      ) : null}
      {draft.ownerExtractors.map((extractor) => (
        <div key={extractor.id} className="grid gap-2 rounded-md border border-[var(--border-subtle)] p-3 sm:grid-cols-[1fr_1fr_2fr_auto]">
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

function AgentVisibilityEditor({
  agents,
  draft,
  onChange
}: {
  agents: readonly Agent[];
  draft: ToolPermissionDraft;
  onChange: React.Dispatch<React.SetStateAction<ToolPermissionDraft>>;
}) {
  return (
    <section className="grid gap-2">
      <h4 className="m-0 text-sm font-medium">Agent visibility</h4>
      {agents.length > 0 ? (
        <div className="grid gap-2 sm:grid-cols-2">
          {agents.map((agent) => (
            <label key={agent.agentId} className="flex items-center gap-2 text-sm">
              <input
                type="checkbox"
                checked={draft.enabledAgentIds.includes(agent.agentId)}
                onChange={(event) =>
                  onChange((current) => ({
                    ...current,
                    enabledAgentIds: toggleValue(
                      current.enabledAgentIds,
                      agent.agentId,
                      event.currentTarget.checked
                    )
                  }))
                }
              />
              <span>{agent.displayName ?? agent.agentId}</span>
            </label>
          ))}
        </div>
      ) : null}
      <LineListField
        label="Enabled agent IDs"
        values={draft.enabledAgentIds}
        onChange={(enabledAgentIds) => onChange((current) => ({ ...current, enabledAgentIds }))}
      />
    </section>
  );
}

function ScopeVisibilityEditor({
  draft,
  onChange
}: {
  draft: ToolPermissionDraft;
  onChange: React.Dispatch<React.SetStateAction<ToolPermissionDraft>>;
}) {
  return (
    <section className="grid gap-2">
      <h4 className="m-0 text-sm font-medium">Scope visibility</h4>
      <LineListField
        label="Enabled scope IDs"
        values={draft.enabledScopeIds}
        onChange={(enabledScopeIds) => onChange((current) => ({ ...current, enabledScopeIds }))}
      />
    </section>
  );
}

function updateExtractor(
  onChange: React.Dispatch<React.SetStateAction<ToolPermissionDraft>>,
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

function validateReadyDraft(draft: ToolPermissionDraft) {
  const hasAnyClassification = [
    draft.readClassification,
    draft.writeClassification,
    draft.exportClassification
  ].some((classification) => classification !== "none");
  const hasMixedClassification = [
    draft.readClassification,
    draft.writeClassification,
    draft.exportClassification
  ].includes("mixed");
  const hasBlankExtractorPath = draft.ownerExtractors.some((extractor) => !extractor.path.trim());

  if (hasBlankExtractorPath) return "Owner extractor paths cannot be empty.";
  if (draft.status !== "ready") return null;
  if (!hasAnyClassification) return "Ready tools need at least one non-none permission axis.";
  if (hasMixedClassification && draft.ownerExtractors.length === 0) {
    return "Mixed tools need at least one owner extractor before they can be ready.";
  }
  if (draft.enabledAgentIds.length === 0) {
    return "Ready tools need at least one enabled agent.";
  }
  if (draft.enabledScopeIds.length === 0) {
    return "Ready tools need at least one enabled scope.";
  }
  return null;
}

function toolPermissionEditorKey(tool: McpTool) {
  return [
    tool.mcpToolId,
    tool.calibration?.readClassification ?? "mixed",
    tool.calibration?.writeClassification ?? "none",
    tool.calibration?.exportClassification ?? "none",
    tool.calibration?.status ?? "needs_review",
    tool.calibration?.ownerExtractors.map((extractor) => extractor.path).join(",") ?? "",
    tool.calibration?.enabledAgentIds.join(",") ?? "",
    tool.calibration?.enabledScopeIds.join(",") ?? ""
  ].join(":");
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

function LineListField({
  label,
  values,
  onChange
}: {
  label: string;
  values: string[];
  onChange: (values: string[]) => void;
}) {
  return (
    <label className="grid gap-1 text-sm font-medium">
      {label}
      <Textarea
        rows={2}
        value={values.join("\n")}
        onChange={(event) => onChange(parseLineList(event.currentTarget.value))}
      />
    </label>
  );
}

function parseLineList(value: string) {
  return value
    .split(/\r?\n|,/)
    .map((item) => item.trim())
    .filter(Boolean);
}

function toggleValue(values: string[], value: string, checked: boolean) {
  if (checked && !values.includes(value)) return [...values, value];
  if (!checked) return values.filter((item) => item !== value);
  return values;
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
  if (error instanceof Error && error.message.includes("enabled agent")) {
    return "Ready tools need at least one enabled agent.";
  }
  if (error instanceof Error && error.message.includes("enabled scope")) {
    return "Ready tools need at least one enabled scope.";
  }
  return "Could not save tool permissions. Check the required fields and try again.";
}

function calibrationIdForTool(mcpToolId: string) {
  const fragment = mcpToolId.replace(/[^A-Za-z0-9_-]+/g, "_").replace(/^_+|_+$/g, "");
  return `tool_calibration:${fragment || "tool"}`;
}

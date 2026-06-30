import * as React from "react";
import { useMutation, useQuery } from "@apollo/client/react";
import { Loader2, Save, ShieldCheck } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogBody,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle
} from "@/components/ui/dialog";
import {
  McpToolsDocument,
  SaveToolCalibrationDocument,
  type McpToolsQuery,
  type SaveToolCalibrationMutation
} from "@/generated/graphql";

type McpTool = McpToolsQuery["mcpTools"][number];

const classificationOptions = ["none", "trusted", "untrusted", "mixed"] as const;
const statusOptions = ["needs_review", "blocked_unresolved_ownership", "ready", "disabled"] as const;

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
            ownerExtractors: [],
            enabledAgentIds: [],
            enabledScopeIds: [],
            status: draft.status,
            reviewedBy: draft.status === "needs_review" ? null : "human:local",
            reviewedMetadataFingerprint:
              draft.status === "needs_review" ? null : tool.metadataFingerprint
          }
        }
      });
      await result.refetch();
    } catch (error) {
      setSaveError(error instanceof Error ? error.message : "Could not save tool permissions");
    }
  }

  const tools = result.data?.mcpTools ?? [];

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Configure tool permissions</DialogTitle>
          <DialogDescription>{serverName ?? serverId ?? "MCP server"}</DialogDescription>
        </DialogHeader>
        <DialogBody className="grid gap-3">
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

type ToolPermissionDraft = {
  readClassification: string;
  writeClassification: string;
  exportClassification: string;
  status: string;
};

function ToolPermissionEditor({
  tool,
  saving,
  onSave
}: {
  tool: McpTool;
  saving: boolean;
  onSave: (draft: ToolPermissionDraft) => void;
}) {
  const [draft, setDraft] = React.useState<ToolPermissionDraft>(() => ({
    readClassification: tool.calibration?.readClassification ?? "mixed",
    writeClassification: tool.calibration?.writeClassification ?? "none",
    exportClassification: tool.calibration?.exportClassification ?? "none",
    status: tool.calibration?.status ?? "needs_review"
  }));

  return (
    <article className="grid gap-3 rounded-md border border-[var(--border-subtle)] p-4">
      <div className="flex min-w-0 items-start gap-2">
        <ShieldCheck className="mt-0.5 size-4 text-[var(--pine-700)]" aria-hidden="true" />
        <div className="grid min-w-0 gap-1">
          <h3 className="m-0 break-words font-heading text-base leading-tight tracking-normal">
            {tool.name}
          </h3>
          {tool.description ? (
            <p className="m-0 text-sm text-muted-foreground">{tool.description}</p>
          ) : null}
        </div>
      </div>
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
      <Button type="button" className="w-fit" disabled={saving} onClick={() => onSave(draft)}>
        <Save className="size-4" aria-hidden="true" />
        Save
      </Button>
    </article>
  );
}

function toolPermissionEditorKey(tool: McpTool) {
  return [
    tool.mcpToolId,
    tool.calibration?.readClassification ?? "mixed",
    tool.calibration?.writeClassification ?? "none",
    tool.calibration?.exportClassification ?? "none",
    tool.calibration?.status ?? "needs_review"
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

function calibrationIdForTool(mcpToolId: string) {
  const fragment = mcpToolId.replace(/[^A-Za-z0-9_-]+/g, "_").replace(/^_+|_+$/g, "");
  return `tool_calibration:${fragment || "tool"}`;
}

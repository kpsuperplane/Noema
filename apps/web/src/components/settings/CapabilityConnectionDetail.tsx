import * as React from "react";
import { useMutation, useQuery } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import { HStack } from "@astryxdesign/core/HStack";
import { TextInput } from "@astryxdesign/core/TextInput";
import { VStack } from "@astryxdesign/core/VStack";
import * as stylex from "@stylexjs/stylex";
import {
  CapabilityConnectionDocument,
  CapabilityIntegrationsDocument,
  ResetCapabilityToolPolicyDocument,
  SaveCapabilityConnectionPolicyDocument,
  SaveCapabilityConnectionLabelDocument,
  SaveCapabilityToolOverrideDocument,
  SetCapabilityToolEnabledDocument,
  type CapabilityConnectionQuery,
  type ResetCapabilityToolPolicyMutation,
  type SaveCapabilityConnectionPolicyMutation,
  type SaveCapabilityConnectionLabelMutation,
  type SaveCapabilityToolOverrideMutation,
  type SetCapabilityToolEnabledMutation
} from "@/generated/graphql";
import {
  CapabilityPolicyChoices,
  type CapabilityDataSharingPolicy,
  type CapabilityUnsafeActionPolicy
} from "@/components/capabilities/CapabilityPolicyChoices";
import { CapabilityToolTable, toolHintSourceDescription } from "./CapabilityToolTable";
import { SettingsEditDialog } from "./SettingsEditDialog";
import { SettingsList, SettingsListItem, SettingsSection } from "./SettingsPrimitives";
import { settingsStatusLabel } from "./settingsStatus";

type ManagedTool = CapabilityConnectionQuery["capabilityTools"][number];
type HintKey = "readOnly" | "idempotent" | "destructive" | "openWorld";
type HintDraft = Record<HintKey, boolean>;

export function CapabilityConnectionDetail({
  kind,
  connectionId,
  sourceActions,
  definitionDetails,
  dangerAction
}: {
  kind: "API" | "MCP";
  connectionId: string;
  sourceActions?: React.ReactNode;
  definitionDetails?: React.ReactNode;
  dangerAction?: React.ReactNode;
}) {
  const reference = { kind, connectionId } as const;
  const result = useQuery<CapabilityConnectionQuery>(CapabilityConnectionDocument, {
    variables: { ref: reference },
    fetchPolicy: "cache-and-network"
  });
  const [savePolicy, policyState] = useMutation<SaveCapabilityConnectionPolicyMutation>(
    SaveCapabilityConnectionPolicyDocument
  );
  const [saveLabel, labelState] = useMutation<SaveCapabilityConnectionLabelMutation>(
    SaveCapabilityConnectionLabelDocument
  );
  const [saveOverride, overrideState] = useMutation<SaveCapabilityToolOverrideMutation>(
    SaveCapabilityToolOverrideDocument
  );
  const [resetTool, resetState] = useMutation<ResetCapabilityToolPolicyMutation>(
    ResetCapabilityToolPolicyDocument
  );
  const [setEnabled, enabledState] = useMutation<SetCapabilityToolEnabledMutation>(
    SetCapabilityToolEnabledDocument
  );
  const connection = result.data?.capabilityConnection ?? null;
  const tools = result.data?.capabilityTools ?? [];
  const [sharingDraft, setSharing] = React.useState<string | null>(null);
  const [unsafeActionsDraft, setUnsafeActions] = React.useState<string | null>(null);
  const [policyEditing, setPolicyEditing] = React.useState(false);
  const [editing, setEditing] = React.useState<string | null>(null);
  const [draft, setDraft] = React.useState<HintDraft | null>(null);
  const [error, setError] = React.useState<string | null>(null);
  const [renaming, setRenaming] = React.useState(false);
  const [labelDraft, setLabelDraft] = React.useState("");
  const editingTool = tools.find((tool) => tool.toolId === editing) ?? null;

  if (result.loading && !result.data) return <p {...stylex.props(styles.muted)}>Loading connection…</p>;
  if (result.error && !connection) {
    return (
      <HStack gap={2} vAlign="center" wrap="wrap">
        <p role="alert" {...stylex.props(styles.error)}>Connection could not load.</p>
        <Button
          type="button"
          size="sm"
          variant="secondary"
          label="Retry"
          onClick={() => void result.refetch().catch(() => undefined)}
        />
      </HStack>
    );
  }
  if (!connection) return <p {...stylex.props(styles.muted)}>This connection no longer exists.</p>;
  const sharing = (sharingDraft ?? connection.dataSharingPolicy ?? "allow_automatically") as CapabilityDataSharingPolicy;
  const unsafeActions = (unsafeActionsDraft ?? connection.unsafeActionPolicy ?? "reviewer_may_approve") as CapabilityUnsafeActionPolicy;

  const fence = {
    kind,
    connectionId,
    expectedConnectionRevision: connection.connectionRevision
  } as const;

  async function refresh() {
    setEditing(null);
    setDraft(null);
    setSharing(null);
    setUnsafeActions(null);
    await result.refetch();
  }

  async function submitPolicy() {
    if (!connection) return;
    setError(null);
    try {
      await savePolicy({ variables: { input: {
        ...fence,
        expectedPolicyRevision: connection.policyRevision,
        dataSharingPolicy: sharing,
        unsafeActionPolicy: unsafeActions
      } } });
      setPolicyEditing(false);
      await refresh();
    } catch {
      setError("This connection changed. Reload it and try again.");
    }
  }

  async function submitLabel() {
    if (!connection) return;
    setError(null);
    try {
      await saveLabel({
        variables: { input: {
          ...fence,
          expectedConnectionLabel: connection.connectionLabel,
          connectionLabel: labelDraft
        } },
        refetchQueries: [{ query: CapabilityIntegrationsDocument, variables: { kind } }]
      });
      setRenaming(false);
      await refresh();
    } catch {
      setError("This connection label changed. Reload it and try again.");
    }
  }

  function beginEdit(tool: ManagedTool) {
    setError(null);
    setEditing(tool.toolId);
    setDraft({
      readOnly: tool.readOnly.value ?? false,
      idempotent: tool.idempotent.value ?? false,
      destructive: tool.destructive.value ?? true,
      openWorld: tool.openWorld.value ?? true
    });
  }

  async function submitOverride(tool: ManagedTool) {
    if (!draft) return;
    setError(null);
    try {
      await saveOverride({ variables: { input: {
        ...fence,
        toolId: tool.toolId,
        sourceRevision: tool.sourceRevision,
        expectedPolicyRevision: tool.policyRevision,
        ...draft
      } } });
      await refresh();
    } catch {
      setError("This tool changed. Reload it and try again.");
    }
  }

  async function reset(tool: ManagedTool) {
    setError(null);
    try {
      await resetTool({ variables: { input: {
        ...fence,
        toolId: tool.toolId,
        sourceRevision: tool.sourceRevision,
        expectedPolicyRevision: tool.policyRevision
      } } });
      await refresh();
    } catch {
      setError(`Could not reset ${tool.name}.`);
    }
  }

  async function toggle(tool: ManagedTool) {
    setError(null);
    try {
      await setEnabled({ variables: { input: {
        ...fence,
        toolId: tool.toolId,
        sourceRevision: tool.sourceRevision,
        expectedPolicyRevision: tool.policyRevision,
        enabled: !tool.enabled
      } } });
      await refresh();
    } catch {
      setError(`Could not ${tool.enabled ? "turn off" : "turn on"} ${tool.name}.`);
    }
  }

  return (
    <VStack gap={5}>
      {result.error ? (
        <HStack gap={2} vAlign="center" wrap="wrap">
          <p role="alert" {...stylex.props(styles.error)}>Connection details may be out of date.</p>
          <Button
            type="button"
            size="sm"
            variant="secondary"
            label="Retry"
            onClick={() => void result.refetch().catch(() => undefined)}
          />
        </HStack>
      ) : null}
      <SettingsSection aria-labelledby="connection-summary-title">
        <HStack hAlign="between" vAlign="center" gap={2}>
          <VStack gap={1}>
            <h2 id="connection-summary-title" {...stylex.props(styles.heading)}>{connection.name}</h2>
            {connectionIssue(connection) ? (
              <p {...stylex.props(styles.muted)}>{connectionIssue(connection)}</p>
            ) : null}
          </VStack>
          <HStack gap={1} vAlign="center">
            {!renaming ? (
              <Button
                type="button"
                variant="secondary"
                size="sm"
                label="Rename"
                onClick={() => {
                  setError(null);
                  setLabelDraft(connection.connectionLabel ?? "");
                  setRenaming(true);
                }}
              />
            ) : null}
          </HStack>
        </HStack>
        {sourceActions ? <HStack gap={1} wrap="wrap" vAlign="center">{sourceActions}</HStack> : null}
      </SettingsSection>

      <SettingsSection aria-labelledby="connection-policy-title">
        <HStack hAlign="between" vAlign="center" gap={2} wrap="wrap">
          <h2 id="connection-policy-title" {...stylex.props(styles.heading)}>Connection policy</h2>
          <Button
            type="button"
            variant="secondary"
            size="sm"
            label="Edit policy"
            onClick={() => {
              setError(null);
              setSharing(sharing);
              setUnsafeActions(unsafeActions);
              setPolicyEditing(true);
            }}
          />
        </HStack>
        <SettingsList density="compact">
          <SettingsListItem label="Data sharing" description={sharingLabel(sharing)} />
          <SettingsListItem label="Risky actions" description={unsafeActionLabel(unsafeActions)} />
        </SettingsList>
      </SettingsSection>

      <SettingsSection aria-labelledby="connection-tools-title">
        <HStack hAlign="between" vAlign="center" gap={2}>
          <h2 id="connection-tools-title" {...stylex.props(styles.heading)}>Tools</h2>
          <span {...stylex.props(styles.muted)}>{connection.availableToolCount}/{connection.toolCount} available</span>
        </HStack>
        <CapabilityToolTable
          tools={tools}
          loading={result.loading}
          busy={resetState.loading || enabledState.loading}
          onEdit={beginEdit}
          onReset={(tool) => void reset(tool)}
          onToggle={(tool) => void toggle(tool)}
        />
      </SettingsSection>

      <VStack as="details" gap={2} {...stylex.props(styles.detailsSection)}>
        <summary {...stylex.props(styles.summary)}>Source details</summary>
        {definitionDetails}
        <strong {...stylex.props(styles.detailHeading)}>Connection metadata</strong>
        <pre {...stylex.props(styles.details)}>{JSON.stringify({
          status: connection.status,
          healthStatus: connection.healthStatus,
          authStatus: connection.authStatus,
          sourceDetails: connection.sourceDetails
        }, null, 2)}</pre>
      </VStack>
      {dangerAction ? (
        <SettingsSection aria-labelledby="connection-danger-title">
          <h2 id="connection-danger-title" {...stylex.props(styles.heading)}>Connection</h2>
          <p {...stylex.props(styles.muted)}>Remove this connection, its credentials, and its tool settings.</p>
          <HStack gap={1} wrap="wrap" vAlign="center">{dangerAction}</HStack>
        </SettingsSection>
      ) : null}
      {error && !renaming && !policyEditing && !editingTool ? (
        <p role="alert" {...stylex.props(styles.error)}>{error}</p>
      ) : null}
      <SettingsEditDialog
        title="Rename connection"
        open={renaming}
        saving={labelState.loading}
        saveLabel="Save label"
        error={renaming ? error : null}
        onOpenChange={(open) => {
          setRenaming(open);
          if (!open) {
            setLabelDraft("");
            setError(null);
          }
        }}
        onSave={submitLabel}
      >
        <TextInput
          hasAutoFocus
          label="Connection label"
          value={labelDraft}
          description="Leave blank to use the generated connection name."
          onChange={setLabelDraft}
        />
      </SettingsEditDialog>
      <SettingsEditDialog
        title="Edit connection policy"
        open={policyEditing}
        saving={policyState.loading}
        saveLabel="Save policy"
        saveDisabled={sharing === "review_every_call" && unsafeActions === "never_ask"}
        error={policyEditing ? error : null}
        width={560}
        onOpenChange={(open) => {
          setPolicyEditing(open);
          if (!open) {
            setSharing(null);
            setUnsafeActions(null);
            setError(null);
          }
        }}
        onSave={submitPolicy}
      >
        <CapabilityPolicyChoices
          hasAutoFocus
          serviceName={connection.name}
          dataSharingPolicy={sharing}
          unsafeActionPolicy={unsafeActions}
          onChange={(policy) => {
            setSharing(policy.dataSharingPolicy);
            setUnsafeActions(policy.unsafeActionPolicy);
          }}
        />
      </SettingsEditDialog>
      <SettingsEditDialog
        title={`Edit ${editingTool?.name ?? "tool"} behavior`}
        open={Boolean(editingTool && draft)}
        saving={overrideState.loading}
        saveLabel="Save behavior"
        error={editingTool ? error : null}
        width={560}
        onOpenChange={(open) => {
          if (!open) {
            setEditing(null);
            setDraft(null);
            setError(null);
          }
        }}
        onSave={() => editingTool ? submitOverride(editingTool) : undefined}
      >
        {editingTool && draft ? (
          <ToolBehaviorEditor tool={editingTool} draft={draft} onChange={setDraft} />
        ) : null}
      </SettingsEditDialog>
    </VStack>
  );
}

function connectionIssue(connection: NonNullable<CapabilityConnectionQuery["capabilityConnection"]>) {
  if (["authentication_required", "needs_auth", "required"].includes(connection.authStatus.toLowerCase())) return null;
  if (!["active", "healthy", "ready"].includes(connection.healthStatus.toLowerCase())) {
    return settingsStatusLabel(connection.healthStatus);
  }
  if (!["active", "ready", "available", "enabled"].includes(connection.status.toLowerCase())) {
    return settingsStatusLabel(connection.status);
  }
  return null;
}

function ToolBehaviorEditor({
  tool,
  draft,
  onChange
}: {
  tool: ManagedTool;
  draft: HintDraft;
  onChange: (draft: HintDraft) => void;
}) {
  return (
    <VStack gap={2}>
      {tool.description ? <p {...stylex.props(styles.editorDescription)}>{tool.description}</p> : null}
      {(["readOnly", "idempotent", "destructive", "openWorld"] as const).map((field) => (
        <HStack key={field} wrap="wrap" vAlign="center" hAlign="between" gap={2} {...stylex.props(styles.hintRow)}>
          <VStack gap={0.5}>
            <strong {...stylex.props(styles.hintQuestion)}>{hintLabel(field)}</strong>
            <span {...stylex.props(styles.source)}>
              {toolHintSourceDescription(tool[field].source)}
            </span>
          </VStack>
          <HStack
            role="group"
            aria-label={hintLabel(field)}
            gap={1}
          >
            <BooleanChoice
              hasAutoFocus={field === "readOnly"}
              selected={draft[field]}
              label="Yes"
              onClick={() => onChange({ ...draft, [field]: true })}
            />
            <BooleanChoice
              selected={!draft[field]}
              label="No"
              onClick={() => onChange({ ...draft, [field]: false })}
            />
          </HStack>
        </HStack>
      ))}
    </VStack>
  );
}

function BooleanChoice({
  hasAutoFocus = false,
  selected,
  label,
  onClick
}: {
  hasAutoFocus?: boolean;
  selected: boolean;
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      data-autofocus={hasAutoFocus || undefined}
      aria-pressed={selected}
      {...stylex.props(styles.booleanChoice, selected && styles.booleanSelected)}
      onClick={onClick}
    >
      {selected ? (
        <span aria-hidden="true" {...stylex.props(styles.booleanDot, styles.selectionMarker)} />
      ) : null}
      {label}
    </button>
  );
}

function hintLabel(value: HintKey) {
  return ({
    readOnly: "Only reads?",
    idempotent: "Same result if repeated?",
    destructive: "Can delete?",
    openWorld: "Can it act outside Noema?"
  } as const)[value];
}

function sharingLabel(value: CapabilityDataSharingPolicy) {
  return value === "review_every_call" ? "Review sharing every time" : "Share when needed";
}

function unsafeActionLabel(value: CapabilityUnsafeActionPolicy) {
  if (value === "always_ask") return "You approve risky calls";
  if (value === "never_ask") return "Risky calls run automatically";
  return "Noema reviews risky calls first";
}

const styles = stylex.create({
  heading: { margin: "var(--spacing-0)", fontFamily: "var(--font-heading)", fontSize: 16, fontWeight: 600 },
  editorDescription: { margin: "var(--spacing-0)", fontSize: 13, lineHeight: 1.5, color: "var(--muted-foreground)" },
  hintRow: { paddingBlock: "var(--spacing-2)", borderBottomWidth: 1, borderBottomStyle: "solid", borderBottomColor: "var(--border-subtle)", fontSize: 14, "@media (max-width: 520px)": { alignItems: "flex-start" } },
  hintQuestion: { fontSize: 14, lineHeight: 1.4 },
  source: { display: "block", marginTop: "var(--spacing-0-5)", fontSize: 12, fontWeight: 400, color: "var(--muted-foreground)" },
  booleanChoice: { display: "inline-flex", minWidth: 56, alignItems: "center", justifyContent: "center", gap: "var(--spacing-1)", padding: "var(--spacing-1-5) var(--spacing-2)", borderWidth: 1, borderStyle: "solid", borderColor: "var(--border-subtle)", borderRadius: 6, backgroundColor: "var(--surface-raised)", color: "var(--foreground)", cursor: "pointer", transitionProperty: "border-color, box-shadow, color", transitionDuration: "var(--motion-spring-micro-duration)", transitionTimingFunction: "var(--motion-spring-critical-easing)" },
  booleanSelected: { borderColor: "var(--primary)", boxShadow: "inset 0 0 0 1px var(--primary)", color: "var(--primary)", fontWeight: 600 },
  booleanDot: { width: 7, height: 7, flexShrink: 0, borderRadius: 999, backgroundColor: "var(--primary)" },
  selectionMarker: { animationName: stylex.keyframes({ from: { opacity: 0, transform: "scale(0.6)" }, to: { opacity: 1, transform: "scale(1)" } }), animationDuration: "var(--motion-spring-micro-duration)", animationTimingFunction: "var(--motion-spring-critical-easing)" },
  fit: { width: "fit-content" },
  muted: { margin: "var(--spacing-0)", color: "var(--muted-foreground)", fontSize: 12 },
  summary: { cursor: "pointer", fontSize: 13, fontWeight: 600 },
  detailsSection: { minWidth: 0 },
  detailHeading: { fontSize: 12 },
  details: { margin: "var(--spacing-0)", overflowX: "auto", fontFamily: "var(--font-mono)", fontSize: 12 },
  error: { margin: "var(--spacing-0)", color: "var(--destructive)", fontSize: 13 }
});

import * as React from "react";
import { useMutation, useQuery } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import { Badge } from "@astryxdesign/core/Badge";
import * as stylex from "@stylexjs/stylex";
import {
  CapabilityConnectionDocument,
  ResetCapabilityToolPolicyDocument,
  SaveCapabilityConnectionPolicyDocument,
  SaveCapabilityToolOverrideDocument,
  SetCapabilityToolEnabledDocument,
  StartAdapterOauthSetupDocument,
  type CapabilityConnectionQuery,
  type ResetCapabilityToolPolicyMutation,
  type SaveCapabilityConnectionPolicyMutation,
  type SaveCapabilityToolOverrideMutation,
  type SetCapabilityToolEnabledMutation,
  type StartAdapterOauthSetupMutation
} from "@/generated/graphql";
import { openExternalUrlForAuth } from "@/graphql/externalUrls";
import {
  CapabilityPolicyChoices,
  type CapabilityDataSharingPolicy,
  type CapabilityUnsafeActionPolicy
} from "@/components/capabilities/CapabilityPolicyChoices";
import { CapabilityToolTable, toolHintSourceDescription } from "./CapabilityToolTable";

type ManagedTool = CapabilityConnectionQuery["capabilityTools"][number];
type HintKey = "readOnly" | "idempotent" | "destructive" | "openWorld";
type HintDraft = Record<HintKey, boolean>;

export function CapabilityConnectionDetail({
  kind,
  connectionId,
  sourceActions,
  dangerAction
}: {
  kind: "API" | "MCP";
  connectionId: string;
  sourceActions?: React.ReactNode;
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
  const [saveOverride, overrideState] = useMutation<SaveCapabilityToolOverrideMutation>(
    SaveCapabilityToolOverrideDocument
  );
  const [resetTool, resetState] = useMutation<ResetCapabilityToolPolicyMutation>(
    ResetCapabilityToolPolicyDocument
  );
  const [setEnabled, enabledState] = useMutation<SetCapabilityToolEnabledMutation>(
    SetCapabilityToolEnabledDocument
  );
  const [startAdapterOauth, oauthState] = useMutation<StartAdapterOauthSetupMutation>(
    StartAdapterOauthSetupDocument
  );
  const connection = result.data?.capabilityConnection ?? null;
  const tools = result.data?.capabilityTools ?? [];
  const [sharingDraft, setSharing] = React.useState<string | null>(null);
  const [unsafeActionsDraft, setUnsafeActions] = React.useState<string | null>(null);
  const [editing, setEditing] = React.useState<string | null>(null);
  const [draft, setDraft] = React.useState<HintDraft | null>(null);
  const [error, setError] = React.useState<string | null>(null);
  const editingTool = tools.find((tool) => tool.toolId === editing) ?? null;

  if (result.loading && !result.data) return <p {...stylex.props(styles.muted)}>Loading connection…</p>;
  if (result.error) return <p role="alert" {...stylex.props(styles.error)}>Couldn't load this connection.</p>;
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
      await refresh();
    } catch {
      setError("This connection changed. Reload it and try again.");
    }
  }

  async function authorizeApi() {
    if (!connection || connection.credentialRevision === null || connection.grantRevision === null) return;
    setError(null);
    try {
      const response = await startAdapterOauth({ variables: { input: {
        connectionId,
        expectedConnectionRevision: Number(connection.connectionRevision),
        expectedCredentialRevision: connection.credentialRevision,
        expectedGrantRevision: connection.grantRevision,
        expectedPolicyRevision: connection.policyRevision
      } } });
      const url = response.data?.startAdapterOauthSetup.authorizationUrl;
      if (!url) throw new Error("missing authorization URL");
      const handled = await openExternalUrlForAuth(url);
      if (!handled) window.open(url, "_blank", "noopener,noreferrer");
      window.addEventListener("focus", () => void refresh(), { once: true });
    } catch {
      setError("Authorization could not be started.");
    }
  }

  function beginEdit(tool: ManagedTool) {
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
    await resetTool({ variables: { input: {
      ...fence,
      toolId: tool.toolId,
      sourceRevision: tool.sourceRevision,
      expectedPolicyRevision: tool.policyRevision
    } } });
    await refresh();
  }

  async function toggle(tool: ManagedTool) {
    await setEnabled({ variables: { input: {
      ...fence,
      toolId: tool.toolId,
      sourceRevision: tool.sourceRevision,
      expectedPolicyRevision: tool.policyRevision,
      enabled: !tool.enabled
    } } });
    await refresh();
  }

  return (
    <div {...stylex.props(styles.stack)}>
      <section {...stylex.props(styles.section)}>
        <div {...stylex.props(styles.headingRow)}>
          <div>
            <h2 {...stylex.props(styles.heading)}>{connection.name}</h2>
            <p {...stylex.props(styles.muted)}>{connection.healthStatus} · {connection.authStatus}</p>
          </div>
          <Badge variant="neutral" label={connection.status.replaceAll("_", " ")} />
        </div>
        {kind === "API" && connection.authStatus === "required" ? (
          <Button
            type="button"
            label="Authorize account"
            isLoading={oauthState.loading}
            {...stylex.props(styles.fit)}
            onClick={() => void authorizeApi()}
          />
        ) : null}
        {sourceActions ? <div {...stylex.props(styles.actions)}>{sourceActions}</div> : null}
      </section>

      <section {...stylex.props(styles.section)}>
        <CapabilityPolicyChoices
          serviceName={connection.name}
          dataSharingPolicy={sharing}
          unsafeActionPolicy={unsafeActions}
          onChange={(policy) => {
            setSharing(policy.dataSharingPolicy);
            setUnsafeActions(policy.unsafeActionPolicy);
          }}
        />
        <div {...stylex.props(styles.policyActions)}>
          <Button
            type="button"
            label="Save policy"
            isLoading={policyState.loading}
            isDisabled={sharing === "review_every_call" && unsafeActions === "never_ask"}
            {...stylex.props(styles.fit)}
            onClick={() => void submitPolicy()}
          />
        </div>
      </section>

      <section {...stylex.props(styles.section)}>
        <div {...stylex.props(styles.headingRow)}>
          <h2 {...stylex.props(styles.heading)}>Tools</h2>
          <span {...stylex.props(styles.muted)}>{connection.availableToolCount}/{connection.toolCount} available</span>
        </div>
        {editingTool && draft ? (
          <div {...stylex.props(styles.editor)}>
            <ToolBehaviorEditor tool={editingTool} draft={draft} onChange={setDraft} />
            <div {...stylex.props(styles.actions)}>
              <Button
                type="button"
                label="Save"
                isLoading={overrideState.loading}
                onClick={() => void submitOverride(editingTool)}
              />
              <Button
                type="button"
                variant="secondary"
                label="Cancel"
                onClick={() => {
                  setEditing(null);
                  setDraft(null);
                }}
              />
            </div>
          </div>
        ) : (
          <CapabilityToolTable
            tools={tools}
            loading={result.loading}
            busy={resetState.loading || enabledState.loading}
            onEdit={beginEdit}
            onReset={(tool) => void reset(tool)}
            onToggle={(tool) => void toggle(tool)}
          />
        )}
      </section>

      <details {...stylex.props(styles.section)}>
        <summary {...stylex.props(styles.summary)}>Source details</summary>
        <pre {...stylex.props(styles.details)}>{JSON.stringify(connection.sourceDetails, null, 2)}</pre>
      </details>
      {dangerAction ? (
        <section {...stylex.props(styles.section)}>
          <h2 {...stylex.props(styles.heading)}>Connection</h2>
          <p {...stylex.props(styles.muted)}>Remove this account, its credentials, and its tool settings.</p>
          <div {...stylex.props(styles.actions)}>{dangerAction}</div>
        </section>
      ) : null}
      {error ? <p role="alert" {...stylex.props(styles.error)}>{error}</p> : null}
    </div>
  );
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
    <div {...stylex.props(styles.behaviorEditor)}>
      {tool.description ? <p {...stylex.props(styles.editorDescription)}>{tool.description}</p> : null}
      {(["readOnly", "idempotent", "destructive", "openWorld"] as const).map((field) => (
        <div key={field} {...stylex.props(styles.hintRow)}>
          <div>
            <strong {...stylex.props(styles.hintQuestion)}>{hintLabel(field)}</strong>
            <span {...stylex.props(styles.source)}>
              {toolHintSourceDescription(tool[field].source)}
            </span>
          </div>
          <div
            role="group"
            aria-label={hintLabel(field)}
            {...stylex.props(styles.booleanChoices)}
          >
            <BooleanChoice
              selected={draft[field]}
              label="Yes"
              onClick={() => onChange({ ...draft, [field]: true })}
            />
            <BooleanChoice
              selected={!draft[field]}
              label="No"
              onClick={() => onChange({ ...draft, [field]: false })}
            />
          </div>
        </div>
      ))}
    </div>
  );
}

function BooleanChoice({
  selected,
  label,
  onClick
}: {
  selected: boolean;
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
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

const styles = stylex.create({
  stack: { display: "grid", gap: "var(--spacing-3)" },
  section: { display: "grid", gap: "var(--spacing-2)", padding: "var(--spacing-3)", borderWidth: 1, borderStyle: "solid", borderColor: "var(--border-subtle)", borderRadius: 6, backgroundColor: "var(--surface-raised)" },
  headingRow: { display: "flex", alignItems: "center", justifyContent: "space-between", gap: "var(--spacing-2)" },
  heading: { margin: 0, fontFamily: "var(--font-heading)", fontSize: 16, fontWeight: 600 },
  policyActions: { display: "flex", justifyContent: "flex-end", paddingTop: "var(--spacing-2)" },
  editor: { display: "grid", gap: "var(--spacing-3)" },
  behaviorEditor: { display: "grid", gap: "var(--spacing-2)" },
  editorDescription: { margin: 0, fontSize: 13, lineHeight: 1.5, color: "var(--muted-foreground)" },
  hintRow: { display: "flex", alignItems: "center", justifyContent: "space-between", gap: "var(--spacing-4)", padding: "var(--spacing-2) 0", borderBottomWidth: 1, borderBottomStyle: "solid", borderBottomColor: "var(--border-subtle)", fontSize: 14, "@media (max-width: 520px)": { alignItems: "flex-start", flexDirection: "column", gap: "var(--spacing-2)" } },
  hintQuestion: { fontSize: 14, lineHeight: 1.4 },
  source: { display: "block", marginTop: "var(--spacing-0-5)", fontSize: 12, fontWeight: 400, color: "var(--muted-foreground)" },
  booleanChoices: { display: "flex", gap: "var(--spacing-1)" },
  booleanChoice: { display: "inline-flex", minWidth: 56, alignItems: "center", justifyContent: "center", gap: "var(--spacing-1)", padding: "var(--spacing-1-5) var(--spacing-2)", borderWidth: 1, borderStyle: "solid", borderColor: "var(--border-subtle)", borderRadius: 6, backgroundColor: "var(--surface-raised)", color: "var(--foreground)", cursor: "pointer", transitionProperty: "border-color, box-shadow, color", transitionDuration: "var(--motion-spring-micro-duration)", transitionTimingFunction: "var(--motion-spring-critical-easing)" },
  booleanSelected: { borderColor: "var(--primary)", boxShadow: "inset 0 0 0 1px var(--primary)", color: "var(--primary)", fontWeight: 600 },
  booleanDot: { width: 7, height: 7, flexShrink: 0, borderRadius: 999, backgroundColor: "var(--primary)" },
  selectionMarker: { animationName: stylex.keyframes({ from: { opacity: 0, transform: "scale(0.6)" }, to: { opacity: 1, transform: "scale(1)" } }), animationDuration: "var(--motion-spring-micro-duration)", animationTimingFunction: "var(--motion-spring-critical-easing)" },
  actions: { display: "flex", alignItems: "center", gap: "var(--spacing-1)", flexWrap: "wrap", gridColumn: "1 / -1" },
  fit: { width: "fit-content" },
  muted: { margin: 0, color: "var(--muted-foreground)", fontSize: 12 },
  summary: { cursor: "pointer", fontSize: 13, fontWeight: 600 },
  details: { margin: 0, overflowX: "auto", fontFamily: "var(--font-mono)", fontSize: 12 },
  error: { margin: 0, color: "var(--destructive)", fontSize: 13 }
});

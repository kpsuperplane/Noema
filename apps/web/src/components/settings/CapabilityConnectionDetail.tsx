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

type ManagedTool = CapabilityConnectionQuery["capabilityTools"][number];
type HintDraft = Pick<ManagedTool, "readOnly" | "idempotent" | "destructive" | "openWorld">;

export function CapabilityConnectionDetail({
  kind,
  connectionId
}: {
  kind: "API" | "MCP";
  connectionId: string;
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
  const [draft, setDraft] = React.useState<Record<keyof HintDraft, boolean> | null>(null);
  const [error, setError] = React.useState<string | null>(null);

  if (result.loading && !result.data) return <p {...stylex.props(styles.muted)}>Loading connection…</p>;
  if (result.error) return <p role="alert" {...stylex.props(styles.error)}>Couldn't load this connection.</p>;
  if (!connection) return <p {...stylex.props(styles.muted)}>This connection no longer exists.</p>;
  const sharing = sharingDraft ?? connection.dataSharingPolicy ?? "allow_automatically";
  const unsafeActions = unsafeActionsDraft ?? connection.unsafeActionPolicy ?? "reviewer_may_approve";

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
      </section>

      <section {...stylex.props(styles.section)}>
        <h2 {...stylex.props(styles.heading)}>Sharing and unsafe calls</h2>
        <div {...stylex.props(styles.policyGrid)}>
          <PolicyChoice
            label="Data sharing"
            value={sharing}
            options={[
              ["allow_automatically", "Share when needed"],
              ["review_every_call", "Review every call"]
            ]}
            onChange={(value) => {
              setSharing(value);
              if (value === "review_every_call" && unsafeActions === "never_ask") {
                setUnsafeActions("reviewer_may_approve");
              }
            }}
          />
          <PolicyChoice
            label="Unsafe calls"
            value={unsafeActions}
            options={[
              ["always_ask", "Always ask me"],
              ["reviewer_may_approve", "LLM review"],
              ["never_ask", "Run immediately"]
            ]}
            onChange={setUnsafeActions}
          />
        </div>
        <Button
          type="button"
          label="Save policy"
          isLoading={policyState.loading}
          isDisabled={sharing === "review_every_call" && unsafeActions === "never_ask"}
          {...stylex.props(styles.fit)}
          onClick={() => void submitPolicy()}
        />
      </section>

      <section {...stylex.props(styles.section)}>
        <div {...stylex.props(styles.headingRow)}>
          <h2 {...stylex.props(styles.heading)}>Tools</h2>
          <span {...stylex.props(styles.muted)}>{connection.availableToolCount}/{connection.toolCount} available</span>
        </div>
        <div {...stylex.props(styles.tools)}>
          {tools.map((tool) => {
            const isEditing = editing === tool.toolId && draft;
            return (
              <article key={tool.toolId} {...stylex.props(styles.tool)}>
                <div {...stylex.props(styles.headingRow)}>
                  <div>
                    <h3 {...stylex.props(styles.toolName)}>{tool.name}</h3>
                    <p {...stylex.props(styles.muted)}>{tool.decisionPreview ?? "Policy required"}</p>
                  </div>
                  <Badge variant="neutral" label={tool.status} />
                </div>
                {isEditing ? (
                  <div {...stylex.props(styles.hints)}>
                    {(["readOnly", "idempotent", "destructive", "openWorld"] as const).map((hint) => (
                      <label key={hint} {...stylex.props(styles.check)}>
                        <input
                          type="checkbox"
                          checked={draft[hint]}
                          onChange={(event) => setDraft({ ...draft, [hint]: event.target.checked })}
                        />
                        {hintLabel(hint)}
                      </label>
                    ))}
                    <div {...stylex.props(styles.actions)}>
                      <Button type="button" label="Save" isLoading={overrideState.loading} onClick={() => void submitOverride(tool)} />
                      <Button type="button" variant="secondary" label="Cancel" onClick={() => { setEditing(null); setDraft(null); }} />
                    </div>
                  </div>
                ) : (
                  <>
                    <p {...stylex.props(styles.hintSummary)}>{hintSummary(tool)}</p>
                    <div {...stylex.props(styles.actions)}>
                      <Button type="button" variant="secondary" label="Edit behavior" onClick={() => beginEdit(tool)} />
                      <Button type="button" variant="ghost" label="Reset" isDisabled={resetState.loading} onClick={() => void reset(tool)} />
                      <Button type="button" variant="ghost" label={tool.enabled ? "Disable" : "Enable"} isDisabled={enabledState.loading} onClick={() => void toggle(tool)} />
                    </div>
                  </>
                )}
              </article>
            );
          })}
        </div>
      </section>

      <details {...stylex.props(styles.section)}>
        <summary {...stylex.props(styles.summary)}>Source details</summary>
        <pre {...stylex.props(styles.details)}>{JSON.stringify(connection.sourceDetails, null, 2)}</pre>
      </details>
      {error ? <p role="alert" {...stylex.props(styles.error)}>{error}</p> : null}
    </div>
  );
}

function PolicyChoice({ label, value, options, onChange }: {
  label: string;
  value: string;
  options: readonly (readonly [string, string])[];
  onChange: (value: string) => void;
}) {
  return (
    <fieldset {...stylex.props(styles.fieldset)}>
      <legend {...stylex.props(styles.legend)}>{label}</legend>
      {options.map(([option, copy]) => (
        <label key={option} {...stylex.props(styles.check)}>
          <input type="radio" checked={value === option} onChange={() => onChange(option)} />
          {copy}
        </label>
      ))}
    </fieldset>
  );
}

function hintLabel(value: keyof HintDraft) {
  return ({ readOnly: "Read only", idempotent: "Idempotent", destructive: "Destructive", openWorld: "Open world" } as const)[value];
}

function hintSummary(tool: ManagedTool) {
  return (["readOnly", "idempotent", "destructive", "openWorld"] as const)
    .map((hint) => `${hintLabel(hint)}: ${tool[hint].value ? "yes" : "no"} (${tool[hint].source ?? "pending"})`)
    .join(" · ");
}

const styles = stylex.create({
  stack: { display: "grid", gap: "var(--spacing-3)" },
  section: { display: "grid", gap: "var(--spacing-2)", padding: "var(--spacing-3)", borderWidth: 1, borderStyle: "solid", borderColor: "var(--border-subtle)", borderRadius: 6, backgroundColor: "var(--surface-raised)" },
  headingRow: { display: "flex", alignItems: "center", justifyContent: "space-between", gap: "var(--spacing-2)" },
  heading: { margin: 0, fontFamily: "var(--font-heading)", fontSize: 16, fontWeight: 600 },
  policyGrid: { display: "grid", gridTemplateColumns: "repeat(2, minmax(0, 1fr))", gap: "var(--spacing-3)", "@media (max-width: 640px)": { gridTemplateColumns: "1fr" } },
  fieldset: { display: "grid", gap: "var(--spacing-1)", margin: 0, padding: 0, borderWidth: 0 },
  legend: { marginBottom: "var(--spacing-1)", fontSize: 13, fontWeight: 600 },
  check: { display: "flex", alignItems: "center", gap: "var(--spacing-1)", fontSize: 13 },
  tools: { display: "grid" },
  tool: { display: "grid", gap: "var(--spacing-2)", paddingBlock: "var(--spacing-2)", borderBottomWidth: 1, borderBottomStyle: "solid", borderBottomColor: "var(--border-subtle)", ":last-child": { borderBottomWidth: 0 } },
  toolName: { margin: 0, fontSize: 14, fontWeight: 600 },
  hints: { display: "grid", gridTemplateColumns: "repeat(2, minmax(0, 1fr))", gap: "var(--spacing-1)" },
  hintSummary: { margin: 0, color: "var(--foreground)", fontSize: 12, lineHeight: 1.5 },
  actions: { display: "flex", alignItems: "center", gap: "var(--spacing-1)", flexWrap: "wrap", gridColumn: "1 / -1" },
  fit: { width: "fit-content" },
  muted: { margin: 0, color: "var(--muted-foreground)", fontSize: 12 },
  summary: { cursor: "pointer", fontSize: 13, fontWeight: 600 },
  details: { margin: 0, overflowX: "auto", fontFamily: "var(--font-mono)", fontSize: 12 },
  error: { margin: 0, color: "var(--destructive)", fontSize: 13 }
});

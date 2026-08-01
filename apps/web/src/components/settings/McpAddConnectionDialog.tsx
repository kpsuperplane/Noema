import * as React from "react";
import { useMutation } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import { Dialog, DialogHeader } from "@/components/ResponsiveDialog";
import { Layout, LayoutContent, LayoutFooter } from "@astryxdesign/core/Layout";
import * as stylex from "@stylexjs/stylex";
import {
  AddMcpConnectionDocument,
  type AddMcpConnectionMutation
} from "@/generated/graphql";
import type { CapabilityIntegration } from "./CapabilityIntegrationList";
import { parseKeyValueLines } from "./mcpSetupForm";

export function McpAddConnectionDialog({ integration, open, onOpenChange, onAdded }: {
  integration: CapabilityIntegration | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onAdded: () => void;
}) {
  const [label, setLabel] = React.useState("");
  const [secretEnv, setSecretEnv] = React.useState("");
  const [secretHeaders, setSecretHeaders] = React.useState("");
  const [clientId, setClientId] = React.useState("");
  const [clientSecret, setClientSecret] = React.useState("");
  const [scopes, setScopes] = React.useState("");
  const [error, setError] = React.useState<string | null>(null);
  const [addConnection, state] = useMutation<AddMcpConnectionMutation>(AddMcpConnectionDocument);

  async function submit() {
    if (!integration) return;
    const env = parseKeyValueLines(secretEnv);
    const headers = parseKeyValueLines(secretHeaders);
    if (env.error || headers.error) {
      setError(env.error ?? headers.error);
      return;
    }
    setError(null);
    try {
      const response = await addConnection({ variables: { input: {
        mcpDefinitionId: integration.definitionId,
        expectedDefinitionRevision: integration.sourceRevision,
        connectionLabel: label.trim() || null,
        secretEnv: env.value,
        secretHeaders: headers.value,
        oauthClientCredentials: clientId.trim() ? {
          clientId: clientId.trim(),
          clientSecret,
          scopes: scopes.split(/[,\s]+/).filter(Boolean)
        } : null,
        authPreference: "USE_ANONYMOUS"
      } } });
      const result = response.data?.addMcpConnection;
      if (!result) throw new Error("missing setup result");
      if (result.setupStatus !== "ready_for_policy" || !result.server) {
        setError(result.setupError ?? "This connection needs different credentials.");
        return;
      }
      onAdded();
      onOpenChange(false);
    } catch {
      setError("The MCP connection could not be added.");
    }
  }

  return (
    <Dialog isOpen={open} onOpenChange={onOpenChange} purpose="form" width={560} aria-label="Add MCP connection">
      <Layout
        header={<DialogHeader title={`Add connection to ${integration?.name ?? "MCP"}`} onOpenChange={onOpenChange} hasDivider />}
        content={
          <LayoutContent>
            <div {...stylex.props(styles.form)}>
              <Field label="Account or installation label" value={label} onChange={setLabel} />
              <label {...stylex.props(styles.field)}>
                <span>Private environment variables</span>
                <textarea rows={3} placeholder="KEY=value" value={secretEnv} onChange={(event) => setSecretEnv(event.currentTarget.value)} {...stylex.props(styles.input)} />
              </label>
              <label {...stylex.props(styles.field)}>
                <span>Private request headers</span>
                <textarea rows={3} placeholder="Header=value" value={secretHeaders} onChange={(event) => setSecretHeaders(event.currentTarget.value)} {...stylex.props(styles.input)} />
              </label>
              <div {...stylex.props(styles.columns)}>
                <Field label="OAuth client ID" value={clientId} onChange={setClientId} />
                <Field label="OAuth client secret" value={clientSecret} type="password" onChange={setClientSecret} />
              </div>
              <Field label="OAuth scopes" value={scopes} onChange={setScopes} />
              {error ? <p role="alert" {...stylex.props(styles.error)}>{error}</p> : null}
            </div>
          </LayoutContent>
        }
        footer={
          <LayoutFooter hasDivider padding={3}>
            <div {...stylex.props(styles.actions)}>
              <Button type="button" variant="secondary" label="Cancel" onClick={() => onOpenChange(false)} />
              <Button type="button" label="Add connection" isLoading={state.loading} onClick={() => void submit()} />
            </div>
          </LayoutFooter>
        }
      />
    </Dialog>
  );
}

function Field({ label, value, type = "text", onChange }: {
  label: string;
  value: string;
  type?: "text" | "password";
  onChange: (value: string) => void;
}) {
  return (
    <label {...stylex.props(styles.field)}>
      <span>{label}</span>
      <input type={type} value={value} onChange={(event) => onChange(event.currentTarget.value)} {...stylex.props(styles.input)} />
    </label>
  );
}

const styles = stylex.create({
  form: { display: "grid", gap: "var(--spacing-2)" },
  columns: { display: "grid", gridTemplateColumns: "repeat(2, minmax(0, 1fr))", gap: "var(--spacing-2)", "@media (max-width: 560px)": { gridTemplateColumns: "1fr" } },
  field: { display: "grid", gap: "var(--spacing-1)", fontSize: 13, fontWeight: 600 },
  input: { width: "100%", boxSizing: "border-box", padding: "var(--spacing-2)", borderWidth: 1, borderStyle: "solid", borderColor: "var(--border-subtle)", borderRadius: 4, backgroundColor: "var(--surface-base)", color: "var(--foreground)", font: "inherit" },
  actions: { display: "flex", justifyContent: "flex-end", gap: "var(--spacing-1)" },
  error: { margin: "var(--spacing-0)", color: "var(--destructive)", fontSize: 13 }
});

import * as React from "react";
import { ExternalLink, KeyRound, RefreshCw } from "lucide-react";
import { Button } from "@astryxdesign/core/Button";
import { Dialog, DialogHeader } from "@astryxdesign/core/Dialog";
import * as stylex from "@stylexjs/stylex";
import type { McpServerSetupResult } from "./McpServerSetupFlow";
import type { McpSettingsServer } from "./mcpMetadata";
import {
  parseKeyValueLines,
  type McpOAuthClientCredentials,
  type McpSetupContinueSubmission
} from "./mcpSetupForm";

export function McpServerReauthenticationDialog({
  server,
  open,
  submitting,
  oauthSubmitting,
  error,
  setupResult,
  onOpenChange,
  onSubmit,
  onStartBrowserOAuth
}: {
  server: McpSettingsServer | null;
  open: boolean;
  submitting: boolean;
  oauthSubmitting: boolean;
  error: string | null;
  setupResult: McpServerSetupResult | null;
  onOpenChange: (open: boolean) => void;
  onSubmit: (input: McpSetupContinueSubmission) => void;
  onStartBrowserOAuth: (mcpServerId: string) => void;
}) {
  const [secretEnv, setSecretEnv] = React.useState("");
  const [secretHeaders, setSecretHeaders] = React.useState("");
  const [oauthClientId, setOauthClientId] = React.useState("");
  const [oauthClientSecret, setOauthClientSecret] = React.useState("");
  const [oauthScopes, setOauthScopes] = React.useState("");
  const [formError, setFormError] = React.useState<string | null>(null);

  const isHttp = server?.transportKind === "streamable_http";
  const usesBrowserOAuth = Boolean(server?.browserOauthReauthenticationSupported);
  const visibleError = formError ?? error ?? setupResult?.setupError ?? null;

  function submit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!server) return;
    if (usesBrowserOAuth) {
      setFormError(null);
      onStartBrowserOAuth(server.mcpServerId);
      return;
    }

    const parsedSecretEnv = parseKeyValueLines(secretEnv);
    const parsedSecretHeaders = parseKeyValueLines(secretHeaders);
    const parsedOAuth = oauthClientCredentialsFromDraft(
      oauthClientId,
      oauthClientSecret,
      oauthScopes
    );
    const parseError =
      parsedSecretEnv.error ?? parsedSecretHeaders.error ?? parsedOAuth.error;
    if (parseError) {
      setFormError(parseError);
      return;
    }

    setFormError(null);
    onSubmit({
      mcpServerId: server.mcpServerId,
      secretEnv: parsedSecretEnv.value ?? {},
      secretHeaders: parsedSecretHeaders.value ?? {},
      oauthClientCredentials: isHttp ? parsedOAuth.value : null
    });
  }

  return (
    <Dialog
      isOpen={open}
      onOpenChange={onOpenChange}
      purpose="form"
      width={620}
      aria-label="Reauthenticate MCP server"
    >
      <div {...stylex.props(styles.dialog)}>
        <DialogHeader
          title="Reauthenticate MCP server"
          subtitle={server?.displayName ?? "MCP server"}
          onOpenChange={onOpenChange}
        />
        <form {...stylex.props(styles.dialogBody)} onSubmit={submit}>
          <div {...stylex.props(styles.inlineHeader)}>
            <KeyRound {...stylex.props(styles.icon)} aria-hidden="true" />
            Connection recovery
          </div>
          {server?.transportKind === "stdio" ? (
            <TextAreaField
              label="Secret env"
              value={secretEnv}
              onChange={setSecretEnv}
            />
          ) : null}
          {isHttp && !usesBrowserOAuth ? (
            <>
              <TextAreaField
                label="Secret headers"
                value={secretHeaders}
                onChange={setSecretHeaders}
              />
              <TextField
                label="OAuth client ID"
                value={oauthClientId}
                onChange={setOauthClientId}
              />
              <TextField
                label="OAuth client secret"
                type="password"
                value={oauthClientSecret}
                onChange={setOauthClientSecret}
              />
              <TextField
                label="OAuth scopes"
                value={oauthScopes}
                onChange={setOauthScopes}
              />
            </>
          ) : null}
          {visibleError ? <p {...stylex.props(styles.errorText)}>{visibleError}</p> : null}
          <div {...stylex.props(styles.actions)}>
            {usesBrowserOAuth ? (
              <Button
                type="submit"
                label="Continue with OAuth"
                icon={
                  !oauthSubmitting ? (
                    <ExternalLink {...stylex.props(styles.icon)} aria-hidden="true" />
                  ) : undefined
                }
                isDisabled={oauthSubmitting || submitting || !server}
                isLoading={oauthSubmitting}
              />
            ) : (
              <Button
                type="submit"
                label="Retry connection"
                icon={!submitting ? <RefreshCw {...stylex.props(styles.icon)} aria-hidden="true" /> : undefined}
                isDisabled={submitting || !server}
                isLoading={submitting}
              />
            )}
            <Button
              type="button"
              variant="secondary"
              label="Cancel"
              isDisabled={submitting || oauthSubmitting}
              onClick={() => onOpenChange(false)}
            />
          </div>
        </form>
      </div>
    </Dialog>
  );
}

function TextField({
  label,
  type = "text",
  value,
  onChange
}: {
  label: string;
  type?: "password" | "text";
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <label {...stylex.props(styles.field)}>
      <span>{label}</span>
      <input
        type={type}
        {...stylex.props(styles.input)}
        value={value}
        onChange={(event) => onChange(event.currentTarget.value)}
      />
    </label>
  );
}

function TextAreaField({
  label,
  value,
  onChange
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <label {...stylex.props(styles.field)}>
      <span>{label}</span>
      <textarea
        {...stylex.props(styles.textarea)}
        value={value}
        onChange={(event) => onChange(event.currentTarget.value)}
      />
    </label>
  );
}

type OAuthClientCredentialsResult =
  | { value: McpOAuthClientCredentials | null; error: null }
  | { value: null; error: string };

function oauthClientCredentialsFromDraft(
  clientId: string,
  clientSecret: string,
  scopes: string
): OAuthClientCredentialsResult {
  const trimmedClientId = clientId.trim();
  const trimmedClientSecret = clientSecret.trim();
  if (!trimmedClientId && !trimmedClientSecret && !scopes.trim()) {
    return { value: null, error: null };
  }
  if (!trimmedClientId || !trimmedClientSecret) {
    return { value: null, error: "OAuth client ID and secret are both required." };
  }
  return {
    value: {
      clientId: trimmedClientId,
      clientSecret: trimmedClientSecret,
      scopes: scopes
        .split(/[\s,]+/u)
        .map((scope) => scope.trim())
        .filter(Boolean)
    },
    error: null
  };
}

const styles = stylex.create({
  dialog: {
    display: "grid",
    minHeight: 0
  },
  dialogBody: {
    display: "grid",
    gap: 12,
    minHeight: 0,
    overflow: "auto",
    padding: 16
  },
  field: {
    display: "grid",
    gap: 4,
    fontSize: 14,
    fontWeight: 500,
    lineHeight: 1.5,
    color: "var(--foreground)"
  },
  input: {
    height: 36,
    minWidth: 0,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border-subtle)",
    borderRadius: 6,
    backgroundColor: "white",
    paddingInline: 12,
    fontSize: 14,
    fontWeight: 400,
    lineHeight: 1.5,
    color: "var(--foreground)"
  },
  textarea: {
    minHeight: 84,
    minWidth: 0,
    resize: "vertical",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border-subtle)",
    borderRadius: 6,
    backgroundColor: "white",
    paddingBlock: 8,
    paddingInline: 12,
    fontFamily: "var(--font-mono)",
    fontSize: 13,
    fontWeight: 400,
    lineHeight: 1.5,
    color: "var(--foreground)"
  },
  inlineHeader: {
    display: "flex",
    alignItems: "center",
    gap: 8,
    fontSize: 14,
    fontWeight: 500,
    lineHeight: 1.5,
    color: "var(--foreground)"
  },
  errorText: {
    margin: 0,
    fontSize: 14,
    lineHeight: 1.5,
    color: "var(--destructive)"
  },
  actions: {
    display: "flex",
    flexWrap: "wrap",
    gap: 8
  },
  icon: {
    width: 16,
    height: 16
  }
});

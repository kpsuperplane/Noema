import * as React from "react";
import { ExternalLink, RefreshCw } from "lucide-react";
import { Button } from "@astryxdesign/core/Button";
import { Dialog, DialogHeader } from "@astryxdesign/core/Dialog";
import { Layout, LayoutContent } from "@astryxdesign/core/Layout";
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
  const title = `Reconnect ${server?.displayName ?? "this connection"}`;

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
      width={560}
      aria-label={title}
    >
      <Layout
        height="auto"
        header={
          <DialogHeader
            title={title}
            subtitle={usesBrowserOAuth ? "Sign in again to reconnect." : "Update the sign-in details to reconnect."}
            onOpenChange={onOpenChange}
          />
        }
        content={
          <LayoutContent>
            <form {...stylex.props(styles.form)} onSubmit={submit}>
              {server?.transportKind === "stdio" ? (
                <TextAreaField
                  label="Private environment variables"
                  value={secretEnv}
                  onChange={setSecretEnv}
                />
              ) : null}
              {isHttp && !usesBrowserOAuth ? (
                <>
                  <TextAreaField
                    label="Private request headers"
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
                <Button
                  type="button"
                  variant="secondary"
                  label="Cancel"
                  isDisabled={submitting || oauthSubmitting}
                  onClick={() => onOpenChange(false)}
                />
                {usesBrowserOAuth ? (
                  <Button
                    type="submit"
                    label="Continue in browser"
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
                    label="Reconnect"
                    icon={!submitting ? <RefreshCw {...stylex.props(styles.icon)} aria-hidden="true" /> : undefined}
                    isDisabled={submitting || !server}
                    isLoading={submitting}
                  />
                )}
              </div>
            </form>
          </LayoutContent>
        }
      />
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
    return { value: null, error: "Enter both the OAuth client ID and client secret." };
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
  form: {
    display: "grid",
    gap: "var(--spacing-3)"
  },
  field: {
    display: "grid",
    gap: "var(--spacing-1-5)",
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
    paddingInline: "var(--spacing-3)",
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
    paddingBlock: "var(--spacing-2)",
    paddingInline: "var(--spacing-3)",
    fontFamily: "var(--font-mono)",
    fontSize: 13,
    fontWeight: 400,
    lineHeight: 1.5,
    color: "var(--foreground)"
  },
  errorText: {
    margin: "var(--spacing-0)",
    fontSize: 14,
    lineHeight: 1.5,
    color: "var(--destructive)"
  },
  actions: {
    display: "flex",
    flexWrap: "wrap",
    justifyContent: "flex-end",
    gap: "var(--spacing-2)",
    paddingTop: "var(--spacing-1)"
  },
  icon: {
    width: 16,
    height: 16
  }
});

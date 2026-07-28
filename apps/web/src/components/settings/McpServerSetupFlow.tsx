import * as React from "react";
import { ArrowLeft, ExternalLink, KeyRound, Plus, ShieldCheck, Trash2 } from "lucide-react";
import { Button } from "@astryxdesign/core/Button";
import { Tab, TabList } from "@astryxdesign/core/TabList";
import * as stylex from "@stylexjs/stylex";
import type { CreateMcpServerMutation } from "@/generated/graphql";
import { createClientId } from "@/shared/clientId";
import type { McpSetupFormSubmission } from "./mcpSetupForm";

export type McpServerSetupResult = CreateMcpServerMutation["createMcpServer"];
type TransportKind = "stdio" | "streamable_http";
type AuthMode = "browser" | "secrets";

export function McpServerSetupFlow({
  setupResult,
  setupSubmitting,
  oauthSubmitting,
  setupError,
  onCreateServer,
  onStartOAuth
}: {
  setupResult: McpServerSetupResult | null;
  setupSubmitting: boolean;
  oauthSubmitting: boolean;
  setupError: string | null;
  onCreateServer: (input: McpSetupFormSubmission) => void;
  onStartOAuth: (input: McpSetupFormSubmission) => void;
}) {
  const [transportKind, setTransportKind] = React.useState<TransportKind>("streamable_http");
  const [authMode, setAuthMode] = React.useState<AuthMode>("browser");
  const [showAuthScreen, setShowAuthScreen] = React.useState(true);
  const [formError, setFormError] = React.useState<string | null>(null);
  const [displayName, setDisplayName] = React.useState("");
  const [command, setCommand] = React.useState("");
  const [cwd, setCwd] = React.useState("");
  const [args, setArgs] = React.useState<RowDraft[]>([]);
  const [env, setEnv] = React.useState<KeyValueDraft[]>([]);
  const [secretEnv, setSecretEnv] = React.useState<KeyValueDraft[]>([]);
  const [url, setUrl] = React.useState("");
  const [headers, setHeaders] = React.useState<KeyValueDraft[]>([]);
  const [secretHeaders, setSecretHeaders] = React.useState<KeyValueDraft[]>([]);
  const [retrySecretEnv, setRetrySecretEnv] = React.useState<KeyValueDraft[]>([]);
  const [retrySecretHeaders, setRetrySecretHeaders] = React.useState<KeyValueDraft[]>([]);
  const [oauthClientId, setOauthClientId] = React.useState("");
  const [oauthClientSecret, setOauthClientSecret] = React.useState("");
  const [oauthScopes, setOauthScopes] = React.useState("");
  const [lastSubmission, setLastSubmission] = React.useState<McpSetupFormSubmission | null>(null);

  function submitCreate(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const safeEnv = keyValueRowsToRecord(env, "Non-sensitive environment variables");
    const hiddenEnv = keyValueRowsToRecord(secretEnv, "Private environment variables");
    const safeHeaders = keyValueRowsToRecord(headers, "Non-sensitive request headers");
    const hiddenHeaders = keyValueRowsToRecord(secretHeaders, "Private request headers");
    const parseError = safeEnv.error ?? hiddenEnv.error ?? safeHeaders.error ?? hiddenHeaders.error;
    if (parseError) {
      setFormError(parseError);
      return;
    }
    setFormError(null);
    if (transportKind === "stdio") {
      const submission = {
        displayName,
        transportKind,
        stdio: {
          command,
          args: rowDraftsToValues(args),
          cwd: cwd.trim() ? cwd.trim() : null,
          env: safeEnv.value,
          secretEnv: hiddenEnv.value
        },
        http: null
      } satisfies McpSetupFormSubmission;
      setLastSubmission(submission);
      setAuthMode("browser");
      setShowAuthScreen(true);
      onCreateServer(submission);
      return;
    }
    const submission = {
      displayName,
      transportKind,
      stdio: null,
      http: {
        url,
        headers: safeHeaders.value,
        secretHeaders: hiddenHeaders.value,
        oauthClientCredentials: null
      }
    } satisfies McpSetupFormSubmission;
    setLastSubmission(submission);
    setAuthMode("browser");
    setShowAuthScreen(true);
    onCreateServer(submission);
  }

  function submitRetry(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!lastSubmission) return;
    const parsedEnv = keyValueRowsToRecord(retrySecretEnv, "Private environment variables");
    const parsedHeaders = keyValueRowsToRecord(retrySecretHeaders, "Private request headers");
    const parsedOAuth = oauthClientCredentialsSupported
      ? oauthClientCredentialsFromDraft(oauthClientId, oauthClientSecret, oauthScopes)
      : { value: null, error: null };
    const parseError = parsedEnv.error ?? parsedHeaders.error ?? parsedOAuth.error;
    if (parseError) {
      setFormError(parseError);
      return;
    }
    setFormError(null);
    const submission = mergeRetrySecrets(
      lastSubmission,
      parsedEnv.value,
      parsedHeaders.value,
      parsedOAuth.value
    );
    setLastSubmission(submission);
    setShowAuthScreen(true);
    onCreateServer(submission);
  }

  const authRequired = setupResult?.setupStatus === "needs_auth";
  const authenticationAvailable = setupResult?.setupStatus === "authentication_available";
  const setupScreen = (authRequired || authenticationAvailable) && showAuthScreen ? "auth" : "details";
  const oauthClientCredentialsSupported =
    setupResult?.auth?.oauthClientCredentialsSupported ?? false;
  const oauthAuthorizationSupported =
    setupResult?.auth?.oauthAuthorizationSupported ?? false;
  const activeAuthMode = oauthAuthorizationSupported ? authMode : "secrets";
  const visibleError = formError ?? setupError ?? null;

  function renderHttpTransportFields() {
    return (
      <div {...stylex.props(styles.twoColumnGrid)}>
        <div {...stylex.props(styles.fullSpan)}>
          <TextField label="Server address" value={url} onChange={setUrl} />
        </div>
        <div {...stylex.props(styles.fullSpan)}>
          <KeyValueEditor
            label="Non-sensitive request headers"
            rows={headers}
            emptyText="No non-sensitive request headers added."
            onChange={setHeaders}
          />
        </div>
        <div {...stylex.props(styles.fullSpan)}>
          <KeyValueEditor
            label="Private request headers"
            rows={secretHeaders}
            emptyText="No private request headers added."
            onChange={setSecretHeaders}
          />
        </div>
      </div>
    );
  }

  return (
    <section {...stylex.props(styles.root)}>
      {setupScreen === "details" ? (
        <form {...stylex.props(styles.form)} onSubmit={submitCreate}>
          <label {...stylex.props(styles.field)}>
            <span>Name in Noema</span>
            <input
              {...stylex.props(styles.input)}
              value={displayName}
              onChange={(event) => setDisplayName(event.currentTarget.value)}
            />
          </label>
          <TabList
            value={transportKind}
            onChange={(value) => setTransportKind(value as TransportKind)}
            hasDivider
            aria-label="Connection type"
          >
            <Tab value="streamable_http" label="Web address" />
            <Tab value="stdio" label="Local command" />
          </TabList>
          {transportKind === "streamable_http" ? renderHttpTransportFields() : null}
          {transportKind === "stdio" ? (
            <div {...stylex.props(styles.twoColumnGrid)}>
              <TextField label="Command" value={command} onChange={setCommand} />
              <TextField label="Run from folder" value={cwd} onChange={setCwd} />
              <div {...stylex.props(styles.fullSpan)}>
                <StringListEditor
                  label="Command options"
                  values={args}
                  emptyText="No command options added."
                  addLabel="Add option"
                  onChange={setArgs}
                />
              </div>
              <div {...stylex.props(styles.fullSpan)}>
                <KeyValueEditor
                  label="Non-sensitive environment variables"
                  rows={env}
                  emptyText="No non-sensitive environment variables added."
                  onChange={setEnv}
                />
              </div>
              <div {...stylex.props(styles.fullSpan)}>
                <KeyValueEditor
                  label="Private environment variables"
                  rows={secretEnv}
                  emptyText="No private environment variables added."
                  onChange={setSecretEnv}
                />
              </div>
            </div>
          ) : null}

          {visibleError ? <p {...stylex.props(styles.errorText)}>{visibleError}</p> : null}
          <div {...stylex.props(styles.endActions)}>
            <Button
              type="submit"
              label="Check connection"
              isDisabled={setupSubmitting}
              isLoading={setupSubmitting}
              icon={!setupSubmitting ? <Plus {...stylex.props(styles.icon)} aria-hidden="true" /> : undefined}
          {...stylex.props(styles.fitButton)}
            />
          </div>
        </form>
      ) : null}

      {setupScreen === "auth" && authenticationAvailable ? (
        <div {...stylex.props(styles.form)}>
          <div {...stylex.props(styles.inlineHeader)}>
            <KeyRound {...stylex.props(styles.icon)} aria-hidden="true" />
            Sign in for full access
          </div>
          <p {...stylex.props(styles.mutedText)}>
            {setupResult.discoveredToolCount} {setupResult.discoveredToolCount === 1 ? "tool is" : "tools are"} available without signing in. Sign in to discover any additional tools.
          </p>
          {visibleError ? <p {...stylex.props(styles.errorText)}>{visibleError}</p> : null}
          <div {...stylex.props(styles.spreadActions)}>
            <BackButton
              onClick={() => {
                setFormError(null);
                setShowAuthScreen(false);
              }}
            />
            <div {...stylex.props(styles.actionGroup)}>
              {lastSubmission ? (
                <>
                  <Button
                    type="button"
                    label="Use public tools only"
                    variant="secondary"
                    isDisabled={setupSubmitting || oauthSubmitting}
                    {...stylex.props(styles.fitButton)}
                    onClick={() =>
                      onCreateServer({
                        ...lastSubmission,
                        authPreference: "USE_ANONYMOUS"
                      })
                    }
                  />
                  <Button
                    type="button"
                    label="Continue in browser"
                    isDisabled={oauthSubmitting || setupSubmitting}
                    isLoading={oauthSubmitting}
                    icon={!oauthSubmitting ? <ExternalLink {...stylex.props(styles.icon)} aria-hidden="true" /> : undefined}
                    {...stylex.props(styles.fitButton)}
                    onClick={() => onStartOAuth(lastSubmission)}
                  />
                </>
              ) : null}
            </div>
          </div>
        </div>
      ) : null}

      {setupScreen === "auth" && authRequired ? (
        <form {...stylex.props(styles.form)} onSubmit={submitRetry}>
          <div {...stylex.props(styles.inlineHeader)}>
            <KeyRound {...stylex.props(styles.icon)} aria-hidden="true" />
            Sign-in required
          </div>
          <TabList
            value={activeAuthMode}
            onChange={(value) => setAuthMode(value as AuthMode)}
            hasDivider
            aria-label="Sign-in method"
          >
            {oauthAuthorizationSupported ? <Tab value="browser" label="Browser sign-in" /> : null}
            <Tab value="secrets" label="Enter credentials" />
          </TabList>
            {oauthAuthorizationSupported ? (
              <div hidden={activeAuthMode !== "browser"}>
                <div {...stylex.props(styles.form)}>
                  {visibleError ? (
                    <p {...stylex.props(styles.errorText)}>{visibleError}</p>
                  ) : null}
                  <div {...stylex.props(styles.spreadActions)}>
                    <BackButton
                      onClick={() => {
                        setFormError(null);
                        setShowAuthScreen(false);
                      }}
                    />
                    {lastSubmission ? (
                      <Button
                        type="button"
                        label="Continue in browser"
                        isDisabled={oauthSubmitting}
                        isLoading={oauthSubmitting}
                        icon={!oauthSubmitting ? <ExternalLink {...stylex.props(styles.icon)} aria-hidden="true" /> : undefined}
          {...stylex.props(styles.fitButton)}
                        onClick={() => onStartOAuth(lastSubmission)}
                      />
                    ) : null}
                  </div>
                </div>
              </div>
            ) : null}
            <div hidden={activeAuthMode !== "secrets"}>
              <div {...stylex.props(styles.form)}>
                <div {...stylex.props(styles.twoColumnGrid)}>
                  <KeyValueEditor
                    label="Private environment variables"
                    rows={retrySecretEnv}
                    emptyText="No private environment variables added."
                    onChange={setRetrySecretEnv}
                  />
                  <KeyValueEditor
                    label="Private request headers"
                    rows={retrySecretHeaders}
                    emptyText="No private request headers added."
                    onChange={setRetrySecretHeaders}
                  />
                  {oauthClientCredentialsSupported ? (
                    <>
                      <TextField label="OAuth client ID" value={oauthClientId} onChange={setOauthClientId} />
                      <TextField
                        label="OAuth client secret"
                        value={oauthClientSecret}
                        onChange={setOauthClientSecret}
                        type="password"
                      />
                      <div {...stylex.props(styles.fullSpan)}>
                        <TextField
                          label="OAuth scopes"
                          value={oauthScopes}
                          onChange={setOauthScopes}
                        />
                      </div>
                    </>
                  ) : null}
                </div>
                {visibleError ? <p {...stylex.props(styles.errorText)}>{visibleError}</p> : null}
                <div {...stylex.props(styles.spreadActions)}>
                  <BackButton
                    onClick={() => {
                      setFormError(null);
                      setShowAuthScreen(false);
                    }}
                  />
                  <Button
                    type="submit"
                    label="Try again"
                    isDisabled={setupSubmitting}
                    isLoading={setupSubmitting}
          {...stylex.props(styles.fitButton)}
                  />
                </div>
              </div>
            </div>
        </form>
      ) : null}

      {setupResult?.setupStatus === "ready_for_policy" ? (
        <div {...stylex.props(styles.policyNotice)}>
          <div {...stylex.props(styles.inlineHeader)}>
            <ShieldCheck {...stylex.props(styles.icon)} aria-hidden="true" />
            Connection ready
          </div>
          <p {...stylex.props(styles.mutedText)}>
            Next, choose sharing and approvals.
          </p>
        </div>
      ) : null}
    </section>
  );
}

function BackButton({ onClick }: { onClick: () => void }) {
  return (
    <Button
      type="button"
      variant="ghost"
      label="Back"
      icon={<ArrowLeft {...stylex.props(styles.icon)} aria-hidden="true" />}
          {...stylex.props(styles.fitButton)}
      onClick={onClick}
    />
  );
}

function mergeRetrySecrets(
  submission: McpSetupFormSubmission,
  secretEnv: Record<string, string>,
  secretHeaders: Record<string, string>,
  oauthClientCredentials: NonNullable<
    NonNullable<McpSetupFormSubmission["http"]>["oauthClientCredentials"]
  > | null
): McpSetupFormSubmission {
  if (submission.transportKind === "stdio" && submission.stdio) {
    return {
      ...submission,
      stdio: {
        ...submission.stdio,
        secretEnv: {
          ...submission.stdio.secretEnv,
          ...secretEnv
        }
      }
    };
  }
  if (
    submission.transportKind === "streamable_http" &&
    submission.http
  ) {
    return {
      ...submission,
      http: {
        ...submission.http,
        secretHeaders: {
          ...submission.http.secretHeaders,
          ...secretHeaders
        },
        oauthClientCredentials:
          oauthClientCredentials ?? submission.http.oauthClientCredentials ?? null
      }
    };
  }
  return submission;
}

type KeyValueRowsResult =
  | { value: Record<string, string>; error: null }
  | { value: Record<string, string>; error: string };

type OAuthClientCredentialsDraft = NonNullable<
  NonNullable<McpSetupFormSubmission["http"]>["oauthClientCredentials"]
>;

type OAuthClientCredentialsResult =
  | { value: OAuthClientCredentialsDraft | null; error: null }
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

type RowDraft = {
  id: string;
  value: string;
};

type KeyValueDraft = {
  id: string;
  key: string;
  value: string;
};

function rowDraftsToValues(rows: readonly RowDraft[]) {
  return rows.map((row) => row.value.trim()).filter(Boolean);
}

function keyValueRowsToRecord(rows: readonly KeyValueDraft[], label: string): KeyValueRowsResult {
  const value: Record<string, string> = {};
  for (const [index, row] of rows.entries()) {
    const key = row.key.trim();
    const parsedValue = row.value.trim();
    if (!key && !parsedValue) continue;
    if (!key) {
      return { value: {}, error: `${label} row ${index + 1} needs a key.` };
    }
    if (Object.prototype.hasOwnProperty.call(value, key)) {
      return { value: {}, error: `${label} contains the key "${key}" more than once.` };
    }
    value[key] = parsedValue;
  }
  return { value, error: null };
}

function newRowDraft(): RowDraft {
  return { id: createClientId(), value: "" };
}

function newKeyValueDraft(): KeyValueDraft {
  return { id: createClientId(), key: "", value: "" };
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

function StringListEditor({
  label,
  values,
  emptyText,
  addLabel,
  onChange
}: {
  label: string;
  values: readonly RowDraft[];
  emptyText: string;
  addLabel: string;
  onChange: React.Dispatch<React.SetStateAction<RowDraft[]>>;
}) {
  return (
    <section {...stylex.props(styles.editorSection)}>
      <div {...stylex.props(styles.sectionHeader)}>
        <h3 {...stylex.props(styles.sectionTitle)}>{label}</h3>
        <Button
          type="button"
          variant="secondary"
          label={addLabel}
          icon={<Plus {...stylex.props(styles.icon)} aria-hidden="true" />}
          {...stylex.props(styles.fitButton)}
          onClick={() => onChange((current) => [...current, newRowDraft()])}
        />
      </div>
      {values.length === 0 ? <p {...stylex.props(styles.mutedText)}>{emptyText}</p> : null}
      <div {...stylex.props(styles.editorRows)}>
        {values.map((row, index) => (
          <div key={row.id} {...stylex.props(styles.stringRow)}>
            <span {...stylex.props(styles.rowIndex)}>
              {index + 1}
            </span>
            <input
              aria-label={`${label} ${index + 1}`}
              {...stylex.props(styles.input)}
              value={row.value}
              onChange={(event) =>
                onChange((current) =>
                  current.map((item) =>
                    item.id === row.id ? { ...item, value: event.currentTarget.value } : item
                  )
                )
              }
            />
            <Button
              type="button"
              variant="ghost"
              label={`Remove ${label} ${index + 1}`}
              icon={<Trash2 {...stylex.props(styles.icon)} aria-hidden="true" />}
              isIconOnly
              onClick={() => onChange((current) => current.filter((item) => item.id !== row.id))}
            />
          </div>
        ))}
      </div>
    </section>
  );
}

function KeyValueEditor({
  label,
  rows,
  emptyText,
  onChange
}: {
  label: string;
  rows: readonly KeyValueDraft[];
  emptyText: string;
  onChange: React.Dispatch<React.SetStateAction<KeyValueDraft[]>>;
}) {
  return (
    <section {...stylex.props(styles.editorSection)}>
      <div {...stylex.props(styles.sectionHeader)}>
        <h3 {...stylex.props(styles.sectionTitle)}>{label}</h3>
        <Button
          type="button"
          variant="secondary"
          label="Add row"
          icon={<Plus {...stylex.props(styles.icon)} aria-hidden="true" />}
          {...stylex.props(styles.fitButton)}
          onClick={() => onChange((current) => [...current, newKeyValueDraft()])}
        />
      </div>
      {rows.length === 0 ? <p {...stylex.props(styles.mutedText)}>{emptyText}</p> : null}
      <div {...stylex.props(styles.editorRows)}>
        {rows.map((row, index) => (
          <div key={row.id} {...stylex.props(styles.keyValueRow)}>
            <input
              aria-label={`${label} key ${index + 1}`}
              placeholder="Key"
              {...stylex.props(styles.input)}
              value={row.key}
              onChange={(event) =>
                updateKeyValueRow(onChange, row.id, { key: event.currentTarget.value })
              }
            />
            <input
              aria-label={`${label} value ${index + 1}`}
              placeholder="Value"
              {...stylex.props(styles.input)}
              value={row.value}
              onChange={(event) =>
                updateKeyValueRow(onChange, row.id, { value: event.currentTarget.value })
              }
            />
            <Button
              type="button"
              variant="ghost"
              label={`Remove ${label} row ${index + 1}`}
              icon={<Trash2 {...stylex.props(styles.icon)} aria-hidden="true" />}
              isIconOnly
              onClick={() => onChange((current) => current.filter((item) => item.id !== row.id))}
            />
          </div>
        ))}
      </div>
    </section>
  );
}

function updateKeyValueRow(
  onChange: React.Dispatch<React.SetStateAction<KeyValueDraft[]>>,
  id: string,
  patch: Partial<Omit<KeyValueDraft, "id">>
) {
  onChange((current) =>
    current.map((row) => (row.id === id ? { ...row, ...patch } : row))
  );
}

const styles = stylex.create({
  root: {
    display: "grid",
    gap: "var(--spacing-4)"
  },
  form: {
    display: "grid",
    gap: "var(--spacing-3)"
  },
  field: {
    display: "grid",
    gap: "var(--spacing-1)",
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
  twoColumnGrid: {
    display: "grid",
    gap: "var(--spacing-3)",
    gridTemplateColumns: "repeat(2, minmax(0, 1fr))",
    "@media (max-width: 640px)": {
      gridTemplateColumns: "1fr"
    }
  },
  fullSpan: {
    gridColumn: "1 / -1"
  },
  errorText: {
    margin: "var(--spacing-0)",
    fontSize: 14,
    lineHeight: 1.5,
    color: "var(--destructive)"
  },
  mutedText: {
    margin: "var(--spacing-0)",
    fontSize: 14,
    lineHeight: 1.5,
    color: "var(--muted-foreground)"
  },
  smallMutedText: {
    margin: "var(--spacing-0)",
    fontSize: 12,
    lineHeight: 1.5,
    color: "var(--muted-foreground)"
  },
  endActions: {
    display: "flex",
    justifyContent: "flex-end"
  },
  spreadActions: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    gap: "var(--spacing-2)"
  },
  actionGroup: {
    display: "flex",
    alignItems: "center",
    justifyContent: "flex-end",
    gap: "var(--spacing-2)",
    flexWrap: "wrap"
  },
  inlineHeader: {
    display: "flex",
    alignItems: "center",
    gap: "var(--spacing-2)",
    fontSize: 14,
    fontWeight: 500,
    lineHeight: 1.5,
    color: "var(--foreground)"
  },
  policyNotice: {
    display: "grid",
    gap: "var(--spacing-2)",
    borderTopWidth: 1,
    borderTopStyle: "solid",
    borderTopColor: "var(--border-subtle)",
    paddingTop: "var(--spacing-4)"
  },
  editorSection: {
    display: "grid",
    gap: "var(--spacing-2)"
  },
  sectionHeader: {
    display: "flex",
    flexWrap: "wrap",
    alignItems: "center",
    justifyContent: "space-between",
    gap: "var(--spacing-2)"
  },
  sectionTitle: {
    margin: "var(--spacing-0)",
    fontSize: 14,
    fontWeight: 500,
    lineHeight: 1.5,
    color: "var(--foreground)"
  },
  editorRows: {
    display: "grid",
    gap: "var(--spacing-2)"
  },
  stringRow: {
    display: "grid",
    gap: "var(--spacing-2)",
    gridTemplateColumns: "auto minmax(0, 1fr) auto",
    "@media (max-width: 640px)": {
      gridTemplateColumns: "1fr"
    }
  },
  keyValueRow: {
    display: "grid",
    gap: "var(--spacing-2)",
    gridTemplateColumns: "minmax(0, 1fr) minmax(0, 1fr) auto",
    "@media (max-width: 640px)": {
      gridTemplateColumns: "1fr"
    }
  },
  rowIndex: {
    alignSelf: "center",
    fontFamily: "var(--font-mono)",
    fontSize: 12,
    lineHeight: 1.5,
    color: "var(--muted-foreground)"
  },
  fitButton: {
    width: "fit-content"
  },
  icon: {
    width: 16,
    height: 16
  }
});

import * as React from "react";
import { ArrowLeft, ExternalLink, KeyRound, Loader2, Plus, ShieldCheck, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import type { CreateMcpServerMutation } from "@/generated/graphql";
import type { McpSetupFormSubmission } from "./mcpSetupForm";

export type McpServerSetupResult = CreateMcpServerMutation["createMcpServer"];
type TransportKind = "stdio" | "sse" | "streamable_http";
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
  const [lastSubmission, setLastSubmission] = React.useState<McpSetupFormSubmission | null>(null);

  function submitCreate(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const safeEnv = keyValueRowsToRecord(env, "Non-secret env");
    const hiddenEnv = keyValueRowsToRecord(secretEnv, "Secret env");
    const safeHeaders = keyValueRowsToRecord(headers, "Non-secret headers");
    const hiddenHeaders = keyValueRowsToRecord(secretHeaders, "Secret headers");
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
    const parsedEnv = keyValueRowsToRecord(retrySecretEnv, "Secret env");
    const parsedHeaders = keyValueRowsToRecord(retrySecretHeaders, "Secret headers");
    const parseError = parsedEnv.error ?? parsedHeaders.error;
    if (parseError) {
      setFormError(parseError);
      return;
    }
    setFormError(null);
    const submission = mergeRetrySecrets(
      lastSubmission,
      parsedEnv.value,
      parsedHeaders.value,
      null
    );
    setLastSubmission(submission);
    setShowAuthScreen(true);
    onCreateServer(submission);
  }

  const authRequired = setupResult?.setupStatus === "needs_auth";
  const setupScreen = authRequired && showAuthScreen ? "auth" : "details";
  const oauthClientCredentialsSupported =
    setupResult?.auth?.oauthClientCredentialsSupported ?? false;
  const oauthAuthorizationSupported =
    setupResult?.auth?.oauthAuthorizationSupported ?? false;
  const activeAuthMode = oauthAuthorizationSupported ? authMode : "secrets";
  const visibleError =
    formError ?? setupError ?? (setupScreen === "auth" ? setupResult?.setupError : null) ?? null;

  function renderHttpTransportFields() {
    return (
      <div className="grid gap-3 sm:grid-cols-2">
        <div className="sm:col-span-2">
          <TextField label="URL" value={url} onChange={setUrl} />
        </div>
        <div className="sm:col-span-2">
          <KeyValueEditor
            label="Non-secret headers"
            rows={headers}
            emptyText="No non-secret headers configured."
            onChange={setHeaders}
          />
        </div>
        <div className="sm:col-span-2">
          <KeyValueEditor
            label="Secret headers"
            rows={secretHeaders}
            emptyText="No secret headers configured."
            onChange={setSecretHeaders}
          />
        </div>
      </div>
    );
  }

  return (
    <section className="grid gap-4">
      {setupScreen === "details" ? (
        <form className="grid gap-3" onSubmit={submitCreate}>
          <label className="grid gap-1 text-sm font-medium">
            Display name
            <input
              className="h-9 rounded-md border border-[var(--border-subtle)] px-3 text-sm font-normal"
              value={displayName}
              onChange={(event) => setDisplayName(event.currentTarget.value)}
            />
          </label>
          <Tabs
            value={transportKind}
            onValueChange={(value) => setTransportKind(value as TransportKind)}
          >
            <TabsList aria-label="Transport">
              <TabsTrigger value="streamable_http">Streamable HTTP</TabsTrigger>
              <TabsTrigger value="sse">SSE</TabsTrigger>
              <TabsTrigger value="stdio">stdio</TabsTrigger>
            </TabsList>
            <TabsContent value="streamable_http">{renderHttpTransportFields()}</TabsContent>
            <TabsContent value="sse">{renderHttpTransportFields()}</TabsContent>
            <TabsContent value="stdio">
              <div className="grid gap-3 sm:grid-cols-2">
                <TextField label="Command" value={command} onChange={setCommand} />
                <TextField label="Working directory" value={cwd} onChange={setCwd} />
                <div className="sm:col-span-2">
                  <StringListEditor
                    label="Args"
                    values={args}
                    emptyText="No arguments configured."
                    addLabel="Add argument"
                    onChange={setArgs}
                  />
                </div>
                <div className="sm:col-span-2">
                  <KeyValueEditor
                    label="Non-secret env"
                    rows={env}
                    emptyText="No non-secret environment variables configured."
                    onChange={setEnv}
                  />
                </div>
                <div className="sm:col-span-2">
                  <KeyValueEditor
                    label="Secret env"
                    rows={secretEnv}
                    emptyText="No secret environment variables configured."
                    onChange={setSecretEnv}
                  />
                </div>
              </div>
            </TabsContent>
          </Tabs>

          {visibleError ? <p className="m-0 text-sm text-destructive">{visibleError}</p> : null}
          <div className="flex justify-end">
            <Button type="submit" className="w-fit" disabled={setupSubmitting}>
              {setupSubmitting ? (
                <Loader2 className="size-4 animate-spin" aria-hidden="true" />
              ) : (
                <Plus className="size-4" aria-hidden="true" />
              )}
              Save and verify
            </Button>
          </div>
        </form>
      ) : null}

      {setupScreen === "auth" && authRequired ? (
        <form className="grid gap-3" onSubmit={submitRetry}>
          <div className="flex items-center gap-2 text-sm font-medium">
            <KeyRound className="size-4" aria-hidden="true" />
            Authentication required
          </div>
          <Tabs value={activeAuthMode} onValueChange={(value) => setAuthMode(value as AuthMode)}>
            <TabsList aria-label="Authentication method">
              {oauthAuthorizationSupported ? (
                <TabsTrigger value="browser">Browser</TabsTrigger>
              ) : null}
              <TabsTrigger value="secrets">Secrets</TabsTrigger>
            </TabsList>
            {oauthAuthorizationSupported ? (
              <TabsContent value="browser">
                <div className="grid gap-3">
                  {visibleError ? (
                    <p className="m-0 text-sm text-destructive">{visibleError}</p>
                  ) : null}
                  <div className="flex items-center justify-between gap-2">
                    <BackButton
                      onClick={() => {
                        setFormError(null);
                        setShowAuthScreen(false);
                      }}
                    />
                    {lastSubmission ? (
                      <Button
                        type="button"
                        className="w-fit"
                        disabled={oauthSubmitting}
                        onClick={() => onStartOAuth(lastSubmission)}
                      >
                        {oauthSubmitting ? (
                          <Loader2 className="size-4 animate-spin" aria-hidden="true" />
                        ) : (
                          <ExternalLink className="size-4" aria-hidden="true" />
                        )}
                        Continue with OAuth
                      </Button>
                    ) : null}
                  </div>
                </div>
              </TabsContent>
            ) : null}
            <TabsContent value="secrets">
              <div className="grid gap-3">
                <div className="grid gap-3 sm:grid-cols-2">
                  <KeyValueEditor
                    label="Secret env"
                    rows={retrySecretEnv}
                    emptyText="No secret environment variables configured."
                    onChange={setRetrySecretEnv}
                  />
                  <KeyValueEditor
                    label="Secret headers"
                    rows={retrySecretHeaders}
                    emptyText="No secret headers configured."
                    onChange={setRetrySecretHeaders}
                  />
                </div>
                {visibleError ? <p className="m-0 text-sm text-destructive">{visibleError}</p> : null}
                <div className="flex items-center justify-between gap-2">
                  <BackButton
                    onClick={() => {
                      setFormError(null);
                      setShowAuthScreen(false);
                    }}
                  />
                  <Button type="submit" className="w-fit" disabled={setupSubmitting}>
                    {setupSubmitting ? (
                      <Loader2 className="size-4 animate-spin" aria-hidden="true" />
                    ) : null}
                    Retry setup
                  </Button>
                </div>
              </div>
            </TabsContent>
          </Tabs>
          {oauthClientCredentialsSupported && !oauthAuthorizationSupported ? (
            <p className="m-0 text-xs text-muted-foreground">
              This server may also support OAuth client credentials through backend configuration.
            </p>
          ) : null}
        </form>
      ) : null}

      {setupResult?.setupStatus === "ready_for_calibration" ? (
        <div className="grid gap-2 border-t border-[var(--border-subtle)] pt-4">
          <div className="flex items-center gap-2 text-sm font-medium">
            <ShieldCheck className="size-4" aria-hidden="true" />
            Configure tools
          </div>
          <p className="m-0 text-sm text-muted-foreground">
            {setupResult.discoveredToolCount} discovered tools are waiting for calibration before
            agent visibility.
          </p>
        </div>
      ) : null}
    </section>
  );
}

function BackButton({ onClick }: { onClick: () => void }) {
  return (
    <Button type="button" variant="ghost" className="w-fit" onClick={onClick}>
      <ArrowLeft className="size-4" aria-hidden="true" />
      Back
    </Button>
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
    (submission.transportKind === "sse" || submission.transportKind === "streamable_http") &&
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
      return { value: {}, error: `${label} row ${index + 1} must include a key.` };
    }
    if (Object.prototype.hasOwnProperty.call(value, key)) {
      return { value: {}, error: `${label} has a duplicate key: ${key}` };
    }
    value[key] = parsedValue;
  }
  return { value, error: null };
}

function newRowDraft(): RowDraft {
  return { id: crypto.randomUUID(), value: "" };
}

function newKeyValueDraft(): KeyValueDraft {
  return { id: crypto.randomUUID(), key: "", value: "" };
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
    <label className="grid gap-1 text-sm font-medium">
      {label}
      <input
        type={type}
        className="h-9 rounded-md border border-[var(--border-subtle)] px-3 text-sm font-normal"
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
    <section className="grid gap-2">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <h3 className="m-0 text-sm font-medium">{label}</h3>
        <Button
          type="button"
          variant="outline"
          className="w-fit"
          onClick={() => onChange((current) => [...current, newRowDraft()])}
        >
          <Plus className="size-4" aria-hidden="true" />
          {addLabel}
        </Button>
      </div>
      {values.length === 0 ? <p className="m-0 text-sm text-muted-foreground">{emptyText}</p> : null}
      <div className="grid gap-2">
        {values.map((row, index) => (
          <div key={row.id} className="grid gap-2 sm:grid-cols-[auto_1fr_auto]">
            <span className="self-center font-mono text-xs text-muted-foreground">
              {index + 1}
            </span>
            <input
              aria-label={`${label} ${index + 1}`}
              className="h-9 rounded-md border border-[var(--border-subtle)] px-3 text-sm"
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
              aria-label={`Remove ${label} ${index + 1}`}
              onClick={() => onChange((current) => current.filter((item) => item.id !== row.id))}
            >
              <Trash2 className="size-4" aria-hidden="true" />
            </Button>
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
    <section className="grid gap-2">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <h3 className="m-0 text-sm font-medium">{label}</h3>
        <Button
          type="button"
          variant="outline"
          className="w-fit"
          onClick={() => onChange((current) => [...current, newKeyValueDraft()])}
        >
          <Plus className="size-4" aria-hidden="true" />
          Add row
        </Button>
      </div>
      {rows.length === 0 ? <p className="m-0 text-sm text-muted-foreground">{emptyText}</p> : null}
      <div className="grid gap-2">
        {rows.map((row, index) => (
          <div key={row.id} className="grid gap-2 sm:grid-cols-[minmax(0,1fr)_minmax(0,1fr)_auto]">
            <input
              aria-label={`${label} key ${index + 1}`}
              placeholder="Key"
              className="h-9 rounded-md border border-[var(--border-subtle)] px-3 text-sm"
              value={row.key}
              onChange={(event) =>
                updateKeyValueRow(onChange, row.id, { key: event.currentTarget.value })
              }
            />
            <input
              aria-label={`${label} value ${index + 1}`}
              placeholder="Value"
              className="h-9 rounded-md border border-[var(--border-subtle)] px-3 text-sm"
              value={row.value}
              onChange={(event) =>
                updateKeyValueRow(onChange, row.id, { value: event.currentTarget.value })
              }
            />
            <Button
              type="button"
              variant="ghost"
              aria-label={`Remove ${label} row ${index + 1}`}
              onClick={() => onChange((current) => current.filter((item) => item.id !== row.id))}
            >
              <Trash2 className="size-4" aria-hidden="true" />
            </Button>
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

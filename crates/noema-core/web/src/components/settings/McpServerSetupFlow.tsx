import * as React from "react";
import {
  AlertTriangle,
  CheckCircle2,
  Circle,
  KeyRound,
  Loader2,
  Plus,
  ShieldCheck
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import type { CreateMcpServerMutation } from "@/generated/graphql";
import {
  parseArgsLines,
  parseKeyValueLines,
  type McpSetupFormSubmission
} from "./mcpSetupForm";

export type McpServerSetupResult = CreateMcpServerMutation["createMcpServer"];

export function McpServerSetupFlow({
  setupResult,
  setupSubmitting,
  setupError,
  onCreateServer,
  onCancel
}: {
  setupResult: McpServerSetupResult | null;
  setupSubmitting: boolean;
  setupError: string | null;
  onCreateServer: (input: McpSetupFormSubmission) => void;
  onCancel?: () => void;
}) {
  const [transportKind, setTransportKind] = React.useState<"stdio" | "http_sse">("stdio");
  const [formError, setFormError] = React.useState<string | null>(null);
  const [displayName, setDisplayName] = React.useState("");
  const [command, setCommand] = React.useState("");
  const [args, setArgs] = React.useState("");
  const [cwd, setCwd] = React.useState("");
  const [env, setEnv] = React.useState("");
  const [secretEnv, setSecretEnv] = React.useState("");
  const [url, setUrl] = React.useState("");
  const [headers, setHeaders] = React.useState("");
  const [secretHeaders, setSecretHeaders] = React.useState("");
  const [retrySecretEnv, setRetrySecretEnv] = React.useState("");
  const [retrySecretHeaders, setRetrySecretHeaders] = React.useState("");
  const [lastSubmission, setLastSubmission] = React.useState<McpSetupFormSubmission | null>(null);

  function submitCreate(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const safeEnv = parseKeyValueLines(env);
    const hiddenEnv = parseKeyValueLines(secretEnv);
    const safeHeaders = parseKeyValueLines(headers);
    const hiddenHeaders = parseKeyValueLines(secretHeaders);
    const parseError = safeEnv.error ?? hiddenEnv.error ?? safeHeaders.error ?? hiddenHeaders.error;
    if (parseError) {
      setFormError(parseError);
      return;
    }
    const safeEnvValue = parsedKeyValue(safeEnv);
    const hiddenEnvValue = parsedKeyValue(hiddenEnv);
    const safeHeadersValue = parsedKeyValue(safeHeaders);
    const hiddenHeadersValue = parsedKeyValue(hiddenHeaders);
    setFormError(null);
    if (transportKind === "stdio") {
      const submission = {
        displayName,
        transportKind,
        stdio: {
          command,
          args: parseArgsLines(args),
          cwd: cwd.trim() ? cwd.trim() : null,
          env: safeEnvValue,
          secretEnv: hiddenEnvValue
        },
        httpSse: null
      } satisfies McpSetupFormSubmission;
      setLastSubmission(submission);
      onCreateServer(submission);
      return;
    }
    const submission = {
      displayName,
      transportKind,
      stdio: null,
      httpSse: {
        url,
        headers: safeHeadersValue,
        secretHeaders: hiddenHeadersValue
      }
    } satisfies McpSetupFormSubmission;
    setLastSubmission(submission);
    onCreateServer(submission);
  }

  function submitRetry(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!lastSubmission) return;
    const parsedEnv = parseKeyValueLines(retrySecretEnv);
    const parsedHeaders = parseKeyValueLines(retrySecretHeaders);
    const parseError = parsedEnv.error ?? parsedHeaders.error;
    if (parseError) {
      setFormError(parseError);
      return;
    }
    const parsedEnvValue = parsedKeyValue(parsedEnv);
    const parsedHeadersValue = parsedKeyValue(parsedHeaders);
    setFormError(null);
    const submission = mergeRetrySecrets(lastSubmission, parsedEnvValue, parsedHeadersValue);
    setLastSubmission(submission);
    onCreateServer(submission);
  }

  const visibleError = formError ?? setupError ?? setupResult?.setupError ?? null;
  const stepStates = setupStepStates(setupResult?.setupStatus ?? null, setupSubmitting);
  const closeLabel = setupResult?.setupStatus === "needs_auth" ? "Close and keep draft" : "Close";

  return (
    <section className="grid gap-4">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="grid gap-1">
          <h2 className="m-0 font-heading text-xl leading-tight tracking-normal">
            Add MCP server
          </h2>
          <p className="m-0 text-sm text-muted-foreground">
            Noema verifies the server, fetches tool schemas, then keeps tools disabled until
            calibration.
          </p>
        </div>
        {setupSubmitting ? <Loader2 className="size-5 animate-spin text-muted-foreground" /> : null}
      </div>

      <ol className="grid gap-2 text-sm text-muted-foreground sm:grid-cols-4">
        {stepStates.map((step) => (
          <li key={step.label} className="flex items-center gap-2">
            <SetupStepIcon state={step.state} />
            <span className={step.state === "current" ? "font-medium text-foreground" : ""}>
              {step.label}
            </span>
          </li>
        ))}
      </ol>

      <form className="grid gap-3" onSubmit={submitCreate}>
        <label className="grid gap-1 text-sm font-medium">
          Display name
          <input
            className="h-9 rounded-md border border-[var(--border-subtle)] px-3 text-sm font-normal"
            value={displayName}
            onChange={(event) => setDisplayName(event.currentTarget.value)}
          />
        </label>
        <div className="flex flex-wrap gap-2" aria-label="Transport">
          <Button
            type="button"
            variant={transportKind === "stdio" ? "default" : "outline"}
            onClick={() => setTransportKind("stdio")}
          >
            stdio
          </Button>
          <Button
            type="button"
            variant={transportKind === "http_sse" ? "default" : "outline"}
            onClick={() => setTransportKind("http_sse")}
          >
            http_sse
          </Button>
        </div>

        {transportKind === "stdio" ? (
          <div className="grid gap-3 sm:grid-cols-2">
            <TextField label="Command" value={command} onChange={setCommand} />
            <TextField label="Working directory" value={cwd} onChange={setCwd} />
            <TextAreaField label="Args" value={args} onChange={setArgs} />
            <TextAreaField label="Non-secret env" value={env} onChange={setEnv} />
            <TextAreaField label="Secret env" value={secretEnv} onChange={setSecretEnv} />
          </div>
        ) : (
          <div className="grid gap-3 sm:grid-cols-2">
            <TextField label="URL" value={url} onChange={setUrl} />
            <TextAreaField label="Non-secret headers" value={headers} onChange={setHeaders} />
            <TextAreaField
              label="Secret headers"
              value={secretHeaders}
              onChange={setSecretHeaders}
            />
          </div>
        )}

        {visibleError ? <p className="m-0 text-sm text-destructive">{visibleError}</p> : null}
        <Button type="submit" className="w-fit" disabled={setupSubmitting}>
          <Plus className="size-4" aria-hidden="true" />
          Save and verify
        </Button>
      </form>

      {setupResult?.setupStatus === "needs_auth" ? (
        <form className="grid gap-3 border-t border-[var(--border-subtle)] pt-4" onSubmit={submitRetry}>
          <div className="flex items-center gap-2 text-sm font-medium">
            <KeyRound className="size-4" aria-hidden="true" />
            Authentication required
          </div>
          <p className="m-0 text-sm text-muted-foreground">
            The server has not been saved yet. Add the secret environment variables or headers this
            MCP server expects, then retry setup to verify and list tools.
          </p>
          <div className="grid gap-3 sm:grid-cols-2">
            <TextAreaField label="Secret env" value={retrySecretEnv} onChange={setRetrySecretEnv} />
            <TextAreaField
              label="Secret headers"
              value={retrySecretHeaders}
              onChange={setRetrySecretHeaders}
            />
          </div>
          <Button type="submit" className="w-fit" disabled={setupSubmitting}>
            Retry setup
          </Button>
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
      {onCancel ? (
        <Button type="button" variant="ghost" className="w-fit" onClick={onCancel}>
          {closeLabel}
        </Button>
      ) : null}
    </section>
  );
}

type SetupStepState = "pending" | "current" | "complete" | "blocked";

function setupStepStates(
  setupStatus: string | null,
  submitting: boolean
): { label: string; state: SetupStepState }[] {
  if (setupStatus === "ready_for_calibration") {
    return [
      { label: "Verify server", state: "complete" },
      { label: "Authenticate if needed", state: "complete" },
      { label: "Fetch tools", state: "complete" },
      { label: "Configure tools", state: "current" }
    ];
  }
  if (setupStatus === "needs_auth") {
    return [
      { label: "Verify server", state: "complete" },
      { label: "Authenticate if needed", state: "current" },
      { label: "Fetch tools", state: "pending" },
      { label: "Configure tools", state: "pending" }
    ];
  }
  if (setupStatus === "unavailable" || setupStatus === "malformed") {
    return [
      { label: "Verify server", state: "blocked" },
      { label: "Authenticate if needed", state: "pending" },
      { label: "Fetch tools", state: "pending" },
      { label: "Configure tools", state: "pending" }
    ];
  }
  if (submitting) {
    return [
      { label: "Verify server", state: "current" },
      { label: "Authenticate if needed", state: "pending" },
      { label: "Fetch tools", state: "pending" },
      { label: "Configure tools", state: "pending" }
    ];
  }
  return [
    { label: "Verify server", state: "pending" },
    { label: "Authenticate if needed", state: "pending" },
    { label: "Fetch tools", state: "pending" },
    { label: "Configure tools", state: "pending" }
  ];
}

function SetupStepIcon({ state }: { state: SetupStepState }) {
  if (state === "complete") {
    return <CheckCircle2 className="size-4 text-[var(--pine-700)]" aria-hidden="true" />;
  }
  if (state === "current") {
    return <Circle className="size-4 text-[var(--pine-700)]" aria-hidden="true" />;
  }
  if (state === "blocked") {
    return <AlertTriangle className="size-4 text-destructive" aria-hidden="true" />;
  }
  return <Circle className="size-4 text-muted-foreground" aria-hidden="true" />;
}

function mergeRetrySecrets(
  submission: McpSetupFormSubmission,
  secretEnv: Record<string, string>,
  secretHeaders: Record<string, string>
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
  if (submission.transportKind === "http_sse" && submission.httpSse) {
    return {
      ...submission,
      httpSse: {
        ...submission.httpSse,
        secretHeaders: {
          ...submission.httpSse.secretHeaders,
          ...secretHeaders
        }
      }
    };
  }
  return submission;
}

function parsedKeyValue(result: ReturnType<typeof parseKeyValueLines>): Record<string, string> {
  return result.value ?? {};
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
    <label className="grid gap-1 text-sm font-medium">
      {label}
      <Textarea rows={3} value={value} onChange={(event) => onChange(event.currentTarget.value)} />
    </label>
  );
}

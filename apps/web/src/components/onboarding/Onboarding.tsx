import { Button } from "@astryxdesign/core/Button";
import { Card } from "@astryxdesign/core/Card";
import * as stylex from "@stylexjs/stylex";
import { CircleStop, Download, LockKeyhole } from "lucide-react";
import { useState } from "react";
import { ErrorMarker } from "../ErrorMarker";
import { AuthAttempt } from "./AuthAttempt";
import { ProviderStatus } from "./ProviderStatus";
import { statusCopy } from "./statusCopy";
import type { LocalModelSetupView, OnboardingStatus, ProviderAuthAttemptView } from "./types";
import { formatBytes, formatGigabytes, installationProgress } from "../settings/localModelMetadata";

export const PROVIDER_AUTH_POLL_INTERVAL_MS = 5_000;

export function Onboarding({
  onboarding,
  localSetup,
  localSetupLoading,
  localSetupError,
  localSaving,
  localSaveError,
  attempt,
  error,
  onConnect,
  onInstallLocal,
  onCancelLocal,
  onRetry
}: {
  onboarding: OnboardingStatus;
  localSetup: LocalModelSetupView | null;
  localSetupLoading: boolean;
  localSetupError: string | null;
  localSaving: boolean;
  localSaveError: string | null;
  attempt: ProviderAuthAttemptView | null;
  error: string | null;
  onConnect: () => void;
  onInstallLocal: (modelId: string, file?: string | null) => void;
  onCancelLocal: (installationId: string) => void;
  onRetry: () => void;
}) {
  const [mode, setMode] = useState<"local" | "cloud">("local");
  const step = onboarding.steps.find((candidate) => candidate.id === "connect_provider_account");
  const providerName = step?.displayName ?? "provider";
  const providerStatus = step?.providerAccountStatus;
  const waiting = attempt?.status === "STARTING" || attempt?.status === "WAITING_FOR_USER";
  const complete = attempt?.status === "COMPLETED";
  const failed = attempt ? isRetryableTerminalStatus(attempt.status) : false;
  const canStart = !attempt && !complete;
  const recommendation = localSetup?.recommendedModel ?? null;
  const installation = localSetup?.installation ?? null;
  const progress = installation ? installationProgress(installation) : null;
  const transferActive =
    installation?.status === "QUEUED" ||
    installation?.status === "DOWNLOADING" ||
    installation?.status === "VERIFYING";

  if (mode === "cloud") {
    return (
      <section {...stylex.props(styles.root)} aria-label="Noema onboarding">
        <Card {...stylex.props(styles.card)} padding={4}>
          <p {...stylex.props(styles.eyebrow)}>Cloud provider</p>
          <h1 {...stylex.props(styles.title)}>Connect {providerName}</h1>
          <p {...stylex.props(styles.description)}>
            Connect a provider account now. You can install a local model later from Settings.
          </p>

          {providerStatus ? <ProviderStatus status={providerStatus} /> : null}

          {canStart ? (
            <div {...stylex.props(styles.actions)}>
              <Button type="button" label={`Connect ${providerName}`} onClick={onConnect}>
                Connect {providerName}
              </Button>
              <Button type="button" variant="secondary" label="Back to local model" onClick={() => setMode("local")}>
                Back to local model
              </Button>
            </div>
          ) : null}

          {waiting ? <AuthAttempt attempt={attempt} /> : null}
          {complete ? <p {...stylex.props(styles.connected)}>Provider connected. Starting chat.</p> : null}
          {failed ? (
            <div {...stylex.props(styles.retry)}>
              <p>{attempt?.errorMessage ?? statusCopy[attempt?.status ?? "FAILED"]}</p>
              <Button type="button" label="Try again" onClick={onRetry}>Try again</Button>
            </div>
          ) : null}
          {error ? <ErrorMarker message={error} /> : null}
        </Card>
      </section>
    );
  }

  return (
    <section {...stylex.props(styles.root)} aria-label="Noema onboarding">
      <Card {...stylex.props(styles.card)} padding={4}>
        <p {...stylex.props(styles.eyebrow)}>Private by default</p>
        <h1 {...stylex.props(styles.title)}>Download and use a local model</h1>
        <p {...stylex.props(styles.description)}>
          Noema will run every current model workload on your machine. You can add cloud providers
          later without changing these selections.
        </p>

        {localSetupLoading ? <p {...stylex.props(styles.description)}>Checking this machine...</p> : null}
        {!localSetupLoading && localSetup && !recommendation && !localSetup.isReady ? (
          <p {...stylex.props(styles.description)}>
            No curated local model fits the detected backend and memory yet. You can continue with a cloud provider.
          </p>
        ) : null}
        {recommendation ? (
          <div {...stylex.props(styles.modelCard)}>
            <div {...stylex.props(styles.modelHeader)}>
              <div>
                <p {...stylex.props(styles.modelLabel)}>Recommended for this machine</p>
                <h2 {...stylex.props(styles.modelName)}>{recommendation.name}</h2>
              </div>
              <LockKeyhole size={20} aria-hidden="true" {...stylex.props(styles.localIcon)} />
            </div>
            <div {...stylex.props(styles.modelMetadata)}>
              {recommendation.selectedBuild ? <span>{formatGigabytes(recommendation.selectedBuild.downloadGb)} download</span> : null}
              <span>{recommendation.license}</span>
              {recommendation.compatibleBackend ? <span>{recommendation.compatibleBackend}</span> : null}
            </div>
            {recommendation.hardwareFit ? (
              <p {...stylex.props(styles.fitExplanation)}>{recommendation.hardwareFit.explanation}</p>
            ) : null}
            {progress !== null && installation?.status !== "INSTALLED" ? (
              <div {...stylex.props(styles.progressBlock)}>
                <progress {...stylex.props(styles.progress)} value={progress} max={1} />
                <span {...stylex.props(styles.progressCopy)}>
                  {formatBytes(installation?.completedBytes ?? 0)} of {formatBytes(installation?.totalBytes ?? 0)}
                </span>
              </div>
            ) : null}
            {installation?.status === "VERIFYING" ? <p {...stylex.props(styles.progressCopy)}>Verifying model integrity...</p> : null}
            {installation?.errorMessage ? <ErrorMarker message={installation.errorMessage} /> : null}
          </div>
        ) : null}

        {localSetup?.isReady ? <p {...stylex.props(styles.connected)}>Local model ready. Starting chat.</p> : null}

        <div {...stylex.props(styles.actions)}>
          {recommendation && !transferActive && !localSetup?.isReady ? (
            <Button
              type="button"
              label={`Download ${recommendation.name}`}
              icon={<Download size={16} aria-hidden="true" />}
              isDisabled={localSaving || !recommendation.selectedBuild}
              onClick={() => onInstallLocal(recommendation.modelId, recommendation.selectedBuild?.file)}
            >
              Download {recommendation.name}
            </Button>
          ) : null}
          {transferActive && installation ? (
            <Button
              type="button"
              variant="secondary"
              label="Cancel download"
              icon={<CircleStop size={16} aria-hidden="true" />}
              isDisabled={localSaving}
              onClick={() => onCancelLocal(installation.installationId)}
            >
              Cancel download
            </Button>
          ) : null}
          <Button
            type="button"
            variant="secondary"
            label="Use a cloud provider instead"
            isDisabled={localSaving}
            onClick={() => setMode("cloud")}
          >
            Use a cloud provider instead
          </Button>
        </div>

        {localSetupError ? <ErrorMarker message={localSetupError} /> : null}
        {localSaveError ? <ErrorMarker message={localSaveError} /> : null}
      </Card>
    </section>
  );
}

const styles = stylex.create({
  root: {
    display: "grid",
    width: "min(760px, 100%)",
    minHeight: "100%",
    alignContent: "center",
    marginInline: "auto",
    padding: "34px 24px",
    "@media (max-width: 760px)": {
      alignContent: "start",
      padding: "28px 20px"
    }
  },
  card: {
    display: "grid",
    minWidth: 0,
    gap: 14,
    paddingTop: 18,
    paddingBottom: 18
  },
  eyebrow: {
    margin: 0,
    fontFamily: "var(--font-mono)",
    fontSize: 11,
    letterSpacing: "0.12em",
    color: "var(--text-accent)",
    textTransform: "uppercase"
  },
  title: {
    margin: 0,
    fontFamily: "var(--font-heading)",
    fontSize: 34,
    lineHeight: 1.1,
    letterSpacing: 0,
    color: "var(--foreground)",
    overflowWrap: "anywhere",
    "@media (max-width: 760px)": {
      fontSize: 30
    }
  },
  description: {
    margin: 0,
    maxWidth: 560,
    color: "var(--muted-foreground)",
    overflowWrap: "anywhere"
  },
  connected: {
    margin: 0,
    color: "var(--pine-700)"
  },
  retry: {
    display: "grid",
    gap: 10
  },
  actions: { display: "flex", flexWrap: "wrap", gap: 10 },
  modelCard: { display: "grid", gap: 12, borderWidth: 1, borderStyle: "solid", borderColor: "var(--pine-100)", borderRadius: 10, backgroundColor: "color-mix(in srgb, var(--pine-50) 50%, white)", padding: 16 },
  modelHeader: { display: "flex", justifyContent: "space-between", alignItems: "start", gap: 16 },
  modelLabel: { margin: 0, fontFamily: "var(--font-mono)", fontSize: 10, letterSpacing: "0.1em", textTransform: "uppercase", color: "var(--pine-700)" },
  modelName: { margin: 0, fontFamily: "var(--font-heading)", fontSize: 22, lineHeight: 1.25, color: "var(--foreground)" },
  localIcon: { color: "var(--pine-600)" },
  modelMetadata: { display: "flex", flexWrap: "wrap", gap: 8, fontFamily: "var(--font-mono)", fontSize: 12, color: "var(--muted-foreground)" },
  fitExplanation: { margin: 0, maxWidth: 560, fontSize: 14, lineHeight: 1.5, color: "var(--foreground)" },
  progressBlock: { display: "grid", gap: 6 },
  progress: { width: "100%", height: 7, accentColor: "var(--pine-500)" },
  progressCopy: { margin: 0, fontSize: 12, color: "var(--muted-foreground)" }
});

function isRetryableTerminalStatus(status: ProviderAuthAttemptView["status"]) {
  return status === "FAILED" || status === "EXPIRED" || status === "CANCELLED";
}

export function isProviderAuthAttemptPending(status: ProviderAuthAttemptView["status"]) {
  return status === "STARTING" || status === "WAITING_FOR_USER";
}

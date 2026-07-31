import { Button } from "@astryxdesign/core/Button";
import { Card } from "@astryxdesign/core/Card";
import { Collapsible } from "@astryxdesign/core/Collapsible";
import { HStack, VStack } from "@astryxdesign/core/Stack";
import { TextInput } from "@astryxdesign/core/TextInput";
import * as stylex from "@stylexjs/stylex";
import { CircleStop, Download, LockKeyhole } from "lucide-react";
import { useState } from "react";
import { ErrorMarker } from "../ErrorMarker";
import { formatBytes, formatGigabytes, installationProgress } from "../settings/localModelMetadata";
import { AuthAttempt } from "./AuthAttempt";
import { statusCopy } from "./statusCopy";
import type {
  LocalModelSetupView,
  OnboardingConnectedAccount,
  OnboardingProviderCatalog,
  OnboardingStatus,
  ProviderAuthAttemptView
} from "./types";

type CloudAuthMethod = "OAUTH_PKCE" | "OAUTH_DEVICE_CODE";

export function Onboarding({
  onboarding,
  providerCatalog,
  connectedAccounts,
  localSetup,
  localSetupLoading,
  localSetupError,
  localSaving,
  localSaveError,
  providerSaving,
  attempt,
  error,
  modelSetupAccountId,
  modelSetupLoading,
  onConnect,
  onContinue,
  onConnectOpenRouterApiKey,
  onCancelProviderAuth,
  onInstallLocal,
  onCancelLocal,
  onRetry
}: {
  onboarding: OnboardingStatus;
  providerCatalog: OnboardingProviderCatalog;
  connectedAccounts: readonly OnboardingConnectedAccount[];
  localSetup: LocalModelSetupView | null;
  localSetupLoading: boolean;
  localSetupError: string | null;
  localSaving: boolean;
  localSaveError: string | null;
  providerSaving: boolean;
  attempt: ProviderAuthAttemptView | null;
  error: string | null;
  modelSetupAccountId: string | null;
  modelSetupLoading: boolean;
  onConnect: (providerKind: string, method: CloudAuthMethod) => void;
  onContinue: (providerAccountId: string) => void;
  onConnectOpenRouterApiKey: (secret: string) => void;
  onCancelProviderAuth: () => void;
  onInstallLocal: (modelId: string, file?: string | null) => void;
  onCancelLocal: (installationId: string) => void;
  onRetry: () => void;
}) {
  const [apiKeyOpen, setApiKeyOpen] = useState(false);
  const [apiKey, setApiKey] = useState("");
  const openRouter = providerCatalog.find((entry) => entry.providerKind === "openrouter");
  const codex = providerCatalog.find((entry) => entry.providerKind === "codex");
  const connected = (providerKind: string) => connectedAccounts.find(
    (account) => account.providerKind === providerKind && account.status === "AUTHENTICATED"
  );
  const localAccount = connected("local_models");
  const openRouterAccount = connected("openrouter");
  const codexAccount = connected("codex");
  const recommendation = localSetup?.recommendedModel ?? null;
  const installation = localSetup?.installation ?? null;
  const progress = installation ? installationProgress(installation) : null;
  const transferActive =
    installation?.status === "QUEUED" ||
    installation?.status === "DOWNLOADING" ||
    installation?.status === "VERIFYING";
  const authPending = attempt?.status === "STARTING" || attempt?.status === "WAITING_FOR_USER";
  const authFailed = attempt ? isRetryableTerminalStatus(attempt.status) : false;
  const modelSetupProviderKind = modelSetupLoading
    ? connectedAccounts.find((account) => account.providerAccountId === modelSetupAccountId)
        ?.providerKind
    : null;
  const activeChoice = transferActive
    ? "local_models"
    : authPending
      ? attempt.providerKind
      : modelSetupProviderKind ?? (apiKeyOpen || providerSaving ? "openrouter" : null);
  const choiceDisabled = (providerKind: string) =>
    activeChoice !== null && activeChoice !== providerKind;

  return (
    <VStack as="section" {...stylex.props(styles.root)} aria-label="Noema onboarding" gap={5}>
      <VStack gap={2} hAlign="center">
        <p {...stylex.props(styles.eyebrow)}>Choose your model provider</p>
        <h1 {...stylex.props(styles.title)}>Set up Noema</h1>
        <p {...stylex.props(styles.description)}>
          Start locally, connect OpenRouter, or use Codex. You can add the other options later.
        </p>
      </VStack>

      <HStack gap={3} vAlign="stretch" {...stylex.props(styles.choices)}>
        <Card padding={4} {...stylex.props(styles.choice)}>
          <VStack gap={3} height="100%">
            <VStack gap={1.5}>
              <HStack justify="between" vAlign="start" gap={3}>
                <h2 {...stylex.props(styles.choiceTitle)}>Local</h2>
                <LockKeyhole size={20} aria-hidden="true" {...stylex.props(styles.localIcon)} />
              </HStack>
              <p {...stylex.props(styles.choiceDescription)}>
                Download a curated model and keep model traffic on this machine.
              </p>
            </VStack>

            {localSetupLoading ? <p {...stylex.props(styles.muted)}>Checking this machine…</p> : null}
            {recommendation ? (
              <VStack gap={2}>
                <strong>{recommendation.name}</strong>
                <HStack gap={2} wrap="wrap" {...stylex.props(styles.metadata)}>
                  {recommendation.selectedBuild ? (
                    <span>{formatGigabytes(recommendation.selectedBuild.downloadGb)} download</span>
                  ) : null}
                  <span>{recommendation.license}</span>
                  {recommendation.compatibleBackend ? <span>{recommendation.compatibleBackend}</span> : null}
                </HStack>
                {recommendation.hardwareFit ? (
                  <p {...stylex.props(styles.muted)}>{recommendation.hardwareFit.explanation}</p>
                ) : null}
              </VStack>
            ) : null}
            {!localSetupLoading && localSetup && !recommendation && !localSetup.isReady ? (
              <p {...stylex.props(styles.muted)}>No curated model fits this machine yet.</p>
            ) : null}
            {progress !== null && installation?.status !== "INSTALLED" ? (
              <VStack gap={1.5}>
                <progress {...stylex.props(styles.progress)} value={progress} max={1} />
                <span {...stylex.props(styles.muted)}>
                  {formatBytes(installation?.completedBytes ?? 0)} of {formatBytes(installation?.totalBytes ?? 0)}
                </span>
              </VStack>
            ) : null}
            {installation?.status === "VERIFYING" ? <p {...stylex.props(styles.muted)}>Verifying model integrity…</p> : null}
            {installation?.errorMessage ? <ErrorMarker message={installation.errorMessage} /> : null}

            <VStack gap={2} {...stylex.props(styles.actions)}>
              {recommendation && !transferActive && !localSetup?.isReady ? (
                <Button
                  type="button"
                  variant="secondary"
                  label={`Download ${recommendation.name}`}
                  icon={<Download size={16} aria-hidden="true" />}
                  isDisabled={localSaving || !recommendation.selectedBuild || choiceDisabled("local_models")}
                  onClick={() => onInstallLocal(recommendation.modelId, recommendation.selectedBuild?.file)}
                />
              ) : null}
              {transferActive && installation ? (
                <Button
                  type="button"
                  variant="secondary"
                  label="Cancel download"
                  icon={<CircleStop size={16} aria-hidden="true" />}
                  isDisabled={localSaving}
                  onClick={() => onCancelLocal(installation.installationId)}
                />
              ) : null}
              {localSetup?.isReady && localAccount ? (
                <Button
                  type="button"
                  variant="primary"
                  label="Continue with Local"
                  isLoading={
                    modelSetupLoading && modelSetupAccountId === localAccount.providerAccountId
                  }
                  isDisabled={choiceDisabled("local_models")}
                  onClick={() => onContinue(localAccount.providerAccountId)}
                />
              ) : null}
            </VStack>
          </VStack>
        </Card>

        <Card padding={4} {...stylex.props(styles.choice)}>
          <VStack gap={3} height="100%">
            <VStack gap={1.5}>
              <h2 {...stylex.props(styles.choiceTitle)}>{openRouter?.displayName ?? "OpenRouter"}</h2>
              <p {...stylex.props(styles.choiceDescription)}>
                Connect once with PKCE and use the models allowed by your OpenRouter account.
              </p>
            </VStack>
            {openRouterAccount ? (
              <VStack gap={2} {...stylex.props(styles.actions)}>
                <p {...stylex.props(styles.connected)}>Connected.</p>
                <Button
                  type="button"
                  variant="primary"
                  label="Continue with OpenRouter"
                  isLoading={
                    modelSetupLoading &&
                    modelSetupAccountId === openRouterAccount.providerAccountId
                  }
                  isDisabled={choiceDisabled("openrouter")}
                  onClick={() => onContinue(openRouterAccount.providerAccountId)}
                />
              </VStack>
            ) : attempt?.providerKind === "openrouter" && authPending ? (
              <AuthAttempt attempt={attempt} onCancel={onCancelProviderAuth} />
            ) : (
              <VStack gap={2} {...stylex.props(styles.actions)}>
                <Button
                  type="button"
                  variant="primary"
                  label="Connect OpenRouter"
                  isDisabled={!openRouter || choiceDisabled("openrouter") || providerSaving}
                  onClick={() => onConnect("openrouter", "OAUTH_PKCE")}
                />
                <Collapsible
                  trigger="Use an API key instead"
                  defaultIsOpen={false}
                  isOpen={apiKeyOpen}
                  onOpenChange={setApiKeyOpen}
                >
                  <VStack gap={2}>
                    <TextInput
                      label="OpenRouter API key"
                      type="password"
                      value={apiKey}
                      onChange={setApiKey}
                    />
                    <Button
                      type="button"
                      variant="secondary"
                      label="Connect with API key"
                      isLoading={providerSaving}
                      isDisabled={!apiKey.trim() || providerSaving}
                      onClick={() => onConnectOpenRouterApiKey(apiKey)}
                    />
                  </VStack>
                </Collapsible>
              </VStack>
            )}
            {attempt?.providerKind === "openrouter" && authFailed ? (
              <RetryMessage attempt={attempt} onRetry={onRetry} />
            ) : null}
          </VStack>
        </Card>

        <Card padding={4} {...stylex.props(styles.choice)}>
          <VStack gap={3} height="100%">
            <VStack gap={1.5}>
              <h2 {...stylex.props(styles.choiceTitle)}>{codex?.displayName ?? "Codex"}</h2>
              <p {...stylex.props(styles.choiceDescription)}>
                Sign in with the existing Codex device authorization flow.
              </p>
            </VStack>
            {codexAccount ? (
              <VStack gap={2} {...stylex.props(styles.actions)}>
                <p {...stylex.props(styles.connected)}>Connected.</p>
                <Button
                  type="button"
                  variant="primary"
                  label="Continue with Codex"
                  isLoading={
                    modelSetupLoading && modelSetupAccountId === codexAccount.providerAccountId
                  }
                  isDisabled={choiceDisabled("codex")}
                  onClick={() => onContinue(codexAccount.providerAccountId)}
                />
              </VStack>
            ) : attempt?.providerKind === "codex" && authPending ? (
              <AuthAttempt attempt={attempt} onCancel={onCancelProviderAuth} />
            ) : (
              <VStack gap={2} {...stylex.props(styles.actions)}>
                <Button
                  type="button"
                  variant="secondary"
                  label="Connect Codex"
                  isDisabled={!codex || choiceDisabled("codex")}
                  onClick={() => onConnect("codex", "OAUTH_DEVICE_CODE")}
                />
              </VStack>
            )}
            {attempt?.providerKind === "codex" && authFailed ? (
              <RetryMessage attempt={attempt} onRetry={onRetry} />
            ) : null}
          </VStack>
        </Card>
      </HStack>

      {localSetupError ? <ErrorMarker message={localSetupError} /> : null}
      {localSaveError ? <ErrorMarker message={localSaveError} /> : null}
      {error ? <ErrorMarker message={error} /> : null}
      {onboarding.isUserOnboarded ? <p {...stylex.props(styles.connected)}>Setup complete. Opening chat.</p> : null}
    </VStack>
  );
}

function RetryMessage({
  attempt,
  onRetry
}: {
  attempt: ProviderAuthAttemptView;
  onRetry: () => void;
}) {
  return (
    <VStack gap={2}>
      <p {...stylex.props(styles.muted)}>{attempt.errorMessage ?? statusCopy[attempt.status]}</p>
      <Button type="button" variant="secondary" label="Try again" onClick={onRetry} />
    </VStack>
  );
}

const styles = stylex.create({
  root: {
    width: "min(1100px, 100%)",
    minHeight: "100%",
    justifyContent: "center",
    marginInline: "auto",
    padding: "var(--spacing-8) var(--spacing-6)",
    "@media (max-width: 760px)": {
      justifyContent: "flex-start",
      padding: "var(--spacing-6) var(--spacing-4)"
    }
  },
  eyebrow: {
    margin: "var(--spacing-0)",
    fontFamily: "var(--font-mono)",
    color: "var(--text-accent)",
    textTransform: "uppercase",
    letterSpacing: "0.1em"
  },
  title: {
    margin: "var(--spacing-0)",
    fontFamily: "var(--font-heading)",
    color: "var(--foreground)"
  },
  description: {
    margin: "var(--spacing-0)",
    maxWidth: 620,
    textAlign: "center",
    color: "var(--muted-foreground)"
  },
  choices: {
    width: "100%",
    "@media (max-width: 760px)": {
      flexDirection: "column"
    }
  },
  choice: {
    flex: "1 1 0",
    minWidth: 0
  },
  choiceTitle: {
    margin: "var(--spacing-0)",
    fontFamily: "var(--font-heading)",
    color: "var(--foreground)"
  },
  choiceDescription: {
    margin: "var(--spacing-0)",
    color: "var(--muted-foreground)"
  },
  localIcon: { color: "var(--pine-600)" },
  metadata: { fontFamily: "var(--font-mono)", color: "var(--muted-foreground)" },
  muted: { margin: "var(--spacing-0)", color: "var(--muted-foreground)" },
  progress: { width: "100%", accentColor: "var(--pine-500)" },
  connected: { margin: "var(--spacing-0)", color: "var(--pine-700)" },
  actions: { marginTop: "auto" }
});

function isRetryableTerminalStatus(status: ProviderAuthAttemptView["status"]) {
  return status === "FAILED" || status === "EXPIRED" || status === "CANCELLED";
}

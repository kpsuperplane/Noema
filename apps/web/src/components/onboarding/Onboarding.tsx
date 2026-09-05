import { ProgressBar } from "@astryxdesign/core/ProgressBar";
import { Button } from "@astryxdesign/core/Button";
import { HStack, VStack } from "@astryxdesign/core/Stack";
import { TextInput } from "@astryxdesign/core/TextInput";
import * as stylex from "@stylexjs/stylex";
import { ChevronRight } from "lucide-react";
import { useState } from "react";
import { reserveExternalAuthNavigation } from "@/graphql/externalUrls";
import { ErrorMarker } from "../ErrorMarker";
import { ListCardButton } from "../ListCardLink";
import { SetupCard, SetupActions, SetupNote } from "../shell/SetupFrame";
import {
  formatBytes,
  formatGigabytes,
  installationProgress
} from "../settings/localModelMetadata";
import { AuthAttempt } from "./AuthAttempt";
import type {
  LocalModelSetupView,
  OnboardingConnectedAccount,
  OnboardingProviderCatalog,
  ProviderAuthAttemptView
} from "./types";

type CloudAuthMethod = "OAUTH_PKCE" | "OAUTH_DEVICE_CODE";
type ProviderKind = "local_models" | "openrouter" | "codex";

export function Onboarding({
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
  onRetry,
  onRetryLocal
}: {
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
  onConnect: (
    providerKind: string,
    method: CloudAuthMethod
  ) => Promise<string | null>;
  onContinue: (providerAccountId: string) => void;
  onConnectOpenRouterApiKey: (secret: string) => void;
  onCancelProviderAuth: () => void;
  onInstallLocal: (modelId: string, file?: string | null) => void;
  onCancelLocal: (installationId: string) => void;
  onRetry: () => void;
  onRetryLocal: () => void;
}) {
  const [apiKeyOpen, setApiKeyOpen] = useState(false);
  const [apiKey, setApiKey] = useState("");
  const [setupProviderKind, setSetupProviderKind] =
    useState<ProviderKind | null>(null);
  const openRouter = providerCatalog.find(
    (entry) => entry.providerKind === "openrouter"
  );
  const codex = providerCatalog.find((entry) => entry.providerKind === "codex");
  const connected = (providerKind: string) =>
    connectedAccounts.find(
      (account) =>
        account.providerKind === providerKind &&
        account.status === "AUTHENTICATED"
    );
  const localAccount = connected("local_models");
  const codexAccount = connected("codex");
  const recommendation = localSetup?.recommendedModel ?? null;
  const installation = localSetup?.installation ?? null;
  const progress = installation ? installationProgress(installation) : null;
  const transferActive =
    installation?.status === "QUEUED" ||
    installation?.status === "DOWNLOADING" ||
    installation?.status === "VERIFYING";
  const authPending =
    attempt?.status === "STARTING" || attempt?.status === "WAITING_FOR_USER";
  const authFailed = attempt
    ? isRetryableTerminalStatus(attempt.status)
    : false;
  const modelSetupProviderKind = modelSetupLoading
    ? toProviderKind(
        connectedAccounts.find(
          (account) => account.providerAccountId === modelSetupAccountId
        )?.providerKind
      )
    : null;
  const activeProviderKind: ProviderKind | null = transferActive
    ? "local_models"
    : authPending
      ? toProviderKind(attempt.providerKind)
      : (modelSetupProviderKind ?? (providerSaving ? "openrouter" : null));
  const visibleProviderKind = activeProviderKind ?? setupProviderKind;

  async function startConnection(kind: "codex" | "openrouter") {
    const authNavigation = reserveExternalAuthNavigation();
    const verificationUrl = await onConnect(
      kind,
      kind === "codex" ? "OAUTH_DEVICE_CODE" : "OAUTH_PKCE"
    );
    if (verificationUrl) {
      await authNavigation.open(verificationUrl);
    } else {
      authNavigation.cancel();
    }
  }

  function selectProvider(providerKind: ProviderKind) {
    setSetupProviderKind(providerKind);
    if (providerKind !== "codex") {
      return;
    }
    onRetry();
    if (codexAccount) {
      onContinue(codexAccount.providerAccountId);
      return;
    }
    void startConnection("codex");
  }

  const account = visibleProviderKind
    ? connected(visibleProviderKind)
    : undefined;
  const loadingModels =
    modelSetupLoading && account?.providerAccountId === modelSetupAccountId;
  const loadFailed = Boolean(account && error && !loadingModels);
  const pending = authPending && attempt?.providerKind === visibleProviderKind;
  const failed = authFailed && attempt?.providerKind === visibleProviderKind;
  const local = visibleProviderKind === "local_models";
  const title =
    visibleProviderKind === null
      ? "Choose how Noema thinks"
      : loadingModels
        ? "Finding your models"
        : loadFailed
          ? "Your models could not load"
          : pending
            ? visibleProviderKind === "codex"
              ? "Finish signing in to Codex"
              : "Finish connecting OpenRouter"
            : failed
              ? attempt.status === "EXPIRED"
                ? "Sign-in time ran out"
                : "Sign-in did not finish"
              : local
                ? installation?.status === "VERIFYING"
                  ? "Checking your download"
                  : transferActive
                    ? "Your model is on its way"
                    : localSetup?.isReady
                      ? "Your local model is ready"
                      : installation?.status === "FAILED"
                        ? "The download stopped"
                        : !localSetupLoading && !recommendation
                          ? "Local models are unavailable"
                          : "Meet your local model"
                : visibleProviderKind === "codex"
                  ? "Sign in to Codex"
                  : "Connect OpenRouter";
  const intro =
    visibleProviderKind === null
      ? "Start with one. Add others later."
      : loadingModels
        ? "Your account is connected."
        : loadFailed
          ? "Your account is still connected."
          : pending
            ? "Keep this page open while you sign in."
            : failed
              ? "Start a new sign-in to continue."
              : local
                ? transferActive
                  ? "You can leave this page open."
                  : localSetup?.isReady
                    ? "One last review, then you’re ready."
                    : "Run models on your Noema server."
                : visibleProviderKind === "codex"
                  ? "Use your ChatGPT account."
                  : "Choose from models across providers.";
  function back() {
    setSetupProviderKind(null);
    setApiKeyOpen(false);
    setApiKey("");
    onRetry();
  }
  function restart() {
    onRetry();
    if (visibleProviderKind === "codex") void startConnection("codex");
    else void startConnection("openrouter");
  }
  return (
    <SetupCard title={title} intro={intro}>
      {visibleProviderKind === null ? (
        <VStack gap={2} role="group" aria-label="Model providers">
          <ProviderOption
            kind="local_models"
            label="Local"
            description="Runs on your Noema server. No provider account needed."
            onSelect={selectProvider}
          />
          <ProviderOption
            kind="openrouter"
            label={openRouter?.displayName ?? "OpenRouter"}
            description="Models from many providers. Requires an OpenRouter account."
            isDisabled={!openRouter}
            onSelect={selectProvider}
          />
          <ProviderOption
            kind="codex"
            label={codex?.displayName ?? "Codex"}
            description="OpenAI models. Requires ChatGPT access to Codex."
            isDisabled={!codex}
            onSelect={selectProvider}
          />
        </VStack>
      ) : loadingModels ? (
        <SetupNote>Loading model choices…</SetupNote>
      ) : loadFailed && account ? (
        <SetupActions>
          <Button variant="secondary" label="Change provider" onClick={back} />
          <Button
            variant="primary"
            label="Try again"
            onClick={() => onContinue(account.providerAccountId)}
          />
        </SetupActions>
      ) : pending ? (
        <AuthAttempt
          attempt={attempt}
          onCancel={() => {
            onCancelProviderAuth();
          }}
        />
      ) : failed ? (
        <SetupActions>
          <Button variant="secondary" label="Change provider" onClick={back} />
          <Button
            variant="primary"
            label="Start sign-in again"
            onClick={restart}
          />
        </SetupActions>
      ) : local ? (
        <>
          {localSetupLoading ? (
            <SetupNote>Checking your server…</SetupNote>
          ) : null}
          {recommendation ? (
            <VStack gap={1.5}>
              <strong {...stylex.props(styles.modelName)}>
                {recommendation.name}
              </strong>
              <HStack gap={2} wrap="wrap" {...stylex.props(styles.metadata)}>
                {recommendation.selectedBuild ? (
                  <span>
                    {formatGigabytes(recommendation.selectedBuild.downloadGb)}{" "}
                    download
                  </span>
                ) : null}
                <span>{recommendation.license}</span>
                {recommendation.compatibleBackend ? (
                  <span>{recommendation.compatibleBackend}</span>
                ) : null}
              </HStack>
              {recommendation.selectedBuild?.file ? (
                <details>
                  <summary>Model file</summary>
                  <p>{recommendation.selectedBuild.file}</p>
                </details>
              ) : null}
            </VStack>
          ) : null}
          {!localSetupLoading &&
          localSetup &&
          !recommendation &&
          !localSetup.isReady ? (
            <SetupNote>
              Try a provider now. You can set up a local model later.
            </SetupNote>
          ) : null}
          {transferActive && installation ? (
            <VStack gap={2} role="status">
              {installation.status === "VERIFYING" ? (
                <SetupNote>Checking the model before it runs.</SetupNote>
              ) : (
                <>
                  <ProgressBar
                    label="Model download"
                    isLabelHidden
                    value={progress ?? 0}
                    isIndeterminate={progress === null}
                    max={1}
                  />
                  <SetupNote>
                    {formatBytes(installation.completedBytes)}
                    {installation.totalBytes
                      ? ` of ${formatBytes(installation.totalBytes)}`
                      : " downloaded"}
                  </SetupNote>
                </>
              )}
            </VStack>
          ) : null}
          {installation?.errorMessage ? (
            <>
              <ErrorMarker message="Noema could not finish the download. Check the details before you try again." />
              <details>
                <summary>Download error details</summary>
                <p>{installation.errorMessage}</p>
              </details>
            </>
          ) : null}
          <SetupActions>
            {transferActive && installation ? (
              <Button
                variant="secondary"
                label="Cancel download"
                isDisabled={localSaving}
                onClick={() => onCancelLocal(installation.installationId)}
              />
            ) : (
              <Button
                variant="secondary"
                label="Change provider"
                onClick={back}
              />
            )}
            {localSetup?.isReady && localAccount ? (
              <Button
                variant="primary"
                label="Review models"
                onClick={() => onContinue(localAccount.providerAccountId)}
              />
            ) : recommendation && !transferActive ? (
              <Button
                variant="primary"
                label={
                  installation?.status === "FAILED"
                    ? "Try download again"
                    : "Download model"
                }
                isLoading={localSaving}
                isDisabled={localSaving || !recommendation.selectedBuild}
                onClick={() =>
                  onInstallLocal(
                    recommendation.modelId,
                    recommendation.selectedBuild?.file
                  )
                }
              />
            ) : localSetupError ? (
              <Button
                variant="primary"
                label="Check again"
                onClick={onRetryLocal}
              />
            ) : null}
          </SetupActions>
        </>
      ) : account ? (
        <SetupActions>
          <Button variant="secondary" label="Change provider" onClick={back} />
          <Button
            variant="primary"
            label="Review models"
            onClick={() => onContinue(account.providerAccountId)}
          />
        </SetupActions>
      ) : visibleProviderKind === "openrouter" ? (
        <>
          <SetupNote>Sign in to OpenRouter to connect your account.</SetupNote>
          <SetupActions>
            <Button
              variant="secondary"
              label="Change provider"
              isDisabled={providerSaving}
              onClick={back}
            />
            <Button
              variant="primary"
              label="Connect OpenRouter"
              isDisabled={!openRouter || providerSaving}
              onClick={() => startConnection("openrouter")}
            />
          </SetupActions>
          <details
            open={apiKeyOpen}
            onToggle={(event) => setApiKeyOpen(event.currentTarget.open)}
          >
            <summary>Use an API key instead</summary>
            <VStack
              as="form"
              gap={2}
              onSubmit={(event) => {
                event.preventDefault();
                onConnectOpenRouterApiKey(apiKey);
                setApiKey("");
              }}
            >
              <TextInput
                label="OpenRouter API key"
                type="password"
                value={apiKey}
                onChange={setApiKey}
                isDisabled={providerSaving}
              />
              <Button
                type="submit"
                variant="secondary"
                label="Connect with API key"
                isLoading={providerSaving}
                isDisabled={!apiKey.trim() || providerSaving}
              />
            </VStack>
          </details>
        </>
      ) : (
        <SetupActions>
          <Button variant="secondary" label="Change provider" onClick={back} />
          <Button
            variant="primary"
            label="Sign in to Codex"
            onClick={() => void startConnection("codex")}
          />
        </SetupActions>
      )}
      {localSetupError && local ? (
        <ErrorMarker message="Noema could not check local models. Try again." />
      ) : null}
      {localSaveError && local ? (
        <ErrorMarker message={localSaveError} />
      ) : null}
      {error || (failed && attempt.errorMessage) ? (
        <details>
          <summary>Error details</summary>
          <p>{error ?? attempt?.errorMessage}</p>
        </details>
      ) : null}
    </SetupCard>
  );
}

function ProviderOption({
  kind,
  label,
  description,
  isDisabled = false,
  onSelect
}: {
  kind: ProviderKind;
  label: string;
  description: string;
  isDisabled?: boolean;
  onSelect: (kind: ProviderKind) => void;
}) {
  return (
    <ListCardButton disabled={isDisabled} onClick={() => onSelect(kind)}>
      <HStack gap={1.5} vAlign="center">
        <VStack as="span" gap={0.5} {...stylex.props(styles.providerCopy)}>
          <strong {...stylex.props(styles.providerName)}>{label}</strong>
          <span {...stylex.props(styles.providerDescription)}>
            {description}
          </span>
        </VStack>
        <ChevronRight
          size={16}
          aria-hidden="true"
          {...stylex.props(styles.chevron)}
        />
      </HStack>
    </ListCardButton>
  );
}

const styles = stylex.create({
  providerCopy: { flex: 1, minWidth: 0 },
  providerName: { fontSize: 13, fontWeight: 650, lineHeight: 1.35 },
  providerDescription: {
    color: "var(--muted-foreground)",
    fontSize: "var(--font-size-sm)",
    lineHeight: 1.35,
    textWrap: "pretty"
  },
  chevron: { flexShrink: 0, color: "var(--muted-foreground)" },
  modelName: {
    fontFamily: "var(--font-display)",
    fontSize: "var(--font-size-lg)"
  },
  metadata: {
    color: "var(--muted-foreground)",
    fontSize: "var(--font-size-sm)"
  }
});

function isRetryableTerminalStatus(status: ProviderAuthAttemptView["status"]) {
  return status === "FAILED" || status === "EXPIRED" || status === "CANCELLED";
}
function toProviderKind(value: string | null | undefined): ProviderKind | null {
  return value === "local_models" || value === "openrouter" || value === "codex"
    ? value
    : null;
}

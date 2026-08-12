import { Button } from "@astryxdesign/core/Button";
import { Card } from "@astryxdesign/core/Card";
import { HStack, VStack } from "@astryxdesign/core/Stack";
import { TextInput } from "@astryxdesign/core/TextInput";
import * as stylex from "@stylexjs/stylex";
import { ArrowLeft, CircleStop, Download, HardDrive, Route, SquareTerminal } from "lucide-react";
import type { ReactNode } from "react";
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
type ProviderKind = "local_models" | "openrouter" | "codex";

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
  const [selectedProviderKind, setSelectedProviderKind] = useState<ProviderKind | null>(null);
  const [setupProviderKind, setSetupProviderKind] = useState<ProviderKind | null>(null);
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
    ? toProviderKind(
        connectedAccounts.find((account) => account.providerAccountId === modelSetupAccountId)
          ?.providerKind
      )
    : null;
  const activeProviderKind: ProviderKind | null = transferActive
    ? "local_models"
    : authPending
      ? toProviderKind(attempt.providerKind)
      : modelSetupProviderKind ?? (providerSaving ? "openrouter" : null);
  const visibleProviderKind = activeProviderKind ?? setupProviderKind;

  return (
    <VStack
      as="section"
      {...stylex.props(styles.root)}
      aria-label="Noema onboarding"
      data-slot="provider-onboarding"
      gap={6}
    >
      <img
        src="/assets/noema-mark.svg"
        width="64"
        height="64"
        alt=""
        {...stylex.props(styles.logo)}
      />

      {visibleProviderKind === null ? (
        <VStack gap={4} {...stylex.props(styles.stage)}>
          <VStack gap={1.5} hAlign="center">
            <p {...stylex.props(styles.eyebrow)}>Welcome to Noema</p>
            <h1 {...stylex.props(styles.title)}>Choose how Noema runs</h1>
            <p {...stylex.props(styles.description)}>
              Pick one provider to start. You can add the others later.
            </p>
          </VStack>

          <VStack as="fieldset" gap={2} {...stylex.props(styles.providerOptions)}>
            <legend {...stylex.props(styles.visuallyHidden)}>Model provider</legend>
            <ProviderOption
              kind="local_models"
              label="Local"
              description={
                localSetup?.isReady
                  ? "Ready on this Mac. Your model traffic stays here."
                  : "Private on this Mac. Downloads one recommended model."
              }
              icon={<HardDrive size={20} aria-hidden="true" />}
              isSelected={selectedProviderKind === "local_models"}
              onSelect={setSelectedProviderKind}
            />
            <ProviderOption
              kind="openrouter"
              label={openRouter?.displayName ?? "OpenRouter"}
              description={
                openRouterAccount
                  ? "Connected. Use models from your OpenRouter account."
                  : "Use models available through your OpenRouter account."
              }
              icon={<Route size={20} aria-hidden="true" />}
              isSelected={selectedProviderKind === "openrouter"}
              isDisabled={!openRouter}
              onSelect={setSelectedProviderKind}
            />
            <ProviderOption
              kind="codex"
              label={codex?.displayName ?? "Codex"}
              description={
                codexAccount
                  ? "Connected. Continue with your Codex sign-in."
                  : "Use your existing Codex sign-in."
              }
              icon={<SquareTerminal size={20} aria-hidden="true" />}
              isSelected={selectedProviderKind === "codex"}
              isDisabled={!codex}
              onSelect={setSelectedProviderKind}
            />
          </VStack>

          <Button
            {...stylex.props(styles.primaryAction)}
            type="button"
            variant="primary"
            size="lg"
            label="Continue"
            isDisabled={selectedProviderKind === null}
            onClick={() => setSetupProviderKind(selectedProviderKind)}
          />
        </VStack>
      ) : (
        <VStack gap={4} {...stylex.props(styles.stage)}>
          {activeProviderKind === null ? (
            <Button
              {...stylex.props(styles.backButton)}
              type="button"
              variant="ghost"
              size="sm"
              label="Choose another provider"
              icon={<ArrowLeft size={16} aria-hidden="true" />}
              onClick={() => {
                setSetupProviderKind(null);
                setApiKeyOpen(false);
              }}
            />
          ) : null}

          <VStack gap={1.5}>
            <p {...stylex.props(styles.eyebrow)}>Provider setup</p>
            <h1 {...stylex.props(styles.title)}>{providerName(visibleProviderKind, openRouter, codex)}</h1>
            <p {...stylex.props(styles.setupDescription)}>
              {providerDescription(visibleProviderKind)}
            </p>
          </VStack>

          <Card padding={5} {...stylex.props(styles.setupCard)}>
            {visibleProviderKind === "local_models" ? (
              <VStack gap={3}>
                {localSetupLoading ? <p {...stylex.props(styles.muted)}>Checking this machine…</p> : null}
                {recommendation ? (
                  <VStack gap={2}>
                    <p {...stylex.props(styles.sectionLabel)}>Recommended for this machine</p>
                    <strong {...stylex.props(styles.modelName)}>{recommendation.name}</strong>
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
                    <span {...stylex.props(styles.muted, styles.progressAmount)}>
                      {formatBytes(installation?.completedBytes ?? 0)} of {formatBytes(installation?.totalBytes ?? 0)}
                    </span>
                  </VStack>
                ) : null}
                {installation?.status === "VERIFYING" ? (
                  <p {...stylex.props(styles.muted)}>Verifying model integrity…</p>
                ) : null}
                {installation?.errorMessage ? <ErrorMarker message={installation.errorMessage} /> : null}

                {recommendation && !transferActive && !localSetup?.isReady ? (
                  <Button
                    type="button"
                    variant="primary"
                    size="lg"
                    label={`Download ${recommendation.name}`}
                    icon={<Download size={16} aria-hidden="true" />}
                    isDisabled={localSaving || !recommendation.selectedBuild}
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
                    size="lg"
                    label="Continue with Local"
                    isLoading={
                      modelSetupLoading && modelSetupAccountId === localAccount.providerAccountId
                    }
                    onClick={() => onContinue(localAccount.providerAccountId)}
                  />
                ) : null}
              </VStack>
            ) : null}

            {visibleProviderKind === "openrouter" ? (
              <VStack gap={3}>
                {openRouterAccount ? (
                  <VStack gap={2}>
                    <p {...stylex.props(styles.connected)}>OpenRouter is connected.</p>
                    <Button
                      type="button"
                      variant="primary"
                      size="lg"
                      label="Continue with OpenRouter"
                      isLoading={
                        modelSetupLoading &&
                        modelSetupAccountId === openRouterAccount.providerAccountId
                      }
                      onClick={() => onContinue(openRouterAccount.providerAccountId)}
                    />
                  </VStack>
                ) : attempt?.providerKind === "openrouter" && authPending ? (
                  <AuthAttempt attempt={attempt} onCancel={onCancelProviderAuth} />
                ) : (
                  <VStack gap={2}>
                    <Button
                      type="button"
                      variant="primary"
                      size="lg"
                      label="Connect OpenRouter"
                      isDisabled={!openRouter || providerSaving}
                      onClick={() => onConnect("openrouter", "OAUTH_PKCE")}
                    />
                    <Button
                      {...stylex.props(styles.apiKeyTrigger)}
                      type="button"
                      variant="ghost"
                      label={apiKeyOpen ? "Hide API key setup" : "Use an API key instead"}
                      aria-controls="openrouter-api-key-setup"
                      aria-expanded={apiKeyOpen}
                      onClick={() => setApiKeyOpen((current) => !current)}
                    />
                    {apiKeyOpen ? (
                      <VStack id="openrouter-api-key-setup" gap={2}>
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
                    ) : null}
                  </VStack>
                )}
                {attempt?.providerKind === "openrouter" && authFailed ? (
                  <RetryMessage attempt={attempt} onRetry={onRetry} />
                ) : null}
              </VStack>
            ) : null}

            {visibleProviderKind === "codex" ? (
              <VStack gap={3}>
                {codexAccount ? (
                  <VStack gap={2}>
                    <p {...stylex.props(styles.connected)}>Codex is connected.</p>
                    <Button
                      type="button"
                      variant="primary"
                      size="lg"
                      label="Continue with Codex"
                      isLoading={
                        modelSetupLoading && modelSetupAccountId === codexAccount.providerAccountId
                      }
                      onClick={() => onContinue(codexAccount.providerAccountId)}
                    />
                  </VStack>
                ) : attempt?.providerKind === "codex" && authPending ? (
                  <AuthAttempt attempt={attempt} onCancel={onCancelProviderAuth} />
                ) : (
                  <Button
                    type="button"
                    variant="primary"
                    size="lg"
                    label="Connect Codex"
                    isDisabled={!codex}
                    onClick={() => onConnect("codex", "OAUTH_DEVICE_CODE")}
                  />
                )}
                {attempt?.providerKind === "codex" && authFailed ? (
                  <RetryMessage attempt={attempt} onRetry={onRetry} />
                ) : null}
              </VStack>
            ) : null}
          </Card>
        </VStack>
      )}

      {localSetupError ? <ErrorMarker message={localSetupError} /> : null}
      {localSaveError ? <ErrorMarker message={localSaveError} /> : null}
      {error ? <ErrorMarker message={error} /> : null}
      {onboarding.isUserOnboarded ? <p {...stylex.props(styles.connected)}>Setup complete. Opening chat.</p> : null}
    </VStack>
  );
}

function ProviderOption({
  kind,
  label,
  description,
  icon,
  isSelected,
  isDisabled = false,
  onSelect
}: {
  kind: ProviderKind;
  label: string;
  description: string;
  icon: ReactNode;
  isSelected: boolean;
  isDisabled?: boolean;
  onSelect: (kind: ProviderKind) => void;
}) {
  return (
    <HStack
      as="label"
      gap={3}
      vAlign="center"
      {...stylex.props(
        styles.providerOption,
        isSelected && styles.providerOptionSelected,
        isDisabled && styles.providerOptionDisabled
      )}
    >
      <HStack as="span" hAlign="center" vAlign="center" {...stylex.props(styles.providerIcon)}>
        {icon}
      </HStack>
      <VStack as="span" gap={0.5} {...stylex.props(styles.providerCopy)}>
        <strong {...stylex.props(styles.providerName)}>{label}</strong>
        <span {...stylex.props(styles.providerDescription)}>{description}</span>
      </VStack>
      <input
        {...stylex.props(styles.providerRadio)}
        type="radio"
        name="model-provider"
        value={kind}
        checked={isSelected}
        disabled={isDisabled}
        onChange={() => onSelect(kind)}
      />
    </HStack>
  );
}

function providerName(
  providerKind: ProviderKind,
  openRouter: OnboardingProviderCatalog[number] | undefined,
  codex: OnboardingProviderCatalog[number] | undefined
) {
  if (providerKind === "openrouter") {
    return openRouter?.displayName ?? "OpenRouter";
  }
  if (providerKind === "codex") {
    return codex?.displayName ?? "Codex";
  }
  return "Local";
}

function providerDescription(providerKind: ProviderKind) {
  if (providerKind === "openrouter") {
    return "Use models available through your OpenRouter account.";
  }
  if (providerKind === "codex") {
    return "Use your existing Codex sign-in.";
  }
  return "Download a recommended model and keep model traffic on this Mac.";
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
    width: "min(640px, 100%)",
    minHeight: "100%",
    alignItems: "center",
    justifyContent: "center",
    marginInline: "auto",
    padding: "var(--spacing-8) var(--spacing-6)",
    "@media (max-width: 760px)": {
      justifyContent: "flex-start",
      padding: "var(--spacing-6) var(--spacing-4)"
    }
  },
  logo: {
    display: "block",
    flexShrink: 0,
    borderRadius: 15,
    boxShadow:
      "0 2px 3px color-mix(in srgb, black 8%, transparent), 0 12px 30px color-mix(in srgb, var(--pine-500) 18%, transparent)"
  },
  stage: {
    width: "100%"
  },
  eyebrow: {
    margin: "var(--spacing-0)",
    color: "var(--muted-foreground)",
    fontSize: "var(--font-size-sm)",
    fontWeight: 600
  },
  title: {
    margin: "var(--spacing-0)",
    color: "var(--foreground)",
    fontSize: "var(--font-size-2xl)",
    lineHeight: 1.15,
    textWrap: "balance"
  },
  description: {
    margin: "var(--spacing-0)",
    maxWidth: 520,
    textAlign: "center",
    color: "var(--muted-foreground)",
    textWrap: "pretty"
  },
  setupDescription: {
    margin: "var(--spacing-0)",
    color: "var(--muted-foreground)",
    textWrap: "pretty"
  },
  providerOptions: {
    width: "100%",
    minWidth: 0,
    margin: "var(--spacing-0)",
    padding: "var(--spacing-0)",
    borderWidth: 0
  },
  providerOption: {
    width: "100%",
    minHeight: 76,
    boxSizing: "border-box",
    padding: "var(--spacing-3) var(--spacing-4)",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border-subtle)",
    borderRadius: "var(--radius-container)",
    backgroundColor: {
      default: "var(--surface-raised)",
      ":hover": "var(--surface-sunken)"
    },
    color: "var(--foreground)",
    transitionProperty: "background-color, border-color, box-shadow",
    transitionDuration: "var(--motion-spring-micro-duration)",
    transitionTimingFunction: "var(--motion-spring-critical-easing)",
    outline: {
      default: "none",
      ":focus-within": "2px solid var(--pine-500)"
    },
    outlineOffset: 2
  },
  providerOptionSelected: {
    borderColor: "var(--pine-500)",
    backgroundColor: "color-mix(in srgb, var(--pine-500) 8%, var(--surface-raised))",
    boxShadow: "inset 0 0 0 1px var(--pine-500)"
  },
  providerOptionDisabled: {
    opacity: 0.5
  },
  providerIcon: {
    width: 40,
    height: 40,
    flexShrink: 0,
    borderRadius: "var(--radius-element)",
    backgroundColor: "var(--surface-sunken)",
    color: "var(--pine-700)"
  },
  providerCopy: {
    flex: 1,
    minWidth: 0
  },
  providerName: {
    color: "var(--foreground)",
    fontFamily: "var(--font-heading)",
    textWrap: "balance"
  },
  providerDescription: {
    color: "var(--muted-foreground)",
    fontSize: "var(--font-size-sm)",
    textWrap: "pretty"
  },
  providerRadio: {
    width: 20,
    height: 20,
    flexShrink: 0,
    margin: "var(--spacing-0)",
    accentColor: "var(--pine-500)"
  },
  primaryAction: {
    width: "100%"
  },
  backButton: {
    width: "fit-content",
    alignSelf: "flex-start"
  },
  setupCard: {
    width: "100%",
    minWidth: 0
  },
  sectionLabel: {
    margin: "var(--spacing-0)",
    color: "var(--muted-foreground)",
    fontSize: "var(--font-size-sm)",
    fontWeight: 600
  },
  modelName: {
    fontFamily: "var(--font-heading)",
    fontSize: "var(--font-size-lg)"
  },
  metadata: { fontFamily: "var(--font-mono)", color: "var(--muted-foreground)" },
  muted: { margin: "var(--spacing-0)", color: "var(--muted-foreground)" },
  progressAmount: { fontVariantNumeric: "tabular-nums" },
  progress: { width: "100%", accentColor: "var(--pine-500)" },
  apiKeyTrigger: { width: "100%", minHeight: 44 },
  connected: { margin: "var(--spacing-0)", color: "var(--pine-700)" },
  visuallyHidden: {
    position: "absolute",
    width: 1,
    height: 1,
    padding: "var(--spacing-0)",
    margin: -1,
    overflow: "hidden",
    clip: "rect(0, 0, 0, 0)",
    whiteSpace: "nowrap",
    borderWidth: 0
  }
});

function isRetryableTerminalStatus(status: ProviderAuthAttemptView["status"]) {
  return status === "FAILED" || status === "EXPIRED" || status === "CANCELLED";
}

function toProviderKind(value: string | null | undefined): ProviderKind | null {
  if (value === "local_models" || value === "openrouter" || value === "codex") {
    return value;
  }
  return null;
}

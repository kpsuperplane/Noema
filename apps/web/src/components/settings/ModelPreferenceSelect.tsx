import { useMemo } from "react";
import type { NoemaModelUseCase } from "@/generated/graphql";
import { ControlledModelPreferenceSelect } from "./ControlledModelPreferenceSelect";
import type {
  ModelPreference,
  ModelPreferenceSaveInput,
  ModelProviderOption
} from "./modelPreferenceTypes";

export function ModelPreferenceSelect({
  options,
  preference,
  useCase,
  saving,
  isDisabled = false,
  requireExplicitSelection = false,
  ariaLabel,
  onSave
}: {
  options: readonly ModelProviderOption[];
  preference?: ModelPreference | null;
  useCase: NoemaModelUseCase;
  saving: boolean;
  isDisabled?: boolean;
  requireExplicitSelection?: boolean;
  ariaLabel: string;
  onSave: (input: ModelPreferenceSaveInput) => Promise<unknown>;
}) {
  const providerOptions = useMemo(() => options, [options]);
  const selection = useMemo(
    () =>
      resolveInitialModelSelection(
        providerOptions,
        preference ?? null,
        useCase,
        requireExplicitSelection
      ),
    [providerOptions, preference, useCase, requireExplicitSelection]
  );
  const hasEnabledChoice = providerOptions.some(
    (provider) =>
      !provider.disabledReason &&
      (provider.profiles.some((profile) => !profile.disabledReason) ||
        provider.recommendations.some(
          (recommendation) => recommendation.useCase === useCase && !recommendation.disabledReason
        ))
  );
  const disabled = isDisabled || saving || !hasEnabledChoice;
  return <ControlledModelPreferenceSelect
    options={providerOptions}
    selection={selection}
    useCase={useCase}
    disabled={disabled}
    ariaLabel={ariaLabel}
    onChange={(next) => void onSave(next)}
  />;
}

function resolveInitialModelSelection(
  options: readonly ModelProviderOption[],
  preference: ModelPreference | null,
  useCase: NoemaModelUseCase,
  requireExplicitSelection: boolean
): ModelPreferenceSaveInput {
  if (requireExplicitSelection && !preference) {
    return { providerAccountId: "", selectionMode: "EXPLICIT_PROFILE", modelProfile: null, reasoningEffort: null, fastMode: false };
  }
  const preferredProvider = preference
    ? options.find((option) => option.providerAccountId === preference.providerAccountId)
    : null;
  const provider = preferredProvider ?? options[0];
  if (!provider) {
    return { providerAccountId: "", selectionMode: "EXPLICIT_PROFILE", modelProfile: null, reasoningEffort: null, fastMode: false };
  }
  if (
    preference?.selectionMode === "NOEMA_RECOMMENDED" &&
    provider.recommendations.some((recommendation) => recommendation.useCase === useCase)
  ) {
    return {
      providerAccountId: provider.providerAccountId,
      selectionMode: "NOEMA_RECOMMENDED",
      modelProfile: null,
      reasoningEffort: null,
      fastMode: preference.fastMode
    };
  }
  if (!preference && !requireExplicitSelection) {
    const recommendation = provider.recommendations.find(
      (candidate) => candidate.useCase === useCase && !candidate.disabledReason
    );
    if (recommendation) {
      return {
        providerAccountId: provider.providerAccountId,
        selectionMode: "NOEMA_RECOMMENDED",
        modelProfile: null,
        reasoningEffort: null,
        fastMode: false
      };
    }
  }
  const preferredProfile =
    preferredProvider &&
    provider.profiles.some((profile) => profile.id === preference?.modelProfile)
      ? preference?.modelProfile
      : null;
  return {
    providerAccountId: provider.providerAccountId,
    selectionMode: "EXPLICIT_PROFILE",
    modelProfile: preferredProfile ?? provider.profiles[0]?.id ?? "",
    reasoningEffort: preferredProfile ? preference?.reasoningEffort ?? null : null,
    fastMode: preferredProfile ? preference?.fastMode ?? false : false
  };
}

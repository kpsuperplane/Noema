import { useMemo } from "react";
import { ControlledModelPreferenceSelect } from "./ControlledModelPreferenceSelect";
import type {
  ModelPreference,
  ModelPreferenceSaveInput,
  ModelProviderOption
} from "./modelPreferenceTypes";

export function ModelPreferenceSelect({
  options,
  preference,
  defaultModelProfile,
  saving,
  isDisabled = false,
  requireExplicitSelection = false,
  ariaLabel,
  onSave
}: {
  options: readonly ModelProviderOption[];
  preference?: ModelPreference | null;
  defaultModelProfile?: string;
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
        defaultModelProfile,
        requireExplicitSelection
      ),
    [providerOptions, preference, defaultModelProfile, requireExplicitSelection]
  );
  const hasEnabledChoice = providerOptions.some(
    (provider) =>
      !provider.disabledReason &&
      provider.profiles.some((profile) => !profile.disabledReason)
  );
  const disabled = isDisabled || saving || !hasEnabledChoice;
  const originLabel = preference
    ? preference.isOverride
      ? "Override"
      : "Default"
    : providerOptions.length === 0
      ? null
      : requireExplicitSelection
        ? "Not configured"
        : "Default";

  return <ControlledModelPreferenceSelect
    options={providerOptions}
    selection={selection}
    disabled={disabled}
    ariaLabel={ariaLabel}
    originLabel={originLabel}
    onChange={(next) => void onSave(next)}
  />;
}

function resolveInitialModelSelection(
  options: readonly ModelProviderOption[],
  preference: ModelPreference | null,
  defaultModelProfile: string | undefined,
  requireExplicitSelection: boolean
): ModelPreferenceSaveInput {
  if (requireExplicitSelection && !preference) {
    return { providerAccountId: "", modelProfile: "", reasoningEffort: null };
  }
  const preferredProvider = preference
    ? options.find((option) => option.providerAccountId === preference.providerAccountId)
    : null;
  const provider = preferredProvider ?? options[0];
  if (!provider) {
    return { providerAccountId: "", modelProfile: "", reasoningEffort: null };
  }
  const preferredProfile =
    preferredProvider &&
    provider.profiles.some((profile) => profile.id === preference?.modelProfile)
      ? preference?.modelProfile
      : null;
  return {
    providerAccountId: provider.providerAccountId,
    modelProfile: preferredProfile ?? defaultProfileForProvider(provider, defaultModelProfile),
    reasoningEffort: preferredProfile ? preference?.reasoningEffort ?? null : null
  };
}

function defaultProfileForProvider(
  provider: ModelProviderOption,
  defaultModelProfile?: string
) {
  if (
    provider.defaultModelProfile &&
    provider.profiles.some((profile) => profile.id === provider.defaultModelProfile)
  ) {
    return provider.defaultModelProfile;
  }
  if (
    defaultModelProfile &&
    provider.profiles.some((profile) => profile.id === defaultModelProfile)
  ) {
    return defaultModelProfile;
  }
  return provider.profiles[0]?.id ?? "";
}

import type { ModelPreference, ModelProviderOption } from "./modelPreferenceTypes";

export function selectedPreferenceWarning(
  preference: ModelPreference | null,
  options: readonly ModelProviderOption[]
) {
  if (!preference) {
    return null;
  }
  const provider = options.find(
    (option) => option.providerAccountId === preference.providerAccountId
  );
  const profile = provider?.profiles.find((candidate) => candidate.id === preference.modelProfile);
  if (!provider) {
    return "Selected provider is not available.";
  }
  if (!profile) {
    return "Selected model is not available.";
  }
  if (preference.reasoningEffort) {
    const hasReasoningEffort = profile.reasoningEfforts.some(
      (effort) => effort === preference.reasoningEffort
    );
    if (!hasReasoningEffort) {
      return "Selected reasoning effort is not available.";
    }
  } else if (profile.reasoningEfforts.length > 0) {
    return "Selected model requires a reasoning effort.";
  }
  return profile.disabledReason ?? provider.disabledReason ?? null;
}

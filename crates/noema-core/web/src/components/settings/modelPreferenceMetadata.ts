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
  return profile.disabledReason ?? provider.disabledReason ?? null;
}

import type { ModelPreference, ModelProviderOption } from "./modelPreferenceTypes";

export function providerPreferenceLabel(
  preference: ModelPreference,
  options: readonly ModelProviderOption[]
) {
  const provider = options.find(
    (option) => option.providerAccountId === preference.providerAccountId
  );
  return provider?.providerDisplayName ?? preference.providerKind;
}

export function modelPreferenceLabel(
  preference: ModelPreference,
  options: readonly ModelProviderOption[]
) {
  const provider = options.find(
    (option) => option.providerAccountId === preference.providerAccountId
  );
  const profile = provider?.profiles.find((candidate) => candidate.id === preference.modelProfile);
  return profile?.label ?? preference.modelProfile;
}

export function modelProfileLabel(
  modelProfile: string,
  options: readonly ModelProviderOption[]
) {
  for (const option of options) {
    const profile = option.profiles.find((candidate) => candidate.id === modelProfile);
    if (profile) {
      return profile.label;
    }
  }
  return modelProfile;
}

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

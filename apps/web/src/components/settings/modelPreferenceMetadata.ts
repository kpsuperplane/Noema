import type { ModelPreference, ModelProviderOption } from "./modelPreferenceTypes";
import type { NoemaModelUseCase } from "@/generated/graphql";

export function selectedPreferenceWarning(
  preference: ModelPreference | null,
  options: readonly ModelProviderOption[],
  useCase: NoemaModelUseCase
) {
  if (!preference) {
    return null;
  }
  const provider = options.find(
    (option) => option.providerAccountId === preference.providerAccountId
  );
  if (!provider) {
    return "Selected provider is not available.";
  }
  if (preference.selectionMode === "NOEMA_RECOMMENDED") {
    const recommendation = provider.recommendations.find(
      (candidate) => candidate.useCase === useCase
    );
    return recommendation?.disabledReason ?? provider.disabledReason ??
      (recommendation ? null : "Noema has no recommendation for this setting.");
  }
  const profile = provider.profiles.find((candidate) => candidate.id === preference.modelProfile);
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

import { useMemo, type CSSProperties } from "react";
import { Selector, type SelectorOptionType } from "@astryxdesign/core/Selector";
import * as stylex from "@stylexjs/stylex";
import type { ReasoningEffort } from "@/generated/graphql";
import type {
  ModelPreference,
  ModelPreferenceSaveInput,
  ModelProviderOption
} from "./modelPreferenceTypes";

const selectorTransitionStyle = {
  transition:
    "opacity 140ms ease, border-color 140ms ease, box-shadow 140ms ease, background-color 140ms ease"
} satisfies CSSProperties;

export function ModelPreferenceSelect({
  options,
  preference,
  defaultModelProfile,
  saving,
  isDisabled = false,
  ariaLabel,
  onSave
}: {
  options: readonly ModelProviderOption[];
  preference?: ModelPreference | null;
  defaultModelProfile?: string;
  saving: boolean;
  isDisabled?: boolean;
  ariaLabel: string;
  onSave: (input: ModelPreferenceSaveInput) => Promise<unknown>;
}) {
  const providerOptions = useMemo(() => options, [options]);
  const modelOptions = useMemo<SelectorOptionType[]>(
    () =>
      providerOptions.map((provider) => ({
        type: "section",
        title: provider.providerDisplayName,
        options: provider.profiles.map((profile) => ({
          value: modelOptionValue({
            providerAccountId: provider.providerAccountId,
            modelProfile: profile.id
          }),
          label: profile.label,
          disabled: Boolean(provider.disabledReason || profile.disabledReason)
        }))
      })),
    [providerOptions]
  );
  const selection = useMemo(
    () => resolveInitialModelSelection(providerOptions, preference ?? null, defaultModelProfile),
    [providerOptions, preference, defaultModelProfile]
  );
  const selectedValue =
    selection.providerAccountId && selection.modelProfile
      ? modelOptionValue(selection)
      : "";
  const selectedProvider = providerOptions.find(
    (provider) => provider.providerAccountId === selection.providerAccountId
  );
  const selectedProfile = selectedProvider?.profiles.find(
    (profile) => profile.id === selection.modelProfile
  );
  const reasoningEfforts = selectedProfile?.reasoningEfforts ?? [];
  const requiresReasoning = reasoningEfforts.length > 0;
  const selectedReasoningEffort =
    selection.reasoningEffort ??
    (requiresReasoning ? selectedProfile?.defaultReasoningEffort ?? null : null);
  const reasoningOptions: SelectorOptionType[] = reasoningEfforts.map((effort) => ({
    value: effort,
    label: reasoningEffortLabel(effort)
  }));
  const hasEnabledChoice = providerOptions.some(
    (provider) =>
      !provider.disabledReason &&
      provider.profiles.some((profile) => !profile.disabledReason)
  );
  const disabled = isDisabled || saving || !hasEnabledChoice;

  return (
    <div {...stylex.props(styles.selector)}>
      <div {...stylex.props(styles.field)}>
        <span {...stylex.props(styles.fieldLabel)}>Model</span>
        <div {...stylex.props(styles.controls)}>
          <Selector
            isLabelHidden
            label={ariaLabel}
            options={modelOptions}
            placement="below"
            placeholder={providerOptions.length === 0 ? "No models available" : "Select a model"}
            value={selectedValue || undefined}
            style={selectorTransitionStyle}
            isDisabled={disabled}
            onChange={(value) => {
              const nextSelection = parseModelOptionValue(value);
              if (!nextSelection || modelOptionValue(nextSelection) === selectedValue) {
                return;
              }
              const provider = providerOptions.find(
                (candidate) =>
                  candidate.providerAccountId === nextSelection.providerAccountId
              );
              const profile = provider?.profiles.find(
                (candidate) => candidate.id === nextSelection.modelProfile
              );
              const efforts = profile?.reasoningEfforts ?? [];
              if (efforts.length === 0) {
                void onSave({ ...nextSelection, reasoningEffort: null });
                return;
              }
              const effort = profile?.defaultReasoningEffort ?? efforts[0] ?? null;
              if (effort) {
                void onSave({ ...nextSelection, reasoningEffort: effort });
              }
            }}
          />
          {requiresReasoning ? (
            <Selector
              isLabelHidden
              label={`${ariaLabel} reasoning`}
              options={reasoningOptions}
              placement="below"
              value={selectedReasoningEffort ?? undefined}
              style={selectorTransitionStyle}
              isDisabled={isDisabled || saving}
              onChange={(value) => {
                void onSave({
                  providerAccountId: selection.providerAccountId,
                  modelProfile: selection.modelProfile,
                  reasoningEffort: value as ReasoningEffort
                });
              }}
            />
          ) : null}
        </div>
      </div>
    </div>
  );
}

function modelOptionValue(selection: ModelPreferenceSaveInput) {
  return JSON.stringify([selection.providerAccountId, selection.modelProfile]);
}

function parseModelOptionValue(value: string): ModelPreferenceSaveInput | null {
  try {
    const parsed: unknown = JSON.parse(value);
    if (
      Array.isArray(parsed) &&
      typeof parsed[0] === "string" &&
      typeof parsed[1] === "string" &&
      parsed[0] &&
      parsed[1]
    ) {
      return {
        providerAccountId: parsed[0],
        modelProfile: parsed[1]
      };
    }
  } catch {
    return null;
  }
  return null;
}

function resolveInitialModelSelection(
  options: readonly ModelProviderOption[],
  preference: ModelPreference | null,
  defaultModelProfile?: string
): ModelPreferenceSaveInput {
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

function reasoningEffortLabel(value: ReasoningEffort): string {
  switch (value) {
    case "NONE":
      return "None";
    case "MINIMAL":
      return "Minimal";
    case "LOW":
      return "Low";
    case "MEDIUM":
      return "Medium";
    case "HIGH":
      return "High";
    case "XHIGH":
      return "XHigh";
  }
}

const styles = stylex.create({
  selector: {
    display: "flex",
    flexWrap: "wrap",
    alignItems: "center",
    justifyContent: "flex-end",
    gap: 8
  },
  field: {
    display: "flex",
    alignItems: "center",
    gap: 8
  },
  controls: {
    display: "flex",
    flexWrap: "wrap",
    alignItems: "center",
    justifyContent: "flex-end",
    gap: 8
  },
  fieldLabel: {
    fontSize: 12,
    fontWeight: 600,
    lineHeight: 1.3,
    color: "var(--muted-foreground)"
  }
});

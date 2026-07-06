import { useMemo } from "react";
import * as stylex from "@stylexjs/stylex";
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
  const selection = useMemo(
    () => resolveInitialModelSelection(providerOptions, preference ?? null, defaultModelProfile),
    [providerOptions, preference, defaultModelProfile]
  );
  const selectedValue =
    selection.providerAccountId && selection.modelProfile
      ? modelOptionValue(selection)
      : "";
  const hasEnabledChoice = providerOptions.some(
    (provider) =>
      !provider.disabledReason &&
      provider.profiles.some((profile) => !profile.disabledReason)
  );

  return (
    <label {...stylex.props(styles.selector)}>
      <span {...stylex.props(styles.fieldLabel)}>Model</span>
      <select
        {...stylex.props(styles.select)}
        aria-label={ariaLabel}
        value={selectedValue}
        disabled={isDisabled || saving || !hasEnabledChoice}
        onChange={(event) => {
          const nextSelection = parseModelOptionValue(event.target.value);
          if (!nextSelection || modelOptionValue(nextSelection) === selectedValue) {
            return;
          }
          void onSave(nextSelection);
        }}
      >
        {selectedValue ? null : (
          <option value="">
            {providerOptions.length === 0 ? "No models available" : "Select a model"}
          </option>
        )}
        {providerOptions.map((provider) => (
          <optgroup
            key={provider.providerAccountId}
            label={provider.providerDisplayName}
            disabled={Boolean(provider.disabledReason)}
          >
            {provider.profiles.length === 0 ? (
              <option value="" disabled>
                No models available
              </option>
            ) : (
              provider.profiles.map((profile) => (
                <option
                  key={`${provider.providerAccountId}:${profile.id}`}
                  value={modelOptionValue({
                    providerAccountId: provider.providerAccountId,
                    modelProfile: profile.id
                  })}
                  disabled={Boolean(provider.disabledReason || profile.disabledReason)}
                >
                  {profile.label}
                </option>
              ))
            )}
          </optgroup>
        ))}
      </select>
      {saving ? <span {...stylex.props(styles.savingText)}>Saving...</span> : null}
    </label>
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
) {
  const preferredProvider = preference
    ? options.find((option) => option.providerAccountId === preference.providerAccountId)
    : null;
  const provider = preferredProvider ?? options[0];
  if (!provider) {
    return { providerAccountId: "", modelProfile: "" };
  }
  const preferredProfile =
    preferredProvider &&
    provider.profiles.some((profile) => profile.id === preference?.modelProfile)
      ? preference?.modelProfile
      : null;
  return {
    providerAccountId: provider.providerAccountId,
    modelProfile: preferredProfile ?? defaultProfileForProvider(provider, defaultModelProfile)
  };
}

function defaultProfileForProvider(
  provider: ModelProviderOption,
  defaultModelProfile?: string
) {
  if (
    defaultModelProfile &&
    provider.profiles.some((profile) => profile.id === defaultModelProfile)
  ) {
    return defaultModelProfile;
  }
  return provider.profiles[0]?.id ?? "";
}

const styles = stylex.create({
  selector: {
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
  },
  select: {
    minHeight: 36,
    maxWidth: 320,
    minWidth: 220,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border)",
    borderRadius: 6,
    backgroundColor: "white",
    paddingInline: 10,
    fontSize: 14,
    color: "var(--foreground)"
  },
  savingText: {
    fontSize: 12,
    lineHeight: 1.3,
    color: "var(--muted-foreground)"
  }
});

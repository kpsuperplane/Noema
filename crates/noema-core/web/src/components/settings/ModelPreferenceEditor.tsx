import { useMemo, useState } from "react";
import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import type {
  ModelPreference,
  ModelPreferenceSaveInput,
  ModelProviderOption
} from "./modelPreferenceTypes";

export function ModelPreferenceEditor({
  options,
  preference,
  defaultModelProfile,
  saving,
  onCancel,
  onSave
}: {
  options: readonly ModelProviderOption[];
  preference?: ModelPreference | null;
  defaultModelProfile?: string;
  saving: boolean;
  onCancel: () => void;
  onSave: (input: ModelPreferenceSaveInput) => Promise<unknown>;
}) {
  const providerOptions = useMemo(() => options, [options]);
  const selectionSeed = useMemo(
    () => modelSelectionSeed(providerOptions, preference ?? null, defaultModelProfile),
    [providerOptions, preference, defaultModelProfile]
  );
  const initialSelection = useMemo(
    () => resolveInitialModelSelection(providerOptions, preference ?? null, defaultModelProfile),
    [providerOptions, preference, defaultModelProfile]
  );

  return (
    <ModelPreferenceEditorFields
      key={selectionSeed}
      providerOptions={providerOptions}
      initialSelection={initialSelection}
      defaultModelProfile={defaultModelProfile}
      saving={saving}
      onCancel={onCancel}
      onSave={onSave}
    />
  );
}

function ModelPreferenceEditorFields({
  providerOptions,
  initialSelection,
  defaultModelProfile,
  saving,
  onCancel,
  onSave
}: {
  providerOptions: readonly ModelProviderOption[];
  initialSelection: ModelPreferenceSaveInput;
  defaultModelProfile?: string;
  saving: boolean;
  onCancel: () => void;
  onSave: (input: ModelPreferenceSaveInput) => Promise<unknown>;
}) {
  const [providerAccountId, setProviderAccountId] = useState(initialSelection.providerAccountId);
  const [modelProfile, setModelProfile] = useState(initialSelection.modelProfile);

  const selectedProvider = useMemo(
    () =>
      providerOptions.find((option) => option.providerAccountId === providerAccountId) ??
      providerOptions[0],
    [providerOptions, providerAccountId]
  );

  const profiles = selectedProvider?.profiles ?? [];
  const effectiveProviderAccountId = selectedProvider?.providerAccountId ?? "";
  const selectedProfileAvailable = profiles.some((profile) => profile.id === modelProfile);
  const effectiveProfile = selectedProfileAvailable ? modelProfile : profiles[0]?.id ?? "";
  const selectedProfile = profiles.find((profile) => profile.id === effectiveProfile);
  const providerDisabled = Boolean(selectedProvider?.disabledReason);
  const profileDisabled = Boolean(selectedProfile?.disabledReason);
  const disabledMessages = useMemo(() => {
    const messages: string[] = [];
    for (const option of providerOptions) {
      if (option.disabledReason) {
        messages.push(`${option.providerDisplayName}: ${option.disabledReason}`);
      }
      for (const profile of option.profiles) {
        if (profile.disabledReason && profile.disabledReason !== option.disabledReason) {
          messages.push(
            `${option.providerDisplayName} / ${profile.label}: ${profile.disabledReason}`
          );
        }
      }
    }
    return Array.from(new Set(messages));
  }, [providerOptions]);
  const canSave = Boolean(
    effectiveProviderAccountId &&
    effectiveProfile &&
    !providerDisabled &&
    !profileDisabled &&
    !saving
  );

  return (
    <form
      {...stylex.props(styles.editor)}
      onSubmit={(event) => {
        event.preventDefault();
        if (canSave) {
          void onSave({
            providerAccountId: effectiveProviderAccountId,
            modelProfile: effectiveProfile
          });
        }
      }}
    >
      <div {...stylex.props(styles.editorGrid)}>
        <label {...stylex.props(styles.field)}>
          <span {...stylex.props(styles.fieldLabel)}>Provider</span>
          <select
            {...stylex.props(styles.select)}
            value={effectiveProviderAccountId}
            onChange={(event) => {
              const nextProvider = providerOptions.find(
                (option) => option.providerAccountId === event.target.value
              );
              setProviderAccountId(event.target.value);
              setModelProfile(
                nextProvider ? defaultProfileForProvider(nextProvider, defaultModelProfile) : ""
              );
            }}
          >
            {providerOptions.length === 0 ? (
              <option value="">No providers available</option>
            ) : (
              providerOptions.map((option) => (
                <option
                  key={option.providerAccountId}
                  value={option.providerAccountId}
                  disabled={Boolean(option.disabledReason)}
                >
                  {option.providerDisplayName}
                </option>
              ))
            )}
          </select>
        </label>
        <label {...stylex.props(styles.field)}>
          <span {...stylex.props(styles.fieldLabel)}>Model</span>
          <select
            {...stylex.props(styles.select)}
            value={effectiveProfile}
            onChange={(event) => setModelProfile(event.target.value)}
            disabled={profiles.length === 0}
          >
            {profiles.length === 0 ? (
              <option value="">No models available</option>
            ) : (
              profiles.map((profile) => (
                <option
                  key={profile.id}
                  value={profile.id}
                  disabled={Boolean(profile.disabledReason)}
                >
                  {profile.label}
                </option>
              ))
            )}
          </select>
        </label>
      </div>
      {disabledMessages.length > 0 ? (
        <ul {...stylex.props(styles.warningList)}>
          {disabledMessages.map((message) => (
            <li key={message}>{message}</li>
          ))}
        </ul>
      ) : null}
      <div {...stylex.props(styles.actions)}>
        <Button type="button" variant="ghost" size="sm" label="Cancel" onClick={onCancel} />
        <Button
          type="submit"
          size="sm"
          label="Save"
          isDisabled={!canSave}
          isLoading={saving}
        />
      </div>
    </form>
  );
}

function modelSelectionSeed(
  options: readonly ModelProviderOption[],
  preference: ModelPreference | null,
  defaultModelProfile?: string
) {
  const optionSeed = options
    .map((option) => {
      const profiles = option.profiles.map((profile) => profile.id).join(",");
      return `${option.providerAccountId}:${profiles}`;
    })
    .join("|");
  return [
    preference?.providerAccountId ?? "",
    preference?.modelProfile ?? "",
    defaultModelProfile ?? "",
    optionSeed
  ].join("::");
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
  editor: {
    display: "grid",
    gap: 12,
    borderTopWidth: 1,
    borderTopStyle: "solid",
    borderTopColor: "var(--border-subtle)",
    paddingTop: 12
  },
  editorGrid: {
    display: "grid",
    gridTemplateColumns: "repeat(2, minmax(0, 1fr))",
    gap: 12,
    "@media (max-width: 640px)": {
      gridTemplateColumns: "1fr"
    }
  },
  field: {
    display: "grid",
    gap: 6
  },
  fieldLabel: {
    fontSize: 12,
    fontWeight: 600,
    lineHeight: 1.3,
    color: "var(--muted-foreground)"
  },
  select: {
    minHeight: 36,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border)",
    borderRadius: 6,
    backgroundColor: "white",
    paddingInline: 10,
    fontSize: 14,
    color: "var(--foreground)"
  },
  warningList: {
    marginBlock: 0,
    paddingInlineStart: 18,
    fontSize: 13,
    lineHeight: 1.5,
    color: "var(--muted-foreground)"
  },
  actions: {
    display: "flex",
    justifyContent: "flex-end",
    gap: 8
  }
});

import { useMemo, useState } from "react";
import { Popover } from "@astryxdesign/core/Popover";
import * as stylex from "@stylexjs/stylex";
import { CheckIcon, ChevronDownIcon, ChevronRightIcon } from "lucide-react";
import type { ReasoningEffort } from "@/generated/graphql";
import type {
  ModelPreference,
  ModelPreferenceSaveInput,
  ModelProviderOption
} from "./modelPreferenceTypes";

type ModelChoice = {
  providerAccountId: string;
  providerDisabledReason?: string | null;
  modelProfile: string;
  label: string;
  disabledReason?: string | null;
  reasoningEfforts: readonly ReasoningEffort[];
};

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
  const modelChoices = useMemo(
    () =>
      providerOptions.flatMap((provider) =>
        provider.profiles.map((profile) => ({
          providerAccountId: provider.providerAccountId,
          providerDisabledReason: provider.disabledReason,
          modelProfile: profile.id,
          label: profile.label,
          disabledReason: profile.disabledReason,
          reasoningEfforts: profile.reasoningEfforts
        }))
      ),
    [providerOptions]
  );
  const selection = useMemo(
    () => resolveInitialModelSelection(providerOptions, preference ?? null, defaultModelProfile),
    [providerOptions, preference, defaultModelProfile]
  );
  const selectedValue = modelChoiceKey(selection.providerAccountId, selection.modelProfile);
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
  const hasEnabledChoice = providerOptions.some(
    (provider) =>
      !provider.disabledReason &&
      provider.profiles.some((profile) => !profile.disabledReason)
  );
  const firstEnabledChoice = modelChoices.find((choice) => !modelChoiceDisabledReason(choice));
  const [activeModelValue, setActiveModelValue] = useState(
    selectedValue || (firstEnabledChoice ? modelChoiceKeyForChoice(firstEnabledChoice) : "")
  );
  const [menuOpen, setMenuOpen] = useState(false);
  const activeChoice =
    modelChoices.find((choice) => modelChoiceKeyForChoice(choice) === activeModelValue) ??
    modelChoices.find((choice) => modelChoiceKeyForChoice(choice) === selectedValue) ??
    firstEnabledChoice ??
    modelChoices[0];
  const currentReasoningLabel = selectedReasoningEffort
    ? reasoningEffortLabel(selectedReasoningEffort)
    : null;
  const disabled = isDisabled || saving || !hasEnabledChoice;
  const triggerLabel = selectedProfile?.label ?? "Select a model";
  const triggerMeta = [
    selectedProvider?.providerDisplayName,
    currentReasoningLabel ? `Reasoning: ${currentReasoningLabel}` : null
  ]
    .filter(Boolean)
    .join(" / ");
  const handleOpenChange = (open: boolean) => {
    setMenuOpen(open);
    if (open) {
      setActiveModelValue(
        selectedValue || (firstEnabledChoice ? modelChoiceKeyForChoice(firstEnabledChoice) : "")
      );
    }
  };

  return (
    <div {...stylex.props(styles.selector)}>
      <div {...stylex.props(styles.field)}>
        <span {...stylex.props(styles.fieldLabel)}>Model</span>
        <Popover
          alignment="end"
          content={
            <div {...stylex.props(styles.menu)}>
              <div {...stylex.props(styles.modelPanel)} role="menu" aria-label="Models">
                {providerOptions.length === 0 ? (
                  <div {...stylex.props(styles.emptyText)}>No models available</div>
                ) : (
                  providerOptions.map((provider) => (
                    <div key={provider.providerAccountId} {...stylex.props(styles.providerGroup)}>
                      <div {...stylex.props(styles.providerLabel)}>
                        {provider.providerDisplayName}
                      </div>
                      {provider.profiles.length === 0 ? (
                        <div {...stylex.props(styles.emptyText)}>No models available</div>
                      ) : (
                        provider.profiles.map((profile) => {
                          const choice: ModelChoice = {
                            providerAccountId: provider.providerAccountId,
                            providerDisabledReason: provider.disabledReason,
                            modelProfile: profile.id,
                            label: profile.label,
                            disabledReason: profile.disabledReason,
                            reasoningEfforts: profile.reasoningEfforts
                          };
                          const key = modelChoiceKeyForChoice(choice);
                          const isSelected = key === selectedValue;
                          const isActive = key === modelChoiceKeyForChoice(activeChoice);
                          const disabledReason = modelChoiceDisabledReason(choice);
                          const hasReasoning = choice.reasoningEfforts.length > 0;
                          return (
                            <button
                              key={key}
                              type="button"
                              role="menuitem"
                              {...stylex.props(
                                styles.modelItem,
                                isSelected && styles.itemSelected,
                                isActive && styles.itemActive
                              )}
                              aria-current={isSelected ? "true" : undefined}
                              disabled={Boolean(disabledReason)}
                              title={disabledReason ?? undefined}
                              onFocus={() => setActiveModelValue(key)}
                              onMouseEnter={() => setActiveModelValue(key)}
                              onClick={() => {
                                setActiveModelValue(key);
                                if (hasReasoning) {
                                  return;
                                }
                                void onSave({
                                  providerAccountId: choice.providerAccountId,
                                  modelProfile: choice.modelProfile,
                                  reasoningEffort: null
                                });
                                setMenuOpen(false);
                              }}
                            >
                              <span {...stylex.props(styles.itemText)}>{choice.label}</span>
                              <span {...stylex.props(styles.itemEnd)}>
                                {isSelected ? (
                                  <CheckIcon aria-hidden="true" size={15} strokeWidth={2} />
                                ) : null}
                                {hasReasoning ? (
                                  <ChevronRightIcon aria-hidden="true" size={15} strokeWidth={2} />
                                ) : null}
                              </span>
                            </button>
                          );
                        })
                      )}
                    </div>
                  ))
                )}
              </div>
              <div {...stylex.props(styles.reasoningPanel)}>
                {activeChoice?.reasoningEfforts.length ? (
                  <>
                    <div {...stylex.props(styles.reasoningHeading)}>
                      <span {...stylex.props(styles.reasoningTitle)}>{activeChoice.label}</span>
                      <span {...stylex.props(styles.reasoningSubtitle)}>Reasoning</span>
                    </div>
                    <div role="menu" aria-label={`${activeChoice.label} reasoning`}>
                      {activeChoice.reasoningEfforts.map((effort) => {
                        const isSelected =
                          activeChoice.providerAccountId === selection.providerAccountId &&
                          activeChoice.modelProfile === selection.modelProfile &&
                          effort === selectedReasoningEffort;
                        return (
                          <button
                            key={effort}
                            type="button"
                            role="menuitemradio"
                            aria-checked={isSelected}
                            {...stylex.props(
                              styles.reasoningItem,
                              isSelected && styles.itemSelected
                            )}
                            onClick={() => {
                              void onSave({
                                providerAccountId: activeChoice.providerAccountId,
                                modelProfile: activeChoice.modelProfile,
                                reasoningEffort: effort
                              });
                              setMenuOpen(false);
                            }}
                          >
                            <span>{reasoningEffortLabel(effort)}</span>
                            {isSelected ? (
                              <CheckIcon aria-hidden="true" size={15} strokeWidth={2} />
                            ) : null}
                          </button>
                        );
                      })}
                    </div>
                  </>
                ) : (
                  <div {...stylex.props(styles.reasoningEmpty)}>
                    Select this model directly; it does not expose reasoning levels.
                  </div>
                )}
              </div>
            </div>
          }
          hasAutoFocus
          hasCloseButton={false}
          isEnabled={!disabled}
          isOpen={menuOpen}
          label={ariaLabel}
          placement="below"
          width="min(520px, calc(100vw - 24px))"
          xstyle={astryxXStyle(styles.popover)}
          onOpenChange={handleOpenChange}
        >
          {(triggerProps) => (
            <button
              type="button"
              {...triggerProps}
              {...stylex.props(styles.trigger)}
              aria-label={ariaLabel}
              disabled={disabled}
            >
              <span {...stylex.props(styles.triggerText)}>
                <span {...stylex.props(styles.triggerPrimary)}>{triggerLabel}</span>
                {triggerMeta ? (
                  <span {...stylex.props(styles.triggerMeta)}>{triggerMeta}</span>
                ) : null}
              </span>
              <ChevronDownIcon aria-hidden="true" size={16} strokeWidth={2} />
            </button>
          )}
        </Popover>
      </div>
      {saving ? <span {...stylex.props(styles.savingText)}>Saving...</span> : null}
    </div>
  );
}

function modelChoiceKey(providerAccountId: string, modelProfile: string) {
  return `${providerAccountId}\u0000${modelProfile}`;
}

function modelChoiceKeyForChoice(choice: ModelChoice | undefined) {
  return choice ? modelChoiceKey(choice.providerAccountId, choice.modelProfile) : "";
}

function modelChoiceDisabledReason(choice: ModelChoice) {
  return choice.providerDisabledReason ?? choice.disabledReason ?? null;
}

function astryxXStyle(xstyle: unknown) {
  return xstyle as never;
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
  fieldLabel: {
    fontSize: 12,
    fontWeight: 600,
    lineHeight: 1.3,
    color: "var(--muted-foreground)"
  },
  trigger: {
    minHeight: 36,
    width: 320,
    maxWidth: "min(320px, 62vw)",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border)",
    borderRadius: 6,
    backgroundColor: "white",
    paddingBlock: 5,
    paddingInline: 10,
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    gap: 10,
    fontSize: 14,
    color: "var(--foreground)",
    textAlign: "left",
    cursor: "pointer",
    ":hover": {
      borderColor: "color-mix(in srgb, var(--foreground) 26%, var(--border))"
    },
    ":disabled": {
      color: "var(--muted-foreground)",
      cursor: "not-allowed",
      opacity: 0.68
    }
  },
  triggerText: {
    minWidth: 0,
    display: "flex",
    flexDirection: "column",
    gap: 1
  },
  triggerPrimary: {
    minWidth: 0,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
    lineHeight: 1.25
  },
  triggerMeta: {
    minWidth: 0,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
    fontSize: 11,
    lineHeight: 1.25,
    color: "var(--muted-foreground)"
  },
  popover: {
    padding: 0,
    overflow: "hidden"
  },
  menu: {
    width: "100%",
    display: "grid",
    gridTemplateColumns: "minmax(170px, 1fr) minmax(140px, 0.8fr)",
    backgroundColor: "white"
  },
  modelPanel: {
    maxHeight: 360,
    overflowY: "auto",
    paddingBlock: 8,
    borderInlineEndWidth: 1,
    borderInlineEndStyle: "solid",
    borderInlineEndColor: "var(--border)"
  },
  providerGroup: {
    paddingBlock: 4
  },
  providerLabel: {
    paddingBlock: 4,
    paddingInline: 12,
    fontSize: 11,
    fontWeight: 700,
    lineHeight: 1.25,
    color: "var(--muted-foreground)"
  },
  modelItem: {
    width: "100%",
    minHeight: 34,
    borderWidth: 0,
    backgroundColor: "transparent",
    color: "var(--foreground)",
    paddingBlock: 7,
    paddingInline: 12,
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    gap: 10,
    font: "inherit",
    textAlign: "left",
    cursor: "pointer",
    ":hover": {
      backgroundColor: "color-mix(in srgb, var(--foreground) 7%, transparent)"
    },
    ":disabled": {
      color: "var(--muted-foreground)",
      cursor: "not-allowed",
      opacity: 0.62
    }
  },
  itemText: {
    minWidth: 0,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
    lineHeight: 1.25
  },
  itemEnd: {
    flexShrink: 0,
    display: "flex",
    alignItems: "center",
    gap: 4,
    color: "var(--muted-foreground)"
  },
  itemSelected: {
    backgroundColor: "color-mix(in srgb, var(--foreground) 10%, transparent)",
    fontWeight: 600
  },
  itemActive: {
    backgroundColor: "color-mix(in srgb, var(--foreground) 7%, transparent)"
  },
  reasoningPanel: {
    minWidth: 0,
    padding: 8
  },
  reasoningHeading: {
    paddingBlock: 6,
    paddingInline: 8,
    display: "flex",
    flexDirection: "column",
    gap: 2
  },
  reasoningTitle: {
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
    fontSize: 13,
    fontWeight: 700,
    lineHeight: 1.25,
    color: "var(--foreground)"
  },
  reasoningSubtitle: {
    fontSize: 11,
    fontWeight: 600,
    lineHeight: 1.25,
    color: "var(--muted-foreground)"
  },
  reasoningItem: {
    width: "100%",
    minHeight: 34,
    borderWidth: 0,
    borderRadius: 5,
    backgroundColor: "transparent",
    color: "var(--foreground)",
    paddingBlock: 7,
    paddingInline: 8,
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    gap: 8,
    font: "inherit",
    textAlign: "left",
    cursor: "pointer",
    ":hover": {
      backgroundColor: "color-mix(in srgb, var(--foreground) 7%, transparent)"
    }
  },
  reasoningEmpty: {
    minHeight: 120,
    display: "flex",
    alignItems: "center",
    color: "var(--muted-foreground)",
    padding: 10,
    fontSize: 12,
    lineHeight: 1.35
  },
  emptyText: {
    paddingBlock: 8,
    paddingInline: 12,
    color: "var(--muted-foreground)",
    fontSize: 12,
    lineHeight: 1.35
  },
  savingText: {
    fontSize: 12,
    lineHeight: 1.3,
    color: "var(--muted-foreground)"
  }
});

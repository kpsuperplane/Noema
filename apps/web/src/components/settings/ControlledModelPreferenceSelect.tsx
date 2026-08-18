import { CheckboxInput } from "@astryxdesign/core/CheckboxInput";
import { Icon } from "@astryxdesign/core/Icon";
import {
  Selector,
  SelectorOption,
  type SelectorOptionType
} from "@astryxdesign/core/Selector";
import { HStack, StackItem } from "@astryxdesign/core/Stack";
import * as stylex from "@stylexjs/stylex";
import { Sparkles } from "lucide-react";
import type { CSSProperties } from "react";
import type { NoemaModelUseCase, ReasoningEffort } from "@/generated/graphql";
import type { ModelPreferenceSaveInput, ModelProviderOption } from "./modelPreferenceTypes";

export function ControlledModelPreferenceSelect({
  options,
  selection,
  useCase,
  disabled = false,
  ariaLabel,
  onChange
}: {
  options: readonly ModelProviderOption[];
  selection: ModelPreferenceSaveInput;
  useCase: NoemaModelUseCase;
  disabled?: boolean;
  ariaLabel: string;
  onChange: (selection: ModelPreferenceSaveInput) => void;
}) {
  const descriptions = new Map<string, string>();
  const recommendedValues = new Set<string>();
  const recommendedLabels = new Map<string, string>();
  const modelOptions: SelectorOptionType[] = options.map((provider) => {
    const recommendation = provider.recommendations.find((item) => item.useCase === useCase);
    const recommendedValue = recommendedOptionValue(provider.providerAccountId);
    if (recommendation) {
      const profileLabel = provider.profiles.find(
        (profile) => profile.id === recommendation.modelProfile
      )?.label ?? recommendation.modelProfile;
      const { label } = modelPresentation(provider.providerKind, profileLabel);
      recommendedLabels.set(recommendedValue, label);
      recommendedValues.add(recommendedValue);
      descriptions.set(recommendedValue, "Recommended");
    }
    return {
      type: "section",
      title: provider.providerDisplayName,
      options: [
        ...(recommendation
          ? [{
              value: recommendedValue,
              label: recommendedLabels.get(recommendedValue) ?? recommendation.modelProfile,
              disabled: Boolean(provider.disabledReason || recommendation.disabledReason)
            }]
          : []),
        ...provider.profiles.map((profile) => {
          const value = explicitOptionValue(provider.providerAccountId, profile.id);
          const presentation = modelPresentation(provider.providerKind, profile.label);
          if (presentation.description) descriptions.set(value, presentation.description);
          return {
            value,
            label: presentation.label,
            disabled: Boolean(provider.disabledReason || profile.disabledReason)
          };
        })
      ]
    };
  });
  const selectedProvider = options.find(
    (provider) => provider.providerAccountId === selection.providerAccountId
  );
  const supportsFastMode = selectedProvider
    ? fastModeProvider(selectedProvider.providerKind)
    : false;
  const selectedProfile = selectedProvider?.profiles.find(
    (profile) => selection.selectionMode === "EXPLICIT_PROFILE" && profile.id === selection.modelProfile
  );
  const selectedRecommendation = selection.selectionMode === "NOEMA_RECOMMENDED"
    ? selectedProvider?.recommendations.find((item) => item.useCase === useCase)
    : undefined;
  const recommendedEffort = selectedRecommendation?.reasoningEffort ?? null;
  const reasoningEfforts = recommendedEffort
    ? [recommendedEffort]
    : selectedProfile?.reasoningEfforts ?? [];
  const reasoningOptions: SelectorOptionType[] = reasoningEfforts.map((effort) => ({
    value: effort,
    label: reasoningEffortLabel(effort)
  }));
  const selectedValue = selection.providerAccountId
    ? selection.selectionMode === "NOEMA_RECOMMENDED"
      ? recommendedOptionValue(selection.providerAccountId)
      : selection.modelProfile
        ? explicitOptionValue(selection.providerAccountId, selection.modelProfile)
        : ""
    : "";
  return (
    <HStack
      gap={2}
      wrap="wrap"
      vAlign="center"
      role="group"
      aria-label={ariaLabel}
      {...stylex.props(styles.field)}
    >
      <HStack gap={2} wrap="wrap" vAlign="center" {...stylex.props(styles.controls)}>
        <StackItem
          size="fill"
          {...stylex.props(styles.modelControl)}
        >
          <Selector
            isLabelHidden
            label={recommendedValues.has(selectedValue) ? `${ariaLabel}, Recommended` : ariaLabel}
            options={modelOptions}
            hasSearch={options.reduce((count, provider) => count + provider.profiles.length + 1, 0) > 8}
            placement="below"
            placeholder={options.length === 0 ? "No models available" : "Select a model"}
            value={selectedValue || undefined}
            width="100%"
            startIcon={recommendedValues.has(selectedValue) ? (
              <Icon
                icon={Sparkles}
                color={disabled ? "disabled" : "accent"}
                size="sm"
              />
            ) : undefined}
            renderOption={(option) => (
              <SelectorOption
                icon={recommendedValues.has(option.value) ? Sparkles : undefined}
                label={option.label}
                description={descriptions.get(option.value)}
              />
            )}
            style={selectorTransitionStyle}
            isDisabled={disabled}
            onChange={(value) => {
              const next = parseModelOptionValue(value);
              if (!next || value === selectedValue) return;
              const provider = options.find(
                (candidate) => candidate.providerAccountId === next.providerAccountId
              );
              const fastMode = Boolean(
                provider
                && fastModeProvider(provider.providerKind)
                && next.providerAccountId === selection.providerAccountId
                && selection.fastMode
              );
              if (next.selectionMode === "NOEMA_RECOMMENDED") {
                onChange({ ...next, fastMode });
                return;
              }
              const profile = provider?.profiles.find(
                (candidate) => candidate.id === next.modelProfile
              );
              const efforts = profile?.reasoningEfforts ?? [];
              onChange({
                ...next,
                fastMode,
                reasoningEffort: efforts.length > 0
                  ? profile?.defaultReasoningEffort ?? efforts[0] ?? null
                  : null
              });
            }}
          />
        </StackItem>
        {reasoningEfforts.length > 0 ? (
          <StackItem
            size="fill"
            {...stylex.props(styles.effortControl)}
          >
            <Selector
              isLabelHidden
              label={`${ariaLabel} reasoning`}
              options={reasoningOptions}
              placement="below"
              value={recommendedEffort ?? selection.reasoningEffort ?? undefined}
              width="100%"
              style={selectorTransitionStyle}
              isDisabled={disabled || selection.selectionMode === "NOEMA_RECOMMENDED"}
              onChange={(value) => onChange({
                ...selection,
                reasoningEffort: value as ReasoningEffort
              })}
            />
          </StackItem>
        ) : null}
        {supportsFastMode ? (
          <StackItem crossAlignSelf="center" {...stylex.props(styles.speedControl)}>
            <CheckboxInput
              label="Fast"
              value={selection.fastMode}
              size="sm"
              isDisabled={disabled}
              onChange={(value) => onChange({
                ...selection,
                fastMode: value
              })}
            />
          </StackItem>
        ) : null}
      </HStack>
    </HStack>
  );
}

const selectorTransitionStyle = {
  transition:
    "opacity var(--motion-spring-standard), border-color var(--motion-spring-standard), box-shadow var(--motion-spring-standard), background-color var(--motion-spring-standard)"
} satisfies CSSProperties;

function recommendedOptionValue(providerAccountId: string) {
  return JSON.stringify([providerAccountId, "NOEMA_RECOMMENDED"]);
}

function explicitOptionValue(providerAccountId: string, modelProfile: string) {
  return JSON.stringify([providerAccountId, "EXPLICIT_PROFILE", modelProfile]);
}

function parseModelOptionValue(value: string): ModelPreferenceSaveInput | null {
  try {
    const parsed: unknown = JSON.parse(value);
    if (
      Array.isArray(parsed) &&
      typeof parsed[0] === "string" &&
      parsed[1] === "NOEMA_RECOMMENDED"
    ) {
      return {
        providerAccountId: parsed[0],
        selectionMode: "NOEMA_RECOMMENDED",
        modelProfile: null,
        reasoningEffort: null,
        fastMode: false
      };
    }
    if (
      Array.isArray(parsed) &&
      typeof parsed[0] === "string" &&
      parsed[1] === "EXPLICIT_PROFILE" &&
      typeof parsed[2] === "string"
    ) {
      return {
        providerAccountId: parsed[0],
        selectionMode: "EXPLICIT_PROFILE",
        modelProfile: parsed[2],
        fastMode: false
      };
    }
  } catch {
    return null;
  }
  return null;
}

function reasoningEffortLabel(value: ReasoningEffort): string {
  return value === "XHIGH"
    ? "XHigh"
    : value.charAt(0) + value.slice(1).toLowerCase();
}

function fastModeProvider(providerKind: string): boolean {
  return providerKind === "codex" || providerKind === "openai";
}

function modelPresentation(providerKind: string, label: string) {
  if (providerKind !== "openrouter") return { label };
  const separator = label.indexOf(": ");
  return separator > 0
    ? { label: label.slice(separator + 2), description: label.slice(0, separator) }
    : { label };
}

const styles = stylex.create({
  field: {
    minWidth: 0,
    justifyContent: "flex-end",
    "@media (max-width: 620px)": {
      width: "100%",
      justifyContent: "flex-start"
    }
  },
  controls: {
    width: "min(100%, 38rem)",
    justifyContent: "flex-end",
    "@media (max-width: 620px)": {
      width: "100%",
      minWidth: 0
    }
  },
  modelControl: {
    minWidth: "8rem",
    flex: "1 1 8rem"
  },
  effortControl: {
    width: "6rem",
    minWidth: "5.5rem",
    flex: "0 1 6rem"
  },
  speedControl: {
    flex: "0 0 auto"
  }
});

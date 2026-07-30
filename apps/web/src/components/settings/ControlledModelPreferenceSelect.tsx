import {
  Selector,
  SelectorOption,
  type SelectorOptionType
} from "@astryxdesign/core/Selector";
import { HStack, VStack } from "@astryxdesign/core/Stack";
import { Text } from "@astryxdesign/core/Text";
import * as stylex from "@stylexjs/stylex";
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
  const recommendedLabels = new Map<string, string>();
  const modelOptions: SelectorOptionType[] = options.map((provider) => {
    const recommendation = provider.recommendations.find((item) => item.useCase === useCase);
    const recommendedValue = recommendedOptionValue(provider.providerAccountId);
    if (recommendation) {
      const label = provider.profiles.find(
        (profile) => profile.id === recommendation.modelProfile
      )?.label ?? recommendation.modelProfile;
      recommendedLabels.set(
        recommendedValue,
        recommendation.reasoningEffort
          ? `${label} · ${reasoningEffortLabel(recommendation.reasoningEffort)}`
          : label
      );
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
        ...provider.profiles.map((profile) => ({
          value: explicitOptionValue(provider.providerAccountId, profile.id),
          label: profile.label,
          disabled: Boolean(provider.disabledReason || profile.disabledReason)
        }))
      ]
    };
  });
  const selectedProvider = options.find(
    (provider) => provider.providerAccountId === selection.providerAccountId
  );
  const selectedProfile = selectedProvider?.profiles.find(
    (profile) => selection.selectionMode === "EXPLICIT_PROFILE" && profile.id === selection.modelProfile
  );
  const reasoningEfforts = selectedProfile?.reasoningEfforts ?? [];
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
  const selectedLabel = recommendedLabels.get(selectedValue);
  const selectedDescription = descriptions.get(selectedValue);

  return (
    <HStack gap={2} wrap="wrap" vAlign="center" {...stylex.props(styles.field)}>
      <HStack gap={2} wrap="wrap" {...stylex.props(styles.controls)}>
        <Selector
          isLabelHidden
          label={ariaLabel}
          options={modelOptions}
          hasSearch={options.reduce((count, provider) => count + provider.profiles.length + 1, 0) > 8}
          placement="below"
          placeholder={options.length === 0 ? "No models available" : "Select a model"}
          value={selectedValue || undefined}
          width="min(20rem, calc(100vw - var(--spacing-8)))"
          startIcon={selectedLabel && selectedDescription ? (
            <VStack
              gap={0}
              aria-hidden="true"
              {...stylex.props(styles.closedValue)}
            >
              <Text
                type="label"
                color={disabled ? "disabled" : "primary"}
                maxLines={1}
                hasTruncateTooltip={false}
              >
                {selectedLabel}
              </Text>
              <Text
                type="supporting"
                color={disabled ? "disabled" : "secondary"}
                maxLines={1}
                hasTruncateTooltip={false}
              >
                {selectedDescription}
              </Text>
            </VStack>
          ) : undefined}
          renderOption={(option) => (
            <SelectorOption label={option.label} description={descriptions.get(option.value)} />
          )}
          className={selectedDescription
            ? stylex.props(styles.closedValueSelector).className
            : undefined}
          style={selectorTransitionStyle}
          isDisabled={disabled}
          onChange={(value) => {
            const next = parseModelOptionValue(value);
            if (!next || value === selectedValue) return;
            if (next.selectionMode === "NOEMA_RECOMMENDED") {
              onChange(next);
              return;
            }
            const provider = options.find(
              (candidate) => candidate.providerAccountId === next.providerAccountId
            );
            const profile = provider?.profiles.find(
              (candidate) => candidate.id === next.modelProfile
            );
            const efforts = profile?.reasoningEfforts ?? [];
            onChange({
              ...next,
              reasoningEffort: efforts.length > 0
                ? profile?.defaultReasoningEffort ?? efforts[0] ?? null
                : null
            });
          }}
        />
        {reasoningEfforts.length > 0 ? (
          <Selector
            isLabelHidden
            label={`${ariaLabel} reasoning`}
            options={reasoningOptions}
            placement="below"
            value={selection.reasoningEffort ?? undefined}
            style={selectorTransitionStyle}
            isDisabled={disabled}
            onChange={(value) => onChange({
              ...selection,
              reasoningEffort: value as ReasoningEffort
            })}
          />
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
        reasoningEffort: null
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
        modelProfile: parsed[2]
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

const styles = stylex.create({
  field: {
    minWidth: 0,
    justifyContent: "flex-end"
  },
  controls: {
    justifyContent: "flex-end"
  },
  closedValueSelector: {
    height: "auto",
    minHeight: "calc(var(--size-element-md) + var(--spacing-4))",
    color: "transparent"
  },
  closedValue: {
    position: "absolute",
    insetInlineStart: "var(--spacing-3)",
    insetInlineEnd: "calc(var(--spacing-3) + var(--size-element-sm))",
    top: "50%",
    minWidth: 0,
    pointerEvents: "none",
    transform: "translateY(-50%)"
  }
});

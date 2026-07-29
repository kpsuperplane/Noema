import { Badge } from "@astryxdesign/core/Badge";
import { Selector, type SelectorOptionType } from "@astryxdesign/core/Selector";
import { HStack } from "@astryxdesign/core/Stack";
import * as stylex from "@stylexjs/stylex";
import type { CSSProperties } from "react";
import type { ReasoningEffort } from "@/generated/graphql";
import type { ModelPreferenceSaveInput, ModelProviderOption } from "./modelPreferenceTypes";

export function ControlledModelPreferenceSelect({
  options,
  selection,
  disabled = false,
  ariaLabel,
  originLabel,
  onChange
}: {
  options: readonly ModelProviderOption[];
  selection: ModelPreferenceSaveInput;
  disabled?: boolean;
  ariaLabel: string;
  originLabel?: string | null;
  onChange: (selection: ModelPreferenceSaveInput) => void;
}) {
  const modelOptions: SelectorOptionType[] = options.map((provider) => ({
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
  }));
  const selectedProvider = options.find(
    (provider) => provider.providerAccountId === selection.providerAccountId
  );
  const selectedProfile = selectedProvider?.profiles.find(
    (profile) => profile.id === selection.modelProfile
  );
  const reasoningEfforts = selectedProfile?.reasoningEfforts ?? [];
  const reasoningOptions: SelectorOptionType[] = reasoningEfforts.map((effort) => ({
    value: effort,
    label: reasoningEffortLabel(effort)
  }));
  const selectedValue = selection.providerAccountId && selection.modelProfile
    ? modelOptionValue(selection)
    : "";

  return (
    <HStack gap={2} wrap="wrap" vAlign="center" {...stylex.props(styles.field)}>
      {originLabel ? (
        <HStack vAlign="center" gap={1.5}>
          <span {...stylex.props(styles.fieldLabel)}>Model</span>
          <Badge variant="neutral" label={originLabel} />
        </HStack>
      ) : null}
      <HStack gap={2} wrap="wrap" {...stylex.props(styles.controls)}>
        <Selector
          isLabelHidden
          label={ariaLabel}
          options={modelOptions}
          hasSearch={options.reduce((count, provider) => count + provider.profiles.length, 0) > 8}
          placement="below"
          placeholder={options.length === 0 ? "No models available" : "Select a model"}
          value={selectedValue || undefined}
          style={selectorTransitionStyle}
          isDisabled={disabled}
          onChange={(value) => {
            const next = parseModelOptionValue(value);
            if (!next || modelOptionValue(next) === selectedValue) return;
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

function modelOptionValue(selection: ModelPreferenceSaveInput) {
  return JSON.stringify([selection.providerAccountId, selection.modelProfile]);
}

function parseModelOptionValue(value: string): ModelPreferenceSaveInput | null {
  try {
    const parsed: unknown = JSON.parse(value);
    if (Array.isArray(parsed) && typeof parsed[0] === "string" && typeof parsed[1] === "string") {
      return { providerAccountId: parsed[0], modelProfile: parsed[1] };
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
  fieldLabel: {
    fontSize: 12,
    fontWeight: 600,
    lineHeight: 1.3,
    color: "var(--muted-foreground)"
  }
});

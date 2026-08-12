import * as stylex from "@stylexjs/stylex";
import { Button } from "@astryxdesign/core/Button";
import { NumberInput } from "@astryxdesign/core/NumberInput";
import { VStack } from "@astryxdesign/core/VStack";
import * as React from "react";
import { SettingsEditDialog } from "./SettingsEditDialog";
import { SettingsList, SettingsListItem, SettingsLocalFeedback, SettingsSection, SettingsSectionInset } from "./SettingsPrimitives";

export type TaskExecutionPolicyValue = {
  maxProviderContinuations: number;
  maxToolCalls: number;
  maxActiveMinutes: number;
  progressAuditInterval: number;
};

type Draft = TaskExecutionPolicyValue;

export function TaskExecutionPolicySettings({
  policy,
  loading,
  error,
  saving,
  saveError,
  onUpdate
}: {
  policy: TaskExecutionPolicyValue | null;
  loading: boolean;
  error: string | null;
  saving: boolean;
  saveError: string | null;
  onUpdate: (value: TaskExecutionPolicyValue) => Promise<unknown>;
}) {
  const [draftOverride, setDraftOverride] = React.useState<Draft | null>(null);
  const [validationError, setValidationError] = React.useState<string | null>(null);
  const [editOpen, setEditOpen] = React.useState(false);
  const draft = draftOverride ?? policyDraft(policy);

  const submit = async () => {
    const parsed = parsePolicyDraft(draft);
    if (!parsed.value) {
      setValidationError(parsed.error);
      return;
    }
    setValidationError(null);
    await onUpdate(parsed.value);
    setDraftOverride(null);
    setEditOpen(false);
  };

  return (
    <SettingsSection
      title="Run limits"
      titleId="task-execution-policy-title"
      action={<Button
            type="button"
            size="sm"
            variant="secondary"
            label="Edit limits"
            isDisabled={!policy || loading}
            onClick={() => {
              setDraftOverride(policyDraft(policy));
              setValidationError(null);
              setEditOpen(true);
            }}
          />}
    >
        {loading && !policy ? (
          <SettingsSectionInset><p {...stylex.props(styles.muted)}>Loading execution limits...</p></SettingsSectionInset>
        ) : error && !policy ? (
          <SettingsSectionInset><p role="alert" {...stylex.props(styles.error)}>Execution limits could not be loaded.</p></SettingsSectionInset>
        ) : (
          <SettingsList density="balanced" hasDividers>
            <PolicyRow label="Provider continuations" value={policy?.maxProviderContinuations} />
            <PolicyRow label="Tool calls" value={policy?.maxToolCalls} />
            <PolicyRow label="Active minutes" value={policy?.maxActiveMinutes} />
            <PolicyRow label="Audit interval" value={policy?.progressAuditInterval} />
          </SettingsList>
        )}
        {error && policy ? <SettingsLocalFeedback><p role="alert" {...stylex.props(styles.error)}>Execution limits could not refresh.</p></SettingsLocalFeedback> : null}
        <SettingsEditDialog
          title="Edit execution limits"
          open={editOpen}
          saving={saving}
          saveLabel="Save limits"
          saveDisabled={!policy}
          error={validationError || saveError}
          width={560}
          onOpenChange={(open) => {
            setEditOpen(open);
            if (!open) {
              setDraftOverride(null);
              setValidationError(null);
            }
          }}
          onSave={submit}
        >
          <VStack gap={3}>
            <NumberField
              hasAutoFocus
              description="Model/tool continuation rounds"
              label="Provider continuations"
              value={draft.maxProviderContinuations}
              onChange={(value) => setDraftOverride({ ...draft, maxProviderContinuations: value })}
            />
            <NumberField
              description="Calls across the complete run"
              label="Tool calls"
              value={draft.maxToolCalls}
              onChange={(value) => setDraftOverride({ ...draft, maxToolCalls: value })}
            />
            <NumberField
              description="Queue time does not count"
              label="Active minutes"
              value={draft.maxActiveMinutes}
              onChange={(value) => setDraftOverride({ ...draft, maxActiveMinutes: value })}
            />
            <NumberField
              description="Continuations between audits"
              label="Progress audit interval"
              value={draft.progressAuditInterval}
              onChange={(value) => setDraftOverride({ ...draft, progressAuditInterval: value })}
            />
          </VStack>
        </SettingsEditDialog>
    </SettingsSection>
  );
}

function PolicyRow({ label, value }: { label: string; value?: number }) {
  return (
    <SettingsListItem
      label={label}
      description={policyDescription(label)}
      endContent={<span {...stylex.props(styles.policyValue)}>{value ?? "—"}</span>}
    />
  );
}

function policyDescription(label: string) {
  switch (label) {
    case "Provider continuations":
      return "Model and tool continuation rounds in one task run.";
    case "Tool calls":
      return "Total tool calls allowed across the complete run.";
    case "Active minutes":
      return "Time spent executing; queue time does not count.";
    case "Audit interval":
      return "Continuation rounds between progress audits.";
    default:
      return undefined;
  }
}

function NumberField({
  hasAutoFocus = false,
  label,
  description,
  value,
  onChange
}: {
  hasAutoFocus?: boolean;
  label: string;
  description: string;
  value: number;
  onChange: (value: number) => void;
}) {
  return (
    <NumberInput
      hasAutoFocus={hasAutoFocus}
      label={label}
      description={description}
      min={1}
      step={1}
      isIntegerOnly
      value={value}
      onChange={onChange}
    />
  );
}

function policyDraft(policy: TaskExecutionPolicyValue | null): Draft {
  return {
    maxProviderContinuations: policy?.maxProviderContinuations ?? 80,
    maxToolCalls: policy?.maxToolCalls ?? 400,
    maxActiveMinutes: policy?.maxActiveMinutes ?? 120,
    progressAuditInterval: policy?.progressAuditInterval ?? 20
  };
}

function parsePolicyDraft(
  draft: Draft
): { value: TaskExecutionPolicyValue; error: null } | { value: null; error: string } {
  const values = draft;
  if (Object.values(values).some((value) => !Number.isSafeInteger(value) || value < 1)) {
    return { value: null, error: "Every execution limit must be a positive whole number." };
  }
  if (values.progressAuditInterval > values.maxProviderContinuations) {
    return { value: null, error: "The progress audit interval cannot exceed the continuation limit." };
  }
  return { value: values, error: null };
}

const styles = stylex.create({
  policyValue: {
    color: "var(--foreground)",
    fontFamily: "var(--font-mono)",
    fontSize: 14,
    fontWeight: 650,
    fontVariantNumeric: "tabular-nums"
  },
  muted: { margin: "var(--spacing-0)", color: "var(--muted-foreground)", fontSize: 13 },
  error: { margin: "var(--spacing-0)", color: "var(--destructive)", fontSize: 13, lineHeight: 1.45 }
});

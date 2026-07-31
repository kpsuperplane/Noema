import * as stylex from "@stylexjs/stylex";
import { Button } from "@astryxdesign/core/Button";
import { HStack } from "@astryxdesign/core/HStack";
import { List, ListItem } from "@astryxdesign/core/List";
import { Section } from "@astryxdesign/core/Section";
import { VStack } from "@astryxdesign/core/VStack";
import * as React from "react";
import { SettingsEditDialog } from "./SettingsEditDialog";

export type TaskExecutionPolicyValue = {
  maxProviderContinuations: number;
  maxToolCalls: number;
  maxActiveMinutes: number;
  progressAuditInterval: number;
};

type Draft = Record<keyof TaskExecutionPolicyValue, string>;

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
    <Section variant="transparent" padding={0} aria-labelledby="task-execution-policy-title">
      <VStack gap={2}>
        <HStack wrap="wrap" gap={3} vAlign="start" hAlign="between">
          <VStack gap={1}>
            <h2 id="task-execution-policy-title" {...stylex.props(styles.sectionTitle)}>
              Run limits
            </h2>
            <p {...stylex.props(styles.description)}>
              Global safety ceilings for every task executor. Complexity changes the model, not these limits.
            </p>
          </VStack>
          <Button
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
          />
        </HStack>
        {loading && !policy ? (
          <p {...stylex.props(styles.muted)}>Loading execution limits...</p>
        ) : error && !policy ? (
          <p role="alert" {...stylex.props(styles.error)}>Execution limits could not be loaded.</p>
        ) : (
          <List density="balanced" hasDividers>
            <PolicyRow label="Provider continuations" value={policy?.maxProviderContinuations} />
            <PolicyRow label="Tool calls" value={policy?.maxToolCalls} />
            <PolicyRow label="Active minutes" value={policy?.maxActiveMinutes} />
            <PolicyRow label="Audit interval" value={policy?.progressAuditInterval} />
          </List>
        )}
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
      </VStack>
    </Section>
  );
}

function PolicyRow({ label, value }: { label: string; value?: number }) {
  return (
    <ListItem
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
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <label {...stylex.props(styles.field)}>
      <span {...stylex.props(styles.fieldLabel)}>{label}</span>
      <input
        data-autofocus={hasAutoFocus || undefined}
        inputMode="numeric"
        min={1}
        step={1}
        type="number"
        value={value}
        {...stylex.props(styles.input)}
        onChange={(event) => onChange(event.currentTarget.value)}
      />
      <span {...stylex.props(styles.fieldDescription)}>{description}</span>
    </label>
  );
}

function policyDraft(policy: TaskExecutionPolicyValue | null): Draft {
  return {
    maxProviderContinuations: String(policy?.maxProviderContinuations ?? 80),
    maxToolCalls: String(policy?.maxToolCalls ?? 400),
    maxActiveMinutes: String(policy?.maxActiveMinutes ?? 120),
    progressAuditInterval: String(policy?.progressAuditInterval ?? 20)
  };
}

function parsePolicyDraft(
  draft: Draft
): { value: TaskExecutionPolicyValue; error: null } | { value: null; error: string } {
  const values = Object.fromEntries(
    Object.entries(draft).map(([key, value]) => [key, Number(value)])
  ) as TaskExecutionPolicyValue;
  if (Object.values(values).some((value) => !Number.isSafeInteger(value) || value < 1)) {
    return { value: null, error: "Every execution limit must be a positive whole number." };
  }
  if (values.progressAuditInterval > values.maxProviderContinuations) {
    return { value: null, error: "The progress audit interval cannot exceed the continuation limit." };
  }
  return { value: values, error: null };
}

const styles = stylex.create({
  sectionTitle: {
    margin: "var(--spacing-0)",
    fontFamily: "var(--font-heading)",
    fontSize: 16,
    lineHeight: 1.3,
    color: "var(--foreground)"
  },
  description: {
    margin: "var(--spacing-0)",
    maxWidth: 640,
    color: "var(--muted-foreground)",
    fontSize: 13,
    lineHeight: 1.5,
    textWrap: "pretty"
  },
  policyValue: {
    color: "var(--foreground)",
    fontFamily: "var(--font-mono)",
    fontSize: 14,
    fontWeight: 650,
    fontVariantNumeric: "tabular-nums"
  },
  field: { display: "grid", gap: "calc(var(--spacing-1) + 1px)", minWidth: 0 },
  fieldLabel: { color: "var(--foreground)", fontSize: 12, fontWeight: 650 },
  fieldDescription: { color: "var(--muted-foreground)", fontSize: 11, lineHeight: 1.35 },
  input: {
    boxSizing: "border-box",
    width: "100%",
    minHeight: 32,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border)",
    borderRadius: 5,
    backgroundColor: "white",
    paddingInline: "calc(var(--spacing-2) + 1px)",
    color: "var(--foreground)",
    fontFamily: "var(--font-mono)",
    fontSize: 13,
    fontVariantNumeric: "tabular-nums",
    ":focus-visible": {
      outlineWidth: 3,
      outlineStyle: "solid",
      outlineColor: "color-mix(in srgb, var(--accent) 24%, transparent)",
      outlineOffset: 1
    }
  },
  muted: { margin: "var(--spacing-0)", color: "var(--muted-foreground)", fontSize: 13 },
  error: { margin: "var(--spacing-0)", color: "var(--destructive)", fontSize: 13, lineHeight: 1.45 }
});

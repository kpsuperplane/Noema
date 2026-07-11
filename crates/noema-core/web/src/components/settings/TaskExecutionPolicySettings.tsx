import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import * as React from "react";

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
  };

  return (
    <section aria-labelledby="task-execution-policy-title" {...stylex.props(styles.section)}>
      <div {...stylex.props(styles.header)}>
        <div {...stylex.props(styles.copy)}>
          <h3 id="task-execution-policy-title" {...stylex.props(styles.title)}>Execution limits</h3>
          <p {...stylex.props(styles.description)}>
            Global safety ceilings for every task executor. Complexity changes the model, not these limits.
          </p>
        </div>
      </div>
      {loading && !policy ? (
        <p {...stylex.props(styles.muted)}>Loading execution limits...</p>
      ) : error && !policy ? (
        <p role="alert" {...stylex.props(styles.error)}>Execution limits could not be loaded.</p>
      ) : (
        <form
          {...stylex.props(styles.form)}
          onSubmit={(event) => {
            event.preventDefault();
            void submit();
          }}
        >
          <div {...stylex.props(styles.fields)}>
            <NumberField
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
          </div>
          {validationError || saveError ? (
            <p role="alert" {...stylex.props(styles.error)}>{validationError || saveError}</p>
          ) : null}
          <div {...stylex.props(styles.actions)}>
            <Button
              clickAction={submit}
              isDisabled={!policy || saving}
              isLoading={saving}
              label="Save limits"
              size="sm"
              variant="primary"
            />
          </div>
        </form>
      )}
    </section>
  );
}

function NumberField({
  label,
  description,
  value,
  onChange
}: {
  label: string;
  description: string;
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <label {...stylex.props(styles.field)}>
      <span {...stylex.props(styles.fieldLabel)}>{label}</span>
      <input
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
  section: { display: "grid", gap: 14, borderTopWidth: 1, borderTopStyle: "solid", borderTopColor: "var(--border-subtle)", paddingTop: 14 },
  header: { display: "flex", flexWrap: "wrap", alignItems: "start", justifyContent: "space-between", gap: 14 },
  copy: { display: "grid", flex: "1 1 360px", gap: 6, minWidth: 0 },
  title: { margin: 0, fontFamily: "var(--font-heading)", fontSize: 16, lineHeight: 1.25, color: "var(--foreground)" },
  description: { margin: 0, maxWidth: 640, color: "var(--muted-foreground)", fontSize: 13, lineHeight: 1.5, textWrap: "pretty" },
  form: { display: "grid", gap: 12 },
  fields: { display: "grid", gridTemplateColumns: "repeat(2, minmax(0, 1fr))", gap: 12, "@media (max-width: 620px)": { gridTemplateColumns: "1fr" } },
  field: { display: "grid", gap: 5, minWidth: 0 },
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
    paddingInline: 9,
    color: "var(--foreground)",
    fontFamily: "var(--font-mono)",
    fontSize: 13,
    fontVariantNumeric: "tabular-nums",
    ":focus-visible": { outlineWidth: 3, outlineStyle: "solid", outlineColor: "color-mix(in srgb, var(--accent) 24%, transparent)", outlineOffset: 1 }
  },
  actions: { display: "flex", justifyContent: "end" },
  muted: { margin: 0, color: "var(--muted-foreground)", fontSize: 13 },
  error: { margin: 0, color: "var(--destructive)", fontSize: 13, lineHeight: 1.45 }
});

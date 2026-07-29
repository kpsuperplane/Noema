import { Badge } from "@astryxdesign/core/Badge";
import { Button } from "@astryxdesign/core/Button";
import { Selector, type SelectorOptionType } from "@astryxdesign/core/Selector";
import { TextInput } from "@astryxdesign/core/TextInput";
import * as stylex from "@stylexjs/stylex";
import { Pencil } from "lucide-react";
import { useMemo, useState } from "react";
import type {
  ReasoningEffort,
  TaskComplexity,
  TaskModelPoolEntryInput,
  TaskModelPoolsQuery
} from "@/generated/graphql";
import type { ModelProviderOption } from "./modelPreferenceTypes";
import { SettingsEditDialog } from "./SettingsEditDialog";

type PoolEntry = TaskModelPoolsQuery["taskModelPools"][number];
const complexities: readonly TaskComplexity[] = ["SIMPLE", "MEDIUM", "DIFFICULT"];

export function TaskModelPoolsSettings({
  entries,
  modelOptions,
  loading,
  error,
  saving,
  saveError,
  onUpdate
}: {
  entries: readonly PoolEntry[];
  modelOptions: readonly ModelProviderOption[];
  loading: boolean;
  error: string | null;
  saving: boolean;
  saveError: string | null;
  onUpdate: (poolEntryId: string, input: TaskModelPoolEntryInput) => Promise<unknown>;
}) {
  const [editingId, setEditingId] = useState<string | null>(null);
  const enabledEntryCount = entries.filter((entry) => entry.enabled).length;
  const editingEntry = entries.find((entry) => entry.poolEntryId === editingId) ?? null;

  const beginEdit = (entry: PoolEntry) => {
    setEditingId(entry.poolEntryId);
  };

  const finishAction = () => {
    setEditingId(null);
  };

  return (
    <section aria-labelledby="task-model-pools-title" {...stylex.props(styles.section)}>
      <div {...stylex.props(styles.header)}>
        <div {...stylex.props(styles.headerCopy)}>
          <div {...stylex.props(styles.titleRow)}>
            <h3 id="task-model-pools-title" {...stylex.props(styles.title)}>Models</h3>
            <Badge variant={enabledEntryCount > 0 ? "success" : "warning"} label={`${enabledEntryCount}/3 enabled`} />
          </div>
          <p {...stylex.props(styles.description)}>
            Every task executor uses the model configured for its complexity tier.
          </p>
        </div>
      </div>
      {saveError ? <p role="alert" {...stylex.props(styles.error)}>{saveError}</p> : null}
      {loading && entries.length === 0 ? (
        <p {...stylex.props(styles.muted)}>Loading task model pools...</p>
      ) : error ? (
        <p role="alert" {...stylex.props(styles.muted)}>Task model pools could not be loaded.</p>
      ) : (
        <div {...stylex.props(styles.tiers)}>
          {complexities.map((complexity) => {
            const entry = entries.find((entry) => entry.complexity === complexity);
            return (
              <div key={complexity} {...stylex.props(styles.tier)}>
                <div {...stylex.props(styles.tierHeader)}>
                  <h3 {...stylex.props(styles.tierTitle)}>{complexityLabel(complexity)}</h3>
                  <span {...stylex.props(styles.tierHint)}>Global setting</span>
                </div>
                {!entry ? (
                  <p {...stylex.props(styles.empty)}>This task model setting is unavailable.</p>
                ) : (
                  <PoolEntryRow entry={entry} onEdit={beginEdit} />
                )}
              </div>
            );
          })}
        </div>
      )}
      {editingEntry ? (
        <PoolEntryEditor
          key={editingEntry.poolEntryId}
          entry={editingEntry}
          modelOptions={modelOptions}
          saving={saving}
          onCancel={finishAction}
          onSave={async (input) => {
            await onUpdate(editingEntry.poolEntryId, input);
            finishAction();
          }}
        />
      ) : null}
    </section>
  );
}

function PoolEntryRow({
  entry,
  onEdit
}: {
  entry: PoolEntry;
  onEdit: (entry: PoolEntry) => void;
}) {
  return (
    <article {...stylex.props(styles.entry, !entry.enabled && styles.disabledEntry)}>
      <div {...stylex.props(styles.entryCopy)}>
        <div {...stylex.props(styles.entryTitleRow)}>
          <strong {...stylex.props(styles.entryTitle)}>{entry.label || entry.modelProfile}</strong>
          <Badge variant="neutral" label={entry.isOverride ? "Override" : "Default"} />
          <Badge variant={entry.enabled ? "success" : "neutral"} label={entry.enabled ? "Enabled" : "Disabled"} />
        </div>
        <p {...stylex.props(styles.entryMeta)}>
          {entry.providerKind} · {entry.modelProfile}
          {entry.reasoningEffort ? ` · ${entry.reasoningEffort.toLowerCase()}` : ""}
        </p>
      </div>
      <div {...stylex.props(styles.entryActions)}>
        <Button
          icon={<Pencil aria-hidden="true" size={13} />}
          isIconOnly
          label={`Edit ${entry.label || entry.modelProfile}`}
          onClick={() => onEdit(entry)}
          size="sm"
          variant="ghost"
        />
      </div>
    </article>
  );
}

function PoolEntryEditor({
  entry,
  modelOptions,
  saving,
  onSave,
  onCancel
}: {
  entry: PoolEntry;
  modelOptions: readonly ModelProviderOption[];
  saving: boolean;
  onSave: (input: TaskModelPoolEntryInput) => Promise<void>;
  onCancel: () => void;
}) {
  const initialModel = { providerKind: entry.providerKind, providerAccountId: entry.providerAccountId, modelProfile: entry.modelProfile, reasoningEffort: entry.reasoningEffort };
  const [label, setLabel] = useState(entry.label ?? "");
  const [providerKind, setProviderKind] = useState(initialModel.providerKind);
  const [providerAccountId, setProviderAccountId] = useState(initialModel.providerAccountId);
  const [modelProfile, setModelProfile] = useState(initialModel.modelProfile);
  const [reasoningEffort, setReasoningEffort] = useState<ReasoningEffort | null>(initialModel.reasoningEffort ?? null);
  const [enabled, setEnabled] = useState(entry.enabled);
  const [error, setError] = useState<string | null>(null);
  const selectedProvider = modelOptions.find(
    (option) => option.providerKind === providerKind && option.providerAccountId === providerAccountId
  );
  const selectedProfile = selectedProvider?.profiles.find((profile) => profile.id === modelProfile);
  const reasoningOptions = selectedProfile?.reasoningEfforts ?? [];
  const modelSelectorOptions = useMemo<SelectorOptionType[]>(
    () => modelOptions.flatMap((provider) => provider.profiles.map((profile) => ({
      value: modelOptionValue(provider.providerKind, provider.providerAccountId, profile.id),
      label: `${provider.providerDisplayName} · ${profile.label}`,
      disabled: Boolean(provider.disabledReason || profile.disabledReason)
    }))),
    [modelOptions]
  );
  const selectedModelValue = providerKind && providerAccountId && modelProfile
    ? modelOptionValue(providerKind, providerAccountId, modelProfile)
    : undefined;
  const canSave = Boolean(providerKind && providerAccountId && modelProfile && !saving);

  const handleModelChange = (value: string) => {
    const next = parseModelValue(value);
    if (!next) return;
    setProviderKind(next.providerKind);
    setProviderAccountId(next.providerAccountId);
    setModelProfile(next.modelProfile);
    const profile = modelOptions
      .find((option) => option.providerKind === next.providerKind && option.providerAccountId === next.providerAccountId)
      ?.profiles.find((candidate) => candidate.id === next.modelProfile);
    setReasoningEffort(profile?.defaultReasoningEffort ?? profile?.reasoningEfforts[0] ?? null);
  };

  const submit = async () => {
    if (!canSave) return;
    setError(null);
    try {
      await onSave({ complexity: entry.complexity, label: label.trim() || null, providerKind, providerAccountId, modelProfile, reasoningEffort, enabled, sortOrder: 0 });
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : "Noema could not save this model.");
    }
  };

  return (
    <SettingsEditDialog
      title={`Edit ${complexityLabel(entry.complexity)} task model`}
      open
      saving={saving}
      saveLabel="Save changes"
      saveDisabled={!canSave}
      error={error}
      width={600}
      onOpenChange={(open) => {
        if (!open) onCancel();
      }}
      onSave={submit}
    >
      <div {...stylex.props(styles.fields)}>
        <Selector
          data-autofocus
          label="Model"
          options={modelSelectorOptions}
          onChange={handleModelChange}
          placement="below"
          placeholder={modelSelectorOptions.length === 0 ? "No models available" : "Select a model"}
          value={selectedModelValue}
          width="100%"
        />
        {reasoningOptions.length > 0 ? (
          <Selector
            label="Reasoning effort"
            options={reasoningOptions.map((value) => ({ value, label: value.toLowerCase() }))}
            onChange={(value) => setReasoningEffort(value as ReasoningEffort)}
            placement="below"
            value={reasoningEffort ?? undefined}
            width="100%"
          />
        ) : null}
        <TextInput label="Label" onChange={setLabel} placeholder="Optional label" value={label} width="100%" />
        <label {...stylex.props(styles.checkbox)}>
          <input checked={enabled} onChange={(event) => setEnabled(event.target.checked)} type="checkbox" />
          <span>Enabled for new tasks</span>
        </label>
      </div>
    </SettingsEditDialog>
  );
}

function modelOptionValue(providerKind: string, providerAccountId: string, modelProfile: string): string {
  return JSON.stringify([providerKind, providerAccountId, modelProfile]);
}

function parseModelValue(value: string): { providerKind: string; providerAccountId: string; modelProfile: string } | null {
  try {
    const parsed: unknown = JSON.parse(value);
    if (Array.isArray(parsed) && parsed.every((item) => typeof item === "string") && parsed.length === 3) {
      return { providerKind: parsed[0], providerAccountId: parsed[1], modelProfile: parsed[2] };
    }
  } catch {
    return null;
  }
  return null;
}

function complexityLabel(value: TaskComplexity): string {
  if (value === "DIFFICULT") return "High";
  return value.charAt(0) + value.slice(1).toLowerCase();
}

const styles = stylex.create({
  section: { display: "grid", gap: "var(--spacing-4)", borderTopWidth: 1, borderTopStyle: "solid", borderTopColor: "var(--border-subtle)", paddingTop: "calc(var(--spacing-3) + var(--spacing-0-5))" },
  header: { display: "flex", flexWrap: "wrap", alignItems: "start", justifyContent: "space-between", gap: "calc(var(--spacing-3) + var(--spacing-0-5))" },
  headerCopy: { display: "grid", flex: "1 1 360px", gap: "var(--spacing-1-5)", minWidth: 0 },
  titleRow: { display: "flex", flexWrap: "wrap", alignItems: "center", gap: "calc(var(--spacing-2) + var(--spacing-0-5))" },
  title: { margin: "var(--spacing-0)", fontFamily: "var(--font-heading)", fontSize: 16, lineHeight: 1.25, color: "var(--foreground)" },
  description: { margin: "var(--spacing-0)", maxWidth: 640, color: "var(--muted-foreground)", fontSize: 13, lineHeight: 1.5 },
  tiers: { display: "grid", gap: "calc(var(--spacing-3) + var(--spacing-0-5))" },
  tier: { display: "grid", gap: "var(--spacing-2)" },
  tierHeader: { display: "flex", alignItems: "baseline", justifyContent: "space-between", gap: "var(--spacing-2)" },
  tierTitle: { margin: "var(--spacing-0)", color: "var(--foreground)", fontSize: 14, fontWeight: 650 },
  tierHint: { color: "var(--muted-foreground)", fontSize: 12 },
  empty: { margin: "var(--spacing-0)", borderRadius: 6, backgroundColor: "var(--surface-sunken, #f8f8f8)", padding: "calc(var(--spacing-2) + var(--spacing-0-5))", color: "var(--muted-foreground)", fontSize: 12 },
  entryList: { display: "grid", gap: "var(--spacing-1-5)" },
  entry: { display: "flex", flexWrap: "wrap", alignItems: "center", justifyContent: "space-between", gap: "calc(var(--spacing-2) + var(--spacing-0-5))", borderWidth: 1, borderStyle: "solid", borderColor: "var(--border-subtle)", borderRadius: 6, padding: "calc(var(--spacing-2) + var(--spacing-0-5))" },
  disabledEntry: { opacity: 0.68 },
  entryCopy: { display: "grid", flex: "1 1 220px", gap: "calc(var(--spacing-1) - 1px)", minWidth: 0 },
  entryTitleRow: { display: "flex", flexWrap: "wrap", alignItems: "center", gap: "var(--spacing-2)" },
  entryTitle: { minWidth: 0, color: "var(--foreground)", fontSize: 13, overflowWrap: "anywhere" },
  entryMeta: { margin: "var(--spacing-0)", color: "var(--muted-foreground)", fontFamily: "var(--font-mono)", fontSize: 11, overflowWrap: "anywhere" },
  entryActions: { display: "flex", alignItems: "center", gap: "var(--spacing-0-5)" },
  fields: { display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(180px, 1fr))", alignItems: "end", gap: "calc(var(--spacing-2) + var(--spacing-0-5))" },
  checkbox: { display: "inline-flex", alignItems: "center", gap: "calc(var(--spacing-1-5) + 1px)", minHeight: 32, color: "var(--foreground)", fontSize: 12 },
  muted: { margin: "var(--spacing-0)", color: "var(--muted-foreground)", fontSize: 13 },
  error: { margin: "var(--spacing-0)", color: "var(--destructive)", fontSize: 13, lineHeight: 1.45 }
});

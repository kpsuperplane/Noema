import { Badge } from "@astryxdesign/core/Badge";
import { Button } from "@astryxdesign/core/Button";
import { Selector, type SelectorOptionType } from "@astryxdesign/core/Selector";
import { TextInput } from "@astryxdesign/core/TextInput";
import * as stylex from "@stylexjs/stylex";
import { Pencil, Plus, Trash2, X } from "lucide-react";
import { useMemo, useState } from "react";
import type {
  ReasoningEffort,
  TaskComplexity,
  TaskModelPoolEntryInput,
  TaskModelPoolsQuery
} from "@/generated/graphql";
import type { ModelProviderOption } from "./modelPreferenceTypes";

type PoolEntry = TaskModelPoolsQuery["taskModelPools"][number];
type SavePoolEntry = (input: TaskModelPoolEntryInput) => Promise<unknown>;

const complexities: readonly TaskComplexity[] = ["SIMPLE", "MEDIUM", "DIFFICULT"];

export function TaskModelPoolsSettings({
  entries,
  modelOptions,
  loading,
  error,
  saving,
  saveError,
  onCreate,
  onUpdate,
  onDelete
}: {
  entries: readonly PoolEntry[];
  modelOptions: readonly ModelProviderOption[];
  loading: boolean;
  error: string | null;
  saving: boolean;
  saveError: string | null;
  onCreate: SavePoolEntry;
  onUpdate: (poolEntryId: string, input: TaskModelPoolEntryInput) => Promise<unknown>;
  onDelete: (poolEntryId: string) => Promise<unknown>;
}) {
  const [editingId, setEditingId] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);
  const [deletingId, setDeletingId] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const enabledEntryCount = entries.filter((entry) => entry.enabled).length;

  const beginAdd = () => {
    setActionError(null);
    setEditingId(null);
    setAdding(true);
  };

  const beginEdit = (entry: PoolEntry) => {
    setActionError(null);
    setAdding(false);
    setEditingId(entry.poolEntryId);
  };

  const finishAction = () => {
    setAdding(false);
    setEditingId(null);
  };

  const handleDelete = async (poolEntryId: string) => {
    setDeletingId(poolEntryId);
    setActionError(null);
    try {
      await onDelete(poolEntryId);
      if (editingId === poolEntryId) {
        setEditingId(null);
      }
    } catch (caught) {
      setActionError(caught instanceof Error ? caught.message : "Noema could not remove this model.");
    } finally {
      setDeletingId(null);
    }
  };

  return (
    <section aria-labelledby="task-model-pools-title" {...stylex.props(styles.card)}>
      <div {...stylex.props(styles.header)}>
        <div {...stylex.props(styles.headerCopy)}>
          <div {...stylex.props(styles.titleRow)}>
            <h2 id="task-model-pools-title" {...stylex.props(styles.title)}>Task executor pools</h2>
            <Badge variant={enabledEntryCount > 0 ? "success" : "warning"} label={enabledEntryCount > 0 ? `${enabledEntryCount} enabled` : "Delegation off"} />
          </div>
          <p {...stylex.props(styles.description)}>
            The primary agent picks one enabled model from the task&apos;s complexity tier when it delegates background work.
          </p>
        </div>
        <Button
          icon={<Plus aria-hidden="true" size={14} />}
          label="Add model"
          onClick={beginAdd}
          size="sm"
          variant="secondary"
        />
      </div>
      {saveError ? <p role="alert" {...stylex.props(styles.error)}>{saveError}</p> : null}
      {actionError ? <p role="alert" {...stylex.props(styles.error)}>{actionError}</p> : null}
      {loading && entries.length === 0 ? (
        <p {...stylex.props(styles.muted)}>Loading task model pools...</p>
      ) : error ? (
        <p role="alert" {...stylex.props(styles.muted)}>Task model pools could not be loaded.</p>
      ) : (
        <div {...stylex.props(styles.tiers)}>
          {complexities.map((complexity) => {
            const tierEntries = entries
              .filter((entry) => entry.complexity === complexity)
              .sort((left, right) => left.sortOrder - right.sortOrder);
            return (
              <div key={complexity} {...stylex.props(styles.tier)}>
                <div {...stylex.props(styles.tierHeader)}>
                  <h3 {...stylex.props(styles.tierTitle)}>{complexityLabel(complexity)}</h3>
                  <span {...stylex.props(styles.tierHint)}>{tierEntries.length} {tierEntries.length === 1 ? "entry" : "entries"}</span>
                </div>
                {tierEntries.length === 0 ? (
                  <p {...stylex.props(styles.empty)}>No models configured for this tier.</p>
                ) : (
                  <div {...stylex.props(styles.entryList)}>
                    {tierEntries.map((entry) => (
                      <PoolEntryRow
                        key={entry.poolEntryId}
                        deleting={deletingId === entry.poolEntryId}
                        entry={entry}
                        onDelete={handleDelete}
                        onEdit={beginEdit}
                      />
                    ))}
                  </div>
                )}
                {editingId && tierEntries.some((entry) => entry.poolEntryId === editingId) ? (
                  <PoolEntryEditor
                    entry={entries.find((entry) => entry.poolEntryId === editingId) ?? null}
                    modelOptions={modelOptions}
                    saving={saving}
                    onCancel={finishAction}
                    onSave={async (input) => {
                      await onUpdate(editingId, input);
                      finishAction();
                    }}
                  />
                ) : null}
              </div>
            );
          })}
        </div>
      )}
      {adding ? (
        <PoolEntryEditor
          entry={null}
          modelOptions={modelOptions}
          saving={saving}
          onCancel={finishAction}
          onSave={async (input) => {
            await onCreate(input);
            finishAction();
          }}
        />
      ) : null}
    </section>
  );
}

function PoolEntryRow({
  entry,
  deleting,
  onEdit,
  onDelete
}: {
  entry: PoolEntry;
  deleting: boolean;
  onEdit: (entry: PoolEntry) => void;
  onDelete: (poolEntryId: string) => Promise<void>;
}) {
  return (
    <article {...stylex.props(styles.entry, !entry.enabled && styles.disabledEntry)}>
      <div {...stylex.props(styles.entryCopy)}>
        <div {...stylex.props(styles.entryTitleRow)}>
          <strong {...stylex.props(styles.entryTitle)}>{entry.label || entry.modelProfile}</strong>
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
        <Button
          clickAction={() => onDelete(entry.poolEntryId)}
          icon={<Trash2 aria-hidden="true" size={13} />}
          isIconOnly
          isLoading={deleting}
          label={`Remove ${entry.label || entry.modelProfile}`}
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
  entry: PoolEntry | null;
  modelOptions: readonly ModelProviderOption[];
  saving: boolean;
  onSave: (input: TaskModelPoolEntryInput) => Promise<void>;
  onCancel: () => void;
}) {
  const initialModel = entry
    ? { providerKind: entry.providerKind, providerAccountId: entry.providerAccountId, modelProfile: entry.modelProfile, reasoningEffort: entry.reasoningEffort }
    : firstModelSelection(modelOptions);
  const [complexity, setComplexity] = useState<TaskComplexity>(entry?.complexity ?? "SIMPLE");
  const [label, setLabel] = useState(entry?.label ?? "");
  const [providerKind, setProviderKind] = useState(initialModel.providerKind);
  const [providerAccountId, setProviderAccountId] = useState(initialModel.providerAccountId);
  const [modelProfile, setModelProfile] = useState(initialModel.modelProfile);
  const [reasoningEffort, setReasoningEffort] = useState<ReasoningEffort | null>(initialModel.reasoningEffort ?? null);
  const [enabled, setEnabled] = useState(entry?.enabled ?? true);
  const [sortOrder, setSortOrder] = useState(String(entry?.sortOrder ?? 0));
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
    const order = Number.parseInt(sortOrder, 10);
    if (!Number.isFinite(order) || order < 0) {
      setError("Sort order must be a non-negative number.");
      return;
    }
    setError(null);
    try {
      await onSave({ complexity, label: label.trim() || null, providerKind, providerAccountId, modelProfile, reasoningEffort, enabled, sortOrder: order });
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : "Noema could not save this model.");
    }
  };

  return (
    <div {...stylex.props(styles.editor)}>
      <div {...stylex.props(styles.editorHeader)}>
        <strong {...stylex.props(styles.editorTitle)}>{entry ? "Edit pool entry" : "Add pool entry"}</strong>
        <Button icon={<X aria-hidden="true" size={14} />} isIconOnly label="Close editor" onClick={onCancel} size="sm" variant="ghost" />
      </div>
      {error ? <p role="alert" {...stylex.props(styles.error)}>{error}</p> : null}
      <div {...stylex.props(styles.fields)}>
        <Selector
          label="Complexity tier"
          options={complexities.map((value) => ({ value, label: complexityLabel(value) }))}
          onChange={(value) => setComplexity(value as TaskComplexity)}
          placement="below"
          value={complexity}
          width="100%"
        />
        <Selector
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
        <TextInput label="Sort order" onChange={setSortOrder} type="text" value={sortOrder} width="100%" />
        <label {...stylex.props(styles.checkbox)}>
          <input checked={enabled} onChange={(event) => setEnabled(event.target.checked)} type="checkbox" />
          <span>Enabled for new tasks</span>
        </label>
      </div>
      <div {...stylex.props(styles.editorActions)}>
        <Button label="Cancel" onClick={onCancel} size="sm" variant="ghost" />
        <Button clickAction={submit} isDisabled={!canSave} isLoading={saving} label={entry ? "Save changes" : "Add model"} size="sm" variant="primary" />
      </div>
    </div>
  );
}

function firstModelSelection(options: readonly ModelProviderOption[]) {
  for (const provider of options) {
    const profile = provider.profiles.find((candidate) => !candidate.disabledReason);
    if (profile && !provider.disabledReason) {
      return {
        providerKind: provider.providerKind,
        providerAccountId: provider.providerAccountId,
        modelProfile: profile.id,
        reasoningEffort: profile.defaultReasoningEffort ?? profile.reasoningEfforts[0] ?? null
      };
    }
  }
  return { providerKind: "", providerAccountId: "", modelProfile: "", reasoningEffort: null };
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
  return value.charAt(0) + value.slice(1).toLowerCase();
}

const styles = stylex.create({
  card: { display: "grid", gap: 16, borderWidth: 1, borderStyle: "solid", borderColor: "var(--border-subtle)", borderRadius: 6, backgroundColor: "white", padding: 16 },
  header: { display: "flex", flexWrap: "wrap", alignItems: "start", justifyContent: "space-between", gap: 14 },
  headerCopy: { display: "grid", flex: "1 1 360px", gap: 6, minWidth: 0 },
  titleRow: { display: "flex", flexWrap: "wrap", alignItems: "center", gap: 10 },
  title: { margin: 0, fontFamily: "var(--font-heading)", fontSize: 20, lineHeight: 1.25, color: "var(--foreground)" },
  description: { margin: 0, maxWidth: 640, color: "var(--muted-foreground)", fontSize: 13, lineHeight: 1.5 },
  tiers: { display: "grid", gap: 14 },
  tier: { display: "grid", gap: 8 },
  tierHeader: { display: "flex", alignItems: "baseline", justifyContent: "space-between", gap: 8 },
  tierTitle: { margin: 0, color: "var(--foreground)", fontSize: 14, fontWeight: 650 },
  tierHint: { color: "var(--muted-foreground)", fontSize: 12 },
  empty: { margin: 0, borderRadius: 6, backgroundColor: "var(--surface-sunken, #f8f8f8)", padding: 10, color: "var(--muted-foreground)", fontSize: 12 },
  entryList: { display: "grid", gap: 6 },
  entry: { display: "flex", flexWrap: "wrap", alignItems: "center", justifyContent: "space-between", gap: 10, borderWidth: 1, borderStyle: "solid", borderColor: "var(--border-subtle)", borderRadius: 6, padding: 10 },
  disabledEntry: { opacity: 0.68 },
  entryCopy: { display: "grid", flex: "1 1 220px", gap: 3, minWidth: 0 },
  entryTitleRow: { display: "flex", flexWrap: "wrap", alignItems: "center", gap: 8 },
  entryTitle: { minWidth: 0, color: "var(--foreground)", fontSize: 13, overflowWrap: "anywhere" },
  entryMeta: { margin: 0, color: "var(--muted-foreground)", fontFamily: "var(--font-mono)", fontSize: 11, overflowWrap: "anywhere" },
  entryActions: { display: "flex", alignItems: "center", gap: 2 },
  editor: { display: "grid", gap: 12, borderWidth: 1, borderStyle: "solid", borderColor: "var(--border-subtle)", borderRadius: 6, backgroundColor: "var(--surface-sunken, #f8f8f8)", padding: 12 },
  editorHeader: { display: "flex", alignItems: "center", justifyContent: "space-between", gap: 8 },
  editorTitle: { color: "var(--foreground)", fontSize: 13 },
  fields: { display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(180px, 1fr))", alignItems: "end", gap: 10 },
  checkbox: { display: "inline-flex", alignItems: "center", gap: 7, minHeight: 32, color: "var(--foreground)", fontSize: 12 },
  editorActions: { display: "flex", justifyContent: "end", gap: 8 },
  muted: { margin: 0, color: "var(--muted-foreground)", fontSize: 13 },
  error: { margin: 0, color: "var(--destructive)", fontSize: 13, lineHeight: 1.45 }
});

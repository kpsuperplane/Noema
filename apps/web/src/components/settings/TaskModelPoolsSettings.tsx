import * as stylex from "@stylexjs/stylex";
import { Badge } from "@astryxdesign/core/Badge";
import { Button } from "@astryxdesign/core/Button";
import { HStack } from "@astryxdesign/core/HStack";
import { List, ListItem } from "@astryxdesign/core/List";
import { Section } from "@astryxdesign/core/Section";
import { TextInput } from "@astryxdesign/core/TextInput";
import { VStack } from "@astryxdesign/core/VStack";
import { Pencil } from "lucide-react";
import { useState } from "react";
import type {
  NoemaModelUseCase,
  TaskComplexity,
  TaskModelPoolEntryInput,
  TaskModelPoolsQuery
} from "@/generated/graphql";
import { ControlledModelPreferenceSelect } from "./ControlledModelPreferenceSelect";
import type { ModelPreferenceSaveInput, ModelProviderOption } from "./modelPreferenceTypes";
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

  return (
    <Section variant="transparent" padding={0} aria-labelledby="task-model-pools-title">
      <VStack gap={2}>
        <HStack wrap="wrap" gap={3} vAlign="center" hAlign="between">
          <VStack gap={1}>
            <h2 id="task-model-pools-title" {...stylex.props(styles.sectionTitle)}>
              Task models
            </h2>
            <p {...stylex.props(styles.description)}>
              Choose the model used for each task complexity tier.
            </p>
          </VStack>
          <Badge
            variant={enabledEntryCount > 0 ? "success" : "warning"}
            label={`${enabledEntryCount}/3 enabled`}
          />
        </HStack>
        {saveError ? <p role="alert" {...stylex.props(styles.error)}>{saveError}</p> : null}
        {loading && entries.length === 0 ? (
          <p {...stylex.props(styles.muted)}>Loading task model pools...</p>
        ) : error ? (
          <p role="alert" {...stylex.props(styles.muted)}>Task model pools could not be loaded.</p>
        ) : (
          <List density="balanced" hasDividers>
            {complexities.map((complexity) => {
              const entry = entries.find((candidate) => candidate.complexity === complexity);
              return entry ? (
                <PoolEntryRow
                  key={complexity}
                  entry={entry}
                  modelOptions={modelOptions}
                  onEdit={setEditingId}
                />
              ) : (
                <ListItem
                  key={complexity}
                  label={complexityLabel(complexity)}
                  description="This task model setting is unavailable."
                />
              );
            })}
          </List>
        )}
        {editingEntry ? (
          <PoolEntryEditor
            key={editingEntry.poolEntryId}
            entry={editingEntry}
            modelOptions={modelOptions}
            saving={saving}
            onCancel={() => setEditingId(null)}
            onSave={async (input) => {
              await onUpdate(editingEntry.poolEntryId, input);
              setEditingId(null);
            }}
          />
        ) : null}
      </VStack>
    </Section>
  );
}

function PoolEntryRow({
  entry,
  modelOptions,
  onEdit
}: {
  entry: PoolEntry;
  modelOptions: readonly ModelProviderOption[];
  onEdit: (entryId: string) => void;
}) {
  const recommendation = modelOptions
    .find((option) => option.providerAccountId === entry.providerAccountId)
    ?.recommendations.find((item) => item.useCase === complexityUseCase(entry.complexity));
  const modelProfile = entry.selectionMode === "NOEMA_RECOMMENDED"
    ? recommendation?.modelProfile
    : entry.modelProfile;
  const reasoningEffort = entry.selectionMode === "NOEMA_RECOMMENDED"
    ? recommendation?.reasoningEffort
    : entry.reasoningEffort;
  const title = entry.label || (entry.selectionMode === "NOEMA_RECOMMENDED"
    ? "Noema Recommended"
    : modelProfile) || "Model unavailable";
  const description = [
    `${entry.providerKind} · ${modelProfile ?? "Unavailable"}`,
    reasoningEffort ? reasoningEffort.toLowerCase() : null
  ].filter(Boolean).join(" · ");

  return (
    <ListItem
      label={
        <HStack gap={2} vAlign="center" wrap="wrap">
          <span {...stylex.props(styles.rowLabel)}>{complexityLabel(entry.complexity)}</span>
          <Badge variant={entry.enabled ? "success" : "neutral"} label={entry.enabled ? "Enabled" : "Disabled"} />
        </HStack>
      }
      description={`${title} · ${description}`}
      endContent={
        <HStack wrap="wrap" gap={2} vAlign="center" {...stylex.props(styles.rowControl)}>
          <Button
            icon={<Pencil aria-hidden="true" size={13} />}
            isIconOnly
            label={`Edit ${complexityLabel(entry.complexity)} task model`}
            onClick={() => onEdit(entry.poolEntryId)}
            size="sm"
            variant="ghost"
          />
        </HStack>
      }
    />
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
  const [label, setLabel] = useState(entry.label ?? "");
  const [selection, setSelection] = useState<ModelPreferenceSaveInput>({
    providerAccountId: entry.providerAccountId,
    selectionMode: entry.selectionMode,
    modelProfile: entry.modelProfile,
    reasoningEffort: entry.reasoningEffort
  });
  const [enabled, setEnabled] = useState(entry.enabled);
  const [error, setError] = useState<string | null>(null);
  const selectedProvider = modelOptions.find(
    (option) => option.providerAccountId === selection.providerAccountId
  );
  const canSave = Boolean(
    selectedProvider &&
    (selection.selectionMode === "NOEMA_RECOMMENDED" || selection.modelProfile) &&
    !saving
  );

  const submit = async () => {
    if (!canSave) return;
    setError(null);
    try {
      await onSave({
        complexity: entry.complexity,
        label: label.trim() || null,
        providerKind: selectedProvider?.providerKind ?? entry.providerKind,
        providerAccountId: selection.providerAccountId,
        selectionMode: selection.selectionMode,
        modelProfile: selection.modelProfile ?? null,
        reasoningEffort: selection.reasoningEffort ?? null,
        enabled,
        sortOrder: 0
      });
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
      <VStack gap={3}>
        <ControlledModelPreferenceSelect
          ariaLabel="Model"
          options={modelOptions}
          selection={selection}
          useCase={complexityUseCase(entry.complexity)}
          onChange={setSelection}
        />
        <TextInput label="Label" onChange={setLabel} placeholder="Optional label" value={label} width="100%" />
        <label {...stylex.props(styles.checkbox)}>
          <input checked={enabled} onChange={(event) => setEnabled(event.target.checked)} type="checkbox" />
          <span>Enabled for new tasks</span>
        </label>
      </VStack>
    </SettingsEditDialog>
  );
}

function complexityUseCase(value: TaskComplexity): NoemaModelUseCase {
  return value === "SIMPLE"
    ? "TASK_SIMPLE"
    : value === "DIFFICULT"
      ? "TASK_DIFFICULT"
      : "TASK_MEDIUM";
}

function complexityLabel(value: TaskComplexity): string {
  if (value === "DIFFICULT") return "High";
  return value.charAt(0) + value.slice(1).toLowerCase();
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
    color: "var(--muted-foreground)",
    fontSize: 13,
    lineHeight: 1.5
  },
  rowLabel: { color: "var(--foreground)", fontWeight: 650 },
  rowControl: {
    justifyContent: "flex-end",
    "@media (max-width: 620px)": {
      width: "100%",
      justifyContent: "flex-start",
      marginInlineStart: "0"
    }
  },
  checkbox: {
    display: "inline-flex",
    alignItems: "center",
    gap: "calc(var(--spacing-1-5) + 1px)",
    minHeight: 32,
    color: "var(--foreground)",
    fontSize: 12
  },
  muted: { margin: "var(--spacing-0)", color: "var(--muted-foreground)", fontSize: 13 },
  error: { margin: "var(--spacing-0)", color: "var(--destructive)", fontSize: 13, lineHeight: 1.45 }
});

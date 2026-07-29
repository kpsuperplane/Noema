import { Button } from "@astryxdesign/core/Button";
import { Card } from "@astryxdesign/core/Card";
import { HStack, VStack } from "@astryxdesign/core/Stack";
import * as stylex from "@stylexjs/stylex";
import { useMemo, useState } from "react";
import type {
  ConfirmOnboardingModelSelectionsInput,
  OnboardingModelSelectionInput,
  OnboardingModelSetupQuery
} from "@/generated/graphql";
import { ControlledModelPreferenceSelect } from "../settings/ControlledModelPreferenceSelect";
import type { ModelPreferenceSaveInput, ModelProviderOption } from "../settings/modelPreferenceTypes";
import { ErrorMarker } from "../ErrorMarker";

type Setup = OnboardingModelSetupQuery["onboardingModelSetup"];
type Draft = Omit<ConfirmOnboardingModelSelectionsInput, "providerAccountId">;
type DraftKey = keyof Draft;

const groups: ReadonlyArray<{
  title: string;
  rows: ReadonlyArray<{ key: DraftKey; label: string; description: string }>;
}> = [
  {
    title: "Chat",
    rows: [{ key: "noema", label: "Noema", description: "Your main conversational model" }]
  },
  {
    title: "Tasks",
    rows: [
      { key: "simpleTasks", label: "Simple tasks", description: "Fast, routine task execution" },
      { key: "mediumTasks", label: "Medium tasks", description: "General task execution" },
      { key: "difficultTasks", label: "High tasks", description: "Complex task execution" },
      { key: "taskReviewer", label: "Task reviewer", description: "Reviews completed task work" }
    ]
  },
  {
    title: "Supporting work",
    rows: [
      { key: "webFetchSummarizer", label: "Web summaries", description: "Condenses fetched pages" },
      { key: "toolProgressAudit", label: "Progress checks", description: "Checks long-running task progress" },
      { key: "actionReviewer", label: "Action reviews", description: "Reviews governed actions" },
      { key: "memoryConsolidation", label: "Memory updates", description: "Maintains long-term memory" }
    ]
  }
];

export function ModelSetup({
  setup,
  saving,
  error,
  onConfirm,
  onUseDifferentProvider
}: {
  setup: Setup;
  saving: boolean;
  error: string | null;
  onConfirm: (input: ConfirmOnboardingModelSelectionsInput) => void;
  onUseDifferentProvider: () => void;
}) {
  const [draft, setDraft] = useState<Draft>(() => setup.proposedSelections);
  const provider = useMemo<ModelProviderOption>(() => ({
    providerKind: setup.providerKind,
    providerAccountId: setup.providerAccountId,
    providerDisplayName: setup.providerDisplayName,
    status: "AUTHENTICATED",
    profiles: setup.profiles
  }), [setup]);
  const reconciledDraft = reconcileDraft(draft, setup);

  const complete = groups.every(({ rows }) => rows.every(({ key }) => {
    if (key === "actionReviewer" && setup.providerKind === "local_models") return true;
    return isValidSelection(reconciledDraft[key], setup);
  }));

  return (
    <VStack as="section" {...stylex.props(styles.root)} aria-label="Model setup" gap={4}>
      <VStack gap={1.5} hAlign="center">
        <p {...stylex.props(styles.eyebrow)}>Connected to {setup.providerDisplayName}</p>
        <h1 {...stylex.props(styles.title)}>Review your models</h1>
        <p {...stylex.props(styles.description)}>
          These defaults cover chat, tasks, and supporting work. You can change them now or later.
        </p>
      </VStack>

      <Card padding={0} {...stylex.props(styles.form)}>
        {groups.map((group) => (
          <VStack as="section" key={group.title} gap={0} {...stylex.props(styles.group)}>
            <h2 {...stylex.props(styles.groupTitle)}>{group.title}</h2>
            {group.rows.map((row) => {
              const selection = reconciledDraft[row.key];
              const isHumanReview = row.key === "actionReviewer" && !selection;
              return (
                <section key={row.key} {...stylex.props(styles.row)}>
                  <VStack gap={0.5}>
                    <strong {...stylex.props(styles.rowLabel)}>{row.label}</strong>
                    <span {...stylex.props(styles.rowDescription)}>{row.description}</span>
                  </VStack>
                  {isHumanReview ? (
                    <span {...stylex.props(styles.humanReview)}>Ask me for approval</span>
                  ) : selection ? (
                    <ControlledModelPreferenceSelect
                      options={[provider]}
                      selection={toPreference(setup.providerAccountId, selection)}
                      disabled={saving}
                      ariaLabel={row.label}
                      onChange={(next) => setDraft({
                        ...reconciledDraft,
                        [row.key]: fromPreference(next)
                      })}
                    />
                  ) : null}
                </section>
              );
            })}
          </VStack>
        ))}
      </Card>

      {error ? <ErrorMarker message={error} /> : null}
      <HStack justify="end" gap={2} wrap="wrap">
        <Button
          type="button"
          variant="secondary"
          label="Use a different provider"
          isDisabled={saving}
          onClick={onUseDifferentProvider}
        />
        <Button
          type="button"
          variant="primary"
          label="Confirm models and start chat"
          isLoading={saving}
          isDisabled={!complete || saving}
          onClick={() => onConfirm({ providerAccountId: setup.providerAccountId, ...reconciledDraft })}
        />
      </HStack>
    </VStack>
  );
}

function reconcileDraft(current: Draft, setup: Setup): Draft {
  const proposed = setup.proposedSelections;
  const keep = (selection: OnboardingModelSelectionInput) =>
    isValidSelection(selection, setup) ? selection : proposed.noema;
  return {
    noema: keep(current.noema),
    simpleTasks: isValidSelection(current.simpleTasks, setup) ? current.simpleTasks : proposed.simpleTasks,
    mediumTasks: isValidSelection(current.mediumTasks, setup) ? current.mediumTasks : proposed.mediumTasks,
    difficultTasks: isValidSelection(current.difficultTasks, setup) ? current.difficultTasks : proposed.difficultTasks,
    taskReviewer: isValidSelection(current.taskReviewer, setup) ? current.taskReviewer : proposed.taskReviewer,
    webFetchSummarizer: isValidSelection(current.webFetchSummarizer, setup) ? current.webFetchSummarizer : proposed.webFetchSummarizer,
    toolProgressAudit: isValidSelection(current.toolProgressAudit, setup) ? current.toolProgressAudit : proposed.toolProgressAudit,
    actionReviewer: isValidSelection(current.actionReviewer, setup) ? current.actionReviewer : proposed.actionReviewer,
    memoryConsolidation: isValidSelection(current.memoryConsolidation, setup) ? current.memoryConsolidation : proposed.memoryConsolidation
  };
}

function isValidSelection(
  selection: OnboardingModelSelectionInput | null | undefined,
  setup: Setup
) {
  if (!selection) return false;
  const profile = setup.profiles.find(
    (candidate) => candidate.id === selection.modelProfile && !candidate.disabledReason
  );
  if (!profile) return false;
  return profile.reasoningEfforts.length === 0
    ? selection.reasoningEffort === null || selection.reasoningEffort === undefined
    : Boolean(selection.reasoningEffort && profile.reasoningEfforts.includes(selection.reasoningEffort));
}

function toPreference(
  providerAccountId: string,
  selection: OnboardingModelSelectionInput
): ModelPreferenceSaveInput {
  return { providerAccountId, ...selection };
}

function fromPreference(selection: ModelPreferenceSaveInput): OnboardingModelSelectionInput {
  return {
    modelProfile: selection.modelProfile,
    reasoningEffort: selection.reasoningEffort ?? null
  };
}

const styles = stylex.create({
  root: {
    width: "min(100%, 900px)",
    marginInline: "auto"
  },
  eyebrow: {
    margin: 0,
    color: "var(--muted-foreground)",
    fontSize: "var(--font-size-sm)",
    fontWeight: 600
  },
  title: {
    margin: 0,
    fontSize: "var(--font-size-2xl)",
    lineHeight: 1.15
  },
  description: {
    margin: 0,
    maxWidth: "60ch",
    color: "var(--muted-foreground)",
    textAlign: "center"
  },
  form: {
    overflow: "hidden"
  },
  group: {
    padding: "var(--spacing-3)",
    borderBottom: "1px solid var(--border)",
    ':last-child': { borderBottom: "none" }
  },
  groupTitle: {
    margin: 0,
    paddingBlockEnd: "var(--spacing-2)",
    fontSize: "var(--font-size-sm)",
    color: "var(--muted-foreground)"
  },
  row: {
    display: "grid",
    gridTemplateColumns: "minmax(180px, 1fr) minmax(300px, 1.4fr)",
    alignItems: "center",
    gap: "var(--spacing-3)",
    paddingBlock: "var(--spacing-2)",
    borderTop: "1px solid var(--border)",
    '@media (max-width: 760px)': {
      gridTemplateColumns: "minmax(0, 1fr)",
      gap: "var(--spacing-1-5)"
    }
  },
  rowLabel: {
    fontSize: "var(--font-size-sm)"
  },
  rowDescription: {
    color: "var(--muted-foreground)",
    fontSize: "var(--font-size-xs)"
  },
  humanReview: {
    justifySelf: "end",
    color: "var(--muted-foreground)",
    fontSize: "var(--font-size-sm)",
    '@media (max-width: 760px)': { justifySelf: "start" }
  }
});

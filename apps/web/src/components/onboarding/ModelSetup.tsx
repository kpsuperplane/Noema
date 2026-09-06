import { Button } from "@astryxdesign/core/Button";
import { VStack } from "@astryxdesign/core/Stack";
import * as stylex from "@stylexjs/stylex";
import { useMemo, useState } from "react";
import type {
  ConfirmOnboardingModelSelectionsInput,
  NoemaModelUseCase,
  OnboardingModelSelectionInput,
  OnboardingModelSetupQuery
} from "@/generated/graphql";
import { ControlledModelPreferenceSelect } from "../settings/ControlledModelPreferenceSelect";
import type {
  ModelPreferenceSaveInput,
  ModelProviderOption
} from "../settings/modelPreferenceTypes";
import { ErrorMarker } from "../ErrorMarker";
import { SetupCard, SetupActions, SetupNote } from "../shell/SetupFrame";

type Setup = OnboardingModelSetupQuery["onboardingModelSetup"];
type Draft = Omit<ConfirmOnboardingModelSelectionsInput, "providerAccountId">;
type DraftKey = keyof Draft;

const groups: ReadonlyArray<{
  title: string;
  rows: ReadonlyArray<{
    key: DraftKey;
    label: string;
    description: string;
    useCase: NoemaModelUseCase;
  }>;
}> = [
  {
    title: "Chat",
    rows: [
      {
        key: "noema",
        label: "Noema",
        description: "Your main conversational model",
        useCase: "PRIMARY"
      }
    ]
  },
  {
    title: "Tasks",
    rows: [
      {
        key: "simpleTasks",
        label: "Simple tasks",
        description: "Fast, routine task execution",
        useCase: "TASK_SIMPLE"
      },
      {
        key: "mediumTasks",
        label: "Medium tasks",
        description: "General task execution",
        useCase: "TASK_MEDIUM"
      },
      {
        key: "difficultTasks",
        label: "Difficult tasks",
        description: "Complex task execution",
        useCase: "TASK_DIFFICULT"
      },
      {
        key: "taskReviewer",
        label: "Task reviewer",
        description: "Reviews completed task work",
        useCase: "TASK_REVIEWER"
      }
    ]
  },
  {
    title: "Supporting work",
    rows: [
      {
        key: "webFetchSummarizer",
        label: "Web summaries",
        description: "Condenses fetched pages",
        useCase: "WEB_FETCH_SUMMARIZER"
      },
      {
        key: "toolProgressAudit",
        label: "Progress checks",
        description: "Checks long-running task progress",
        useCase: "TOOL_PROGRESS_AUDIT"
      },
      {
        key: "actionReviewer",
        label: "Action reviews",
        description: "Reviews governed actions",
        useCase: "ACTION_REVIEWER"
      },
      {
        key: "memoryConsolidation",
        label: "Memory updates",
        description: "Maintains long-term memory",
        useCase: "MEMORY_CONSOLIDATION"
      }
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
  const provider = useMemo<ModelProviderOption>(
    () => ({
      providerKind: setup.providerKind,
      providerAccountId: setup.providerAccountId,
      providerDisplayName: setup.providerDisplayName,
      status: "AUTHENTICATED",
      profiles: setup.profiles,
      recommendations: setup.recommendations
    }),
    [setup]
  );
  const reconciledDraft = reconcileDraft(draft, setup);

  const complete = groups.every(({ rows }) =>
    rows.every(({ key }) => {
      if (key === "actionReviewer" && setup.providerKind === "local_models")
        return true;
      return isValidSelection(
        reconciledDraft[key],
        setup,
        modelUseCaseForKey(key)
      );
    })
  );

  const rows = groups.flatMap((group) => group.rows);
  const resolved = (key: DraftKey) => {
    const selection = reconciledDraft[key];
    if (!selection) return null;
    return selection.selectionMode === "NOEMA_RECOMMENDED"
      ? setup.recommendations.find(
          (item) => item.useCase === modelUseCaseForKey(key)
        )?.modelProfile
      : selection.modelProfile;
  };
  const summaries = new Map<string, { name: string; jobs: string[] }>();
  const jobs: Array<[string, DraftKey[]]> = [
    ["Chat", ["noema"]],
    ["Routine tasks", ["simpleTasks", "mediumTasks", "taskReviewer"]],
    ["Difficult tasks", ["difficultTasks"]],
    [
      "Supporting work",
      [
        "webFetchSummarizer",
        "toolProgressAudit",
        "actionReviewer",
        "memoryConsolidation"
      ]
    ]
  ];
  for (const [label, keys] of jobs) {
    const assigned = keys.filter((key) => reconciledDraft[key]);
    const combined = assigned.every(
      (key) => resolved(key) === resolved(assigned[0])
    );
    for (const key of combined ? assigned.slice(0, 1) : assigned) {
      const profileId = resolved(key);
      const name =
        setup.profiles.find((item) => item.id === profileId)?.label ??
        "Model unavailable";
      const summary = summaries.get(profileId ?? key) ?? { name, jobs: [] };
      summary.jobs.push(
        combined ? label : rows.find((row) => row.key === key)!.label
      );
      summaries.set(profileId ?? key, summary);
    }
  }
  return (
    <SetupCard
      title="Review your models"
      intro="Ready for chat, tasks, and more."
      step={`Connected to ${setup.providerDisplayName}`}
    >
      <VStack gap={3} aria-label="Model assignments">
        {[...summaries.entries()].map(([id, summary]) => (
          <VStack key={id} gap={0.5}>
            <strong>{summary.name}</strong>
            <p {...stylex.props(styles.description)}>
              {summary.jobs.join(" · ")}
            </p>
          </VStack>
        ))}
        {setup.providerKind === "local_models" &&
        !reconciledDraft.actionReviewer ? (
          <SetupNote>
            Noema will ask you to approve actions that need review.
          </SetupNote>
        ) : null}
      </VStack>
      <details>
        <summary {...stylex.props(styles.customize)}>Customize models</summary>
        <VStack gap={3}>
          {groups.map((group) => (
            <VStack as="section" key={group.title} gap={2}>
              <h2 {...stylex.props(styles.groupTitle)}>{group.title}</h2>
              {group.rows.map((row) => {
                const selection = reconciledDraft[row.key];
                return (
                  <VStack key={row.key} gap={1.5}>
                    <strong {...stylex.props(styles.rowLabel)}>
                      {row.label}
                    </strong>
                    {selection ? (
                      <ControlledModelPreferenceSelect
                        options={[provider]}
                        selection={toPreference(
                          setup.providerAccountId,
                          selection
                        )}
                        useCase={row.useCase}
                        disabled={saving}
                        ariaLabel={row.label}
                        onChange={(next) =>
                          setDraft({
                            ...reconciledDraft,
                            [row.key]: fromPreference(next)
                          })
                        }
                      />
                    ) : (
                      <p {...stylex.props(styles.description)}>
                        Ask me for approval
                      </p>
                    )}
                  </VStack>
                );
              })}
            </VStack>
          ))}
        </VStack>
      </details>
      {error ? (
        <>
          <ErrorMarker message="Noema could not finish setup. Your choices are still here." />
          <details>
            <summary>Error details</summary>
            <p>{error}</p>
          </details>
        </>
      ) : null}
      <SetupActions>
        <Button
          variant="secondary"
          label="Change provider"
          isDisabled={saving}
          onClick={onUseDifferentProvider}
        />
        <Button
          variant="primary"
          label={error ? "Try again" : "Start chatting"}
          isLoading={saving}
          isDisabled={!complete || saving}
          onClick={() =>
            onConfirm({
              providerAccountId: setup.providerAccountId,
              ...reconciledDraft
            })
          }
        />
      </SetupActions>
      <SetupNote>You can change these in Settings.</SetupNote>
    </SetupCard>
  );
}

function reconcileDraft(current: Draft, setup: Setup): Draft {
  const proposed = setup.proposedSelections;
  const keep = (selection: OnboardingModelSelectionInput) =>
    toSelectionInput(
      isValidSelection(selection, setup, "PRIMARY") ? selection : proposed.noema
    );
  const actionReviewer = isValidSelection(
    current.actionReviewer,
    setup,
    "ACTION_REVIEWER"
  )
    ? current.actionReviewer
    : proposed.actionReviewer;
  return {
    noema: keep(current.noema),
    simpleTasks: toSelectionInput(
      isValidSelection(current.simpleTasks, setup, "TASK_SIMPLE")
        ? current.simpleTasks
        : proposed.simpleTasks
    ),
    mediumTasks: toSelectionInput(
      isValidSelection(current.mediumTasks, setup, "TASK_MEDIUM")
        ? current.mediumTasks
        : proposed.mediumTasks
    ),
    difficultTasks: toSelectionInput(
      isValidSelection(current.difficultTasks, setup, "TASK_DIFFICULT")
        ? current.difficultTasks
        : proposed.difficultTasks
    ),
    taskReviewer: toSelectionInput(
      isValidSelection(current.taskReviewer, setup, "TASK_REVIEWER")
        ? current.taskReviewer
        : proposed.taskReviewer
    ),
    webFetchSummarizer: toSelectionInput(
      isValidSelection(
        current.webFetchSummarizer,
        setup,
        "WEB_FETCH_SUMMARIZER"
      )
        ? current.webFetchSummarizer
        : proposed.webFetchSummarizer
    ),
    toolProgressAudit: toSelectionInput(
      isValidSelection(current.toolProgressAudit, setup, "TOOL_PROGRESS_AUDIT")
        ? current.toolProgressAudit
        : proposed.toolProgressAudit
    ),
    actionReviewer: actionReviewer ? toSelectionInput(actionReviewer) : null,
    memoryConsolidation: toSelectionInput(
      isValidSelection(
        current.memoryConsolidation,
        setup,
        "MEMORY_CONSOLIDATION"
      )
        ? current.memoryConsolidation
        : proposed.memoryConsolidation
    )
  };
}

function toSelectionInput(
  selection: OnboardingModelSelectionInput
): OnboardingModelSelectionInput {
  return {
    selectionMode: selection.selectionMode,
    modelProfile: selection.modelProfile ?? null,
    reasoningEffort: selection.reasoningEffort ?? null,
    fastMode: selection.fastMode
  };
}

function isValidSelection(
  selection: OnboardingModelSelectionInput | null | undefined,
  setup: Setup,
  useCase: NoemaModelUseCase
) {
  if (!selection) return false;
  if (selection.selectionMode === "NOEMA_RECOMMENDED") {
    return setup.recommendations.some(
      (recommendation) =>
        recommendation.useCase === useCase && !recommendation.disabledReason
    );
  }
  const profile = setup.profiles.find(
    (candidate) =>
      candidate.id === selection.modelProfile && !candidate.disabledReason
  );
  if (!profile) return false;
  return profile.reasoningEfforts.length === 0
    ? selection.reasoningEffort === null ||
        selection.reasoningEffort === undefined
    : Boolean(
        selection.reasoningEffort &&
        profile.reasoningEfforts.includes(selection.reasoningEffort)
      );
}

function toPreference(
  providerAccountId: string,
  selection: OnboardingModelSelectionInput
): ModelPreferenceSaveInput {
  return { providerAccountId, ...selection };
}

function fromPreference(
  selection: ModelPreferenceSaveInput
): OnboardingModelSelectionInput {
  return {
    selectionMode: selection.selectionMode,
    modelProfile: selection.modelProfile,
    reasoningEffort: selection.reasoningEffort ?? null,
    fastMode: selection.fastMode
  };
}

function modelUseCaseForKey(key: DraftKey): NoemaModelUseCase {
  return (
    groups.flatMap((group) => group.rows).find((row) => row.key === key)
      ?.useCase ?? "PRIMARY"
  );
}

const styles = stylex.create({
  description: {
    margin: 0,
    color: "var(--muted-foreground)",
    fontSize: "var(--font-size-sm)",
    textWrap: "pretty"
  },
  customize: { paddingBlock: "var(--spacing-2)", fontWeight: 600 },
  groupTitle: {
    margin: 0,
    fontSize: "var(--font-size-base)",
    color: "var(--muted-foreground)"
  },
  rowLabel: { fontSize: "var(--font-size-sm)" }
});

import * as React from "react";
import { useLazyQuery } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import { IconButton } from "@astryxdesign/core/IconButton";
import { TextArea } from "@astryxdesign/core/TextArea";
import { VStack } from "@astryxdesign/core/VStack";
import * as stylex from "@stylexjs/stylex";
import { Ban, CalendarClock, CalendarX, CircleEllipsis, MessageSquareReply, Play, RefreshCcw, RotateCcw, SendHorizontal, Settings2 } from "lucide-react";
import type { TasksProject } from "./tasksTypes";
import { useTaskCommands, type TaskCommandSubject } from "./useTaskCommands";
import { TaskActionDialog } from "./TaskActionDialog";
import { orderTaskActions, taskActionLabel } from "./taskActionModel";
import { TasksTaskEditFieldsDocument } from "@/generated/graphql";
import { composerDraftInlineSize, measureComposerDraftInlineSize } from "@/components/Composer";
import { snapshotTaskSubject, taskSubjectChanged } from "./semanticCommand";
import { TaskScheduleDialog } from "./TaskScheduleDialog";
import { useTaskEditFlush } from "./TaskDocumentFields";
import { TaskOptions } from "./TaskOptions";

type ActiveCommand = {
  action: string;
  subject: TaskCommandSubject;
};

type TaskEditField = "TITLE" | "DOCUMENT" | "SETTINGS";

export type TaskInlineEditController = {
  flush: () => Promise<boolean>;
  registerFlush: (flush: (() => Promise<boolean>) | null) => void;
  options: React.ReactNode;
  timing: React.ReactNode;
  field: Exclude<TaskEditField, "SETTINGS"> | null;
  task: TaskCommandSubject;
  canEdit: boolean;
  canStart: boolean;
  busy: boolean;
  error: string | null;
  requiresAcknowledgement: boolean;
  actionUnavailable: boolean;
  start: (field: Exclude<TaskEditField, "SETTINGS">) => Promise<void>;
  cancel: () => void;
  saveTitle: (title: string) => Promise<void>;
  saveDocument: (taskDocument: string) => Promise<void>;
  acknowledge: () => Promise<void>;
};

type TaskActionsProps = {
  task: TaskCommandSubject;
  validActions: readonly string[];
  projects?: readonly TasksProject[];
  inlineResponse?: boolean;
  answerChoices?: readonly string[];
  onUpdated?: () => void | Promise<void>;
  children?: (actions: React.ReactNode, controls: React.ReactNode, edit: TaskInlineEditController) => React.ReactNode;
};

export function TaskActions({
  task,
  validActions,
  projects = [],
  inlineResponse = false,
  answerChoices = [],
  onUpdated,
  children
}: TaskActionsProps) {
  const [activeCommand, setActiveCommand] = React.useState<ActiveCommand | null>(null);
  const { flush, registerFlush } = useTaskEditFlush();
  const [editField, setEditField] = React.useState<TaskEditField | null>(null);
  const [scheduleAction, setScheduleAction] = React.useState<"SCHEDULE" | "RESCHEDULE" | "UNSCHEDULE" | null>(null);
  const [editLoadError, setEditLoadError] = React.useState<string | null>(null);
  const [answer, setAnswer] = React.useState("");
  const [pendingChoice, setPendingChoice] = React.useState<string | null>(null);
  const [pendingApprovalDecision, setPendingApprovalDecision] = React.useState<"APPROVED" | "DECLINED" | null>(null);
  const answerInputRef = React.useRef<HTMLTextAreaElement>(null);
  const answerComposerRef = React.useRef<HTMLDivElement>(null);
  const [loadEditTask, editLoad] = useLazyQuery(TasksTaskEditFieldsDocument, { fetchPolicy: "network-only" });
  const commandTask = activeCommand?.subject ?? task;
  const refresh = React.useCallback(async () => {
    await onUpdated?.();
  }, [onUpdated]);
  const commands = useTaskCommands({ task: commandTask, onUpdated: refresh });
  const activeAction = activeCommand?.action ?? null;
  const canEdit = validActions.includes("EDIT");
  const canAnswer = validActions.includes("ANSWER");
  const canRetry = validActions.includes("RETRY");
  const hasInlineResponse = inlineResponse && (canAnswer || canRetry);
  const approval = task.activeGate?.kind === "APPROVAL";
  const visibleAnswerChoices = canAnswer && task.activeGate?.kind !== "APPROVAL"
    ? answerChoices
    : [];
  const responseAction = answer.trim() && canAnswer ? "ANSWER" : canRetry ? "RETRY" : "ANSWER";
  const answerPlaceholder = approval
    ? "Optional note"
    : visibleAnswerChoices.length
      ? "Or type another answer"
      : canAnswer && canRetry
        ? "Answer, or leave blank to retry"
        : canRetry
          ? "Optional retry guidance"
          : "Type your answer";
  const commandActions = orderTaskActions(validActions.filter(
    (action) => !["EDIT", "SCHEDULE", "RESCHEDULE", "UNSCHEDULE"].includes(action) && !(hasInlineResponse && (action === "ANSWER" || action === "RETRY"))
  ));
  const liveSubjectChanged = activeCommand ? taskSubjectChanged(activeCommand.subject, task) : false;
  const requiresAcknowledgement = liveSubjectChanged || commands.requiresAcknowledgement;
  const actionUnavailable = Boolean(activeAction && !validActions.includes(activeAction));

  const answerApproval = React.useCallback(async (decision: "APPROVED" | "DECLINED") => {
    if (commands.busy || commands.requiresAcknowledgement) return;
    setPendingApprovalDecision(decision);
    try {
      await commands.run("ANSWER", {
        message: answer.trim() || undefined,
        approvalDecision: decision
      });
      setAnswer("");
    } finally {
      setPendingApprovalDecision(null);
    }
  }, [answer, commands]);

  React.useLayoutEffect(() => {
    const input = answerInputRef.current;
    if (!input) return;
    input.style.minHeight = "24px";
    input.style.height = "24px";
    input.style.overflowY = "hidden";
    input.style.resize = "none";
    input.style.fontSize = "16px";
    input.style.lineHeight = "24px";
    if (answer.length > 0) {
      const nextHeight = Math.max(24, input.scrollHeight);
      input.style.height = `${Math.min(144, nextHeight)}px`;
      input.style.overflowY = nextHeight > 144 ? "auto" : "hidden";
    }
    const inlineSize = measureComposerDraftInlineSize({
      textarea: input,
      value: answer,
      placeholder: answerPlaceholder
    });
    const composer = answerComposerRef.current;
    if (inlineSize && composer) {
      const style = taskAnswerComposerStyle(inlineSize);
      composer.style.width = style.width as string;
      composer.style.minWidth = style.minWidth as string;
    }
  }, [answer, answerPlaceholder]);

  const openEdit = React.useCallback(async (field: TaskEditField) => {
    commands.clearError();
    setEditLoadError(null);
    if (!canEdit || scheduleAction) return;
    const handoff = activeCommand?.action === "EDIT" && editField !== field;
    if (activeCommand && (!handoff || !(await flush()))) return;
    if (!handoff && task.taskDocument !== undefined) {
      setEditField(field);
      setActiveCommand({ action: "EDIT", subject: snapshotTaskSubject(task) });
      return;
    }
    try {
      const result = await loadEditTask({ variables: { taskId: task.taskId } });
      if (!result.data?.task) throw new Error("Task unavailable");
      setEditField(field);
      setActiveCommand({ action: "EDIT", subject: snapshotTaskSubject(result.data.task) });
    } catch {
      setEditLoadError("Task details could not be loaded for editing.");
    }
  }, [activeCommand, canEdit, commands, editField, flush, loadEditTask, scheduleAction, task]);

  const openAction = React.useCallback(async (action: string) => {
    let actionTask = task;
    if (activeCommand?.action === "EDIT") {
      if (!(await flush())) return;
      const latest = await loadEditTask({ variables: { taskId: task.taskId } });
      if (!latest.data?.task) return;
      actionTask = { ...task, ...latest.data.task };
    }
    commands.clearError();
    setEditLoadError(null);
    if (action === "SCHEDULE" || action === "RESCHEDULE" || action === "UNSCHEDULE") {
      setScheduleAction(action);
      return;
    }
    if (action === "QUEUE" || action === "RUN_NOW") {
      await commands.run(action, {}, actionTask).catch(() => undefined);
      return;
    }
    if (action === "EDIT") {
      await openEdit("SETTINGS");
      return;
    }
    if (action !== "EDIT") {
      setActiveCommand({ action, subject: snapshotTaskSubject(actionTask) });
    }
  }, [activeCommand, commands, flush, loadEditTask, openEdit, task]);


  const acknowledgeLatest = React.useCallback(async () => {
    if (!activeCommand) return;
    const result = await loadEditTask({ variables: { taskId: task.taskId } });
    if (!result.data?.task) throw new Error("Task unavailable");
    const latest: TaskCommandSubject = result.data.task;
    setActiveCommand({ action: activeCommand.action, subject: snapshotTaskSubject(latest) });
    commands.acknowledge();
  }, [activeCommand, commands, loadEditTask, task]);

  const cancelEdit = React.useCallback(() => {
    commands.clearError();
    setEditLoadError(null);
    setEditField(null);
    setActiveCommand(null);
  }, [commands]);

  const saveEdit = React.useCallback(async (draft: Parameters<typeof commands.run>[1]) => {
    if (requiresAcknowledgement || actionUnavailable) return;
    await commands.run("EDIT", {
      title: commandTask.title,
      taskDocument: commandTask.taskDocument ?? "",
      projectId: commandTask.project?.projectId ?? null,
      executorAgentId: commandTask.executorAgentId,
      cwdOverride: commandTask.cwdOverride ?? null,
      ...draft
    });
    setEditField(null);
    setActiveCommand(null);
  }, [actionUnavailable, commandTask, commands, requiresAcknowledgement]);

  const editController = React.useMemo<TaskInlineEditController>(() => ({
    flush,
    registerFlush,
    options: <TaskOptions readOnly={!canEdit} task={commandTask} projects={projects} busy={commands.busy !== null || (activeCommand !== null && editField !== "SETTINGS")} requiresAcknowledgement={requiresAcknowledgement} error={editField === "SETTINGS" ? commands.error : null} onAcknowledge={acknowledgeLatest} onSubmit={async (draft) => { setEditField("SETTINGS"); setActiveCommand({ action: "EDIT", subject: snapshotTaskSubject(commandTask) }); await saveEdit(draft); }} />,
    timing: <span>{["SCHEDULE", "RESCHEDULE", "UNSCHEDULE"].filter((action) => validActions.includes(action)).map((action) => <Button key={action} size="sm" variant="ghost" icon={taskCommandIcon(action, { "aria-hidden": true, size: 15, strokeWidth: 2 })} label={action === "SCHEDULE" ? "Schedule" : action === "RESCHEDULE" ? "Reschedule" : "Remove schedule"} isDisabled={commands.busy !== null || activeCommand !== null} onClick={() => void openAction(action)} />)}</span>,
    field: editField === "TITLE" || editField === "DOCUMENT" ? editField : null,
    task: commandTask,
    canEdit,
    canStart: canEdit && activeCommand === null && scheduleAction === null,
    busy: commands.busy !== null || editLoad.loading,
    error: editLoadError ?? commands.error,
    requiresAcknowledgement,
    actionUnavailable,
    start: openEdit,
    cancel: cancelEdit,
    saveTitle: async (title) => saveEdit({ title }),
    saveDocument: async (taskDocument) => saveEdit({ taskDocument }),
    acknowledge: acknowledgeLatest
  }), [acknowledgeLatest, actionUnavailable, activeCommand, canEdit, cancelEdit, commandTask, commands, editField, editLoad.loading, editLoadError, openEdit, requiresAcknowledgement, saveEdit, scheduleAction, projects, validActions, openAction, flush, registerFlush]);

  const controls = (
    <TaskControlsRow
      busy={(commands.busy !== null && activeCommand?.action !== "EDIT") || editLoad.loading || (activeCommand !== null && activeCommand.action !== "EDIT") || scheduleAction !== null}
      commandActions={commandActions}
      onCommand={openAction}
    />
  );
  const hasActionBody = hasInlineResponse || Boolean(editLoadError) || Boolean(commands.error && !activeAction);
  const actionBody = hasActionBody ? (
    <div {...stylex.props(styles.frame)}>
      {hasInlineResponse ? (
        <form
          aria-label={canRetry ? "Respond or retry" : "Answer this request"}
          {...stylex.props(styles.answerForm)}
          onSubmit={(event) => {
            event.preventDefault();
            if (approval) return;
            if ((responseAction === "ANSWER" && !answer.trim()) || commands.busy || commands.requiresAcknowledgement) return;
            void commands.run(responseAction, {
              message: answer.trim() || undefined
            }).then(() => setAnswer("")).catch(() => undefined);
          }}
        >
          {visibleAnswerChoices.length ? (
            <VStack as="div" gap={1} hAlign="end" width="100%" className={stylex.props(styles.answerChoices).className}>
              {visibleAnswerChoices.map((choice, index) => (
                <Button
                  key={`${index}:${choice}`}
                  data-slot="task-answer-choice"
                  type="button"
                  size="sm"
                  variant="secondary"
                  label={choice}
                  isLoading={pendingChoice === choice && commands.busy === "ANSWER"}
                  isDisabled={commands.busy !== null || commands.requiresAcknowledgement}
                  xstyle={styles.answerChoice}
                  onClick={() => {
                    setPendingChoice(choice);
                    void commands.run("ANSWER", { message: choice })
                      .finally(() => setPendingChoice(null));
                  }}
                />
              ))}
            </VStack>
          ) : null}
          <div
            ref={answerComposerRef}
            data-slot="task-answer-composer"
            {...stylex.props(styles.answerComposerRow, approval && styles.approvalComposerRow)}
            style={taskAnswerComposerStyle(composerDraftInlineSize({
              value: answer,
              placeholder: answerPlaceholder
            }))}
          >
            <TextArea
              ref={answerInputRef}
              isLabelHidden
              label={approval ? "Optional note" : canRetry ? "Response or retry guidance" : "Answer"}
              onChange={setAnswer}
              onKeyDown={(event) => {
                if (approval) return;
                if (event.key === "Enter" && !event.shiftKey) {
                  event.preventDefault();
                  answerInputRef.current?.form?.requestSubmit();
                }
              }}
              placeholder={answerPlaceholder}
              rows={1}
              value={answer}
              width="100%"
              className={stylex.props(styles.answerInputField).className}
            />
            {!approval ? (
              <Button
                type="submit"
                size="sm"
                variant="secondary"
                label={taskActionLabel(responseAction, false)}
                isIconOnly
                icon={<SendHorizontal aria-hidden="true" size={14} strokeWidth={2} />}
                isLoading={commands.busy === responseAction}
                isDisabled={(responseAction === "ANSWER" && !answer.trim()) || commands.busy !== null || commands.requiresAcknowledgement}
                xstyle={styles.answerSubmit}
              />
            ) : null}
          </div>
          {approval ? (
            <div role="group" aria-label="Approval decision" {...stylex.props(styles.approvalActions)}>
              <Button
                type="button"
                size="sm"
                variant="ghost"
                label="Decline"
                isLoading={pendingApprovalDecision === "DECLINED"}
                isDisabled={commands.busy !== null || commands.requiresAcknowledgement}
                onClick={() => void answerApproval("DECLINED").catch(() => undefined)}
              />
              <Button
                type="button"
                size="sm"
                variant="primary"
                label="Approve"
                isLoading={pendingApprovalDecision === "APPROVED"}
                isDisabled={commands.busy !== null || commands.requiresAcknowledgement}
                onClick={() => void answerApproval("APPROVED").catch(() => undefined)}
              />
            </div>
          ) : null}
          {commands.requiresAcknowledgement ? (
            <div role="alert" {...stylex.props(styles.stale)}>
              <span>{commands.error}</span>
              <Button
                type="button"
                size="sm"
                variant="secondary"
                label="Use latest task"
                onClick={commands.acknowledge}
              />
            </div>
          ) : commands.error ? (
            <p role="alert" {...stylex.props(styles.inlineError)}>{commands.error}</p>
          ) : null}
        </form>
      ) : null}
      {!hasInlineResponse && !activeAction && commands.error ? (
        <p role="alert" {...stylex.props(styles.inlineError)}>{commands.error}</p>
      ) : null}
      {editLoadError ? <span role="alert" {...stylex.props(styles.loadError)}>{editLoadError}</span> : null}
    </div>
  ) : null;

  return (
    <>
      {children ? children(actionBody ? <div {...stylex.props(styles.defaultFrame)}>{actionBody}</div> : null, controls, editController) : (
        <>{controls}{actionBody ? <div {...stylex.props(styles.defaultFrame)}>{actionBody}</div> : null}</>
      )}
      <span aria-live="polite" {...stylex.props(styles.srOnly)}>{commands.notice}</span>
      <TaskActionDialog
        key={activeAction ?? "closed"}
        action={activeAction === "EDIT" ? null : activeAction}
        task={commandTask}
        busy={commands.busy !== null}
        acknowledging={editLoad.loading}
        requiresAcknowledgement={requiresAcknowledgement}
        actionUnavailable={actionUnavailable}
        error={commands.error}
        onAcknowledge={() => acknowledgeLatest().catch(() => setEditLoadError("The latest task details could not be loaded."))}
        onClose={() => setActiveCommand(null)}
        onSubmit={async (draft) => {
          if (!activeAction || requiresAcknowledgement || actionUnavailable) return;
          await commands.run(activeAction, draft);
          setActiveCommand(null);
        }}
      />
      <TaskScheduleDialog key={`${task.taskId}:${task.revision}:${scheduleAction}`} action={scheduleAction} task={task} onClose={() => setScheduleAction(null)} />
    </>
  );
}

function TaskControlsRow({
  busy = false,
  commandActions = [],
  onCommand
}: {
  busy?: boolean;
  commandActions?: readonly string[];
  onCommand?: (action: string) => void | Promise<void>;
}) {
  const iconProps = { "aria-hidden": true, size: 15, strokeWidth: 2 } as const;
  return (
    <span role="group" aria-label="Task controls" {...stylex.props(styles.taskControlsGroup)}>
      {commandActions.map((action) => {
        const label = taskCommandLabel(action);
        return (
          <IconButton
            key={action}
            type="button"
            size="sm"
            variant="ghost"
            label={label}
            tooltip={label}
            icon={taskCommandIcon(action, iconProps)}
            isDisabled={busy}
            onClick={() => void onCommand?.(action)}
            xstyle={styles.taskControlButton}
          />
        );
      })}
    </span>
  );
}

function taskCommandLabel(action: string): string {
  const labels: Record<string, string> = {
    QUEUE: "Start task",
    RUN_NOW: "Run task now",
    SCHEDULE: "Schedule task",
    RESCHEDULE: "Reschedule task",
    UNSCHEDULE: "Unschedule task",
    EDIT: "Edit task settings",
    ANSWER: "Answer request",
    RETRY: "Retry task",
    REOPEN: "Reopen task",
    CANCEL: "Cancel task"
  };
  return labels[action] ?? taskActionLabel(action, false);
}

function taskCommandIcon(action: string, iconProps: { "aria-hidden": true; size: number; strokeWidth: number }) {
  switch (action) {
    case "QUEUE": return <Play {...iconProps} color="var(--noema-pine-700)" fill="var(--noema-pine-700)" />;
    case "RUN_NOW": return <Play {...iconProps} color="var(--noema-pine-700)" fill="var(--noema-pine-700)" />;
    case "SCHEDULE":
    case "RESCHEDULE": return <CalendarClock {...iconProps} />;
    case "UNSCHEDULE": return <CalendarX {...iconProps} />;
    case "EDIT": return <Settings2 {...iconProps} />;
    case "ANSWER": return <MessageSquareReply {...iconProps} />;
    case "RETRY": return <RefreshCcw {...iconProps} />;
    case "REOPEN": return <RotateCcw {...iconProps} />;
    case "CANCEL": return <Ban {...iconProps} color="var(--color-error)" />;
    default: return <CircleEllipsis {...iconProps} />;
  }
}

const styles = stylex.create({
  frame: { display: "grid", gap: "var(--spacing-1-5)" },
  defaultFrame: { display: "grid", gap: "var(--spacing-1-5)", minWidth: 0 },
  taskControlsGroup: { display: "inline-flex", alignItems: "center", gap: "var(--spacing-1)" },
  taskControlButton: {
    minWidth: 28,
    width: 28,
    height: 28,
    borderWidth: 0,
    backgroundColor: "transparent",
    boxShadow: "none"
  },
  answerForm: { display: "grid", gap: "var(--spacing-1-5)", minWidth: 0, borderWidth: 0, borderColor: "transparent", backgroundColor: "transparent", padding: "var(--spacing-0)" },
  answerChoices: { minWidth: 0 },
  answerChoice: {
    width: "fit-content",
    maxWidth: "min(100%, 32rem)",
    minHeight: {
      default: 32,
      "@media (hover: none) and (pointer: coarse)": 44
    },
    height: "auto",
    justifyContent: "flex-start",
    borderWidth: 0,
    borderRadius: "calc(var(--radius) * 2.2)",
    cornerShape: "var(--corner-shape-element)",
    backgroundColor: "var(--primary)",
    paddingBlock: "var(--spacing-1-5)",
    paddingInline: "var(--spacing-3)",
    color: "var(--primary-foreground)",
    textAlign: "start",
    whiteSpace: "normal",
    boxShadow: "var(--shadow-composer)"
  },
  answerComposerRow: {
    position: "relative",
    display: "flex",
    alignItems: "flex-end",
    justifySelf: "end",
    minWidth: 0,
    borderRadius: "calc(var(--radius) * 2.2)",
    cornerShape: "var(--corner-shape-element)",
    backgroundColor: "var(--primary)",
    padding: "var(--spacing-1)",
    paddingInlineEnd: {
      default: 40,
      "@media (hover: none) and (pointer: coarse)": 52
    },
    color: "var(--primary-foreground)",
    boxShadow: "var(--shadow-composer)",
    ":focus-within": {
      outlineWidth: 2,
      outlineStyle: "solid",
      outlineColor: "var(--ring)",
      outlineOffset: 2
    }
  },
  answerInputField: {
    "--color-text-primary": "var(--primary-foreground)",
    "--color-text-secondary": "color-mix(in srgb, var(--primary-foreground) 70%, transparent)",
    flex: 1,
    minWidth: 0,
    minHeight: 32,
    borderWidth: 0,
    borderColor: "transparent",
    backgroundColor: "transparent",
    paddingBlock: "var(--spacing-1)",
    paddingInline: "var(--spacing-2)",
    color: "var(--primary-foreground)",
    outline: {
      default: null,
      ":focus-visible": "none"
    },
    boxShadow: "none"
  },
  approvalComposerRow: {
    paddingInlineEnd: "var(--spacing-1)"
  },
  answerSubmit: {
    position: "absolute",
    insetInlineEnd: {
      default: 4,
      "@media (hover: none) and (pointer: coarse)": 2
    },
    insetBlockEnd: {
      default: 4,
      "@media (hover: none) and (pointer: coarse)": 2
    },
    width: {
      default: 32,
      "@media (hover: none) and (pointer: coarse)": 44
    },
    height: {
      default: 32,
      "@media (hover: none) and (pointer: coarse)": 44
    },
    borderRadius: 999,
    cornerShape: "var(--corner-shape-full)",
    backgroundColor: "var(--primary-foreground)",
    color: {
      default: "var(--primary)",
      ":disabled": "color-mix(in srgb, var(--primary) 70%, transparent)"
    }
  },
  approvalActions: { display: "flex", justifyContent: "flex-end", gap: "var(--spacing-2)" },
  stale: { display: "grid", justifyItems: "start", gap: "var(--spacing-2)", borderRadius: 7, backgroundColor: "var(--noema-surface-card)", padding: "var(--spacing-2)", color: "var(--noema-clay-600)", fontSize: 12, lineHeight: 1.4 },
  inlineError: { margin: "var(--spacing-0)", color: "var(--noema-red-700)", fontSize: 12, lineHeight: 1.4 },
  srOnly: { position: "absolute", width: 1, height: 1, overflow: "hidden", clip: "rect(0 0 0 0)", whiteSpace: "nowrap" },
  loadError: { color: "var(--destructive)", fontSize: 12 }
});

function taskAnswerComposerStyle(
  inlineSize: ReturnType<typeof composerDraftInlineSize>
): React.CSSProperties {
  return {
    width: `min(calc(${inlineSize.width} + var(--spacing-12)), 100%)`,
    minWidth: `min(calc(${inlineSize.minWidth} + var(--spacing-12)), 100%)`
  };
}

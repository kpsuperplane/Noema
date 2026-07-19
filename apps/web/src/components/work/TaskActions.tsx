import * as React from "react";
import { createPortal } from "react-dom";
import { useLazyQuery } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import { IconButton } from "@astryxdesign/core/IconButton";
import { TextArea } from "@astryxdesign/core/TextArea";
import * as stylex from "@stylexjs/stylex";
import { Ban, ExternalLink, RotateCcw, X } from "lucide-react";
import { TaskDecisionCard } from "@/components/chatDetail/task/TaskDecisionCard";
import type { TaskDetail } from "@/components/chatDetail/task/taskTypes";
import type { WorkProject } from "./workTypes";
import { useTaskCommands, type TaskCommandSubject } from "./useTaskCommands";
import { TaskActionDialog } from "./TaskActionDialog";
import { taskActionLabel } from "./taskActionModel";
import { WorkTaskEditFieldsDocument } from "@/generated/graphql";
import { snapshotTaskSubject, taskSubjectChanged } from "./semanticCommand";

type ActiveCommand = {
  action: string;
  subject: TaskCommandSubject;
};

type TaskActionNavigation = {
  taskId: string;
  showWorkLink: boolean;
  onClose?: () => void;
};

type TaskActionDecision = {
  attention: NonNullable<TaskDetail["attention"]>;
  question?: string | null;
};

type TaskActionsProps = {
  task: TaskCommandSubject;
  validActions: readonly string[];
  navigation: TaskActionNavigation;
  closeButtonRef?: React.RefObject<HTMLButtonElement | null>;
  controlsHostRef: React.RefObject<HTMLElement | null>;
  decision?: TaskActionDecision;
  projects?: readonly WorkProject[];
  compact?: boolean;
  inlineAnswer?: boolean;
  onUpdated?: () => void | Promise<void>;
};

export function TaskNavigationControls({
  navigation,
  closeButtonRef,
  controlsHostRef
}: {
  navigation: TaskActionNavigation;
  closeButtonRef?: React.RefObject<HTMLButtonElement | null>;
  controlsHostRef: React.RefObject<HTMLElement | null>;
}) {
  return (
    <TaskControlsPortal hostRef={controlsHostRef}>
      <TaskControlsRow navigation={navigation} closeButtonRef={closeButtonRef} />
    </TaskControlsPortal>
  );
}

export function TaskActions({
  task,
  validActions,
  navigation,
  closeButtonRef,
  controlsHostRef,
  decision,
  projects = [],
  compact = false,
  inlineAnswer = false,
  onUpdated
}: TaskActionsProps) {
  const [activeCommand, setActiveCommand] = React.useState<ActiveCommand | null>(null);
  const [editLoadError, setEditLoadError] = React.useState<string | null>(null);
  const [answer, setAnswer] = React.useState("");
  const [approvalDecision, setApprovalDecision] = React.useState<"APPROVED" | "DECLINED">("APPROVED");
  const [loadEditTask, editLoad] = useLazyQuery(WorkTaskEditFieldsDocument, { fetchPolicy: "network-only" });
  const commandTask = activeCommand?.subject ?? task;
  const refresh = React.useCallback(async () => {
    await onUpdated?.();
  }, [onUpdated]);
  const commands = useTaskCommands({ task: commandTask, onUpdated: refresh });
  const activeAction = activeCommand?.action ?? null;
  const hasInlineAnswer = inlineAnswer && validActions.includes("ANSWER");
  const buttonActions = validActions.filter(
    (action) => !(hasInlineAnswer && action === "ANSWER") && !isControlAction(action)
  );
  const primaryAction = buttonActions[0] ?? null;
  const secondaryActions = buttonActions.slice(1);
  const liveSubjectChanged = activeCommand ? taskSubjectChanged(activeCommand.subject, task) : false;
  const requiresAcknowledgement = liveSubjectChanged || commands.requiresAcknowledgement;
  const actionUnavailable = Boolean(activeAction && !validActions.includes(activeAction));

  const openAction = React.useCallback(async (action: string) => {
    commands.clearError();
    setEditLoadError(null);
    if (action !== "EDIT" || task.description !== undefined) {
      setActiveCommand({ action, subject: snapshotTaskSubject(task) });
      return;
    }
    try {
      const result = await loadEditTask({ variables: { taskId: task.taskId } });
      if (!result.data?.task) throw new Error("Task unavailable");
      setActiveCommand({ action, subject: snapshotTaskSubject(result.data.task) });
    } catch {
      setEditLoadError("Task details could not be loaded for editing.");
    }
  }, [commands, loadEditTask, task]);

  const acknowledgeLatest = React.useCallback(async () => {
    if (!activeCommand) return;
    const result = await loadEditTask({ variables: { taskId: task.taskId } });
    if (!result.data?.task) throw new Error("Task unavailable");
    const latest: TaskCommandSubject = result.data.task;
    setActiveCommand({ action: activeCommand.action, subject: snapshotTaskSubject(latest) });
    commands.acknowledge();
  }, [activeCommand, commands, loadEditTask, task]);

  const controls = (
    <TaskControlsPortal hostRef={controlsHostRef}>
      <TaskControlsRow
        navigation={navigation}
        closeButtonRef={closeButtonRef}
        busy={commands.busy !== null || editLoad.loading}
        onCancel={validActions.includes("CANCEL") ? () => openAction("CANCEL") : undefined}
        onRetry={validActions.includes("RETRY") ? () => openAction("RETRY") : undefined}
      />
    </TaskControlsPortal>
  );
  const hasActionBody = hasInlineAnswer || buttonActions.length > 0 || Boolean(editLoadError);
  const actionBody = hasActionBody ? (
    <div {...stylex.props(styles.frame)}>
      {hasInlineAnswer ? (
        <form
          aria-label="Answer this request"
          {...stylex.props(styles.answerForm)}
          onSubmit={(event) => {
            event.preventDefault();
            if (!answer.trim() || commands.busy || commands.requiresAcknowledgement) return;
            void commands.run("ANSWER", {
              message: answer,
              approvalDecision: task.activeGate?.kind === "APPROVAL" ? approvalDecision : undefined
            }).then(() => setAnswer("")).catch(() => undefined);
          }}
        >
          {task.activeGate?.kind === "APPROVAL" ? (
            <label {...stylex.props(styles.decisionField)}>
              <span>Decision</span>
              <select
                value={approvalDecision}
                {...stylex.props(styles.decisionSelect)}
                onChange={(event) => setApprovalDecision(event.currentTarget.value as "APPROVED" | "DECLINED")}
              >
                <option value="APPROVED">Approve</option>
                <option value="DECLINED">Decline</option>
              </select>
            </label>
          ) : null}
          <TextArea
            isLabelHidden
            label="Answer"
            onChange={setAnswer}
            placeholder={task.activeGate?.kind === "APPROVAL" ? "Explain your decision" : "Type your answer"}
            rows={3}
            value={answer}
            width="100%"
          />
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
          <div {...stylex.props(styles.answerActions)}>
            <Button
              type="submit"
              size="sm"
              variant="primary"
              label="Answer"
              isLoading={commands.busy === "ANSWER"}
              isDisabled={!answer.trim() || commands.busy !== null || commands.requiresAcknowledgement}
            />
          </div>
        </form>
      ) : null}
      {buttonActions.length ? <div aria-label="Task actions" {...stylex.props(styles.actions, compact && styles.compactActions)}>
        {primaryAction ? (
          <Button
            type="button"
            size="sm"
            variant="primary"
            label={taskActionLabel(primaryAction, compact)}
            isDisabled={commands.busy !== null || editLoad.loading}
            onClick={(event) => { event.stopPropagation(); void openAction(primaryAction); }}
          />
        ) : null}
        {compact && secondaryActions.length ? (
          <details {...stylex.props(styles.more)}>
            <summary {...stylex.props(styles.moreSummary)} onClick={(event) => event.stopPropagation()}>More</summary>
            <div {...stylex.props(styles.moreMenu)}>
              {secondaryActions.map((action) => (
                <Button
                  key={action}
                  type="button"
                  size="sm"
                  variant="ghost"
                  label={taskActionLabel(action, false)}
                  isDisabled={commands.busy !== null || editLoad.loading}
                  onClick={(event) => { event.stopPropagation(); void openAction(action); }}
                />
              ))}
            </div>
          </details>
        ) : secondaryActions.map((action) => (
          <Button
            key={action}
            type="button"
            size="sm"
            variant="secondary"
            label={taskActionLabel(action, false)}
            isDisabled={commands.busy !== null || editLoad.loading}
            onClick={(event) => { event.stopPropagation(); void openAction(action); }}
          />
        ))}
      </div> : null}
      {editLoadError ? <span role="alert" {...stylex.props(styles.loadError)}>{editLoadError}</span> : null}
    </div>
  ) : null;

  return (
    <>
      {controls}
      {decision ? (
        <TaskDecisionCard
          attention={decision.attention}
          question={decision.question}
        >
          {actionBody}
        </TaskDecisionCard>
      ) : (
        actionBody ? <div {...stylex.props(styles.defaultFrame)}>{actionBody}</div> : null
      )}
      <span aria-live="polite" {...stylex.props(styles.srOnly)}>{commands.notice}</span>
      <TaskActionDialog
        key={activeAction ?? "closed"}
        action={activeAction}
        task={commandTask}
        projects={projects}
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
    </>
  );
}

function TaskControlsPortal({
  hostRef,
  children
}: {
  hostRef: React.RefObject<HTMLElement | null>;
  children: React.ReactNode;
}) {
  const [host, setHost] = React.useState<HTMLElement | null>(null);

  React.useLayoutEffect(() => {
    setHost(hostRef.current);
  }, [hostRef]);

  return host ? createPortal(children, host) : null;
}

function TaskControlsRow({
  navigation,
  closeButtonRef,
  busy = false,
  onCancel,
  onRetry
}: {
  navigation: TaskActionNavigation;
  closeButtonRef?: React.RefObject<HTMLButtonElement | null>;
  busy?: boolean;
  onCancel?: () => void | Promise<void>;
  onRetry?: () => void | Promise<void>;
}) {
  const iconProps = { "aria-hidden": true, size: 15, strokeWidth: 2 } as const;
  return (
    <div role="toolbar" aria-label="Task controls" {...stylex.props(styles.controls)}>
      {onCancel ? (
        <IconButton
          type="button"
          size="sm"
          variant="destructive"
          label="Cancel task"
          tooltip="Cancel task"
          icon={<Ban {...iconProps} />}
          isDisabled={busy}
          onClick={() => void onCancel()}
        />
      ) : null}
      {onRetry ? (
        <IconButton
          type="button"
          size="sm"
          variant="ghost"
          label="Retry task"
          tooltip="Retry task"
          icon={<RotateCcw {...iconProps} />}
          isDisabled={busy}
          onClick={() => void onRetry()}
        />
      ) : null}
      {navigation.showWorkLink ? (
        <IconButton
          href={`/work/tasks/${encodeURIComponent(navigation.taskId)}`}
          size="sm"
          variant="ghost"
          label="Open in Work"
          tooltip="Open in Work"
          icon={<ExternalLink {...iconProps} />}
        />
      ) : null}
      {navigation.onClose ? (
        <IconButton
          ref={closeButtonRef}
          type="button"
          size="sm"
          variant="ghost"
          label="Close task details"
          tooltip="Close task details"
          icon={<X {...iconProps} />}
          onClick={navigation.onClose}
        />
      ) : null}
    </div>
  );
}

function isControlAction(action: string): boolean {
  return action === "CANCEL" || action === "RETRY";
}

const styles = stylex.create({
  frame: { display: "grid", gap: 6 },
  defaultFrame: { display: "grid", gap: "var(--spacing-1-5)", minWidth: 0, paddingBlock: "var(--spacing-1)", paddingInline: "var(--spacing-2)" },
  controls: { display: "flex", minHeight: 28, alignItems: "center", justifyContent: "flex-end", gap: "var(--spacing-0-5)" },
  answerForm: { display: "grid", gap: 9, minWidth: 0 },
  decisionField: { display: "grid", gap: 5, color: "var(--noema-text-secondary)", fontSize: 11, fontWeight: 650 },
  decisionSelect: { minHeight: 34, width: "100%", borderWidth: 1, borderStyle: "solid", borderColor: "var(--noema-border-default)", borderRadius: 7, backgroundColor: "var(--noema-surface-card)", paddingInline: 9, color: "var(--noema-text-primary)", font: "inherit", ":focus-visible": { outlineWidth: 2, outlineStyle: "solid", outlineColor: "var(--noema-pine-500)", outlineOffset: 1 } },
  answerActions: { display: "flex", justifyContent: "flex-end" },
  stale: { display: "grid", justifyItems: "start", gap: 7, borderRadius: 7, backgroundColor: "var(--noema-surface-card)", padding: 9, color: "var(--noema-clay-600)", fontSize: 11, lineHeight: 1.4 },
  inlineError: { margin: 0, color: "var(--noema-red-700)", fontSize: 11, lineHeight: 1.4 },
  actions: { display: "flex", flexWrap: "wrap", gap: 6 },
  compactActions: { gap: 4 },
  more: { position: "relative" },
  moreSummary: { minHeight: 30, display: "inline-flex", alignItems: "center", borderRadius: 6, paddingInline: 8, color: "var(--noema-text-secondary)", fontSize: 11, fontWeight: 650, cursor: "pointer", listStyle: "none", ":hover": { backgroundColor: "var(--noema-surface-hover)" }, ":focus-visible": { outlineWidth: 2, outlineStyle: "solid", outlineColor: "var(--noema-pine-500)", outlineOffset: 1 } },
  moreMenu: { position: "absolute", top: "calc(100% + 4px)", right: 0, zIndex: 3, display: "grid", minWidth: 144, gap: 2, borderWidth: 1, borderStyle: "solid", borderColor: "var(--noema-border-subtle)", borderRadius: 7, backgroundColor: "var(--noema-surface-card)", padding: 4, boxShadow: "0 8px 24px color-mix(in srgb, var(--noema-text-primary) 12%, transparent)" },
  srOnly: { position: "absolute", width: 1, height: 1, overflow: "hidden", clip: "rect(0 0 0 0)", whiteSpace: "nowrap" },
  loadError: { color: "var(--destructive)", fontSize: 11 }
});

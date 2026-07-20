import * as React from "react";
import { createPortal } from "react-dom";
import { useLazyQuery } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import { ButtonGroup } from "@astryxdesign/core/ButtonGroup";
import { IconButton } from "@astryxdesign/core/IconButton";
import { TextArea } from "@astryxdesign/core/TextArea";
import { Tooltip } from "@astryxdesign/core/Tooltip";
import * as stylex from "@stylexjs/stylex";
import { Ban, ExternalLink, X } from "lucide-react";
import type { WorkProject } from "./workTypes";
import { useTaskCommands, type TaskCommandSubject } from "./useTaskCommands";
import { TaskActionDialog } from "./TaskActionDialog";
import { orderTaskActions, taskActionLabel } from "./taskActionModel";
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

type TaskActionsProps = {
  task: TaskCommandSubject;
  validActions: readonly string[];
  navigation: TaskActionNavigation;
  closeButtonRef?: React.RefObject<HTMLButtonElement | null>;
  controlsHostRef: React.RefObject<HTMLElement | null>;
  projects?: readonly WorkProject[];
  compact?: boolean;
  inlineResponse?: boolean;
  onUpdated?: () => void | Promise<void>;
  children?: (actions: React.ReactNode) => React.ReactNode;
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
  projects = [],
  compact = false,
  inlineResponse = false,
  onUpdated,
  children
}: TaskActionsProps) {
  const [activeCommand, setActiveCommand] = React.useState<ActiveCommand | null>(null);
  const [editLoadError, setEditLoadError] = React.useState<string | null>(null);
  const [answer, setAnswer] = React.useState("");
  const [approvalDecision, setApprovalDecision] = React.useState<"APPROVED" | "DECLINED">("APPROVED");
  const answerInputRef = React.useRef<HTMLTextAreaElement>(null);
  const [loadEditTask, editLoad] = useLazyQuery(WorkTaskEditFieldsDocument, { fetchPolicy: "network-only" });
  const commandTask = activeCommand?.subject ?? task;
  const refresh = React.useCallback(async () => {
    await onUpdated?.();
  }, [onUpdated]);
  const commands = useTaskCommands({ task: commandTask, onUpdated: refresh });
  const activeAction = activeCommand?.action ?? null;
  const canAnswer = validActions.includes("ANSWER");
  const canRetry = validActions.includes("RETRY");
  const hasInlineResponse = inlineResponse && (canAnswer || canRetry);
  const responseAction = answer.trim() && canAnswer ? "ANSWER" : canRetry ? "RETRY" : "ANSWER";
  const buttonActions = orderTaskActions(validActions.filter(
    (action) => action !== "CANCEL" && !(hasInlineResponse && (action === "ANSWER" || action === "RETRY"))
  ));
  const primaryAction = buttonActions.find((action) => action !== "CANCEL") ?? null;
  const secondaryActions = buttonActions.filter(
    (action) => action !== primaryAction
  );
  const visibleSecondaryAction = compact
    ? secondaryActions.find((action) => action !== "CANCEL") ?? null
    : null;
  const overflowActions = compact
    ? secondaryActions.filter((action) => action !== visibleSecondaryAction)
    : [];
  const selectedApprovalDecision = task.activeGate?.kind === "APPROVAL" ? approvalDecision : undefined;
  const liveSubjectChanged = activeCommand ? taskSubjectChanged(activeCommand.subject, task) : false;
  const requiresAcknowledgement = liveSubjectChanged || commands.requiresAcknowledgement;
  const actionUnavailable = Boolean(activeAction && !validActions.includes(activeAction));

  React.useLayoutEffect(() => {
    const input = answerInputRef.current;
    if (!input) return;
    input.style.height = "0px";
    input.style.overflowY = "hidden";
    input.style.resize = "none";
    input.style.height = `${input.scrollHeight}px`;
  }, [answer]);

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
        busy={commands.busy !== null || editLoad.loading || activeCommand !== null}
        onCancel={validActions.includes("CANCEL") ? () => openAction("CANCEL") : undefined}
      />
    </TaskControlsPortal>
  );
  const hasActionBody = hasInlineResponse || buttonActions.length > 0 || Boolean(editLoadError);
  const actionBody = hasActionBody ? (
    <div {...stylex.props(styles.frame)}>
      {hasInlineResponse ? (
        <form
          aria-label={canRetry ? "Respond or retry" : "Answer this request"}
          {...stylex.props(styles.answerForm)}
          onSubmit={(event) => {
            event.preventDefault();
            if ((responseAction === "ANSWER" && !answer.trim()) || commands.busy || commands.requiresAcknowledgement) return;
            void commands.run(responseAction, {
              message: answer.trim() || undefined,
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
          <div {...stylex.props(styles.answerComposerRow)}>
            <TextArea
              ref={answerInputRef}
              isLabelHidden
              label={canRetry ? "Response or retry guidance" : "Answer"}
              onChange={setAnswer}
              placeholder={task.activeGate?.kind === "APPROVAL"
                ? "Explain your decision"
                : canAnswer && canRetry
                  ? "Answer, or leave blank to retry"
                  : canRetry
                    ? "Optional retry guidance"
                    : "Type your answer"}
              rows={1}
              value={answer}
              width="100%"
              className={stylex.props(styles.answerInputField).className}
            />
            <Button
              type="submit"
              size="sm"
              variant="primary"
              label={taskActionLabel(responseAction, false, task.activeGate?.kind === "APPROVAL" ? approvalDecision : undefined)}
              isLoading={commands.busy === responseAction}
              isDisabled={(responseAction === "ANSWER" && !answer.trim()) || commands.busy !== null || commands.requiresAcknowledgement}
            />
          </div>
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
      {buttonActions.length ? <div aria-label="Task actions" {...stylex.props(styles.actions, compact && styles.compactActions)}>
        {primaryAction ? (
          <Button
            type="button"
            size="sm"
            variant="primary"
            label={taskActionLabel(primaryAction, compact, selectedApprovalDecision)}
            isDisabled={commands.busy !== null || editLoad.loading}
            onClick={(event) => { event.stopPropagation(); void openAction(primaryAction); }}
          />
        ) : null}
        {visibleSecondaryAction ? (
          <Button
            type="button"
            size="sm"
            variant="secondary"
            label={taskActionLabel(visibleSecondaryAction, false, selectedApprovalDecision)}
            isDisabled={commands.busy !== null || editLoad.loading}
            onClick={(event) => { event.stopPropagation(); void openAction(visibleSecondaryAction); }}
          />
        ) : null}
        {compact && overflowActions.length ? (
          <details {...stylex.props(styles.more)}>
            <summary {...stylex.props(styles.moreSummary)} onClick={(event) => event.stopPropagation()}>More</summary>
            <div {...stylex.props(styles.moreMenu)}>
              {overflowActions.map((action) => (
                <Button
                  key={action}
                  type="button"
                  size="sm"
                  variant={action === "CANCEL" ? "destructive" : "ghost"}
                  label={taskActionLabel(action, false, selectedApprovalDecision)}
                  isDisabled={commands.busy !== null || editLoad.loading}
                  onClick={(event) => { event.stopPropagation(); void openAction(action); }}
                />
              ))}
            </div>
          </details>
        ) : !compact ? secondaryActions.map((action) => (
          <Button
            key={action}
            type="button"
            size="sm"
            variant="secondary"
            label={taskActionLabel(action, false, selectedApprovalDecision)}
            isDisabled={commands.busy !== null || editLoad.loading}
            onClick={(event) => { event.stopPropagation(); void openAction(action); }}
          />
        )) : null}
      </div> : null}
      {editLoadError ? <span role="alert" {...stylex.props(styles.loadError)}>{editLoadError}</span> : null}
    </div>
  ) : null;

  return (
    <>
      {controls}
      {children ? children(actionBody ? <div {...stylex.props(styles.defaultFrame)}>{actionBody}</div> : null) : (
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
  onCancel
}: {
  navigation: TaskActionNavigation;
  closeButtonRef?: React.RefObject<HTMLButtonElement | null>;
  busy?: boolean;
  onCancel?: () => void | Promise<void>;
}) {
  const iconProps = { "aria-hidden": true, size: 17, strokeWidth: 2 } as const;
  const cancelAnchorRef = React.useRef<HTMLButtonElement>(null);
  const workAnchorRef = React.useRef<HTMLButtonElement>(null);
  const fallbackCloseAnchorRef = React.useRef<HTMLButtonElement>(null);
  const closeAnchorRef = closeButtonRef ?? fallbackCloseAnchorRef;
  const taskControlsGroupClassName = stylex.props(styles.taskControlsGroup).className;
  return (
    <>
      <ButtonGroup label="Task controls" size="lg" className={taskControlsGroupClassName}>
        {onCancel ? (
          <IconButton
            ref={cancelAnchorRef}
            type="button"
            size="sm"
            variant="secondary"
            label="Cancel task"
            icon={<Ban {...iconProps} color="var(--color-error)" />}
            isDisabled={busy}
            onClick={() => void onCancel()}
          />
        ) : null}
        {navigation.showWorkLink ? (
          <IconButton
            ref={workAnchorRef}
            href={`/work/tasks/${encodeURIComponent(navigation.taskId)}`}
            size="sm"
            variant="secondary"
            label="Open in Work"
            icon={<ExternalLink {...iconProps} />}
          />
        ) : null}
        {navigation.onClose ? (
          <IconButton
            ref={closeButtonRef}
            type="button"
            size="sm"
            variant="secondary"
            label="Close task details"
            icon={<X {...iconProps} />}
            onClick={navigation.onClose}
          />
        ) : null}
      </ButtonGroup>
      {onCancel ? <Tooltip anchorRef={cancelAnchorRef} content="Cancel task" /> : null}
      {navigation.showWorkLink ? <Tooltip anchorRef={workAnchorRef} content="Open in Work" /> : null}
      {navigation.onClose ? <Tooltip anchorRef={closeAnchorRef} content="Close task details" /> : null}
    </>
  );
}

const styles = stylex.create({
  frame: { display: "grid", gap: "var(--spacing-1-5)" },
  defaultFrame: { display: "grid", gap: "var(--spacing-1-5)", minWidth: 0 },
  taskControlsGroup: {
    borderRadius: "var(--radius-element)",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--noema-border-subtle)",
    backgroundColor: "#fff",
    padding: "var(--spacing-1)",
    boxShadow: "0 8px 22px color-mix(in srgb, var(--noema-text-primary) 14%, transparent)",
  },
  answerForm: { display: "grid", gap: "var(--spacing-1-5)", minWidth: 0, borderWidth: 0, borderColor: "transparent", backgroundColor: "transparent", padding: 0 },
  answerComposerRow: { display: "flex", minWidth: 0, alignItems: "flex-end", gap: "var(--spacing-1-5)" },
  answerInputField: { flex: 1, minWidth: 0, borderWidth: 0, borderColor: "transparent", backgroundColor: "transparent", boxShadow: "none", paddingBlock: 0, paddingInline: 0 },
  decisionField: { display: "grid", gap: "var(--spacing-1-5)", color: "var(--noema-text-secondary)", fontSize: 11, fontWeight: 650 },
  decisionSelect: { minHeight: 34, width: "100%", borderWidth: 1, borderStyle: "solid", borderColor: "var(--noema-border-default)", borderRadius: 7, backgroundColor: "var(--noema-surface-card)", paddingInline: "var(--spacing-2)", color: "var(--noema-text-primary)", font: "inherit", ":focus-visible": { outlineWidth: 2, outlineStyle: "solid", outlineColor: "var(--noema-pine-500)", outlineOffset: 1 } },
  stale: { display: "grid", justifyItems: "start", gap: "var(--spacing-2)", borderRadius: 7, backgroundColor: "var(--noema-surface-card)", padding: "var(--spacing-2)", color: "var(--noema-clay-600)", fontSize: 11, lineHeight: 1.4 },
  inlineError: { margin: 0, color: "var(--noema-red-700)", fontSize: 11, lineHeight: 1.4 },
  actions: { display: "flex", flexWrap: "wrap", gap: "var(--spacing-1-5)" },
  compactActions: { gap: "var(--spacing-1)" },
  more: { position: "relative" },
  moreSummary: { minHeight: 30, display: "inline-flex", alignItems: "center", borderRadius: 6, paddingInline: "var(--spacing-2)", color: "var(--noema-text-secondary)", fontSize: 11, fontWeight: 650, cursor: "pointer", listStyle: "none", ":hover": { backgroundColor: "var(--noema-surface-hover)" }, ":focus-visible": { outlineWidth: 2, outlineStyle: "solid", outlineColor: "var(--noema-pine-500)", outlineOffset: 1 } },
  moreMenu: { position: "absolute", top: "calc(100% + var(--spacing-1))", right: 0, zIndex: 3, display: "grid", minWidth: 144, gap: "var(--spacing-0-5)", borderWidth: 1, borderStyle: "solid", borderColor: "var(--noema-border-subtle)", borderRadius: 7, backgroundColor: "var(--noema-surface-card)", padding: "var(--spacing-1)", boxShadow: "0 8px 24px color-mix(in srgb, var(--noema-text-primary) 12%, transparent)" },
  srOnly: { position: "absolute", width: 1, height: 1, overflow: "hidden", clip: "rect(0 0 0 0)", whiteSpace: "nowrap" },
  loadError: { color: "var(--destructive)", fontSize: 11 }
});

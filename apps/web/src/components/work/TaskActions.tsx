import * as React from "react";
import { useLazyQuery } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
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

export function TaskActions({ task, validActions, projects = [], compact = false, onUpdated }: { task: TaskCommandSubject; validActions: readonly string[]; projects?: readonly WorkProject[]; compact?: boolean; onUpdated?: () => void | Promise<void> }) {
  const [activeCommand, setActiveCommand] = React.useState<ActiveCommand | null>(null);
  const [editLoadError, setEditLoadError] = React.useState<string | null>(null);
  const [loadEditTask, editLoad] = useLazyQuery(WorkTaskEditFieldsDocument, { fetchPolicy: "network-only" });
  const commandTask = activeCommand?.subject ?? task;
  const refresh = React.useCallback(async () => {
    await onUpdated?.();
  }, [onUpdated]);
  const commands = useTaskCommands({ task: commandTask, onUpdated: refresh });
  const activeAction = activeCommand?.action ?? null;
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

  if (validActions.length === 0 && !activeCommand) return null;
  return (
    <div {...stylex.props(styles.frame)}>
      <div aria-label="Task actions" {...stylex.props(styles.actions)}>
        {validActions.map((action, index) => <Button key={action} type="button" size="sm" variant={action === "CANCEL" ? "destructive" : index === 0 ? "primary" : "secondary"} label={taskActionLabel(action, compact)} isDisabled={commands.busy !== null || editLoad.loading} onClick={(event) => { event.stopPropagation(); void openAction(action); }} />)}
      </div>
      <span aria-live="polite" {...stylex.props(styles.srOnly)}>{commands.notice}</span>
      {editLoadError ? <span role="alert" {...stylex.props(styles.loadError)}>{editLoadError}</span> : null}
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
    </div>
  );
}

const styles = stylex.create({
  frame: { display: "grid", gap: 6 },
  actions: { display: "flex", flexWrap: "wrap", gap: 6 },
  srOnly: { position: "absolute", width: 1, height: 1, overflow: "hidden", clip: "rect(0 0 0 0)", whiteSpace: "nowrap" },
  loadError: { color: "var(--destructive)", fontSize: 11 }
});

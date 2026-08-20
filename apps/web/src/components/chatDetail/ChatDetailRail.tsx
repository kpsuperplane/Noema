import React from "react";
import { Button } from "@astryxdesign/core/Button";
import { IconButton } from "@astryxdesign/core/IconButton";
import * as stylex from "@stylexjs/stylex";
import { Link } from "@tanstack/react-router";
import { ArrowLeft, Check, Pencil, RefreshCcw, X } from "lucide-react";
import {
  AnimatePresence,
  useIsPresent,
  useReducedMotion,
  type Variants
} from "motion/react";
import * as m from "motion/react-m";
import { springs } from "@/motion/springs";
import { useDetailPanePresentation } from "@/components/shell/MasterDetailLayout";
import {
  ArtifactDetailPanel,
  ArtifactDownloadAction,
  type ArtifactDetail
} from "./ArtifactDetailPanel";
import type { ChatDetailTarget } from "./chatDetailTypes";
import { TaskDetailQueryPanel } from "./task/TaskDetailQueryPanel";
import type { TaskDetail } from "./task/taskTypes";
import { TaskRecurrenceDetailPanel, type RecurrenceInlineEditController } from "../tasks/TaskRecurrenceDetailPanel";
import { taskScheduleTimestampLabel } from "../tasks/tasksModel";
import { normalizeTasksSearch } from "../tasks/tasksTypes";
import { ChatDetailCloseButton } from "./ChatDetailCloseButton";
import type { TaskInlineEditController } from "@/components/tasks/TaskActions";
import { ErrorMarker } from "@/components/ErrorMarker";
import { RenderErrorBoundary } from "@/components/errors/RenderErrorBoundary";

export function ChatDetailRail({
  target,
  onClose,
  animateEntrance = true,
  showTasksLink = true
}: {
  target: ChatDetailTarget;
  onClose: () => void;
  animateEntrance?: boolean;
  showTasksLink?: boolean;
}) {
  const targetKey = detailTargetKey(target);
  return (
    <RenderErrorBoundary
      errorScope={`detail.${targetKey}`}
      resetKey={targetKey}
      fallback={({ retry }) => (
        <ChatDetailRailError onClose={onClose} onRetry={retry} />
      )}
    >
      <RoutedChatDetailRail
        key={targetKey}
        animateEntrance={animateEntrance}
        initialTarget={target}
        onClose={onClose}
        showTasksLink={showTasksLink}
      />
    </RenderErrorBoundary>
  );
}

function ChatDetailRailError({
  onClose,
  onRetry
}: {
  onClose: () => void;
  onRetry: () => void;
}) {
  const closeButtonRef = React.useRef<HTMLButtonElement>(null);
  const contained = useDetailPanePresentation() === "drawer";
  return (
    <aside
      aria-label="Detail recovery"
      role={contained ? "region" : "complementary"}
      {...stylex.props(styles.rail, contained && styles.containedRail)}
    >
      <div {...stylex.props(styles.surface, styles.errorSurface)}>
        <header {...stylex.props(styles.header, styles.errorHeader)}>
          <h2 {...stylex.props(styles.title)}>Details unavailable</h2>
          <ChatDetailCloseButton
            closeButtonRef={closeButtonRef}
            onClose={onClose}
          />
        </header>
        <div {...stylex.props(styles.errorBody)}>
          <ErrorMarker
            message="This detail could not display. Chat and other pages remain available."
            recoverable={false}
          />
          <Button
            type="button"
            size="sm"
            variant="secondary"
            label="Retry details"
            onClick={onRetry}
          />
        </div>
      </div>
    </aside>
  );
}

function RoutedChatDetailRail({
  animateEntrance,
  initialTarget,
  onClose,
  showTasksLink
}: {
  animateEntrance: boolean;
  initialTarget: ChatDetailTarget;
  onClose: () => void;
  showTasksLink: boolean;
}) {
  const [history, setHistory] = React.useState<readonly ChatDetailTarget[]>([initialTarget]);
  const target = history.at(-1) ?? initialTarget;
  const canGoBack = history.length > 1;
  const isContained = useDetailPanePresentation() === "drawer";
  const isModal = useNarrowViewport() && !isContained;
  const isPresent = useIsPresent();
  const reduceMotion = useReducedMotion();
  const railRef = React.useRef<HTMLElement>(null);
  const taskCloseButtonRef = React.useRef<HTMLButtonElement>(null);
  const artifactCloseButtonRef = React.useRef<HTMLButtonElement>(null);
  const hasEnteredRef = React.useRef(!animateEntrance);
  const returnFocusRef = React.useRef<HTMLElement | null>(null);
  const primaryTarget = initialTarget.type === "artifact" ? null : initialTarget;
  const taskTargetId = primaryTarget?.type === "task" ? primaryTarget.taskId : null;
  const recurrenceTargetId = primaryTarget?.type === "recurrence" ? primaryTarget.recurrenceId : null;
  const [taskHeaderState, setTaskHeaderState] = React.useState<{
    taskId: string;
    title: string;
    schedule?: TaskDetail["schedule"];
    edit: TaskInlineEditController;
  } | null>(null);
  const [recurrenceHeader, setRecurrenceHeader] = React.useState<{
    title: string;
    edit: RecurrenceInlineEditController;
  } | null>(null);
  const [artifactDetailState, setArtifactDetailState] = React.useState<{
    version: string;
    detail: ArtifactDetail | null;
    latestDetail: ArtifactDetail | null;
  } | null>(null);
  const artifactVersion = target.type === "artifact" ? target.version : null;
  const artifactDetail = artifactVersion && artifactDetailState?.version === artifactVersion
    ? artifactDetailState.detail
    : null;
  const latestArtifactDetail = artifactDetailState?.latestDetail ?? null;
  const title = artifactDetail?.title ??
    latestArtifactDetail?.title ??
    (target.type === "artifact" ? "Artifact" : "Task details");
  const currentTaskHeader = taskTargetId && taskHeaderState?.taskId === taskTargetId
    ? taskHeaderState
    : null;
  const recurringOccurrence = currentTaskHeader?.schedule?.recurrenceId
    ? currentTaskHeader.schedule
    : null;
  const primaryTitle = recurrenceTargetId ? recurrenceHeader?.title ?? "Recurring task" : currentTaskHeader
    ? currentTaskHeader.title
    : "Task details";
  const handleTaskHeaderChange = React.useCallback((detail: Pick<TaskDetail, "title" | "schedule">, edit: TaskInlineEditController) => {
    if (taskTargetId) setTaskHeaderState({ taskId: taskTargetId, ...detail, edit });
  }, [taskTargetId]);
  const handleRecurrenceHeaderChange = React.useCallback((title: string, edit: RecurrenceInlineEditController) => {
    setRecurrenceHeader({ title, edit });
  }, []);
  React.useEffect(() => {
    if (isContained) {
      returnFocusRef.current = null;
      return;
    }
    const active = document.activeElement;
    if (
      isPresent &&
      returnFocusRef.current === null &&
      active instanceof HTMLElement &&
      active !== document.body &&
      !railRef.current?.contains(active)
    ) {
      returnFocusRef.current = active;
    } else if (!isPresent) {
      returnFocusRef.current?.focus({ preventScroll: true });
      returnFocusRef.current = null;
    }
  }, [isContained, isPresent]);

  React.useEffect(() => {
    if ((isModal || isContained) && isPresent && (reduceMotion || hasEnteredRef.current)) {
      const closeButton = target.type === "artifact"
        ? artifactCloseButtonRef.current
        : taskCloseButtonRef.current;
      closeButton?.focus({ preventScroll: true });
    }
  }, [isContained, isModal, isPresent, reduceMotion, target]);

  const updateArtifactDetail = React.useCallback(
    (detail: ArtifactDetail | null) => {
      if (!artifactVersion) {
        return;
      }
      setArtifactDetailState((previous) => ({
        version: artifactVersion,
        detail,
        latestDetail: detail ?? previous?.latestDetail ?? null
      }));
    },
    [artifactVersion]
  );
  const openDetail = React.useCallback((nextTarget: ChatDetailTarget) => {
    setHistory((previous) => [...previous, nextTarget]);
  }, []);
  const changeArtifactVersion = React.useCallback((version: string) => {
    setHistory((previous) => [
      ...previous.slice(0, -1),
      { type: "artifact", version }
    ]);
  }, []);
  const goBack = React.useCallback(() => {
    setHistory((previous) => previous.length > 1 ? previous.slice(0, -1) : previous);
  }, []);

  const handleKeyDown = React.useCallback((event: React.KeyboardEvent<HTMLElement>) => {
    if (!isContained && event.key === "Escape") {
      event.preventDefault();
      onClose();
      return;
    }
    if (!isModal || event.key !== "Tab") {
      return;
    }
    const focusable = focusableElements(railRef.current);
    if (focusable.length === 0) {
      event.preventDefault();
      railRef.current?.focus();
      return;
    }
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    if (event.shiftKey && event.target === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && event.target === last) {
      event.preventDefault();
      first.focus();
    }
  }, [isContained, isModal, onClose]);

  return (
    <m.aside
      data-slot="chat-detail-rail"
      data-state={isPresent ? "open" : "exiting"}
      aria-modal={isModal ? "true" : undefined}
      aria-hidden={isPresent ? undefined : "true"}
      aria-label={target.type === "task" ? "Task details" : target.type === "recurrence" ? "Recurring task details" : "Artifact details"}
      ref={railRef}
      role={isModal ? "dialog" : isContained ? "region" : "complementary"}
      inert={!isPresent}
      variants={chatDetailRailVariants}
      initial={isContained || reduceMotion || !animateEntrance ? false : "hidden"}
      animate="visible"
      exit={isContained ? "visible" : "hidden"}
      transition={reduceMotion ? { duration: 0 } : springs.surface}
      {...stylex.props(styles.rail, isContained && styles.containedRail)}
      tabIndex={isModal ? -1 : undefined}
      onKeyDown={handleKeyDown}
      onAnimationComplete={(definition) => {
        if (definition === "visible") {
          hasEnteredRef.current = true;
          if (isModal && isPresent) {
            const closeButton = target.type === "artifact"
              ? artifactCloseButtonRef.current
              : taskCloseButtonRef.current;
            closeButton?.focus({ preventScroll: true });
          }
        }
      }}
    >
      <div data-slot="chat-detail-rail-surface" {...stylex.props(styles.surface)}>
        {primaryTarget ? (
          <m.div
            data-slot="chat-detail-task-route"
            aria-hidden={target.type === "artifact" ? "true" : undefined}
            inert={target.type === "artifact"}
            variants={taskRouteVariants}
            initial={false}
            animate={target.type === "artifact" ? "nested" : "visible"}
            transition={reduceMotion ? { duration: 0 } : springs.surface}
            {...stylex.props(styles.routeFrame)}
          >
            <header {...stylex.props(styles.header, styles.taskHeader)}>
              <div {...stylex.props(styles.taskTitleBar)}>
                <div {...stylex.props(styles.taskIdentity)}>
                  <EditableTaskTitle title={primaryTitle} edit={recurrenceTargetId ? recurrenceHeader?.edit : currentTaskHeader?.edit} />
                  {recurringOccurrence?.recurrenceId ? (
                    <div {...stylex.props(styles.taskSubtitle)}>
                      <span>Scheduled for {taskScheduleTimestampLabel(recurringOccurrence.scheduledFor, recurringOccurrence.timeZone)}</span>
                      <span aria-hidden="true"> · </span>
                      <Link
                        to="/tasks/recurrences/$recurrenceId"
                        params={{ recurrenceId: recurringOccurrence.recurrenceId }}
                        search={(current) => normalizeTasksSearch(current)}
                        {...stylex.props(styles.taskRelationLink)}
                      >
                        View recurring task
                      </Link>
                    </div>
                  ) : null}
                </div>
                <div {...stylex.props(styles.taskHeaderActions)}>
                  <ChatDetailCloseButton
                    closeButtonRef={taskCloseButtonRef}
                    onClose={onClose}
                  />
                </div>
              </div>
            </header>
            <div {...stylex.props(styles.body, styles.taskBody)}>
              {taskTargetId ? <TaskDetailQueryPanel
                onOpenDetail={openDetail}
                onTaskHeaderChange={handleTaskHeaderChange}
                showTasksLink={showTasksLink}
                taskId={taskTargetId}
              /> : recurrenceTargetId ? <TaskRecurrenceDetailPanel recurrenceId={recurrenceTargetId} onTitleChange={handleRecurrenceHeaderChange} /> : null}
            </div>
          </m.div>
        ) : null}
        <AnimatePresence initial={false}>
          {target.type === "artifact" ? (
            <ArtifactRouteFrame
              key="artifact-route"
              artifactDetail={artifactDetail}
              canGoBack={canGoBack}
              closeButtonRef={artifactCloseButtonRef}
              reduceMotion={Boolean(reduceMotion)}
              title={title}
              version={target.version}
              onChangeVersion={changeArtifactVersion}
              onDetailChange={updateArtifactDetail}
              onClose={onClose}
              onGoBack={goBack}
            />
          ) : null}
        </AnimatePresence>
      </div>
    </m.aside>
  );
}

type InlineTitleEditController = TaskInlineEditController | RecurrenceInlineEditController;

function EditableTaskTitle({ title, edit }: { title: string; edit?: InlineTitleEditController }) {
  const subject = edit && "recurrence" in edit ? "recurring task" : "task";
  if (!edit) return <h2 {...stylex.props(styles.title)}>{title}</h2>;
  if (edit.field === "TITLE") return <TaskTitleEditor key="editing" title={title} edit={edit} subject={subject} />;
  return (
    <div {...stylex.props(styles.editableTitle)}>
      <h2 {...stylex.props(styles.title)}>{title}</h2>
      <span {...stylex.props(styles.titleReadControls)}>
        {edit.canEdit ? <IconButton type="button" size="sm" variant="ghost" label={`Edit ${subject} title`} tooltip="Edit title" icon={<Pencil aria-hidden="true" size={14} />} isDisabled={edit.busy || !edit.canStart} onClick={() => void edit.start("TITLE")} /> : null}
      </span>
    </div>
  );
}

function TaskTitleEditor({ title, edit, subject }: { title: string; edit: InlineTitleEditController; subject: string }) {
  const [draft, setDraft] = React.useState(title);
  return (
    <form {...stylex.props(styles.titleEditor)} onSubmit={(event) => { event.preventDefault(); if (draft.trim()) void edit.saveTitle(draft.trim()).catch(() => undefined); }}>
      <input autoFocus aria-label={`${subject} title`} aria-invalid={Boolean(edit.error || edit.actionUnavailable)} value={draft} {...stylex.props(styles.titleInput)} onChange={(event) => setDraft(event.currentTarget.value)} onKeyDown={(event) => { if (event.key === "Escape") edit.cancel(); }} />
      <span {...stylex.props(styles.titleEditControls)}>
        {edit.requiresAcknowledgement ? <IconButton type="button" size="sm" variant="ghost" label={`Use latest ${subject}`} tooltip={`Use latest ${subject}`} icon={<RefreshCcw aria-hidden="true" size={14} />} isDisabled={edit.busy} onClick={() => void edit.acknowledge().catch(() => undefined)} /> : <IconButton type="submit" size="sm" variant="ghost" label={`Save ${subject} title`} tooltip="Save title" icon={<Check aria-hidden="true" size={14} />} isDisabled={edit.busy || edit.actionUnavailable || !draft.trim()} />}
        <IconButton type="button" size="sm" variant="ghost" label="Cancel title edit" tooltip="Cancel" icon={<X aria-hidden="true" size={14} />} isDisabled={edit.busy} onClick={edit.cancel} />
      </span>
      {edit.error ? <span role="alert" {...stylex.props(styles.srOnly)}>{edit.error}</span> : null}
    </form>
  );
}

function ArtifactRouteFrame({
  artifactDetail,
  canGoBack,
  closeButtonRef,
  reduceMotion,
  title,
  version,
  onChangeVersion,
  onClose,
  onDetailChange,
  onGoBack
}: {
  artifactDetail: ArtifactDetail | null;
  canGoBack: boolean;
  closeButtonRef: React.RefObject<HTMLButtonElement | null>;
  reduceMotion: boolean;
  title: string;
  version: string;
  onChangeVersion: (version: string) => void;
  onClose: () => void;
  onDetailChange: (detail: ArtifactDetail | null) => void;
  onGoBack: () => void;
}) {
  const isPresent = useIsPresent();

  return (
    <m.div
      data-slot="chat-detail-artifact-route"
      aria-hidden={isPresent ? undefined : "true"}
      inert={!isPresent}
      variants={artifactRouteVariants}
      initial={reduceMotion ? false : "nested"}
      animate="visible"
      exit="nested"
      transition={reduceMotion ? { duration: 0 } : springs.surface}
      {...stylex.props(styles.routeFrame)}
    >
      <header {...stylex.props(styles.header)}>
        <div {...stylex.props(styles.actionRow)}>
          <div {...stylex.props(styles.artifactIdentity)}>
            {canGoBack ? (
              <Button
                type="button"
                variant="ghost"
                size="sm"
                label="Back to task details"
                tooltip="Back to task details"
                icon={<ArrowLeft aria-hidden="true" size={16} />}
                isIconOnly
                onClick={onGoBack}
              />
            ) : null}
            <h2 title={title} {...stylex.props(styles.title, styles.artifactTitle)}>
              {title}
            </h2>
          </div>
          <div {...stylex.props(styles.actions)}>
            <ArtifactDownloadAction detail={artifactDetail} />
            <ChatDetailCloseButton closeButtonRef={closeButtonRef} onClose={onClose} />
          </div>
        </div>
      </header>
      <div {...stylex.props(styles.body)}>
        <ArtifactDetailPanel
          version={version}
          onChangeVersion={onChangeVersion}
          onDetailChange={onDetailChange}
        />
      </div>
    </m.div>
  );
}

function detailTargetKey(target: ChatDetailTarget): string {
  if (target.type === "task") return `task:${target.taskId}`;
  if (target.type === "recurrence") return `recurrence:${target.recurrenceId}`;
  return `artifact:${target.version}`;
}

const chatDetailRailVariants: Variants = {
  hidden: { opacity: 0.72, x: "100%" },
  visible: { opacity: 1, x: "0%" }
};

const taskRouteVariants: Variants = {
  visible: { x: "0%" },
  nested: { x: "-100%" }
};

const artifactRouteVariants: Variants = {
  visible: { x: "0%" },
  nested: { x: "100%" }
};

const narrowViewportQuery = "(max-width: 979px)";

function useNarrowViewport() {
  const [isNarrow, setIsNarrow] = React.useState(() => (
    typeof window !== "undefined" && typeof window.matchMedia === "function"
      ? window.matchMedia(narrowViewportQuery).matches
      : false
  ));

  React.useEffect(() => {
    if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
      return;
    }

    const query = window.matchMedia(narrowViewportQuery);
    const sync = () => setIsNarrow(query.matches);
    sync();
    query.addEventListener("change", sync);
    return () => query.removeEventListener("change", sync);
  }, []);

  return isNarrow;
}

function focusableElements(root: HTMLElement | null): HTMLElement[] {
  if (!root) {
    return [];
  }
  return Array.from(
    root.querySelectorAll<HTMLElement>(
      'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), summary, textarea:not([disabled]), [tabindex]:not([tabindex="-1"])'
    )
  ).filter((element) => !element.closest('[aria-hidden="true"], [inert]') && element.getClientRects().length > 0);
}

const styles = stylex.create({
  rail: {
    position: "absolute",
    top: 0,
    right: 0,
    bottom: 0,
    left: {
      default: 0,
      "@media (min-width: 980px)": "auto"
    },
    zIndex: 4,
    minWidth: 0,
    width: {
      default: "100%",
      "@media (min-width: 980px)": "var(--chat-detail-rail-width)"
    },
    overflow: "hidden"
  },
  containedRail: {
    position: "relative",
    top: "auto",
    right: "auto",
    bottom: "auto",
    left: "auto",
    zIndex: "auto",
    width: "100%",
    height: "100%"
  },
  surface: {
    position: "relative",
    minWidth: 0,
    height: "100%",
    overflow: "hidden",
    backgroundColor: "var(--noema-surface-card)"
  },
  errorSurface: {
    display: "grid",
    gridTemplateRows: "auto minmax(0, 1fr)"
  },
  errorHeader: {
    gridTemplateColumns: "minmax(0, 1fr) auto",
    alignItems: "center"
  },
  errorBody: {
    display: "grid",
    alignContent: "center",
    justifyItems: "center",
    gap: "var(--spacing-3)",
    padding: "var(--spacing-4)"
  },
  routeFrame: {
    position: "absolute",
    inset: 0,
    display: "grid",
    gridTemplateRows: "auto minmax(0, 1fr)",
    minWidth: 0,
    height: "100%",
    backgroundColor: "var(--noema-surface-card)",
    willChange: "transform"
  },
  header: {
    minWidth: 0,
    display: "grid",
    gap: "var(--spacing-2)",
    paddingBlock: "var(--spacing-3)",
    paddingInline: "var(--spacing-4)",
    borderBottomWidth: 1,
    borderBottomStyle: "solid",
    borderBottomColor: "var(--noema-border-subtle)"
  },
  taskHeader: {
    zIndex: 5,
    paddingBlock: "var(--spacing-2)",
    paddingInline: "var(--spacing-4)",
    borderBottomWidth: 0,
    pointerEvents: "auto"
  },
  taskTitleBar: { display: "grid", minWidth: 0, gridTemplateColumns: "minmax(0, 1fr) auto", alignItems: "center", gap: "var(--spacing-3)" },
  taskIdentity: { display: "grid", minWidth: 0, gap: "var(--spacing-0-5)" },
  editableTitle: { display: "inline-flex", width: "fit-content", maxWidth: "100%", minWidth: 0, alignItems: "center", gap: "var(--spacing-1)" },
  titleEditor: { display: "grid", width: "100%", minWidth: 0, gridTemplateColumns: "minmax(0, 1fr) 64px", alignItems: "center", gap: "var(--spacing-1)" },
  titleReadControls: { display: "inline-flex", width: 28, minHeight: 28, flexShrink: 0, alignItems: "center" },
  titleEditControls: { display: "inline-flex", width: 64, minHeight: 28, alignItems: "center", justifyContent: "flex-end", gap: "var(--spacing-1)" },
  titleInput: { minWidth: 0, width: "100%", height: 28, borderWidth: 1, borderStyle: "solid", borderColor: "var(--border)", borderRadius: "var(--radius-element)", backgroundColor: "var(--background)", paddingInline: "var(--spacing-2)", color: "var(--noema-text-primary)", font: "inherit", fontSize: 15, fontWeight: 650, lineHeight: 1.25, ":focus-visible": { outlineWidth: 2, outlineStyle: "solid", outlineColor: "var(--ring)", outlineOffset: 1 } },
  taskSubtitle: { minWidth: 0, color: "var(--noema-text-muted)", fontSize: 12, lineHeight: 1.35, overflowWrap: "anywhere" },
  taskRelationLink: { color: "var(--noema-pine-700)", fontWeight: 650, textDecoration: "none", cursor: "pointer", ":hover": { textDecoration: "underline" } },
  taskHeaderActions: { display: "inline-flex", alignItems: "center", justifyContent: "flex-end", gap: "var(--spacing-1)" },
  actionRow: {
    minWidth: 0,
    display: "grid",
    gridTemplateColumns: "minmax(0, 1fr) auto",
    alignItems: "center",
    gap: "var(--spacing-3)"
  },
  artifactIdentity: {
    display: "flex",
    minWidth: 0,
    alignItems: "center",
    gap: "var(--spacing-2)"
  },
  actions: {
    display: "flex",
    alignItems: "center",
    gap: "var(--spacing-2)"
  },
  title: {
    margin: "var(--spacing-0)",
    color: "var(--noema-text-primary)",
    fontSize: 15,
    fontWeight: 650,
    lineHeight: 1.25,
    overflowWrap: "anywhere"
  },
  artifactTitle: {
    minWidth: 0,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap"
  },
  srOnly: { position: "absolute", width: 1, height: 1, overflow: "hidden", clip: "rect(0 0 0 0)", whiteSpace: "nowrap" },
  body: {
    minHeight: 0,
    overflow: "auto",
    padding: "var(--spacing-4)"
  },
  taskBody: {
    overflow: "hidden",
    padding: "var(--spacing-0)"
  }
});

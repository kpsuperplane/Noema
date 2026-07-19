import React from "react";
import * as stylex from "@stylexjs/stylex";
import {
  ArtifactDetailPanel,
  ArtifactDownloadAction,
  type ArtifactDetail
} from "./ArtifactDetailPanel";
import type { ChatDetailTarget } from "./chatDetailTypes";
import { TaskDetailQueryPanel, type TaskDetailHeader } from "./task/TaskDetailQueryPanel";
import { TaskStatusBadge } from "./task/TaskStatusBadge";
import { ArtifactVersionSelector } from "./ArtifactVersionSelector";
import { ChatDetailCloseButton } from "./ChatDetailCloseButton";

export function ChatDetailRail({
  target,
  motionState,
  onChangeVersion,
  onClose,
  onMotionEnd,
  showWorkLink = true
}: {
  target: ChatDetailTarget;
  motionState: "opening" | "entering" | "open" | "exiting";
  onChangeVersion: (version: string) => void;
  onClose: () => void;
  onMotionEnd: () => void;
  showWorkLink?: boolean;
}) {
  const isModal = useNarrowViewport();
  const railRef = React.useRef<HTMLElement>(null);
  const closeButtonRef = React.useRef<HTMLButtonElement>(null);
  const taskControlsHostRef = React.useRef<HTMLDivElement>(null);
  const returnFocusRef = React.useRef<HTMLElement | null>(null);
  const [artifactDetailState, setArtifactDetailState] = React.useState<{
    version: string;
    detail: ArtifactDetail | null;
    latestDetail: ArtifactDetail | null;
  } | null>(null);
  const [taskHeaderState, setTaskHeaderState] = React.useState<{ taskId: string; header: TaskDetailHeader } | null>(null);
  const artifactVersion = target.type === "artifact" ? target.version : null;
  const taskId = target.type === "task" ? target.taskId : null;
  const artifactDetail = artifactVersion && artifactDetailState?.version === artifactVersion
    ? artifactDetailState.detail
    : null;
  const latestArtifactDetail = artifactDetailState?.latestDetail ?? null;
  const taskHeader = taskHeaderState?.taskId === taskId ? taskHeaderState.header : null;
  const title = artifactDetail?.title ??
    latestArtifactDetail?.title ??
    (target.type === "artifact"
      ? "Artifact"
      : taskHeader
        ? taskHeader.title
        : "Task details");
  const handleTaskHeaderChange = React.useCallback(
    (nextHeader: TaskDetailHeader | null) => {
      setTaskHeaderState(nextHeader && taskId ? { taskId, header: nextHeader } : null);
    },
    [taskId]
  );

  React.useEffect(() => {
    if (isModal && motionState === "opening" && returnFocusRef.current === null && document.activeElement instanceof HTMLElement) {
      returnFocusRef.current = document.activeElement;
    }
  }, [isModal, motionState]);

  React.useEffect(() => {
    if (isModal && motionState === "open") {
      closeButtonRef.current?.focus({ preventScroll: true });
    }
  }, [isModal, motionState, target]);

  const handleTransitionEnd = React.useCallback(
    (event: React.TransitionEvent<HTMLElement>) => {
      if (event.currentTarget === event.target && event.propertyName === "transform") {
        if (motionState === "exiting") {
          returnFocusRef.current?.focus();
          returnFocusRef.current = null;
        }
        onMotionEnd();
      }
    },
    [motionState, onMotionEnd]
  );

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

  const handleKeyDown = React.useCallback((event: React.KeyboardEvent<HTMLElement>) => {
    if (event.key === "Escape") {
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
  }, [isModal, onClose]);

  return (
    <aside
      data-slot="chat-detail-rail"
      data-state={motionState}
      aria-modal={isModal ? "true" : undefined}
      aria-label={target.type === "task" ? "Task details" : "Artifact details"}
      ref={railRef}
      role={isModal ? "dialog" : "complementary"}
      {...stylex.props(styles.rail)}
      tabIndex={isModal ? -1 : undefined}
      onKeyDown={handleKeyDown}
      onTransitionEnd={handleTransitionEnd}
    >
      <div
        data-slot="chat-detail-rail-surface"
        data-state={motionState}
        {...stylex.props(styles.surface)}
      >
        <header {...stylex.props(styles.header)}>
          {target.type === "task" ? (
            <div {...stylex.props(styles.taskTitleRow)}>
              <h2 {...stylex.props(styles.title)}>{title}</h2>
              {taskHeader ? (
                <TaskStatusBadge stageBehavior={taskHeader.stageBehavior} status={taskHeader.status} />
              ) : null}
              <div ref={taskControlsHostRef} {...stylex.props(styles.taskControls)} />
            </div>
          ) : (
            <>
              <div {...stylex.props(styles.actionRow)}>
                <div {...stylex.props(styles.versionSlot)}>
                  <ArtifactVersionSelector
                    detail={artifactDetail ?? latestArtifactDetail}
                    selectedVersion={target.version}
                    onChangeVersion={onChangeVersion}
                  />
                </div>
                <div {...stylex.props(styles.actions)}>
                  <ArtifactDownloadAction detail={artifactDetail} />
                  <ChatDetailCloseButton closeButtonRef={closeButtonRef} onClose={onClose} />
                </div>
              </div>
              <h2 {...stylex.props(styles.title)}>{title}</h2>
            </>
          )}
        </header>
        <div {...stylex.props(styles.body, target.type === "task" && styles.taskBody)}>
          {target.type === "artifact" ? (
            <ArtifactDetailPanel version={target.version} onDetailChange={updateArtifactDetail} />
          ) : (
            <TaskDetailQueryPanel
              closeButtonRef={closeButtonRef}
              controlsHostRef={taskControlsHostRef}
              onClose={onClose}
              onHeaderChange={handleTaskHeaderChange}
              showWorkLink={showWorkLink}
              taskId={target.taskId}
            />
          )}
        </div>
      </div>
    </aside>
  );
}

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
  surface: {
    display: "grid",
    gridTemplateRows: "auto minmax(0, 1fr)",
    minWidth: 0,
    height: "100%",
    backgroundColor: "var(--noema-surface-card)"
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
  actionRow: {
    minWidth: 0,
    display: "grid",
    gridTemplateColumns: "minmax(0, 1fr) auto",
    alignItems: "start",
    gap: "var(--spacing-3)"
  },
  taskTitleRow: {
    minWidth: 0,
    display: "grid",
    gridTemplateColumns: "minmax(0, 1fr) auto auto",
    alignItems: "center",
    gap: "var(--spacing-3)"
  },
  taskControls: {
    display: "flex",
    minHeight: 28,
    alignItems: "center",
    justifyContent: "flex-end"
  },
  versionSlot: {
    minWidth: 0
  },
  actions: {
    display: "flex",
    alignItems: "start",
    gap: "var(--spacing-2)"
  },
  title: {
    margin: 0,
    color: "var(--noema-text-primary)",
    fontSize: 15,
    fontWeight: 650,
    lineHeight: 1.25,
    overflowWrap: "anywhere"
  },
  body: {
    minHeight: 0,
    overflow: "auto",
    padding: "var(--spacing-4)"
  },
  taskBody: {
    overflow: "hidden",
    padding: 0
  }
});

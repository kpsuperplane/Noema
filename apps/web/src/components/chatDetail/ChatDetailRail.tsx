import React from "react";
import * as stylex from "@stylexjs/stylex";
import {
  ArtifactDetailPanel,
  ArtifactDownloadAction,
  type ArtifactDetail
} from "./ArtifactDetailPanel";
import type { ChatDetailTarget } from "./chatDetailTypes";
import { TaskDetailQueryPanel } from "./task/TaskDetailQueryPanel";
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
  const railRef = React.useRef<HTMLElement>(null);
  const closeButtonRef = React.useRef<HTMLButtonElement>(null);
  const returnFocusRef = React.useRef<HTMLElement | null>(null);
  const [artifactDetailState, setArtifactDetailState] = React.useState<{
    version: string;
    detail: ArtifactDetail | null;
    latestDetail: ArtifactDetail | null;
  } | null>(null);
  const [taskTitleState, setTaskTitleState] = React.useState<{ taskId: string; title: string } | null>(null);
  const artifactVersion = target.type === "artifact" ? target.version : null;
  const taskId = target.type === "task" ? target.taskId : null;
  const artifactDetail = artifactVersion && artifactDetailState?.version === artifactVersion
    ? artifactDetailState.detail
    : null;
  const latestArtifactDetail = artifactDetailState?.latestDetail ?? null;
  const title = artifactDetail?.title ??
    latestArtifactDetail?.title ??
    (target.type === "artifact"
      ? "Artifact"
      : taskTitleState?.taskId === taskId
        ? taskTitleState.title
        : "Task details");
  const handleTaskTitleChange = React.useCallback(
    (nextTitle: string | null) => {
      if (nextTitle && taskId) {
        setTaskTitleState({ taskId, title: nextTitle });
      }
    },
    [taskId]
  );

  React.useEffect(() => {
    if (motionState === "opening" && returnFocusRef.current === null && document.activeElement instanceof HTMLElement) {
      returnFocusRef.current = document.activeElement;
    }
  }, [motionState]);

  React.useEffect(() => {
    if (motionState === "open") {
      closeButtonRef.current?.focus({ preventScroll: true });
    }
  }, [motionState, target]);

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
    if (event.key !== "Tab") {
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
  }, [onClose]);

  return (
    <aside
      data-slot="chat-detail-rail"
      data-state={motionState}
      aria-modal="true"
      aria-label="Chat detail"
      ref={railRef}
      role="dialog"
      {...stylex.props(styles.rail)}
      tabIndex={-1}
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
            <h2 {...stylex.props(styles.title)}>{title}</h2>
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
              onClose={onClose}
              onTitleChange={handleTaskTitleChange}
              showWorkLink={showWorkLink}
              taskId={target.taskId}
            />
          )}
        </div>
      </div>
    </aside>
  );
}

function focusableElements(root: HTMLElement | null): HTMLElement[] {
  if (!root) {
    return [];
  }
  return Array.from(
    root.querySelectorAll<HTMLElement>(
      'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])'
    )
  ).filter((element) => element.getAttribute("aria-hidden") !== "true");
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
    gap: 10,
    paddingBlock: 14,
    paddingInline: 16,
    borderBottomWidth: 1,
    borderBottomStyle: "solid",
    borderBottomColor: "var(--noema-border-subtle)"
  },
  actionRow: {
    minWidth: 0,
    display: "grid",
    gridTemplateColumns: "minmax(0, 1fr) auto",
    alignItems: "start",
    gap: 12
  },
  versionSlot: {
    minWidth: 0
  },
  actions: {
    display: "flex",
    alignItems: "start",
    gap: 8
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
    padding: 16
  },
  taskBody: {
    overflow: "hidden",
    padding: 0
  }
});

import React from "react";
import * as stylex from "@stylexjs/stylex";
import { useIsPresent, useReducedMotion, type Variants } from "motion/react";
import * as m from "motion/react-m";
import { springs } from "@/motion/springs";
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
  onChangeVersion,
  onClose,
  showWorkLink = true
}: {
  target: ChatDetailTarget;
  onChangeVersion: (version: string) => void;
  onClose: () => void;
  showWorkLink?: boolean;
}) {
  const isModal = useNarrowViewport();
  const isPresent = useIsPresent();
  const reduceMotion = useReducedMotion();
  const railRef = React.useRef<HTMLElement>(null);
  const closeButtonRef = React.useRef<HTMLButtonElement>(null);
  const hasEnteredRef = React.useRef(false);
  const taskControlsHostRef = React.useRef<HTMLDivElement>(null);
  const returnFocusRef = React.useRef<HTMLElement | null>(null);
  const taskTargetId = target.type === "task" ? target.taskId : null;
  const [taskTitleState, setTaskTitleState] = React.useState<{ taskId: string; title: string } | null>(null);
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
  const taskTitle = target.type === "task" && taskTitleState?.taskId === target.taskId
    ? taskTitleState.title
    : "Task details";
  const handleTaskTitleChange = React.useCallback((nextTitle: string) => {
    if (taskTargetId) setTaskTitleState({ taskId: taskTargetId, title: nextTitle });
  }, [taskTargetId]);
  React.useEffect(() => {
    if (!isModal) {
      returnFocusRef.current = null;
      return;
    }
    if (isPresent && returnFocusRef.current === null && document.activeElement instanceof HTMLElement) {
      returnFocusRef.current = document.activeElement;
    } else if (!isPresent) {
      returnFocusRef.current?.focus({ preventScroll: true });
      returnFocusRef.current = null;
    }
  }, [isModal, isPresent]);

  React.useEffect(() => {
    if (isModal && isPresent && (reduceMotion || hasEnteredRef.current)) {
      closeButtonRef.current?.focus({ preventScroll: true });
    }
  }, [isModal, isPresent, reduceMotion, target]);

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
    <m.aside
      data-slot="chat-detail-rail"
      data-state={isPresent ? "open" : "exiting"}
      aria-modal={isModal ? "true" : undefined}
      aria-hidden={isPresent ? undefined : "true"}
      aria-label={target.type === "task" ? "Task details" : "Artifact details"}
      ref={railRef}
      role={isModal ? "dialog" : "complementary"}
      inert={!isPresent}
      variants={chatDetailRailVariants}
      initial={reduceMotion ? false : "hidden"}
      animate="visible"
      exit="hidden"
      transition={reduceMotion ? { duration: 0 } : springs.surface}
      {...stylex.props(styles.rail)}
      tabIndex={isModal ? -1 : undefined}
      onKeyDown={handleKeyDown}
      onAnimationComplete={(definition) => {
        if (definition === "visible") {
          hasEnteredRef.current = true;
          if (isModal && isPresent) {
            closeButtonRef.current?.focus({ preventScroll: true });
          }
        }
      }}
    >
      <div
        data-slot="chat-detail-rail-surface"
        {...stylex.props(styles.surface, target.type === "task" && styles.taskSurface)}
      >
        <header {...stylex.props(styles.header, target.type === "task" && styles.taskHeader)}>
          {target.type === "task" ? (
            <div {...stylex.props(styles.taskTitleBar)}>
              <h2 {...stylex.props(styles.title)}>{taskTitle}</h2>
              <div {...stylex.props(styles.taskHeaderActions)}>
                <ChatDetailCloseButton closeButtonRef={closeButtonRef} onClose={onClose} />
              </div>
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
              onTaskTitleChange={handleTaskTitleChange}
              showWorkLink={showWorkLink}
              taskId={target.taskId}
            />
          )}
        </div>
      </div>
    </m.aside>
  );
}

const chatDetailRailVariants: Variants = {
  hidden: { opacity: 0.72, x: "100%" },
  visible: { opacity: 1, x: "0%" }
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
  surface: {
    display: "grid",
    gridTemplateRows: "auto minmax(0, 1fr)",
    minWidth: 0,
    height: "100%",
    backgroundColor: "var(--noema-surface-card)"
  },
  taskSurface: {
    position: "relative",
    gridTemplateRows: "auto minmax(0, 1fr)"
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
    pointerEvents: "auto"
  },
  taskTitleBar: { display: "grid", minWidth: 0, gridTemplateColumns: "minmax(0, 1fr) auto", alignItems: "center", gap: "var(--spacing-3)" },
  taskHeaderActions: { display: "inline-flex", alignItems: "center", justifyContent: "flex-end", gap: "var(--spacing-1)" },
  actionRow: {
    minWidth: 0,
    display: "grid",
    gridTemplateColumns: "minmax(0, 1fr) auto",
    alignItems: "start",
    gap: "var(--spacing-3)"
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

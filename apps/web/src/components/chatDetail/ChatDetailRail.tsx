import React from "react";
import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import { ArrowLeft } from "lucide-react";
import {
  AnimatePresence,
  useIsPresent,
  useReducedMotion,
  type Variants
} from "motion/react";
import * as m from "motion/react-m";
import { springs } from "@/motion/springs";
import {
  ArtifactDetailPanel,
  ArtifactDownloadAction,
  type ArtifactDetail
} from "./ArtifactDetailPanel";
import type { ChatDetailTarget } from "./chatDetailTypes";
import { TaskDetailQueryPanel } from "./task/TaskDetailQueryPanel";
import { ChatDetailCloseButton } from "./ChatDetailCloseButton";

export function ChatDetailRail({
  target,
  onClose,
  animateEntrance = true,
  showWorkLink = true
}: {
  target: ChatDetailTarget;
  onClose: () => void;
  animateEntrance?: boolean;
  showWorkLink?: boolean;
}) {
  return (
    <RoutedChatDetailRail
      key={detailTargetKey(target)}
      animateEntrance={animateEntrance}
      initialTarget={target}
      onClose={onClose}
      showWorkLink={showWorkLink}
    />
  );
}

function RoutedChatDetailRail({
  animateEntrance,
  initialTarget,
  onClose,
  showWorkLink
}: {
  animateEntrance: boolean;
  initialTarget: ChatDetailTarget;
  onClose: () => void;
  showWorkLink: boolean;
}) {
  const [history, setHistory] = React.useState<readonly ChatDetailTarget[]>([initialTarget]);
  const target = history.at(-1) ?? initialTarget;
  const canGoBack = history.length > 1;
  const isModal = useNarrowViewport();
  const isPresent = useIsPresent();
  const reduceMotion = useReducedMotion();
  const railRef = React.useRef<HTMLElement>(null);
  const taskCloseButtonRef = React.useRef<HTMLButtonElement>(null);
  const artifactCloseButtonRef = React.useRef<HTMLButtonElement>(null);
  const hasEnteredRef = React.useRef(!animateEntrance);
  const returnFocusRef = React.useRef<HTMLElement | null>(null);
  const taskTargetId = initialTarget.type === "task" ? initialTarget.taskId : null;
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
  const taskTitle = taskTargetId && taskTitleState?.taskId === taskTargetId
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
      const closeButton = target.type === "task"
        ? taskCloseButtonRef.current
        : artifactCloseButtonRef.current;
      closeButton?.focus({ preventScroll: true });
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
      initial={reduceMotion || !animateEntrance ? false : "hidden"}
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
            const closeButton = target.type === "task"
              ? taskCloseButtonRef.current
              : artifactCloseButtonRef.current;
            closeButton?.focus({ preventScroll: true });
          }
        }
      }}
    >
      <div data-slot="chat-detail-rail-surface" {...stylex.props(styles.surface)}>
        {taskTargetId ? (
          <m.div
            data-slot="chat-detail-task-route"
            aria-hidden={target.type !== "task" ? "true" : undefined}
            inert={target.type !== "task"}
            variants={taskRouteVariants}
            initial={false}
            animate={target.type === "task" ? "visible" : "nested"}
            transition={reduceMotion ? { duration: 0 } : springs.surface}
            {...stylex.props(styles.routeFrame)}
          >
            <header {...stylex.props(styles.header, styles.taskHeader)}>
              <div {...stylex.props(styles.taskTitleBar)}>
                <h2 {...stylex.props(styles.title)}>{taskTitle}</h2>
                <div {...stylex.props(styles.taskHeaderActions)}>
                  <ChatDetailCloseButton
                    closeButtonRef={taskCloseButtonRef}
                    onClose={onClose}
                  />
                </div>
              </div>
            </header>
            <div {...stylex.props(styles.body, styles.taskBody)}>
              <TaskDetailQueryPanel
                onOpenDetail={openDetail}
                onTaskTitleChange={handleTaskTitleChange}
                showWorkLink={showWorkLink}
                taskId={taskTargetId}
              />
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
  return target.type === "task" ? `task:${target.taskId}` : `artifact:${target.version}`;
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
  surface: {
    position: "relative",
    minWidth: 0,
    height: "100%",
    overflow: "hidden",
    backgroundColor: "var(--noema-surface-card)"
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
    margin: 0,
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

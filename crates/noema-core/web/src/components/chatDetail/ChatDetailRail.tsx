import React from "react";
import { Button } from "@astryxdesign/core/Button";
import { Selector } from "@astryxdesign/core/Selector";
import * as stylex from "@stylexjs/stylex";
import { X } from "lucide-react";
import {
  ArtifactDetailPanel,
  ArtifactDownloadAction,
  type ArtifactDetail
} from "./ArtifactDetailPanel";
import type { ChatDetailTarget } from "./chatDetailTypes";
import { TaskDetailQueryPanel } from "./task/TaskDetailQueryPanel";

export function ChatDetailRail({
  target,
  motionState,
  onChangeVersion,
  onClose,
  onMotionEnd,
  onCancelTask,
  onResumeTask
}: {
  target: ChatDetailTarget;
  motionState: "opening" | "entering" | "open" | "exiting";
  onChangeVersion: (version: string) => void;
  onClose: () => void;
  onMotionEnd: () => void;
  onCancelTask?: (taskId: string) => void | Promise<void>;
  onResumeTask?: (taskId: string, message?: string) => void | Promise<void>;
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
            <div {...stylex.props(styles.taskTitleRow)}>
              <h2 {...stylex.props(styles.title)}>{title}</h2>
              <CloseButton closeButtonRef={closeButtonRef} onClose={onClose} />
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
                  <CloseButton closeButtonRef={closeButtonRef} onClose={onClose} />
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
              onTitleChange={handleTaskTitleChange}
              onCancelTask={onCancelTask}
              onResumeTask={onResumeTask}
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

function CloseButton({
  closeButtonRef,
  onClose
}: {
  closeButtonRef: React.RefObject<HTMLButtonElement | null>;
  onClose: () => void;
}) {
  return (
    <Button
      ref={closeButtonRef}
      type="button"
      variant="ghost"
      size="sm"
      label="Close detail"
      icon={<X aria-hidden="true" size={16} />}
      isIconOnly
      onClick={onClose}
    />
  );
}

function ArtifactVersionSelector({
  detail,
  selectedVersion,
  onChangeVersion
}: {
  detail: ArtifactDetail | null;
  selectedVersion: string;
  onChangeVersion: (version: string) => void;
}) {
  if (!detail || detail.versions.length === 0) {
    return null;
  }

  const options = detail.versions.map((version) => ({
    value: version.artifactVersionId,
    label: `Version ${version.versionIndex}`
  }));

  return (
    <div {...stylex.props(styles.versionSelector)}>
      <Selector
        isDisabled={detail.versions.length <= 1}
        isLabelHidden
        label="Artifact version"
        onChange={onChangeVersion}
        options={options}
        placement="below"
        size="sm"
        value={selectedVersion}
        width={128}
      />
    </div>
  );
}

const styles = stylex.create({
  rail: {
    position: {
      default: "absolute",
      "@media (min-width: 980px)": "absolute"
    },
    inset: {
      default: 0,
      "@media (min-width: 980px)": "auto"
    },
    top: {
      default: "auto",
      "@media (min-width: 980px)": 0
    },
    right: {
      default: "auto",
      "@media (min-width: 980px)": 0
    },
    bottom: {
      default: "auto",
      "@media (min-width: 980px)": 0
    },
    zIndex: 4,
    minWidth: 0,
    width: {
      default: "auto",
      "@media (min-width: 980px)": "var(--chat-detail-rail-width)"
    },
    height: "100%",
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
  taskTitleRow: {
    minWidth: 0,
    display: "grid",
    gridTemplateColumns: "minmax(0, 1fr) auto",
    alignItems: "center",
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
  versionSelector: {
    alignSelf: "start",
    width: 128
  },
  body: {
    minHeight: 0,
    overflow: "auto",
    padding: 16
  },
  taskBody: {
    padding: 0
  }
});

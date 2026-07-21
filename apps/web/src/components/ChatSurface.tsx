import React from "react";
import { ResizeHandle, useResizable } from "@astryxdesign/core/Resizable";
import * as stylex from "@stylexjs/stylex";
import { ChatDetailRail } from "./chatDetail/ChatDetailRail";
import type { ChatDetailTarget } from "./chatDetail/chatDetailTypes";
import { Composer } from "./Composer";
import { Transcript } from "./Transcript";
import { TranscriptLoadingSkeleton } from "./transcript/TranscriptLoadingSkeleton";
import { ChatWorkPanel } from "./work/ChatWorkPanel";
import { PendingGovernedActions } from "./actions/PendingGovernedActions";
import {
  type ShellSurfaceVisibility,
  useShellSurface
} from "./shell/ShellSurfaceContext";
import type { ConversationAgentStatus, TranscriptEntry } from "@/shared/types";

export function shouldFocusChatComposer({
  ready,
  visibility
}: {
  ready: boolean;
  visibility: ShellSurfaceVisibility;
}) {
  return ready && visibility === "visible";
}

export function composerPlaceholder({
  ready,
  agentName
}: {
  ready: boolean;
  agentName: string | null | undefined;
}) {
  if (!ready) {
    return "Starting Noema chat...";
  }

  const trimmedName = agentName?.trim();
  return trimmedName ? `Message ${trimmedName}` : "Send a message";
}

export type ChatSurfaceProps = {
  conversationId: string | null;
  transcript: TranscriptEntry[];
  loadingOlderTranscript: boolean;
  hasMoreTranscriptBefore: boolean;
  olderTranscriptPageError: string | null;
  pending: boolean;
  agentStatus: ConversationAgentStatus;
  awaitingAssistantTurn: boolean;
  expandedActivities: Set<string>;
  sentMessageScrollRequest: number;
  draft: string;
  ready: boolean;
  loadingInitialTranscript?: boolean;
  agentName: string | null;
  onToggleActivity: (id: string) => void;
  onDraftChange: (value: string) => void;
  onLoadOlderTranscript: () => void;
  onSubmit: (value: string) => void;
  onSubmitMultipleChoiceSelection: (promptItemId: string, selectedOptionIds: string[]) => void;
};

type DetailMotionState = "opening" | "entering" | "open" | "exiting";

export function ChatSurface({
  conversationId,
  transcript,
  loadingOlderTranscript,
  hasMoreTranscriptBefore,
  olderTranscriptPageError,
  pending,
  agentStatus,
  awaitingAssistantTurn,
  expandedActivities,
  sentMessageScrollRequest,
  draft,
  ready,
  loadingInitialTranscript = false,
  agentName,
  onToggleActivity,
  onDraftChange,
  onLoadOlderTranscript,
  onSubmit,
  onSubmitMultipleChoiceSelection
}: ChatSurfaceProps) {
  const { visibility } = useShellSurface();
  const composerRef = React.useRef<HTMLTextAreaElement>(null);
  const composerDockRef = React.useRef<HTMLDivElement>(null);
  const [composerDockHeight, setComposerDockHeight] = React.useState(96);
  const [detailTarget, setDetailTarget] = React.useState<ChatDetailTarget | null>(null);
  const [workPanelOpen, setWorkPanelOpen] = React.useState(false);
  const [detailMotionState, setDetailMotionState] = React.useState<DetailMotionState>("open");
  const detailReservesSpace = detailTarget && (detailMotionState === "entering" || detailMotionState === "open");
  const detailRail = useResizable({
    defaultSize: 380,
    minSizePx: 320,
    maxSizePx: 640,
    autoSaveId: "noema-chat-detail-rail"
  });

  React.useLayoutEffect(() => {
    const dock = composerDockRef.current;
    if (!dock || typeof ResizeObserver === "undefined") {
      return;
    }

    const syncComposerDockHeight = () => {
      setComposerDockHeight(Math.ceil(dock.getBoundingClientRect().height));
    };

    syncComposerDockHeight();
    const observer = new ResizeObserver(syncComposerDockHeight);
    observer.observe(dock);

    return () => {
      observer.disconnect();
    };
  }, []);

  React.useEffect(() => {
    if (shouldFocusChatComposer({ ready, visibility })) {
      composerRef.current?.focus();
    }
  }, [ready, visibility]);

  React.useEffect(() => {
    if (!detailTarget || detailMotionState !== "opening") {
      return;
    }

    let secondFrame = 0;
    const firstFrame = window.requestAnimationFrame(() => {
      secondFrame = window.requestAnimationFrame(() => {
        setDetailMotionState("entering");
      });
    });

    return () => {
      window.cancelAnimationFrame(firstFrame);
      if (secondFrame !== 0) {
        window.cancelAnimationFrame(secondFrame);
      }
    };
  }, [detailMotionState, detailTarget]);

  const rootStyle = React.useMemo(
    () =>
      ({
        "--chat-composer-dock-height": `${composerDockHeight}px`,
        "--chat-transcript-bottom-fade": `calc(${composerDockHeight}px + 8px)`,
        "--chat-detail-rail-width": `${detailRail.size}px`
      }) as React.CSSProperties,
    [composerDockHeight, detailRail.size]
  );

  const openDetail = React.useCallback(
    (target: ChatDetailTarget) => {
      setDetailTarget(target);
      setDetailMotionState(detailTarget && detailMotionState !== "exiting" ? "open" : "opening");
    },
    [detailMotionState, detailTarget]
  );

  const closeDetail = React.useCallback(() => {
    setDetailMotionState((state) => (state === "exiting" ? state : "exiting"));
  }, []);

  const selectDetailVersion = React.useCallback((version: string) => {
    setDetailTarget({ type: "artifact", version });
    setDetailMotionState("open");
  }, []);

  const handleDetailMotionEnd = React.useCallback(() => {
    if (detailMotionState === "entering") {
      setDetailMotionState("open");
      return;
    }
    if (detailMotionState === "exiting") {
      setDetailTarget(null);
      setDetailMotionState("open");
    }
  }, [detailMotionState]);

  return (
    <section
      data-slot="chat-surface"
      data-detail-open={detailTarget ? "true" : undefined}
      data-detail-motion-state={detailTarget ? detailMotionState : undefined}
      {...stylex.props(styles.root, detailTarget && styles.rootWithDetail)}
      style={rootStyle}
      aria-label="Noema chat"
    >
      <div data-slot="chat-main-pane" {...stylex.props(styles.mainPane, detailReservesSpace && styles.mainPaneWithDetail)}>
        <ChatWorkPanel open={workPanelOpen} onToggle={() => setWorkPanelOpen((value) => !value)} onOpenDetail={openDetail} />
        <div {...stylex.props(styles.contentLayer)}>
          {loadingInitialTranscript || transcript.length === 0 ? (
            <TranscriptLoadingSkeleton />
          ) : (
            <Transcript
              entries={transcript}
              loadingOlderTranscript={loadingOlderTranscript}
              hasMoreTranscriptBefore={hasMoreTranscriptBefore}
              olderTranscriptPageError={olderTranscriptPageError}
              pending={pending}
              agentStatus={agentStatus}
              awaitingAssistantTurn={awaitingAssistantTurn}
              expandedActivities={expandedActivities}
              sentMessageScrollRequest={sentMessageScrollRequest}
              onToggleActivity={onToggleActivity}
              onSubmitMultipleChoiceSelection={onSubmitMultipleChoiceSelection}
              onLoadOlderTranscript={onLoadOlderTranscript}
              onOpenDetail={openDetail}
              collapseConsecutiveToolCalls
            />
          )}
        </div>

        <div ref={composerDockRef} data-slot="chat-composer-dock" {...stylex.props(styles.composerDock)}>
          <div aria-hidden="true" data-slot="chat-composer-scrim" {...stylex.props(styles.composerScrim)} />
          <div {...stylex.props(styles.composerLayer)}>
            <PendingGovernedActions conversationId={conversationId} compact />
            <Composer
              ref={composerRef}
              value={draft}
              ready={ready}
              pending={pending}
              placeholder={composerPlaceholder({ ready, agentName })}
              onChange={onDraftChange}
              onSubmit={onSubmit}
            />
          </div>
        </div>
      </div>

      {detailTarget ? (
        <>
          <div data-slot="chat-detail-resize-handle" {...stylex.props(styles.resizeHandleSlot)}>
            <ResizeHandle
              direction="horizontal"
              hasDivider
              isReversed
              label="Resize detail sidebar"
              pillPlacement="center"
              resizable={detailRail.props}
            />
          </div>
          <ChatDetailRail
            target={detailTarget}
            motionState={detailMotionState}
            onChangeVersion={selectDetailVersion}
            onClose={closeDetail}
            onMotionEnd={handleDetailMotionEnd}
          />
        </>
      ) : null}
    </section>
  );
}

const styles = stylex.create({
  root: {
    "--chat-column-width": {
      default: "min(860px, calc(100% - 48px))",
      "@media (max-width: 760px)": "calc(100% - 40px)"
    },
    "--chat-opposite-avatar-gutter": {
      default: "96px",
      "@media (max-width: 760px)": "72px"
    },
    display: "grid",
    position: "relative",
    gridTemplateColumns: "minmax(0, 1fr)",
    gridTemplateRows: "minmax(0, 1fr)",
    minHeight: 0,
    height: "100%",
    width: "100%",
    overflow: "hidden"
  },
  rootWithDetail: {
    gridTemplateColumns: {
      default: "minmax(0, 1fr)",
      "@media (min-width: 980px)": "minmax(0, 1fr)"
    }
  },
  mainPane: {
    display: "grid",
    gridTemplateRows: "minmax(0, 1fr)",
    minWidth: 0,
    minHeight: 0,
    overflow: "hidden",
    paddingInlineEnd: {
      default: 0,
      "@media (min-width: 980px)": 0
    }
  },
  mainPaneWithDetail: {
    paddingInlineEnd: {
      default: 0,
      "@media (min-width: 980px)": "calc(var(--chat-detail-rail-width) + 1px)"
    }
  },
  contentLayer: {
    containerName: "chat-transcript",
    containerType: "inline-size",
    gridArea: "1 / 1",
    minHeight: 0,
    overflow: "hidden"
  },
  composerDock: {
    position: "relative",
    zIndex: 2,
    display: "grid",
    gridArea: "1 / 1",
    alignSelf: "end",
    paddingBottom: {
      default: 22,
      "@media (hover: none) and (pointer: coarse)": "max(18px, env(safe-area-inset-bottom))"
    },
    pointerEvents: "none"
  },
  composerScrim: {
    position: "absolute",
    top: -96,
    right: 0,
    bottom: 0,
    left: 0,
    zIndex: 0,
    pointerEvents: "none",
    background:
      "linear-gradient(to bottom, rgb(255 255 255 / 0), rgb(255 255 255 / 0.74) 42px, var(--background) 96px)"
  },
  composerLayer: {
    position: "relative",
    zIndex: 1,
    pointerEvents: "auto"
  },
  resizeHandleSlot: {
    display: {
      default: "none",
      "@media (min-width: 980px)": "block"
    },
    position: {
      default: "relative",
      "@media (min-width: 980px)": "absolute"
    },
    top: {
      default: "auto",
      "@media (min-width: 980px)": 0
    },
    right: {
      default: "auto",
      "@media (min-width: 980px)": "var(--chat-detail-rail-width)"
    },
    bottom: {
      default: "auto",
      "@media (min-width: 980px)": 0
    },
    minHeight: 0,
    height: "100%",
    zIndex: 5
  }
});

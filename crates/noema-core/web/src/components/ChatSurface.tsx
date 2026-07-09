import React from "react";
import { ResizeHandle, useResizable } from "@astryxdesign/core/Resizable";
import * as stylex from "@stylexjs/stylex";
import { ChatDetailRail } from "./chatDetail/ChatDetailRail";
import type { ChatDetailTarget } from "./chatDetail/chatDetailTypes";
import { Composer } from "./Composer";
import { Transcript } from "./Transcript";
import { TranscriptLoadingSkeleton } from "./transcript/TranscriptLoadingSkeleton";
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

type DetailMotionState = "entering" | "open" | "exiting";

export function ChatSurface({
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
  const [detailMotionState, setDetailMotionState] = React.useState<DetailMotionState>("open");
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
      setDetailMotionState(detailTarget && detailMotionState !== "exiting" ? "open" : "entering");
    },
    [detailMotionState, detailTarget]
  );

  const closeDetail = React.useCallback(() => {
    setDetailMotionState((state) => (state === "exiting" ? state : "exiting"));
  }, []);

  const handleDetailAnimationEnd = React.useCallback(() => {
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
      <div data-slot="chat-main-pane" {...stylex.props(styles.mainPane)}>
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
            />
          )}
        </div>

        <div ref={composerDockRef} data-slot="chat-composer-dock" {...stylex.props(styles.composerDock)}>
          <div aria-hidden="true" data-slot="chat-composer-scrim" {...stylex.props(styles.composerScrim)} />
          <div {...stylex.props(styles.composerLayer)}>
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
            onClose={closeDetail}
            onMotionEnd={handleDetailAnimationEnd}
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
    display: "grid",
    position: "relative",
    gridTemplateColumns: "minmax(0, 1fr)",
    gridTemplateRows: "minmax(0, 1fr)",
    minHeight: 0,
    height: "calc(100% + var(--shell-deck-header-height, 44px))",
    marginTop: "calc(var(--shell-deck-header-height, 44px) * -1)",
    width: "100%",
    overflow: "hidden"
  },
  rootWithDetail: {
    gridTemplateColumns: {
      default: "minmax(0, 1fr)",
      "@media (min-width: 980px)": "minmax(0, 1fr) 1px var(--chat-detail-rail-width)"
    }
  },
  mainPane: {
    display: "grid",
    gridTemplateRows: "minmax(0, 1fr)",
    minWidth: 0,
    minHeight: 0,
    overflow: "hidden"
  },
  contentLayer: {
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
    minHeight: 0,
    height: "100%",
    zIndex: 5
  }
});

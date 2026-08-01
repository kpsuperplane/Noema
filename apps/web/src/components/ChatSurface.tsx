import React from "react";
import {
  ResizeHandle,
  useResizable,
  type ResizeHandleProps
} from "@astryxdesign/core/Resizable";
import { useMediaQuery } from "@astryxdesign/core/hooks";
import * as stylex from "@stylexjs/stylex";
import { AnimatePresence, useIsPresent, useReducedMotion } from "motion/react";
import * as m from "motion/react-m";
import { ChatDetailRail } from "./chatDetail/ChatDetailRail";
import type { ChatDetailTarget } from "./chatDetail/chatDetailTypes";
import { Composer } from "./Composer";
import { Transcript } from "./Transcript";
import { TranscriptLoadingSkeleton } from "./transcript/TranscriptLoadingSkeleton";
import { TranscriptSystemNotice } from "./transcript/TranscriptSystemNotice";
import { PendingHumanInterventions } from "./actions/PendingGovernedActions";
import {
  type ShellSurfaceVisibility,
  useShellSurface
} from "./shell/ShellSurfaceContext";
import type { A2UIActionSubmission, ConversationAgentStatus, TranscriptEntry } from "@/shared/types";
import { springs } from "@/motion/springs";
import { MobileDrawer, useLatchedDrawerPresentation } from "./MobileDrawer";
import { DetailPanePresentationProvider } from "./shell/MasterDetailLayout";

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
  offline?: boolean;
  loadingInitialTranscript?: boolean;
  agentName: string | null;
  onToggleActivity: (id: string) => void;
  onDraftChange: (value: string) => void;
  onLoadOlderTranscript: () => void;
  onSubmit: (value: string) => void;
  onSubmitA2UIAction: (action: A2UIActionSubmission) => void;
  onSubmitMultipleChoiceSelection: (promptItemId: string, selectedOptionIds: string[]) => void;
};

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
  offline = false,
  loadingInitialTranscript = false,
  agentName,
  onToggleActivity,
  onDraftChange,
  onLoadOlderTranscript,
  onSubmit,
  onSubmitA2UIAction,
  onSubmitMultipleChoiceSelection
}: ChatSurfaceProps) {
  const { visibility } = useShellSurface();
  const composerRef = React.useRef<HTMLTextAreaElement>(null);
  const composerDockRef = React.useRef<HTMLDivElement>(null);
  const [composerDockHeight, setComposerDockHeight] = React.useState(96);
  const [detailTarget, setDetailTarget] = React.useState<ChatDetailTarget | null>(null);
  const [detailPresenceAnimating, setDetailPresenceAnimating] = React.useState(false);
  const reduceMotion = useReducedMotion();
  const wideDetailViewport = useMediaQuery("(min-width: 980px)");
  const detailOpen = Boolean(detailTarget);
  const detailInDrawer = useLatchedDrawerPresentation(detailOpen, !wideDetailViewport);
  const detailRail = useResizable({
    defaultSize: 380,
    minSizePx: 320,
    maxSizePx: 640,
    autoSaveId: "noema-chat-detail-rail"
  });
  const detailRailResizeProps = React.useMemo<ResizeHandleProps["resizable"]>(() => ({
    ...detailRail.props,
    _onResizeStart: () => {
      setDetailPresenceAnimating(false);
      detailRail.props._onResizeStart();
    }
  }), [detailRail.props]);

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
        "--chat-composer-scrim-height": "var(--spacing-12)",
        "--chat-detail-rail-width": `${detailRail.size}px`
      }) as React.CSSProperties,
    [composerDockHeight, detailRail.size]
  );

  const openDetail = React.useCallback((target: ChatDetailTarget) => {
    setDetailPresenceAnimating(!detailTarget);
    setDetailTarget(target);
  }, [detailTarget]);

  const closeDetail = React.useCallback(() => {
    setDetailPresenceAnimating(true);
    setDetailTarget(null);
  }, []);

  return (
    <section
      data-slot="chat-surface"
      data-detail-open={detailTarget ? "true" : undefined}
      {...stylex.props(styles.root)}
      style={rootStyle}
      aria-label="Noema chat"
    >
      <m.div
        data-slot="chat-main-pane"
        animate={{
          paddingRight: detailOpen && !detailInDrawer ? detailRail.size + 1 : 0
        }}
        transition={
          reduceMotion || !detailPresenceAnimating
            ? { duration: 0 }
            : springs.surface
        }
        onAnimationComplete={() => setDetailPresenceAnimating(false)}
        {...stylex.props(styles.mainPane)}
      >
        <div {...stylex.props(styles.contentLayer)}>
          {offline && transcript.length === 0 ? (
            <TranscriptSystemNotice role="status" tone="warning">
              Reconnect to load this conversation.
            </TranscriptSystemNotice>
          ) : loadingInitialTranscript || transcript.length === 0 ? (
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
              onSubmitA2UIAction={ready ? onSubmitA2UIAction : undefined}
              onSubmitMultipleChoiceSelection={onSubmitMultipleChoiceSelection}
              onLoadOlderTranscript={onLoadOlderTranscript}
              onOpenDetail={openDetail}
              collapseConsecutiveToolCalls
            />
          )}
        </div>

        <div ref={composerDockRef} data-slot="chat-composer-dock" {...stylex.props(styles.composerDock)}>
          <div aria-hidden="true" data-slot="chat-composer-scrim" {...stylex.props(styles.composerScrim)} />
          <div data-slot="chat-composer-layer" {...stylex.props(styles.composerLayer)}>
            <PendingHumanInterventions conversationId={conversationId} />
            <Composer
              ref={composerRef}
              value={draft}
              ready={ready}
              editable={offline || ready}
              pending={pending}
              placeholder={offline ? "Write a draft while offline" : composerPlaceholder({ ready, agentName })}
              onChange={onDraftChange}
              onSubmit={onSubmit}
            />
          </div>
        </div>
      </m.div>

      <AnimatePresence
        initial={false}
        onExitComplete={() => setDetailPresenceAnimating(false)}
      >
        {detailTarget && !detailInDrawer ? (
          <ChatDetailResizeHandle
            key="chat-detail-resize-handle"
            railSize={detailRail.size}
            resizable={detailRailResizeProps}
          />
        ) : null}
      </AnimatePresence>
      {detailInDrawer ? (
        <MobileDrawer
          isOpen={detailOpen}
          onOpenChange={(open) => {
            if (!open) closeDetail();
          }}
          label={detailTarget?.type === "artifact" ? "Artifact details" : "Task details"}
          height="calc(100dvh - var(--spacing-6))"
        >
          <DetailPanePresentationProvider presentation="drawer">
            {detailTarget ? (
              <ChatDetailRail
                animateEntrance={false}
                target={detailTarget}
                onClose={closeDetail}
              />
            ) : null}
          </DetailPanePresentationProvider>
        </MobileDrawer>
      ) : (
        <AnimatePresence initial={false}>
          {detailTarget ? (
            <ChatDetailRail
              key="chat-detail-rail"
              animateEntrance={detailPresenceAnimating}
              target={detailTarget}
              onClose={closeDetail}
            />
          ) : null}
        </AnimatePresence>
      )}
    </section>
  );
}

function ChatDetailResizeHandle({
  railSize,
  resizable
}: {
  railSize: number;
  resizable: ResizeHandleProps["resizable"];
}) {
  const isPresent = useIsPresent();
  const reduceMotion = useReducedMotion();

  return (
    <m.div
      data-slot="chat-detail-resize-handle"
      aria-hidden={isPresent ? undefined : "true"}
      inert={!isPresent}
      initial={reduceMotion ? false : { opacity: 0, x: railSize }}
      animate={{ opacity: 1, x: 0 }}
      exit={{ opacity: 0, x: railSize, pointerEvents: "none" }}
      transition={reduceMotion ? { duration: 0 } : springs.surface}
      {...stylex.props(styles.resizeHandleSlot)}
    >
      <ResizeHandle
        direction="horizontal"
        hasDivider
        isReversed
        label="Resize detail sidebar"
        pillPlacement="center"
        resizable={resizable}
      />
    </m.div>
  );
}

const styles = stylex.create({
  root: {
    "--chat-column-width": {
      default: "min(var(--shell-content-max-width), calc(100% - 48px))",
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
  mainPane: {
    display: "grid",
    gridArea: "1 / 1",
    gridTemplateRows: "minmax(0, 1fr)",
    minWidth: 0,
    minHeight: 0,
    overflow: "hidden"
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
    top: "calc(-1 * var(--chat-composer-scrim-height, 48px))",
    right: "var(--spacing-4)",
    bottom: 0,
    left: 0,
    zIndex: 0,
    pointerEvents: "none",
    backgroundImage:
      "linear-gradient(to bottom, rgb(255 255 255 / 0), rgb(255 255 255 / 0.38) 50%, var(--background) 100%)"
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

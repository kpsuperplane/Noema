import React from "react";
import * as stylex from "@stylexjs/stylex";
import { Composer } from "./Composer";
import { EmptyState } from "./EmptyState";
import { Transcript } from "./Transcript";
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
  onPickStarter: (starter: string) => void;
  onToggleActivity: (id: string) => void;
  onDraftChange: (value: string) => void;
  onLoadOlderTranscript: () => void;
  onSubmit: (value: string) => void;
};

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
  onPickStarter,
  onToggleActivity,
  onDraftChange,
  onLoadOlderTranscript,
  onSubmit
}: ChatSurfaceProps) {
  const { visibility } = useShellSurface();
  const composerRef = React.useRef<HTMLTextAreaElement>(null);
  const composerDockRef = React.useRef<HTMLDivElement>(null);
  const [composerDockHeight, setComposerDockHeight] = React.useState(96);

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
        "--chat-transcript-bottom-fade": `calc(${composerDockHeight}px + 8px)`
      }) as React.CSSProperties,
    [composerDockHeight]
  );

  return (
    <section
      data-slot="chat-surface"
      {...stylex.props(styles.root)}
      style={rootStyle}
      aria-label="Noema chat"
    >
      <div {...stylex.props(styles.contentLayer, transcript.length === 0 && styles.emptyContentLayer)}>
        {loadingInitialTranscript ? (
          <ChatLoadingSkeleton />
        ) : transcript.length === 0 ? (
          <EmptyState onPick={onPickStarter} />
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
            onLoadOlderTranscript={onLoadOlderTranscript}
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
    </section>
  );
}

function ChatLoadingSkeleton() {
  return (
    <div {...stylex.props(styles.loadingRoot)} aria-label="Loading chat">
      <div data-slot="skeleton-glimmer" {...stylex.props(styles.loadingEyebrow)} />
      <div data-slot="skeleton-glimmer" {...stylex.props(styles.loadingTitle)} />
      <div data-slot="skeleton-glimmer" {...stylex.props(styles.loadingLine)} />
      <div data-slot="skeleton-glimmer" {...stylex.props(styles.loadingLine, styles.loadingLineShort)} />
      <div {...stylex.props(styles.loadingStarters)}>
        <div data-slot="skeleton-glimmer" {...stylex.props(styles.loadingStarter)} />
        <div data-slot="skeleton-glimmer" {...stylex.props(styles.loadingStarter)} />
        <div data-slot="skeleton-glimmer" {...stylex.props(styles.loadingStarter)} />
      </div>
    </div>
  );
}

const styles = stylex.create({
  root: {
    "--chat-column-width": {
      default: "min(860px, calc(100% - 48px))",
      "@media (max-width: 760px)": "calc(100% - 40px)"
    },
    display: "grid",
    gridTemplateRows: "minmax(0, 1fr)",
    minHeight: 0,
    height: "calc(100% + var(--shell-deck-header-height, 44px))",
    marginTop: "calc(var(--shell-deck-header-height, 44px) * -1)",
    width: "100%",
    overflow: "hidden"
  },
  contentLayer: {
    gridArea: "1 / 1",
    minHeight: 0,
    overflow: "hidden"
  },
  emptyContentLayer: {
    paddingTop: 24,
    paddingBottom: "var(--chat-composer-dock-height)"
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
  loadingRoot: {
    display: "grid",
    minHeight: "56vh",
    alignContent: "center",
    justifyItems: "center",
    gap: 14,
    paddingInline: 20
  },
  loadingEyebrow: {
    width: 72,
    height: 12,
    borderRadius: 6,
    backgroundColor: "var(--color-skeleton)"
  },
  loadingTitle: {
    width: "min(520px, 82vw)",
    height: 58,
    borderRadius: 8,
    backgroundColor: "var(--color-skeleton)",
    "@media (max-width: 760px)": {
      width: "min(360px, 78vw)",
      height: 44
    }
  },
  loadingLine: {
    width: "min(560px, 76vw)",
    height: 18,
    borderRadius: 7,
    backgroundColor: "var(--color-skeleton)"
  },
  loadingLineShort: {
    width: "min(390px, 62vw)"
  },
  loadingStarters: {
    display: "grid",
    width: "min(720px, 100%)",
    gridTemplateColumns: "repeat(3, minmax(0, 1fr))",
    gap: 10,
    marginTop: 10,
    "@media (max-width: 760px)": {
      gridTemplateColumns: "1fr"
    }
  },
  loadingStarter: {
    height: 54,
    borderRadius: 8,
    backgroundColor: "var(--color-skeleton)"
  }
});

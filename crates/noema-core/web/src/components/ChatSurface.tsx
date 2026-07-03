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
  pending: boolean;
  agentStatus: ConversationAgentStatus;
  awaitingAssistantTurn: boolean;
  expandedActivities: Set<string>;
  draft: string;
  ready: boolean;
  agentName: string | null;
  onPickStarter: (starter: string) => void;
  onToggleActivity: (id: string) => void;
  onDraftChange: (value: string) => void;
  onSubmit: (value: string) => void;
};

export function ChatSurface({
  transcript,
  pending,
  agentStatus,
  awaitingAssistantTurn,
  expandedActivities,
  draft,
  ready,
  agentName,
  onPickStarter,
  onToggleActivity,
  onDraftChange,
  onSubmit
}: ChatSurfaceProps) {
  const { visibility } = useShellSurface();
  const composerRef = React.useRef<HTMLTextAreaElement>(null);

  React.useEffect(() => {
    if (shouldFocusChatComposer({ ready, visibility })) {
      composerRef.current?.focus();
    }
  }, [ready, visibility]);

  return (
    <section
      data-slot="chat-surface"
      {...stylex.props(styles.root)}
      aria-label="Noema chat"
    >
      {transcript.length === 0 ? (
        <EmptyState onPick={onPickStarter} />
      ) : (
        <Transcript
          entries={transcript}
          pending={pending}
          agentStatus={agentStatus}
          awaitingAssistantTurn={awaitingAssistantTurn}
          expandedActivities={expandedActivities}
          onToggleActivity={onToggleActivity}
        />
      )}

      <Composer
        ref={composerRef}
        value={draft}
        ready={ready}
        pending={pending}
        placeholder={composerPlaceholder({ ready, agentName })}
        onChange={onDraftChange}
        onSubmit={onSubmit}
      />
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
    gridTemplateRows: "minmax(0, 1fr) auto",
    minHeight: 0,
    height: "100%",
    width: "100%",
    overflow: "hidden",
    paddingBottom: {
      default: 22,
      "@media (hover: none) and (pointer: coarse)": "max(18px, env(safe-area-inset-bottom))"
    }
  }
});

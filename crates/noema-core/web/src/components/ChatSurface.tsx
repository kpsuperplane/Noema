import React from "react";
import { Composer } from "./Composer";
import { EmptyState } from "./EmptyState";
import { Transcript } from "./Transcript";
import {
  type ShellSurfaceVisibility,
  useShellSurface
} from "./shell/ShellSurfaceContext";
import type { ConversationAgentStatus, TranscriptEntry } from "../types";

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
      className="grid h-full min-h-0 w-full grid-rows-[minmax(0,1fr)_auto] overflow-hidden pb-[22px] [--chat-column-width:min(860px,calc(100%_-_48px))] max-[760px]:[--chat-column-width:calc(100%_-_40px)]"
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

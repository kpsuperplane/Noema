import type { AgentStatus, TurnActivityStatus } from "@/generated/graphql";

export type SocketState = "connecting" | "ready" | "closed";
export type ConversationAgentStatus = AgentStatus | "connecting" | "closed";

export type TranscriptEntrySource = "replay";

export type TurnTranscriptItem =
  | { kind: "user_text"; text: string }
  | { kind: "assistant_text"; text: string }
  | {
      kind: "activity";
      id: string;
      activity_kind: string;
      status: TurnActivityStatus;
      title: string;
      summary?: string | null;
      metadata: unknown;
    }
  | { kind: "a2ui_card"; id: string; schema: string; payload: unknown }
  | { kind: "error_notice"; message: string; recoverable: boolean };

export type TranscriptEntry =
  | {
      id: string;
      itemId?: string;
      cursor?: string | null;
      source?: TranscriptEntrySource;
      turnId?: string;
      type: "user";
      text: string;
    }
  | {
      id: string;
      itemId?: string;
      cursor?: string | null;
      source?: TranscriptEntrySource;
      turnId?: string;
      type: "assistant";
      streamId?: string;
      responseIndex?: number;
      metadata?: unknown;
      text: string;
    }
  | {
      id: string;
      source?: TranscriptEntrySource;
      turnId?: string;
      type: "assistant_stream";
      streamId: string;
      responseIndex?: number;
      text: string;
    }
  | {
      id: string;
      itemId?: string;
      cursor?: string | null;
      source?: TranscriptEntrySource;
      turnId?: string;
      type: "activity";
      item: Extract<TurnTranscriptItem, { kind: "activity" }>;
    }
  | {
      id: string;
      itemId?: string;
      cursor?: string | null;
      source?: TranscriptEntrySource;
      turnId?: string;
      type: "card";
      item: Extract<TurnTranscriptItem, { kind: "a2ui_card" }>;
    }
  | {
      id: string;
      itemId?: string;
      cursor?: string | null;
      source?: TranscriptEntrySource;
      turnId?: string;
      type: "error";
      message: string;
      recoverable: boolean;
    };

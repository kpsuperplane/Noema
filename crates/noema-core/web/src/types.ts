import type { AgentStatus, TurnTranscriptItem } from "./generated/noema";

export type SocketState = "connecting" | "ready" | "closed";
export type ConversationAgentStatus = AgentStatus | "connecting" | "closed";

export type TranscriptEntry =
  | { id: string; itemId?: string; turnId?: string; type: "user"; text: string }
  | { id: string; itemId?: string; turnId?: string; type: "assistant"; text: string }
  | {
      id: string;
      itemId?: string;
      turnId?: string;
      type: "activity";
      item: Extract<TurnTranscriptItem, { kind: "activity" }>;
    }
  | {
      id: string;
      itemId?: string;
      turnId?: string;
      type: "card";
      item: Extract<TurnTranscriptItem, { kind: "a2ui_card" }>;
    }
  | { id: string; itemId?: string; turnId?: string; type: "error"; message: string; recoverable: boolean };

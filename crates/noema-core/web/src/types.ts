import type { GraphqlAgentStatus, GraphqlTurnActivityStatus } from "./generated/graphql";

export type SocketState = "connecting" | "ready" | "closed";
export type ConversationAgentStatus = GraphqlAgentStatus | "connecting" | "closed";

export type TurnTranscriptItem =
  | { kind: "user_text"; text: string }
  | { kind: "assistant_text"; text: string }
  | {
      kind: "activity";
      id: string;
      activity_kind: string;
      status: GraphqlTurnActivityStatus;
      title: string;
      summary?: string | null;
      metadata: unknown;
    }
  | { kind: "a2ui_card"; id: string; schema: string; payload: unknown }
  | { kind: "error_notice"; message: string; recoverable: boolean };

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

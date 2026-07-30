import type {
  AgentStatus,
  MultipleChoiceSelectionMode,
  RuntimeDebugScopeKind,
  TurnActivityStatus
} from "@/generated/graphql";

export type SocketState = "connecting" | "ready" | "closed";
export type ConversationAgentStatus = AgentStatus | "connecting" | "closed";

export type TranscriptEntrySource = "replay";

export type RuntimeDebugScope = {
  kind: RuntimeDebugScopeKind;
  scopeId: string;
};

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
  | {
      kind: "a2ui_surface";
      id: string;
      interaction_id: string | null;
      surface_id: string;
      version: string;
      revision: number;
      interaction_revision: number | null;
      lifecycle: string;
      catalog: unknown;
      snapshot: unknown;
      has_actions: boolean;
    }
  | {
      kind: "multiple_choice_prompt";
      prompt: string;
      selection_mode: MultipleChoiceSelectionMode;
      options: MultipleChoiceOption[];
    }
  | {
      kind: "multiple_choice_selection";
      prompt_item_id: string;
      selection_mode: MultipleChoiceSelectionMode;
      selected_options: MultipleChoiceOption[];
    }
  | {
      kind: "artifact_reference";
      artifact_id: string;
      artifact_version_id: string | null;
      title: string;
      artifact_kind: string;
      storage_kind: string;
      external_url: string | null;
      download_url: string | null;
      media_type: string | null;
    }
  | {
      kind: "task_reference";
      task_id: string;
    }
  | { kind: "error_notice"; message: string; recoverable: boolean };

export type MultipleChoiceOption = {
  id: string;
  label: string;
};

export type A2UIActionSubmission = {
  interaction_id: string;
  expected_revision: number;
  surface_id: string;
  source_component_id: string;
  action_name: string;
  context?: Record<string, unknown>;
  data_model?: unknown;
};

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
      type: "input";
      label?: string;
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
      debugRoundIndex?: number;
      debugScope?: RuntimeDebugScope;
      metadata?: unknown;
      taskReferences?: Extract<TurnTranscriptItem, { kind: "task_reference" }>[];
      text: string;
    }
  | {
      id: string;
      source?: TranscriptEntrySource;
      turnId?: string;
      type: "assistant_stream";
      streamId: string;
      responseIndex?: number;
      debugScope?: RuntimeDebugScope;
      text: string;
    }
  | {
      id: string;
      itemId?: string;
      cursor?: string | null;
      source?: TranscriptEntrySource;
      turnId?: string;
      debugScope?: RuntimeDebugScope;
      type: "activity";
      item: Extract<TurnTranscriptItem, { kind: "activity" }>;
    }
  | {
      id: string;
      itemId?: string;
      cursor?: string | null;
      source?: TranscriptEntrySource;
      turnId?: string;
      type: "a2ui_surface";
      item: Extract<TurnTranscriptItem, { kind: "a2ui_surface" }>;
    }
  | {
      id: string;
      itemId?: string;
      cursor?: string | null;
      source?: TranscriptEntrySource;
      turnId?: string;
      type: "multiple_choice_prompt";
      item: Extract<TurnTranscriptItem, { kind: "multiple_choice_prompt" }>;
    }
  | {
      id: string;
      itemId?: string;
      cursor?: string | null;
      source?: TranscriptEntrySource;
      turnId?: string;
      type: "multiple_choice_selection";
      item: Extract<TurnTranscriptItem, { kind: "multiple_choice_selection" }>;
    }
  | {
      id: string;
      itemId?: string;
      cursor?: string | null;
      source?: TranscriptEntrySource;
      turnId?: string;
      type: "artifact";
      item: Extract<TurnTranscriptItem, { kind: "artifact_reference" }>;
    }
  | {
      id: string;
      itemId?: string;
      cursor?: string | null;
      source?: TranscriptEntrySource;
      turnId?: string;
      metadata?: unknown;
      type: "task";
      item: Extract<TurnTranscriptItem, { kind: "task_reference" }>;
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

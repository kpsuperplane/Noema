import * as stylex from "@stylexjs/stylex";
import * as React from "react";
import { ToolMarker } from "@/components/transcript/ToolMarker";
import type { ToolMarkerGroup } from "@/components/transcript/renderModel";
import type { TranscriptEntry } from "@/shared/types";

type ActivityEntry = Extract<TranscriptEntry, { type: "activity" }>;
export type TaskToolMarkerStatus = "pending" | "running" | "complete" | "error";

type TaskToolMarkerProps = {
  id: string;
  name: string;
  target?: string;
  status: TaskToolMarkerStatus;
  errorMessage?: string;
  presentation?: "activity" | "content";
} & (
  | {
      input?: unknown;
      output?: unknown;
      detail?: React.ReactNode;
      activationLabel?: never;
      onActivate?: never;
    }
  | {
      input?: never;
      output?: never;
      detail?: never;
      activationLabel?: string;
      onActivate: () => void;
    }
);

export function TaskToolMarker({
  id,
  name,
  target,
  status,
  input,
  output,
  detail,
  errorMessage,
  presentation,
  activationLabel,
  onActivate
}: TaskToolMarkerProps) {
  const [open, setOpen] = React.useState(false);
  const marker = taskToolMarker({ id, name, target, status, input, output, errorMessage });
  const content = (
    <ToolMarker
      data={{ kind: "tool", marker }}
      detail={detail}
      open={open}
      onToggle={() => setOpen((value) => !value)}
      presentation={presentation}
    />
  );

  if (!onActivate) {
    return content;
  }

  return (
    <button
      type="button"
      aria-label={activationLabel ?? name}
      onClick={onActivate}
      {...stylex.props(styles.activation)}
    >
      {content}
    </button>
  );
}

function taskToolMarker({
  id,
  name,
  target,
  status,
  input,
  output,
  errorMessage
}: {
  id: string;
  name: string;
  target?: string;
  status: TaskToolMarkerStatus;
  input?: unknown;
  output?: unknown;
  errorMessage?: string;
}): ToolMarkerGroup {
  const callId = `${id}:call`;
  const call = activityEntry({
    id: callId,
    activityKind: "tool_call",
    status: status === "running" ? "STARTED" : "COMPLETED",
    name,
    target,
    action: { id: callId, name, payload: input }
  });
  const result = status === "complete" || status === "error"
    ? activityEntry({
        id: `${id}:result`,
        activityKind: "tool_result",
        status: status === "error" ? "FAILED" : "COMPLETED",
        name,
        target,
        summary: errorMessage,
        action: { call_id: callId, name, success: status === "complete", payload: output }
      })
    : undefined;
  return { id, call, result };
}

function activityEntry({
  id,
  activityKind,
  status,
  name,
  target,
  summary,
  action
}: {
  id: string;
  activityKind: "tool_call" | "tool_result";
  status: ActivityEntry["item"]["status"];
  name: string;
  target?: string;
  summary?: string;
  action: Record<string, unknown>;
}): ActivityEntry {
  return {
    id,
    source: "replay",
    type: "activity",
    item: {
      kind: "activity",
      id,
      activity_kind: activityKind,
      status,
      title: name,
      summary,
      metadata: {
        action,
        display: { name, target }
      }
    }
  };
}

const styles = stylex.create({
  activation: {
    display: "block",
    width: "100%",
    minWidth: 0,
    borderWidth: 0,
    backgroundColor: "transparent",
    padding: 0,
    font: "inherit",
    textAlign: "left",
    cursor: "pointer",
    transitionDuration: "var(--motion-spring-micro-duration)",
    transitionProperty: "opacity",
    transitionTimingFunction: "var(--motion-spring-critical-easing)",
    ":hover": {
      opacity: 0.72
    },
    ":focus-visible": {
      borderRadius: 5,
      outlineWidth: 3,
      outlineStyle: "solid",
      outlineColor: "color-mix(in srgb, var(--noema-pine-500) 24%, transparent)"
    }
  }
});

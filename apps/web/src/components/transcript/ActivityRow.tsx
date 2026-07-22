import * as stylex from "@stylexjs/stylex";
import { ChevronDown } from "lucide-react";
import { SpringDisclosure } from "@/motion/SpringDisclosure";
import { statusLabel } from "@/shared/format";
import type { TurnTranscriptItem } from "@/shared/types";
import { IdentityAvatar } from "../IdentityAvatar";
import { TranscriptAttachmentCard } from "./TranscriptAttachmentCard";
import { TranscriptSystemNotice } from "./TranscriptSystemNotice";

const styles = stylex.create({
  root: {
    display: "grid",
    width: "100%",
    maxWidth: 760,
    minWidth: 0,
    justifyItems: "center",
    paddingBlock: 4
  },
  detail: {
    maxHeight: 280,
    margin: 0,
    overflow: "auto",
    overflowWrap: "anywhere",
    color: "var(--noema-text-secondary)",
    fontFamily: "var(--noema-font-mono)",
    fontSize: 11,
    lineHeight: 1.45,
    whiteSpace: "pre-wrap",
    wordBreak: "break-word"
  },
  toggleMeta: {
    display: "inline-flex",
    alignItems: "center",
    gap: 3,
    whiteSpace: "nowrap"
  },
  toggleIcon: {
    transitionDuration: "var(--motion-spring-micro-duration)",
    transitionProperty: "transform",
    transitionTimingFunction: "var(--motion-spring-critical-easing)"
  },
  toggleIconOpen: {
    transform: "rotate(180deg)"
  }
});

export function ActivityRow({
  item,
  open,
  onToggle
}: {
  item: Extract<TurnTranscriptItem, { kind: "activity" }>;
  open: boolean;
  onToggle: () => void;
}) {
  const status = statusLabel(item.status);
  const neutral = activityPresentationTone(item) === "neutral";
  const noticeTone = neutral
    ? "default"
    : item.status === "FAILED"
      ? "error"
      : item.status === "COMPLETED"
        ? "success"
        : "default";
  const detail = activityDetail(item);
  const avatar = activityInstanceName(item)
    ? <IdentityAvatar actorId={`subagent:${activityInstanceName(item)}`} actorType="agent" size="xs" />
    : undefined;

  if (item.activity_kind === "task_run_start" || item.activity_kind === "task_run_end") {
    return (
      <TranscriptSystemNotice avatar={avatar} role={item.status === "FAILED" ? "alert" : "status"} tone={noticeTone}>
        {item.title}
      </TranscriptSystemNotice>
    );
  }

  if (detail) {
    return (
      <div {...stylex.props(styles.root)}>
        <TranscriptAttachmentCard
          title={item.title}
          description={item.summary || undefined}
          meta={
            <span {...stylex.props(styles.toggleMeta)}>
              {status}
              <ChevronDown
                aria-hidden="true"
                size={14}
                {...stylex.props(styles.toggleIcon, open && styles.toggleIconOpen)}
              />
            </span>
          }
          tone={neutral ? "default" : noticeTone === "error" ? "error" : noticeTone === "success" ? "success" : "info"}
          headerButtonProps={{
            "aria-expanded": open,
            "aria-controls": `${item.id}-detail`,
            onClick: onToggle,
            type: "button"
          }}
        >
          <SpringDisclosure open={open} id={`${item.id}-detail`}>
            <pre {...stylex.props(styles.detail)}>{detail}</pre>
          </SpringDisclosure>
        </TranscriptAttachmentCard>
      </div>
    );
  }

  return (
    <TranscriptSystemNotice label={status} role={item.status === "FAILED" ? "alert" : "status"} tone={noticeTone}>
      {item.summary || item.title}
    </TranscriptSystemNotice>
  );
}

function activityPresentationTone(
  item: Extract<TurnTranscriptItem, { kind: "activity" }>
): string | null {
  if (!item.metadata || typeof item.metadata !== "object" || !("presentation" in item.metadata)) {
    return null;
  }
  const presentation = item.metadata.presentation;
  if (!presentation || typeof presentation !== "object" || !("tone" in presentation)) {
    return null;
  }
  return typeof presentation.tone === "string" ? presentation.tone : null;
}

function activityDetail(item: Extract<TurnTranscriptItem, { kind: "activity" }>): string | null {
  if (!item.metadata || typeof item.metadata !== "object" || !("detail" in item.metadata)) {
    return null;
  }
  const detail = item.metadata.detail;
  return typeof detail === "string" && detail.trim() ? detail : null;
}

function activityInstanceName(item: Extract<TurnTranscriptItem, { kind: "activity" }>): string | null {
  if (!item.metadata || typeof item.metadata !== "object" || !("instance_name" in item.metadata)) {
    return null;
  }
  const instanceName = item.metadata.instance_name;
  return typeof instanceName === "string" && instanceName.trim() ? instanceName : null;
}

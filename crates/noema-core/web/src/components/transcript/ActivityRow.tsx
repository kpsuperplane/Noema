import { statusLabel } from "@/shared/format";
import type { TurnTranscriptItem } from "@/shared/types";
import { TranscriptSystemNotice } from "./TranscriptSystemNotice";

export function ActivityRow({
  item
}: {
  item: Extract<TurnTranscriptItem, { kind: "activity" }>;
  open: boolean;
  onToggle: () => void;
}) {
  const status = statusLabel(item.status);
  const noticeTone = item.status === "FAILED" ? "error" : item.status === "COMPLETED" ? "success" : "default";

  return (
    <TranscriptSystemNotice label={status} role={item.status === "FAILED" ? "alert" : "status"} tone={noticeTone}>
      {item.summary || item.title}
    </TranscriptSystemNotice>
  );
}

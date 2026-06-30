import {
  Attachment,
  AttachmentContent,
  AttachmentDescription,
  AttachmentMedia,
  AttachmentTitle
} from "@/components/ui/attachment";
import { WrenchIcon } from "lucide-react";
import { statusLabel } from "../../format";
import { formatToolDetail, toolMarkerLabel } from "./markerModel";
import type { ToolMarkerGroup } from "./renderModel";
import { ToolDetailRow } from "./ToolDetailRow";

export function ToolDetailAttachment({ id, marker }: { id: string; marker: ToolMarkerGroup }) {
  const result = marker.result?.item;
  const call = marker.call?.item;
  const failed = result?.status === "FAILED";
  const title = toolMarkerLabel(marker);
  const description = result?.summary ?? call?.summary ?? statusLabel(result?.status ?? call?.status ?? "COMPLETED");

  return (
    <Attachment id={id} state={failed ? "error" : "done"} className="max-w-full">
      <AttachmentMedia className={failed ? "text-[var(--red-700)]" : "text-[var(--blue-700)]"}>
        <WrenchIcon />
      </AttachmentMedia>
      <AttachmentContent>
        <AttachmentTitle>{title}</AttachmentTitle>
        <AttachmentDescription>{description}</AttachmentDescription>
        <dl className="mt-3 grid gap-2 border-t border-[var(--border-subtle)] pt-2.5">
          {call ? <ToolDetailRow label="Call" value={formatToolDetail(call.title, call.metadata)} /> : null}
          {result ? (
            <ToolDetailRow label="Result" value={formatToolDetail(result.summary ?? result.title, result.metadata)} />
          ) : null}
          <ToolDetailRow label="Status" value={statusLabel(result?.status ?? call?.status ?? "COMPLETED")} />
        </dl>
      </AttachmentContent>
    </Attachment>
  );
}

import { WrenchIcon } from "lucide-react";
import { toolMarkerLabel, toolMarkerPending, toolMarkerTone } from "./markerModel";
import type { ToolMarkerGroup } from "./renderModel";
import { ToolDetailAttachment } from "./ToolDetailAttachment";
import { TranscriptMarkerFrame } from "./TranscriptMarkerFrame";

export function ToolMarker({
  marker,
  open,
  onToggle
}: {
  marker: ToolMarkerGroup;
  open: boolean;
  onToggle: () => void;
}) {
  const tone = toolMarkerTone(marker);
  const pending = toolMarkerPending(marker);

  return (
    <div className="grid w-full max-w-full gap-2">
      <TranscriptMarkerFrame
        tone={tone}
        pending={pending}
        icon={<WrenchIcon />}
        buttonProps={{
          "aria-expanded": open,
          "aria-controls": `${marker.id}-details`,
          onClick: onToggle
        }}
      >
        {toolMarkerLabel(marker)}
      </TranscriptMarkerFrame>
      {open ? <ToolDetailAttachment id={`${marker.id}-details`} marker={marker} /> : null}
    </div>
  );
}

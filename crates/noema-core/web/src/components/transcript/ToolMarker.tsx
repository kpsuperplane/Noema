import { WrenchIcon } from "lucide-react";
import * as stylex from "@stylexjs/stylex";
import { toolMarkerLabel, toolMarkerPending, toolMarkerTone } from "./markerModel";
import type { ToolMarkerGroup } from "./renderModel";
import { ToolDetailAttachment } from "./ToolDetailAttachment";
import { TranscriptMarkerFrame } from "./TranscriptMarkerFrame";

const styles = stylex.create({
  root: {
    display: "grid",
    width: "100%",
    maxWidth: "100%",
    gap: 8
  }
});

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
    <div {...stylex.props(styles.root)}>
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

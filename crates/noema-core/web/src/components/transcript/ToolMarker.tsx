import { Marker, MarkerContent, MarkerIcon } from "@/components/ui/marker";
import { WrenchIcon } from "lucide-react";
import { toolMarkerLabel, toolMarkerPending, toolMarkerTone } from "./markerModel";
import type { ToolMarkerGroup } from "./renderModel";
import { ToolDetailAttachment } from "./ToolDetailAttachment";

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
      <Marker
        render={<button type="button" />}
        aria-expanded={open}
        aria-controls={`${marker.id}-details`}
        onClick={onToggle}
        tone={tone}
        pending={pending}
        className="w-fit rounded-lg px-2 py-1 transition-colors hover:bg-muted focus-visible:ring-3 focus-visible:ring-ring/30 focus-visible:outline-none"
      >
        <MarkerIcon>
          <WrenchIcon />
        </MarkerIcon>
        <MarkerContent>{toolMarkerLabel(marker)}</MarkerContent>
      </Marker>
      {open ? <ToolDetailAttachment id={`${marker.id}-details`} marker={marker} /> : null}
    </div>
  );
}

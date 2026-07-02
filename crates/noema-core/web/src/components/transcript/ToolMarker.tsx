import { ChatToolCalls, type ChatToolCallItem, type ChatToolCallStatus } from "@astryxdesign/core/Chat";
import * as stylex from "@stylexjs/stylex";
import { toolMarkerName, toolMarkerPending } from "./markerModel";
import type { ToolMarkerGroup } from "./renderModel";
import { ToolDetailAttachment } from "./ToolDetailAttachment";

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
  const calls = toolMarkerCalls(marker);

  return (
    <div {...stylex.props(styles.root)}>
      <ChatToolCalls
        calls={calls}
        isExpanded={open}
        onExpandedChange={(nextOpen) => {
          if (nextOpen !== open) {
            onToggle();
          }
        }}
      />
    </div>
  );
}

function toolMarkerCalls(marker: ToolMarkerGroup): ChatToolCallItem[] {
  const target = marker.result?.item.summary ?? marker.call?.item.summary;
  const errorMessage =
    marker.result?.item.status === "FAILED" ? marker.result.item.summary ?? marker.result.item.title : undefined;
  const call: ChatToolCallItem = {
    key: marker.id,
    name: toolMarkerName(marker),
    status: toolMarkerStatus(marker),
    resultDetail: <ToolDetailAttachment id={`${marker.id}-details`} marker={marker} />
  };

  if (target) {
    call.target = target;
  }
  if (errorMessage) {
    call.errorMessage = errorMessage;
  }

  return [call];
}

function toolMarkerStatus(marker: ToolMarkerGroup): ChatToolCallStatus {
  if (marker.result?.item.status === "FAILED") {
    return "error";
  }
  if (marker.result) {
    return "complete";
  }
  if (toolMarkerPending(marker)) {
    return "running";
  }
  return "pending";
}

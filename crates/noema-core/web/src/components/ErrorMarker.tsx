import { TranscriptMarkerFrame } from "@/components/transcript/TranscriptMarkerFrame";

export function ErrorMarker({
  message,
  label,
  recoverable = true
}: {
  message: string;
  label?: string;
  recoverable?: boolean;
}) {
  return (
    <TranscriptMarkerFrame role={recoverable ? "status" : "alert"} tone="error">
      {label ? <strong>{label}</strong> : null}
      <span>{message}</span>
    </TranscriptMarkerFrame>
  );
}

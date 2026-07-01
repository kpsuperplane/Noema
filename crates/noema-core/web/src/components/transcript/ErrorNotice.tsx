import { TranscriptMarkerFrame } from "./TranscriptMarkerFrame";

export function ErrorNotice({ message, recoverable }: { message: string; recoverable: boolean }) {
  return (
    <TranscriptMarkerFrame role={recoverable ? "status" : "alert"} tone="error">
      <strong>{recoverable ? "Notice" : "Error"}</strong>
      <span>{message}</span>
    </TranscriptMarkerFrame>
  );
}

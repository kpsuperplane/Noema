import { TranscriptSystemNotice } from "./TranscriptSystemNotice";

export function ErrorNotice({ message, recoverable }: { message: string; recoverable: boolean }) {
  return (
    <TranscriptSystemNotice role={recoverable ? "status" : "alert"} tone="error">
      {message}
    </TranscriptSystemNotice>
  );
}

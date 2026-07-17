import { TranscriptSystemNotice } from "./TranscriptSystemNotice";

export function ErrorNotice({ message, recoverable }: { message: string; recoverable: boolean }) {
  return (
    <TranscriptSystemNotice label={recoverable ? "Notice" : "Error"} role={recoverable ? "status" : "alert"} tone="error">
      {message}
    </TranscriptSystemNotice>
  );
}

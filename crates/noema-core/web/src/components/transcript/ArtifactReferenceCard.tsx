import type { TurnTranscriptItem } from "@/shared/types";
import { TranscriptAttachmentCard } from "./TranscriptAttachmentCard";

export function ArtifactReferenceCard({
  item
}: {
  item: Extract<TurnTranscriptItem, { kind: "artifact_reference" }>;
}) {
  const href = item.download_url ?? item.external_url ?? undefined;
  const description = [item.artifact_kind, item.media_type].filter(Boolean).join(" · ");
  return (
    <TranscriptAttachmentCard
      title={href ? <a href={href}>{item.title}</a> : item.title}
      description={description || undefined}
      meta={item.storage_kind === "local_file" ? "file" : "link"}
      icon={item.storage_kind === "local_file" ? "FILE" : "URL"}
      tone="info"
    />
  );
}

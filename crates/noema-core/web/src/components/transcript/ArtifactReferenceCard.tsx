import type { TurnTranscriptItem } from "@/shared/types";
import { TranscriptAttachmentCard } from "./TranscriptAttachmentCard";

export function ArtifactReferenceCard({
  item
}: {
  item: Extract<TurnTranscriptItem, { kind: "artifact_reference" }>;
}) {
  const link = resolveArtifactLink(item.download_url, item.external_url);
  const description = [item.artifact_kind, item.media_type].filter(Boolean).join(" · ");
  return (
    <TranscriptAttachmentCard
      title={
        link ? (
          <a href={link.href} target={link.external ? "_blank" : undefined} rel={link.external ? "noreferrer" : undefined}>
            {item.title}
          </a>
        ) : (
          item.title
        )
      }
      description={description || undefined}
      meta={item.storage_kind === "local_file" ? "file" : "link"}
      icon={item.storage_kind === "local_file" ? "FILE" : "URL"}
      tone="info"
    />
  );
}

type ArtifactLink = {
  href: string;
  external: boolean;
};

function resolveArtifactLink(downloadUrl: string | null, externalUrl: string | null): ArtifactLink | null {
  const localDownload = trustedLocalDownloadHref(downloadUrl);
  if (localDownload) {
    return { href: localDownload, external: false };
  }

  const externalLink = trustedExternalHref(externalUrl);
  if (externalLink) {
    return { href: externalLink, external: true };
  }

  return null;
}

function trustedLocalDownloadHref(href: string | null): string | null {
  if (!href || href.startsWith("http://") || href.startsWith("https://")) {
    return null;
  }

  if (!/^\/artifacts\/[^/]+\/download(?:[?#].*)?$/.test(href)) {
    return null;
  }

  return href;
}

function trustedExternalHref(href: string | null): string | null {
  if (!href) {
    return null;
  }

  try {
    const url = new URL(href);
    return url.protocol === "http:" || url.protocol === "https:" ? url.toString() : null;
  } catch {
    return null;
  }
}

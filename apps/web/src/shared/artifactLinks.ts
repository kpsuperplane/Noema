import { isTauriRuntime } from "@/graphql/transportMode";

export type TrustedArtifactLink = {
  href: string;
  external: boolean;
};

type ArtifactLinkOptions = {
  isDesktop?: boolean;
};

export function artifactDownloadHref(
  downloadUrl: string | null,
  options: ArtifactLinkOptions = {}
): string | null {
  // Local download routes exist only on the authenticated daemon HTTP transport.
  if (options.isDesktop ?? isTauriRuntime()) {
    return null;
  }
  return trustedLocalDownloadHref(downloadUrl);
}

export function resolveArtifactReferenceLink(
  downloadUrl: string | null,
  externalUrl: string | null,
  options: ArtifactLinkOptions = {}
): TrustedArtifactLink | null {
  const localDownload = artifactDownloadHref(downloadUrl, options);
  if (localDownload) {
    return { href: localDownload, external: false };
  }

  const externalLink = trustedExternalHref(externalUrl);
  return externalLink ? { href: externalLink, external: true } : null;
}

export function resolveTaskArtifactLink(
  href: string | null,
  options: ArtifactLinkOptions = {}
): TrustedArtifactLink | null {
  const localDownload = artifactDownloadHref(href, options);
  if (localDownload) {
    return { href: localDownload, external: false };
  }

  const externalLink = trustedExternalHref(href);
  return externalLink ? { href: externalLink, external: true } : null;
}

function trustedLocalDownloadHref(href: string | null): string | null {
  if (!href || href.startsWith("http://") || href.startsWith("https://")) {
    return null;
  }

  if (!/^\/artifacts\/(?:versions\/[^/:]+|[^/]+)\/download(?:[?#].*)?$/.test(href)) {
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

import * as stylex from "@stylexjs/stylex";
import { Globe } from "lucide-react";
import { useState } from "react";

type FaviconImageProps = {
  hostname: string;
  size?: "compact" | "default";
};

const styles = stylex.create({
  root: {
    position: "relative",
    display: "inline-grid",
    placeItems: "center",
    flexShrink: 0,
    overflow: "hidden",
    borderRadius: "var(--radius-full)",
    backgroundColor: "var(--color-background-muted)",
    color: "var(--color-text-secondary)",
    fontWeight: "var(--font-weight-semibold)",
    lineHeight: 1
  },
  compact: {
    width: "var(--spacing-3)",
    height: "var(--spacing-3)",
    fontSize: "var(--font-size-3xs)"
  },
  default: {
    width: "var(--spacing-4)",
    height: "var(--spacing-4)",
    fontSize: "var(--font-size-xs)"
  },
  image: {
    position: "absolute",
    inset: 0,
    width: "100%",
    height: "100%",
    objectFit: "contain"
  },
  hidden: { opacity: 0 },
  globe: { width: "var(--spacing-2)", height: "var(--spacing-2)" }
});

export function FaviconImage({ hostname, size = "default" }: FaviconImageProps) {
  const [loadedHostname, setLoadedHostname] = useState<string | null>(null);
  const [failedHostname, setFailedHostname] = useState<string | null>(null);
  const loaded = loadedHostname === hostname;
  const failed = failedHostname === hostname;
  const initial = siteInitial(hostname);

  return (
    <span
      aria-hidden="true"
      {...stylex.props(styles.root, size === "compact" ? styles.compact : styles.default)}
    >
      {initial ?? <Globe aria-hidden="true" {...stylex.props(styles.globe)} />}
      {!failed ? (
        <img
          src={faviconUrl(hostname)}
          alt=""
          aria-hidden="true"
          onLoad={() => setLoadedHostname(hostname)}
          onError={() => setFailedHostname(hostname)}
          {...stylex.props(styles.image, !loaded && styles.hidden)}
        />
      ) : null}
    </span>
  );
}

export function faviconUrl(hostname: string): string {
  return `/favicons/${encodeURIComponent(hostname)}`;
}

export function siteInitial(hostname: string): string | null {
  const labels = hostname.toLowerCase().split(".").filter(Boolean);
  const prefix = labels[0] ?? "";
  if (/^www\d*$/.test(prefix) || prefix === "m" || prefix === "mobile") {
    labels.shift();
  }
  const character = Array.from(labels[0] ?? "").find((value) => /[\p{L}\p{N}]/u.test(value));
  return character?.toLocaleUpperCase() ?? null;
}

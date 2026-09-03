import * as stylex from "@stylexjs/stylex";
import { type ReactNode, useState } from "react";

type FaviconImageProps = {
  hostname: string;
  fallback?: ReactNode;
  grouped?: boolean;
  size?: "compact" | "default" | "large";
};

const styles = stylex.create({
  root: {
    position: "relative",
    display: "inline-grid",
    placeItems: "center",
    flexShrink: 0,
    overflow: "hidden",
    borderRadius: "var(--radius-full)"
  },
  compact: {
    width: "var(--spacing-3)",
    height: "var(--spacing-3)",
  },
  default: {
    width: "var(--spacing-4)",
    height: "var(--spacing-4)",
  },
  large: {
    width: "var(--spacing-6)",
    height: "var(--spacing-6)",
  },
  grouped: {
    zIndex: 0,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--color-background-surface)",
    marginInlineStart: "calc(-1 * var(--spacing-1))",
    ":first-child": { marginInlineStart: 0 }
  },
  image: {
    position: "absolute",
    inset: 0,
    width: "100%",
    height: "100%",
    objectFit: "contain"
  },
  hidden: { opacity: 0 }
});

export function FaviconImage({
  hostname,
  fallback,
  grouped = false,
  size = "default"
}: FaviconImageProps) {
  const [loadedHostname, setLoadedHostname] = useState<string | null>(null);
  const [failedHostname, setFailedHostname] = useState<string | null>(null);
  const loaded = loadedHostname === hostname;
  const failed = failedHostname === hostname;
  if (failed) return fallback ?? null;

  return (
    <span
      aria-hidden="true"
      {...stylex.props(
        styles.root,
        size === "compact" ? styles.compact : size === "large" ? styles.large : styles.default,
        grouped && styles.grouped,
        !loaded && styles.hidden
      )}
    >
      <img
        src={faviconUrl(hostname)}
        alt=""
        aria-hidden="true"
        onLoad={() => setLoadedHostname(hostname)}
        onError={() => setFailedHostname(hostname)}
        {...stylex.props(styles.image)}
      />
    </span>
  );
}

export function faviconUrl(hostname: string): string {
  return `/favicons/${encodeURIComponent(hostname)}`;
}

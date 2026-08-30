import React from "react";
import { useQuery } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import { Download } from "lucide-react";
import { MarkdownContent } from "@/components/MarkdownContent";
import {
  ArtifactVersionDetailDocument,
  type ArtifactVersionDetailQuery
} from "@/generated/graphql";
import { artifactDownloadHref, artifactPreviewHref } from "@/shared/artifactLinks";
import { ArtifactVersionSelector } from "./ArtifactVersionSelector";

export type ArtifactDetail = NonNullable<ArtifactVersionDetailQuery["artifactVersionDetail"]>;

export function ArtifactDetailPanel({
  version,
  onChangeVersion,
  onDetailChange
}: {
  version: string;
  onChangeVersion: (version: string) => void;
  onDetailChange: (detail: ArtifactDetail | null) => void;
}) {
  const { data, error, loading, refetch } = useQuery(ArtifactVersionDetailDocument, {
    fetchPolicy: "cache-and-network",
    variables: { artifactVersionId: version }
  });
  const detail = data?.artifactVersionDetail ?? null;

  React.useEffect(() => {
    onDetailChange(detail);
  }, [detail, onDetailChange]);

  if (loading && !detail) {
    return (
      <div role="status" {...stylex.props(styles.status)}>
        Loading artifact...
      </div>
    );
  }

  if (error) {
    if (!detail) {
      return (
        <ArtifactUnavailable
          message="Artifact could not load."
          onRetry={() => void refetch().catch(() => undefined)}
        />
      );
    }
  }

  if (!detail) {
    return <ArtifactUnavailable message="Artifact unavailable" />;
  }

  return (
    <div {...stylex.props(styles.root)}>
      {error ? (
        <div {...stylex.props(styles.recovery)}>
          <p role="alert" {...stylex.props(styles.recoveryText)}>This preview may be out of date.</p>
          <Button
            type="button"
            size="sm"
            variant="secondary"
            label="Retry"
            onClick={() => void refetch().catch(() => undefined)}
          />
        </div>
      ) : null}
      <ArtifactMeta
        detail={detail}
        selectedVersion={version}
        onChangeVersion={onChangeVersion}
      />
      <ArtifactPreview detail={detail} />
    </div>
  );
}

function ArtifactPreview({ detail }: { detail: ArtifactDetail }) {
  const previewHref = artifactPreviewHref(detail.previewUrl ?? null);
  if (detail.previewKind === "MARKDOWN" && detail.markdown) {
    return (
      <MarkdownContent density="default" headingLevelStart={1} xstyle={styles.markdown}>
        {detail.markdown}
      </MarkdownContent>
    );
  }
  if (detail.previewKind === "PLAIN_TEXT" && detail.plainText !== null) {
    return <pre {...stylex.props(styles.plainText)}>{detail.plainText}</pre>;
  }
  if (detail.previewKind === "IMAGE" && previewHref) {
    return (
      <img
        alt={detail.title}
        loading="lazy"
        src={previewHref}
        {...stylex.props(styles.imagePreview)}
      />
    );
  }
  if (detail.previewKind === "PDF" && previewHref) {
    return (
      <iframe
        loading="lazy"
        referrerPolicy="no-referrer"
        src={previewHref}
        title={`${detail.title} preview`}
        {...stylex.props(styles.documentFrame)}
      />
    );
  }
  if (detail.previewKind === "HTML" && detail.html !== null) {
    return <IsolatedHtmlPreview html={detail.html} title={detail.title} />;
  }
  return <ArtifactUnavailable message="Preview unavailable" />;
}

function IsolatedHtmlPreview({ html, title }: { html: string; title: string }) {
  const isolatedHtml = React.useMemo(() => isolateHtmlPreview(html), [html]);
  return (
    <iframe
      referrerPolicy="no-referrer"
      sandbox=""
      srcDoc={isolatedHtml}
      title={`${title} preview`}
      {...stylex.props(styles.documentFrame)}
    />
  );
}

function isolateHtmlPreview(source: string): string {
  const document = new DOMParser().parseFromString(source, "text/html");
  for (const element of document.querySelectorAll(
    "script, iframe, object, embed, form, base, link, template, meta[http-equiv]"
  )) {
    element.remove();
  }
  for (const element of document.querySelectorAll("*")) {
    for (const attribute of Array.from(element.attributes)) {
      const name = attribute.name.toLowerCase();
      const value = attribute.value.trim();
      if (
        name.startsWith("on") ||
        [
          "href",
          "xlink:href",
          "srcset",
          "srcdoc",
          "action",
          "formaction",
          "poster",
          "ping",
          "background"
        ].includes(name) ||
        unsafeCss(value)
      ) {
        element.removeAttribute(attribute.name);
        continue;
      }
      if (name === "src" && !/^data:image\/(?:png|jpeg|gif|webp|avif);base64,/i.test(value)) {
        element.removeAttribute(attribute.name);
        continue;
      }
    }
  }
  for (const style of document.querySelectorAll("style")) {
    if (unsafeCss(style.textContent ?? "")) {
      style.remove();
    }
  }
  const policy = document.createElement("meta");
  policy.httpEquiv = "Content-Security-Policy";
  policy.content =
    "default-src 'none'; img-src data:; style-src 'unsafe-inline'; font-src data:; base-uri 'none'; form-action 'none'";
  document.head.prepend(policy);
  return `<!doctype html>${document.documentElement.outerHTML}`;
}

function unsafeCss(value: string): boolean {
  return /url\s*\(|@import|expression\s*\(|behavior\s*:/i.test(value);
}

export function ArtifactDownloadAction({ detail }: { detail: ArtifactDetail | null }) {
  const downloadHref = artifactDownloadHref(detail?.downloadUrl ?? null);
  if (!downloadHref) {
    return null;
  }

  return (
    <Button
      href={downloadHref}
      icon={<Download aria-hidden="true" size={15} />}
      isIconOnly
      label="Download"
      size="sm"
      variant="secondary"
    />
  );
}

function ArtifactMeta({
  detail,
  selectedVersion,
  onChangeVersion
}: {
  detail: ArtifactDetail;
  selectedVersion: string;
  onChangeVersion: (version: string) => void;
}) {
  const parts = [detail.artifactKind, detail.mediaType].filter(Boolean);
  return (
    <div {...stylex.props(styles.meta)}>
      {parts.length > 0 ? <span {...stylex.props(styles.metaText)}>{parts.join(" · ")}</span> : null}
      <ArtifactVersionSelector
        versions={detail.versions}
        selectedVersion={selectedVersion}
        onChangeVersion={onChangeVersion}
      />
    </div>
  );
}

function ArtifactUnavailable({
  message,
  onRetry
}: {
  message: string;
  onRetry?: () => void;
}) {
  return (
    <div role={onRetry ? undefined : "status"} {...stylex.props(styles.empty)}>
      <p role={onRetry ? "alert" : undefined}>{message}</p>
      {onRetry ? (
        <Button type="button" size="sm" variant="secondary" label="Retry" onClick={onRetry} />
      ) : null}
    </div>
  );
}

const styles = stylex.create({
  root: {
    minWidth: 0,
    display: "grid",
    gap: "var(--spacing-4)"
  },
  status: {
    color: "var(--noema-text-secondary)",
    fontSize: 13
  },
  meta: {
    minWidth: 0,
    display: "grid",
    gridTemplateColumns: "minmax(0, 1fr) auto",
    alignItems: "center",
    gap: "var(--spacing-2)",
    color: "var(--noema-text-muted)",
    fontSize: 12
  },
  metaText: {
    minWidth: 0,
    overflowWrap: "anywhere"
  },
  markdown: {
    color: "var(--noema-text-primary)",
    fontSize: 14,
    lineHeight: 1.55
  },
  plainText: {
    margin: "var(--spacing-0)",
    color: "var(--noema-text-primary)",
    fontFamily: "var(--noema-font-mono)",
    fontSize: 13,
    lineHeight: 1.55,
    overflowWrap: "anywhere",
    whiteSpace: "pre-wrap"
  },
  imagePreview: {
    display: "block",
    width: "100%",
    maxHeight: "70vh",
    objectFit: "contain",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--noema-border-subtle)",
    borderRadius: "var(--radius-element)",
    backgroundColor: "var(--noema-surface-sunken)"
  },
  documentFrame: {
    display: "block",
    width: "100%",
    height: "70vh",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--noema-border-subtle)",
    borderRadius: "var(--radius-element)",
    backgroundColor: "var(--noema-surface-card)"
  },
  empty: {
    display: "grid",
    gap: "var(--spacing-3)",
    alignContent: "start",
    color: "var(--noema-text-secondary)",
    fontSize: 13
  },
  recovery: {
    display: "flex",
    flexWrap: "wrap",
    alignItems: "center",
    gap: "var(--spacing-2)",
    color: "var(--noema-red-700)",
    fontSize: 13
  },
  recoveryText: { margin: "var(--spacing-0)" }
});

import React from "react";
import { useQuery } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import { Markdown, type MarkdownProps } from "@astryxdesign/core/Markdown";
import * as stylex from "@stylexjs/stylex";
import { Download } from "lucide-react";
import {
  ArtifactVersionDetailDocument,
  type ArtifactVersionDetailQuery
} from "@/generated/graphql";
import { artifactDownloadHref } from "@/shared/artifactLinks";
import { ArtifactVersionSelector } from "./ArtifactVersionSelector";

type MarkdownXStyle = MarkdownProps["xstyle"];
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
  const { data, error, loading } = useQuery(ArtifactVersionDetailDocument, {
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
    return <ArtifactUnavailable message={`Error loading artifact: ${error.message}`} />;
  }

  if (!detail) {
    return <ArtifactUnavailable message="Artifact unavailable" />;
  }

  return (
    <div {...stylex.props(styles.root)}>
      <ArtifactMeta
        detail={detail}
        selectedVersion={version}
        onChangeVersion={onChangeVersion}
      />
      {detail.previewKind === "MARKDOWN" && detail.markdown ? (
        <Markdown
          autolink="gfm"
          contentWidth="100%"
          density="default"
          headingLevelStart={1}
          xstyle={markdownXStyle(styles.markdown)}
        >
          {detail.markdown}
        </Markdown>
      ) : detail.previewKind === "PLAIN_TEXT" && detail.plainText !== null ? (
        <pre {...stylex.props(styles.plainText)}>{detail.plainText}</pre>
      ) : (
        <ArtifactUnavailable message="Preview unavailable" />
      )}
    </div>
  );
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

function ArtifactUnavailable({ message }: { message: string }) {
  return (
    <div role="status" {...stylex.props(styles.empty)}>
      <p>{message}</p>
    </div>
  );
}

function markdownXStyle(...xstyle: unknown[]): MarkdownXStyle {
  return xstyle as unknown as MarkdownXStyle;
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
    margin: 0,
    color: "var(--noema-text-primary)",
    fontFamily: "var(--noema-font-mono)",
    fontSize: 13,
    lineHeight: 1.55,
    overflowWrap: "anywhere",
    whiteSpace: "pre-wrap"
  },
  empty: {
    display: "grid",
    gap: "var(--spacing-3)",
    alignContent: "start",
    color: "var(--noema-text-secondary)",
    fontSize: 13
  }
});

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

type MarkdownXStyle = MarkdownProps["xstyle"];
export type ArtifactDetail = NonNullable<ArtifactVersionDetailQuery["artifactVersionDetail"]>;

export function ArtifactDetailPanel({
  version,
  onDetailChange
}: {
  version: string;
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
      <ArtifactMeta detail={detail} />
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
      ) : (
        <ArtifactUnavailable message="Preview unavailable" />
      )}
    </div>
  );
}

export function ArtifactDownloadAction({ detail }: { detail: ArtifactDetail | null }) {
  if (!detail?.downloadUrl) {
    return null;
  }

  return (
    <Button
      href={detail.downloadUrl}
      icon={<Download aria-hidden="true" size={15} />}
      label="Download"
      size="sm"
      variant="secondary"
    />
  );
}

function ArtifactMeta({ detail }: { detail: ArtifactDetail }) {
  const parts = [detail.artifactKind, detail.mediaType].filter(Boolean);
  return parts.length > 0 ? <div {...stylex.props(styles.meta)}>{parts.join(" · ")}</div> : null;
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
    gap: 16
  },
  status: {
    color: "var(--noema-text-secondary)",
    fontSize: 13
  },
  meta: {
    color: "var(--noema-text-muted)",
    fontSize: 12
  },
  markdown: {
    color: "var(--noema-text-primary)",
    fontSize: 14,
    lineHeight: 1.55
  },
  empty: {
    display: "grid",
    gap: 12,
    alignContent: "start",
    color: "var(--noema-text-secondary)",
    fontSize: 13
  }
});

import type { TurnTranscriptItem } from "@/shared/types";
import { artifactDetailTarget, type ChatDetailTarget } from "@/components/chatDetail/chatDetailTypes";
import {
  resolveArtifactReferenceLink,
  type TrustedArtifactLink
} from "@/shared/artifactLinks";
import { IconButton, type IconButtonProps } from "@astryxdesign/core/IconButton";
import { Item, type ItemProps } from "@astryxdesign/core/Item";
import * as stylex from "@stylexjs/stylex";
import {
  AudioLines,
  Braces,
  Download,
  ExternalLink,
  File,
  FileArchive,
  FileJson,
  FileText,
  Image as ImageIcon,
  Link2,
  PanelRightOpen,
  Table2,
  Video
} from "lucide-react";
import type { ReactNode } from "react";

type ArtifactReferenceItem = Extract<TurnTranscriptItem, { kind: "artifact_reference" }>;
type ItemXStyle = ItemProps["xstyle"];
type IconButtonXStyle = IconButtonProps["xstyle"];

const styles = stylex.create({
  item: {
    display: "inline-flex",
    width: "fit-content",
    maxWidth: "min(100%, 520px)",
    minWidth: "min(220px, 100%)",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--noema-border-subtle)",
    borderRadius: 8,
    backgroundColor: "var(--noema-surface-card)",
    color: "var(--noema-text-primary)",
    boxShadow: "0 1px 0 color-mix(in srgb, black 4%, transparent)",
    transition: "background-color 140ms ease, border-color 140ms ease, box-shadow 140ms ease",
    ":hover": {
      borderColor: "color-mix(in srgb, var(--noema-pine-500) 30%, var(--noema-border-subtle))",
      backgroundColor: "var(--noema-surface-hover)"
    }
  },
  disabledItem: {
    opacity: 0.72
  },
  detailItem: {
    cursor: "default"
  },
  actionItem: {
    paddingInlineEnd: 48
  },
  iconFrame: {
    display: "inline-flex",
    width: 32,
    height: 32,
    alignItems: "center",
    justifyContent: "center",
    borderRadius: 8,
    backgroundColor: "var(--noema-surface-sunken)",
    color: "var(--noema-pine-700)",
    flexShrink: 0
  },
  action: {
    position: "absolute",
    insetInlineEnd: 8,
    insetBlockEnd: 8,
    zIndex: 1,
    display: "inline-flex",
    color: "var(--noema-text-muted)"
  }
});

export function ArtifactReferenceCard({
  item,
  onOpenDetail
}: {
  item: ArtifactReferenceItem;
  onOpenDetail?: (target: ChatDetailTarget) => void;
}) {
  const link = resolveArtifactReferenceLink(item.download_url, item.external_url);
  const detailTarget =
    item.storage_kind === "local_file" ? artifactDetailTarget(item.artifact_version_id) : null;
  const opensDetail = Boolean(detailTarget && onOpenDetail);
  const description = artifactDescription(item);
  const icon = artifactIcon(item);
  const actionIcon = opensDetail ? (
    <PanelRightOpen aria-hidden="true" size={14} strokeWidth={2} />
  ) : link ? (
    artifactActionIcon(link)
  ) : null;
  const actionLabel = opensDetail
    ? "Open artifact"
    : link
      ? link.external
        ? "Open external artifact"
        : "Download artifact"
      : null;
  const handleOpenDetail =
    opensDetail && detailTarget
      ? () => {
          onOpenDetail?.(detailTarget);
        }
      : undefined;

  return (
    <Item
      align="start"
      data-slot="artifact-reference-item"
      data-testid="artifact-reference-item"
      density="balanced"
      description={description}
      descriptionLines={1}
      endContent={
        actionIcon && actionLabel ? (
          <IconButton
            data-slot="artifact-reference-action"
            href={opensDetail ? undefined : link?.href}
            icon={actionIcon}
            label={actionLabel}
            onClick={handleOpenDetail}
            rel={!opensDetail && link?.external ? "noreferrer" : undefined}
            size="sm"
            tabIndex={-1}
            target={!opensDetail && link?.external ? "_blank" : undefined}
            tooltip={actionLabel}
            variant="ghost"
            xstyle={iconButtonXStyle(styles.action)}
          />
        ) : undefined
      }
      href={opensDetail ? undefined : link?.href}
      isDisabled={!link && !opensDetail}
      label={item.title}
      labelLines={2}
      onClick={handleOpenDetail}
      rel={!opensDetail && link?.external ? "noreferrer" : undefined}
      startContent={
        <span {...stylex.props(styles.iconFrame)} aria-hidden="true">
          {icon}
        </span>
      }
      target={!opensDetail && link?.external ? "_blank" : undefined}
      xstyle={itemXStyle(
        styles.item,
        opensDetail && styles.detailItem,
        actionIcon && styles.actionItem,
        !link && !opensDetail && styles.disabledItem
      )}
    />
  );
}

function artifactDescription(item: ArtifactReferenceItem): string {
  return [humanizeArtifactKind(item.artifact_kind), humanizeMediaType(item.media_type)]
    .filter((part): part is string => Boolean(part))
    .join(" · ");
}

function artifactActionIcon(link: TrustedArtifactLink): ReactNode {
  const iconProps = { "aria-hidden": true, size: 14, strokeWidth: 2 } as const;
  return link.external ? <ExternalLink {...iconProps} /> : <Download {...iconProps} />;
}

function artifactIcon(item: ArtifactReferenceItem): ReactNode {
  const iconProps = { size: 18, strokeWidth: 2 } as const;
  const mediaType = item.media_type?.toLowerCase() ?? "";
  const artifactKind = item.artifact_kind.toLowerCase();

  if (mediaType.startsWith("image/")) {
    return <ImageIcon {...iconProps} />;
  }
  if (mediaType.startsWith("video/")) {
    return <Video {...iconProps} />;
  }
  if (mediaType.startsWith("audio/")) {
    return <AudioLines {...iconProps} />;
  }
  if (mediaType.includes("json")) {
    return <FileJson {...iconProps} />;
  }
  if (
    mediaType.includes("csv") ||
    mediaType.includes("spreadsheet") ||
    mediaType.includes("tab-separated-values") ||
    artifactKind.includes("spreadsheet") ||
    artifactKind.includes("table")
  ) {
    return <Table2 {...iconProps} />;
  }
  if (
    mediaType.includes("zip") ||
    mediaType.includes("gzip") ||
    mediaType.includes("tar") ||
    mediaType.includes("compressed")
  ) {
    return <FileArchive {...iconProps} />;
  }
  if (
    mediaType.includes("javascript") ||
    mediaType.includes("typescript") ||
    mediaType.includes("html") ||
    mediaType.includes("css") ||
    mediaType.includes("xml") ||
    artifactKind.includes("code")
  ) {
    return <Braces {...iconProps} />;
  }
  if (mediaType.startsWith("text/") || mediaType.includes("pdf") || textLikeArtifactKind(artifactKind)) {
    return <FileText {...iconProps} />;
  }
  if (item.storage_kind === "external_url" || item.external_url) {
    return <Link2 {...iconProps} />;
  }
  return <File {...iconProps} />;
}

function textLikeArtifactKind(artifactKind: string): boolean {
  return ["document", "note", "report", "markdown", "text"].some((kind) => artifactKind.includes(kind));
}

function humanizeArtifactKind(kind: string): string | null {
  const trimmed = kind.trim();
  if (!trimmed) {
    return null;
  }
  return titleCase(trimmed.replace(/[_-]+/g, " "));
}

function humanizeMediaType(mediaType: string | null): string | null {
  if (!mediaType) {
    return null;
  }

  const normalized = mediaType.trim().toLowerCase();
  if (!normalized) {
    return null;
  }
  if (normalized === "text/markdown") {
    return "Markdown";
  }
  if (normalized === "text/plain") {
    return "Plain text";
  }
  if (normalized === "text/csv") {
    return "CSV";
  }
  if (normalized === "application/json") {
    return "JSON";
  }
  if (normalized === "application/pdf") {
    return "PDF";
  }

  const subtype = normalized.split("/")[1]?.split(";")[0]?.replace(/[-+._]+/g, " ");
  return subtype ? titleCase(subtype) : normalized;
}

function titleCase(value: string): string {
  return value.replace(/\b\w/g, (letter) => letter.toUpperCase());
}

function itemXStyle(...xstyle: unknown[]): ItemXStyle {
  return xstyle as unknown as ItemXStyle;
}

function iconButtonXStyle(...xstyle: unknown[]): IconButtonXStyle {
  return xstyle as unknown as IconButtonXStyle;
}

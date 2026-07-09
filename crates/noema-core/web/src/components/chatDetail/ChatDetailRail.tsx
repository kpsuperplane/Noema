import React from "react";
import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import { X } from "lucide-react";
import {
  ArtifactDetailPanel,
  ArtifactDownloadAction,
  type ArtifactDetail
} from "./ArtifactDetailPanel";
import type { ChatDetailTarget } from "./chatDetailTypes";

export function ChatDetailRail({
  target,
  onClose
}: {
  target: ChatDetailTarget;
  onClose: () => void;
}) {
  const closeButtonRef = React.useRef<HTMLButtonElement>(null);
  const [artifactDetailState, setArtifactDetailState] = React.useState<{
    version: string;
    detail: ArtifactDetail | null;
  } | null>(null);
  const artifactDetail = artifactDetailState?.version === target.version ? artifactDetailState.detail : null;
  const title = artifactDetail?.title ?? (target.type === "artifact" ? "Artifact" : "Details");

  React.useEffect(() => {
    closeButtonRef.current?.focus();
  }, [target]);

  const updateArtifactDetail = React.useCallback(
    (detail: ArtifactDetail | null) => {
      setArtifactDetailState({ version: target.version, detail });
    },
    [target.version]
  );

  return (
    <aside data-slot="chat-detail-rail" aria-label="Chat detail" {...stylex.props(styles.rail)}>
      <header {...stylex.props(styles.header)}>
        <div {...stylex.props(styles.titleBlock)}>
          <div {...stylex.props(styles.kicker)}>Detail</div>
          <h2 {...stylex.props(styles.title)}>{title}</h2>
        </div>
        {target.type === "artifact" ? <ArtifactDownloadAction detail={artifactDetail} /> : null}
        <Button
          ref={closeButtonRef}
          type="button"
          variant="ghost"
          size="sm"
          label="Close detail"
          icon={<X aria-hidden="true" size={16} />}
          isIconOnly
          onClick={onClose}
        />
      </header>
      <div {...stylex.props(styles.body)}>
        {target.type === "artifact" ? (
          <ArtifactDetailPanel version={target.version} onDetailChange={updateArtifactDetail} />
        ) : null}
      </div>
    </aside>
  );
}

const styles = stylex.create({
  rail: {
    display: "grid",
    gridTemplateRows: "auto minmax(0, 1fr)",
    minWidth: 0,
    height: "100%",
    borderLeftWidth: 1,
    borderLeftStyle: "solid",
    borderLeftColor: "var(--noema-border-subtle)",
    backgroundColor: "var(--noema-surface-card)"
  },
  header: {
    minWidth: 0,
    display: "grid",
    gridTemplateColumns: "minmax(0, 1fr) auto auto",
    alignItems: "start",
    gap: 12,
    paddingBlock: 14,
    paddingInline: 16,
    borderBottomWidth: 1,
    borderBottomStyle: "solid",
    borderBottomColor: "var(--noema-border-subtle)"
  },
  titleBlock: {
    minWidth: 0,
    display: "grid",
    gap: 2
  },
  kicker: {
    color: "var(--noema-text-muted)",
    fontSize: 11,
    fontWeight: 600,
    textTransform: "uppercase"
  },
  title: {
    margin: 0,
    color: "var(--noema-text-primary)",
    fontSize: 15,
    fontWeight: 650,
    lineHeight: 1.25,
    overflowWrap: "anywhere"
  },
  body: {
    minHeight: 0,
    overflow: "auto",
    padding: 16
  }
});

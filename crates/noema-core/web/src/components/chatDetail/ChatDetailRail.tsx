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
  motionState,
  onClose,
  onMotionEnd
}: {
  target: ChatDetailTarget;
  motionState: "entering" | "open" | "exiting";
  onClose: () => void;
  onMotionEnd: () => void;
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

  const handleAnimationEnd = React.useCallback(
    (event: React.AnimationEvent<HTMLElement>) => {
      if (event.currentTarget === event.target) {
        onMotionEnd();
      }
    },
    [onMotionEnd]
  );

  const updateArtifactDetail = React.useCallback(
    (detail: ArtifactDetail | null) => {
      setArtifactDetailState({ version: target.version, detail });
    },
    [target.version]
  );

  return (
    <aside
      data-slot="chat-detail-rail"
      data-state={motionState}
      aria-label="Chat detail"
      {...stylex.props(styles.rail)}
    >
      <div
        data-slot="chat-detail-rail-surface"
        data-state={motionState}
        {...stylex.props(styles.surface)}
        onAnimationEnd={handleAnimationEnd}
      >
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
      </div>
    </aside>
  );
}

const styles = stylex.create({
  rail: {
    position: {
      default: "absolute",
      "@media (min-width: 980px)": "relative"
    },
    inset: {
      default: 0,
      "@media (min-width: 980px)": "auto"
    },
    zIndex: 4,
    minWidth: 0,
    height: "100%",
    overflow: "hidden"
  },
  surface: {
    display: "grid",
    gridTemplateRows: "auto minmax(0, 1fr)",
    minWidth: 0,
    height: "100%",
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

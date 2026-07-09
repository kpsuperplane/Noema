import React from "react";
import { Button } from "@astryxdesign/core/Button";
import { Selector } from "@astryxdesign/core/Selector";
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
  onChangeVersion,
  onClose,
  onMotionEnd
}: {
  target: ChatDetailTarget;
  motionState: "opening" | "entering" | "open" | "exiting";
  onChangeVersion: (version: string) => void;
  onClose: () => void;
  onMotionEnd: () => void;
}) {
  const closeButtonRef = React.useRef<HTMLButtonElement>(null);
  const [artifactDetailState, setArtifactDetailState] = React.useState<{
    version: string;
    detail: ArtifactDetail | null;
    latestDetail: ArtifactDetail | null;
  } | null>(null);
  const artifactDetail = artifactDetailState?.version === target.version ? artifactDetailState.detail : null;
  const latestArtifactDetail = artifactDetailState?.latestDetail ?? null;
  const title = artifactDetail?.title ?? latestArtifactDetail?.title ?? (target.type === "artifact" ? "Artifact" : "Details");

  React.useEffect(() => {
    if (motionState === "open") {
      closeButtonRef.current?.focus();
    }
  }, [motionState, target]);

  const handleTransitionEnd = React.useCallback(
    (event: React.TransitionEvent<HTMLElement>) => {
      if (event.currentTarget === event.target && event.propertyName === "transform") {
        onMotionEnd();
      }
    },
    [onMotionEnd]
  );

  const updateArtifactDetail = React.useCallback(
    (detail: ArtifactDetail | null) => {
      setArtifactDetailState((previous) => ({
        version: target.version,
        detail,
        latestDetail: detail ?? previous?.latestDetail ?? null
      }));
    },
    [target.version]
  );

  return (
    <aside
      data-slot="chat-detail-rail"
      data-state={motionState}
      aria-label="Chat detail"
      {...stylex.props(styles.rail)}
      onTransitionEnd={handleTransitionEnd}
    >
      <div
        data-slot="chat-detail-rail-surface"
        data-state={motionState}
        {...stylex.props(styles.surface)}
      >
        <header {...stylex.props(styles.header)}>
          <div {...stylex.props(styles.actionRow)}>
            <div {...stylex.props(styles.versionSlot)}>
              {target.type === "artifact" ? (
                <ArtifactVersionSelector
                  detail={artifactDetail ?? latestArtifactDetail}
                  selectedVersion={target.version}
                  onChangeVersion={onChangeVersion}
                />
              ) : null}
            </div>
            <div {...stylex.props(styles.actions)}>
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
            </div>
          </div>
          <h2 {...stylex.props(styles.title)}>{title}</h2>
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

function ArtifactVersionSelector({
  detail,
  selectedVersion,
  onChangeVersion
}: {
  detail: ArtifactDetail | null;
  selectedVersion: string;
  onChangeVersion: (version: string) => void;
}) {
  if (!detail || detail.versions.length === 0) {
    return null;
  }

  const options = detail.versions.map((version) => ({
    value: version.artifactVersionId,
    label: `Version ${version.versionIndex}`
  }));

  return (
    <div {...stylex.props(styles.versionSelector)}>
      <Selector
        isDisabled={detail.versions.length <= 1}
        isLabelHidden
        label="Artifact version"
        onChange={onChangeVersion}
        options={options}
        placement="below"
        size="sm"
        value={selectedVersion}
        width={128}
      />
    </div>
  );
}

const styles = stylex.create({
  rail: {
    position: {
      default: "absolute",
      "@media (min-width: 980px)": "absolute"
    },
    inset: {
      default: 0,
      "@media (min-width: 980px)": "auto"
    },
    top: {
      default: "auto",
      "@media (min-width: 980px)": 0
    },
    right: {
      default: "auto",
      "@media (min-width: 980px)": 0
    },
    bottom: {
      default: "auto",
      "@media (min-width: 980px)": 0
    },
    zIndex: 4,
    minWidth: 0,
    width: {
      default: "auto",
      "@media (min-width: 980px)": "var(--chat-detail-rail-width)"
    },
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
    gap: 10,
    paddingBlock: 14,
    paddingInline: 16,
    borderBottomWidth: 1,
    borderBottomStyle: "solid",
    borderBottomColor: "var(--noema-border-subtle)"
  },
  actionRow: {
    minWidth: 0,
    display: "grid",
    gridTemplateColumns: "minmax(0, 1fr) auto",
    alignItems: "start",
    gap: 12
  },
  versionSlot: {
    minWidth: 0
  },
  actions: {
    display: "flex",
    alignItems: "start",
    gap: 8
  },
  title: {
    margin: 0,
    color: "var(--noema-text-primary)",
    fontSize: 15,
    fontWeight: 650,
    lineHeight: 1.25,
    overflowWrap: "anywhere"
  },
  versionSelector: {
    alignSelf: "start",
    width: 128
  },
  body: {
    minHeight: 0,
    overflow: "auto",
    padding: 16
  }
});

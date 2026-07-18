import { Selector } from "@astryxdesign/core/Selector";
import * as stylex from "@stylexjs/stylex";
import type { ArtifactDetail } from "./ArtifactDetailPanel";

export function ArtifactVersionSelector({ detail, selectedVersion, onChangeVersion }: { detail: ArtifactDetail | null; selectedVersion: string; onChangeVersion: (version: string) => void }) {
  if (!detail || detail.versions.length === 0) return null;
  const options = detail.versions.map((version) => ({ value: version.artifactVersionId, label: `Version ${version.versionIndex}` }));
  return <div {...stylex.props(styles.root)}><Selector isDisabled={detail.versions.length <= 1} isLabelHidden label="Artifact version" onChange={onChangeVersion} options={options} placement="below" size="sm" value={selectedVersion} width={128} /></div>;
}

const styles = stylex.create({ root: { alignSelf: "start", width: 128 } });

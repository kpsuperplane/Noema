import { Selector } from "@astryxdesign/core/Selector";
import * as stylex from "@stylexjs/stylex";

export function ArtifactVersionSelector({
  versions,
  selectedVersion,
  onChangeVersion
}: {
  versions: readonly { artifactVersionId: string; versionIndex: number }[];
  selectedVersion: string;
  onChangeVersion: (version: string) => void;
}) {
  if (versions.length === 0) return null;
  const options = versions.map((version) => ({
    value: version.artifactVersionId,
    label: `Version ${version.versionIndex}`
  }));
  return (
    <div {...stylex.props(styles.root)}>
      <Selector
        isDisabled={versions.length <= 1}
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

const styles = stylex.create({ root: { alignSelf: "center", width: 128 } });

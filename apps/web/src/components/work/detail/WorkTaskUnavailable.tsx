import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";

export function WorkTaskUnavailable({ compact, onBack }: { compact: boolean; onBack?: () => void }) {
  return <div {...stylex.props(styles.root, compact && styles.compact)}><h1 {...stylex.props(styles.title)}>Task unavailable</h1><p {...stylex.props(styles.text)}>This task may have been removed or isn’t available to this workspace.</p>{onBack ? <Button size="sm" variant="secondary" label="Back to Work" onClick={onBack} /> : null}</div>;
}

const styles = stylex.create({
  root: { display: "grid", alignContent: "center", justifyItems: "start", gap: 8, minHeight: "100%", padding: 24 }, title: { margin: 0, fontFamily: "var(--font-heading)", fontSize: 24 }, text: { margin: 0, maxWidth: 480, color: "var(--muted-foreground)" },
  compact: { paddingBlock: 8, paddingInline: 8 }
});

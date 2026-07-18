import * as stylex from "@stylexjs/stylex";

export function WorkTaskDetailSkeleton({ compact }: { compact: boolean }) {
  return <div role="status" aria-label="Loading task details" {...stylex.props(styles.root, compact && styles.compact)}><span {...stylex.props(styles.first)} /><span {...stylex.props(styles.row)} /><span {...stylex.props(styles.row)} /><span {...stylex.props(styles.row)} /></div>;
}

const styles = stylex.create({
  root: { display: "grid", gap: 12, padding: 24 }, row: { height: 110, borderRadius: 14, backgroundColor: "var(--paper-100)" }, first: { height: 180, borderRadius: 14, backgroundColor: "var(--paper-100)" },
  compact: { paddingBlock: 8, paddingInline: 8 }
});

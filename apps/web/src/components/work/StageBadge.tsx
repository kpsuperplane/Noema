import * as stylex from "@stylexjs/stylex";
import { RollingText } from "@/components/RollingText";

export function StageBadge({ name }: { name: string; behavior?: string }) {
  return <RollingText value={name} {...stylex.props(styles.label)} />;
}

const styles = stylex.create({
  label: { color: "var(--noema-text-secondary)", fontSize: 10, fontWeight: 650 }
});

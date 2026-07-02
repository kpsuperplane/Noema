import { Badge } from "@astryxdesign/core/Badge";
import * as stylex from "@stylexjs/stylex";
import type { ProviderAccountStatus } from "./types";
import { statusCopy } from "./statusCopy";

export function ProviderStatus({ status }: { status: ProviderAccountStatus }) {
  return (
    <Badge
      {...stylex.props(styles.badge)}
      variant="neutral"
      label={`Provider status: ${statusCopy[status]}`}
    />
  );
}

const styles = stylex.create({
  badge: {
    width: "fit-content",
    fontFamily: "var(--font-mono)",
    fontSize: 12,
    color: "var(--muted-foreground)"
  }
});

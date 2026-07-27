import type { ReactNode } from "react";
import { Link } from "@tanstack/react-router";
import * as stylex from "@stylexjs/stylex";
import { CapabilityConnectionDetail } from "./CapabilityConnectionDetail";

export function CapabilityManagementLayout({
  kind,
  connectionId,
  list,
  sourceActions,
  dangerAction
}: {
  kind: "API" | "MCP";
  connectionId?: string;
  list: ReactNode;
  sourceActions?: ReactNode;
  dangerAction?: ReactNode;
}) {
  const hasDetail = connectionId !== undefined;

  return (
    <div {...stylex.props(styles.layout, hasDetail && styles.split)}>
      <div {...stylex.props(styles.list, hasDetail && styles.listBehindDetail)}>{list}</div>
      {connectionId ? (
        <aside aria-label="Manage connection" {...stylex.props(styles.detail)}>
          <Link
            to={kind === "API" ? "/settings/tools/apis" : "/settings/tools/mcps"}
            {...stylex.props(styles.backLink)}
          >
            Back to accounts
          </Link>
          <CapabilityConnectionDetail
            kind={kind}
            connectionId={connectionId}
            sourceActions={sourceActions}
            dangerAction={dangerAction}
          />
        </aside>
      ) : null}
    </div>
  );
}

const styles = stylex.create({
  layout: { display: "grid", minWidth: 0 },
  split: {
    "@media (min-width: 980px)": {
      gridTemplateColumns: "minmax(280px, 0.8fr) minmax(0, 1.2fr)"
    }
  },
  list: { minWidth: 0 },
  listBehindDetail: {
    "@media (max-width: 979px)": { display: "none" }
  },
  detail: {
    display: "grid",
    minWidth: 0,
    gap: "var(--spacing-3)",
    "@media (min-width: 980px)": {
      paddingLeft: "var(--spacing-4)",
      borderLeftWidth: 1,
      borderLeftStyle: "solid",
      borderLeftColor: "var(--border-subtle)"
    }
  },
  backLink: {
    width: "fit-content",
    color: "var(--text-accent)",
    fontSize: 13,
    fontWeight: 600,
    textDecoration: "none",
    "@media (min-width: 980px)": { display: "none" }
  }
});

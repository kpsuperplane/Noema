import type { ReactNode } from "react";
import { Link } from "@tanstack/react-router";
import * as stylex from "@stylexjs/stylex";
import { MasterDetailLayout } from "@/components/shell/MasterDetailLayout";
import { CapabilityConnectionDetail } from "./CapabilityConnectionDetail";

export function CapabilityManagementLayout({
  kind,
  connectionId,
  list,
  sourceActions,
  definitionDetails,
  dangerAction
}: {
  kind: "API" | "MCP";
  connectionId?: string;
  list: ReactNode;
  sourceActions?: ReactNode;
  definitionDetails?: ReactNode;
  dangerAction?: ReactNode;
}) {
  const hasDetail = connectionId !== undefined;

  return (
    <MasterDetailLayout
      detailOpen={hasDetail}
      detailLabel="Manage connection"
      list={
        <div {...stylex.props(styles.scroller)}>
          <div {...stylex.props(styles.listContent)}>{list}</div>
        </div>
      }
      detail={connectionId ? (
        <div {...stylex.props(styles.scroller)}>
          <div {...stylex.props(styles.detailContent)}>
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
              definitionDetails={definitionDetails}
              dangerAction={dangerAction}
            />
          </div>
        </div>
      ) : null}
    />
  );
}

const styles = stylex.create({
  scroller: {
    height: "100%",
    minHeight: 0,
    overflowY: "auto",
    overflowX: "hidden",
    overscrollBehavior: "contain",
    scrollbarWidth: "thin"
  },
  listContent: {
    display: "grid",
    minWidth: 0,
    padding: "var(--spacing-4)",
    "@media (max-width: 760px)": {
      padding: "var(--spacing-3)"
    }
  },
  detailContent: {
    display: "grid",
    minWidth: 0,
    gap: "var(--spacing-3)",
    padding: "var(--spacing-4)",
    "@media (max-width: 760px)": {
      padding: "var(--spacing-3)"
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

import type { ReactNode } from "react";
import * as stylex from "@stylexjs/stylex";

export function MasterDetailLayout({
  list,
  detail,
  detailOpen,
  detailLabel
}: {
  list: ReactNode;
  detail?: ReactNode;
  detailOpen: boolean;
  detailLabel: string;
}) {
  return (
    <div data-slot="master-detail-layout" {...stylex.props(styles.layout)}>
      <div
        data-slot="master-detail-list"
        {...stylex.props(styles.listPane, detailOpen && styles.listPaneDetailOpen)}
      >
        {list}
      </div>
      <div
        data-slot="master-detail-detail"
        role="region"
        aria-label={detailLabel}
        {...stylex.props(styles.detailPane, detailOpen && styles.detailPaneOpen)}
      >
        {detail}
      </div>
    </div>
  );
}

const styles = stylex.create({
  layout: {
    display: "grid",
    position: "relative",
    gridTemplateColumns: "minmax(0, 1fr)",
    height: "100%",
    minHeight: 0,
    overflow: "hidden",
    backgroundColor: "var(--noema-surface-card)",
    "@media (min-width: 980px)": {
      gridTemplateColumns: "minmax(280px, 360px) minmax(0, 1fr)"
    }
  },
  listPane: {
    display: "flex",
    gridColumn: 1,
    minWidth: 0,
    minHeight: 0,
    flexDirection: "column",
    overflow: "hidden"
  },
  listPaneDetailOpen: {
    "@media (max-width: 979px)": {
      visibility: "hidden",
      pointerEvents: "none"
    }
  },
  detailPane: {
    display: "none",
    position: "relative",
    minWidth: 0,
    minHeight: 0,
    borderLeftWidth: 1,
    borderLeftStyle: "solid",
    borderLeftColor: "var(--noema-border-subtle)",
    "@media (min-width: 980px)": {
      display: "block",
      gridColumn: 2
    }
  },
  detailPaneOpen: {
    "@media (max-width: 979px)": {
      display: "block",
      position: "absolute",
      inset: 0,
      zIndex: 4,
      backgroundColor: "var(--noema-surface-card)"
    }
  }
});

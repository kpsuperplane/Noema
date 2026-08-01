import { createContext, useContext, type ReactNode } from "react";
import { useMediaQuery } from "@astryxdesign/core/hooks";
import * as stylex from "@stylexjs/stylex";
import {
  MobileDrawer,
  useLatchedDrawerPresentation
} from "@/components/MobileDrawer";

type DetailPanePresentation = "pane" | "drawer";
const DetailPanePresentationContext = createContext<DetailPanePresentation>("pane");

export function DetailPanePresentationProvider({
  presentation,
  children
}: {
  presentation: DetailPanePresentation;
  children: ReactNode;
}) {
  return (
    <DetailPanePresentationContext value={presentation}>
      {children}
    </DetailPanePresentationContext>
  );
}

export function useDetailPanePresentation(): DetailPanePresentation {
  return useContext(DetailPanePresentationContext);
}

export function MasterDetailLayout({
  list,
  detail,
  detailOpen,
  detailLabel,
  onDetailOpenChange
}: {
  list: ReactNode;
  detail?: ReactNode;
  detailOpen: boolean;
  detailLabel: string;
  onDetailOpenChange: (open: boolean) => void;
}) {
  const drawerViewport = useMediaQuery("(max-width: 979px)");
  const useDrawer = useLatchedDrawerPresentation(detailOpen, drawerViewport);

  return (
    <div data-slot="master-detail-layout" {...stylex.props(styles.layout)}>
      <div data-slot="master-detail-list" {...stylex.props(styles.listPane)}>
        {list}
      </div>
      <div
        data-slot="master-detail-detail"
        role="region"
        aria-label={detailLabel}
        {...stylex.props(
          styles.detailPane,
          detailOpen && !useDrawer && styles.detailPaneOpen
        )}
      >
        {useDrawer ? null : (
          <DetailPanePresentationProvider presentation="pane">
            {detail}
          </DetailPanePresentationProvider>
        )}
      </div>
      {useDrawer ? (
        <MobileDrawer
          isOpen={detailOpen}
          onOpenChange={onDetailOpenChange}
          label={detailLabel}
          height="calc(100dvh - var(--spacing-6))"
        >
          <DetailPanePresentationProvider presentation="drawer">
            {detail}
          </DetailPanePresentationProvider>
        </MobileDrawer>
      ) : null}
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

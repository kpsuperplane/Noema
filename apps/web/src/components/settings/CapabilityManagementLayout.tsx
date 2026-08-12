import type { ReactNode } from "react";
import { useEffect } from "react";
import { Link, useNavigate } from "@tanstack/react-router";
import { Button } from "@astryxdesign/core/Button";
import { useMediaQuery } from "@astryxdesign/core/hooks";
import { IconButton } from "@astryxdesign/core/IconButton";
import { VStack } from "@astryxdesign/core/VStack";
import { Plus } from "lucide-react";
import * as stylex from "@stylexjs/stylex";
import {
  MasterDetailLayout,
  useDetailPanePresentation
} from "@/components/shell/MasterDetailLayout";
import { ShellSectionHeader } from "@/components/shell/ShellSectionHeader";
import { CapabilityConnectionDetail } from "./CapabilityConnectionDetail";

export function CapabilityManagementLayout({
  kind,
  connectionId,
  defaultConnectionId,
  title,
  primaryAction,
  list,
  serviceName,
  connectionName,
  sourceActions,
  definitionDetails,
  dangerAction
}: {
  kind: "API" | "MCP";
  connectionId?: string;
  defaultConnectionId?: string;
  title: string;
  primaryAction?: { label: string; onClick: () => void };
  list: ReactNode;
  serviceName?: string;
  connectionName?: string;
  sourceActions?: ReactNode;
  definitionDetails?: ReactNode;
  dangerAction?: ReactNode;
}) {
  const hasDetail = connectionId !== undefined;
  const navigate = useNavigate();
  const desktop = useMediaQuery("(min-width: 980px)");
  const listRoute = kind === "API" ? "/settings/tools/apis" : "/settings/tools/mcps";

  useEffect(() => {
    if (!desktop || connectionId || !defaultConnectionId) return;
    void navigate({
      to: kind === "API"
        ? "/settings/tools/apis/$connectionId"
        : "/settings/tools/mcps/$connectionId",
      params: { connectionId: defaultConnectionId },
      replace: true
    });
  }, [connectionId, defaultConnectionId, desktop, kind, navigate]);

  return (
    <MasterDetailLayout
      detailOpen={hasDetail}
      detailLabel="Manage connection"
      onDetailOpenChange={(open) => {
        if (!open) void navigate({ to: listRoute });
      }}
      list={
        <VStack {...stylex.props(styles.scroller)}>
          <IntegrationToolbar title={title} action={primaryAction} />
          <VStack {...stylex.props(styles.listContent)}>{list}</VStack>
        </VStack>
      }
      detail={connectionId ? (
        <VStack {...stylex.props(styles.scroller)}>
          <CapabilityDetailContent listRoute={listRoute}>
            <CapabilityConnectionDetail
              kind={kind}
              connectionId={connectionId}
              serviceName={serviceName}
              connectionName={connectionName}
              sourceActions={sourceActions}
              definitionDetails={definitionDetails}
              dangerAction={dangerAction}
            />
          </CapabilityDetailContent>
        </VStack>
      ) : null}
    />
  );
}

function CapabilityDetailContent({
  listRoute,
  children
}: {
  listRoute: "/settings/tools/apis" | "/settings/tools/mcps";
  children: ReactNode;
}) {
  const isDrawer = useDetailPanePresentation() === "drawer";
  return (
    <VStack gap={3} {...stylex.props(styles.detailContent, isDrawer && styles.drawerDetailContent)}>
      {!isDrawer ? (
        <Link to={listRoute} {...stylex.props(styles.backLink)}>Back to connections</Link>
      ) : null}
      {children}
    </VStack>
  );
}

function IntegrationToolbar({
  title,
  action
}: {
  title: string;
  action?: { label: string; onClick: () => void };
}) {
  return (
    <header {...stylex.props(styles.toolbar)}>
      <ShellSectionHeader title={title} titleId="settings-surface-title" />
      {action ? <>
        <Button
          type="button"
          size="sm"
          variant="primary"
          label={action.label}
          icon={<Plus aria-hidden="true" size={15} />}
          xstyle={styles.desktopAction}
          onClick={action.onClick}
        />
        <IconButton
          label={action.label}
          tooltip={action.label}
          size="lg"
          variant="primary"
          icon={<Plus aria-hidden="true" size={20} />}
          xstyle={styles.mobileAction}
          onClick={action.onClick}
        />
      </> : null}
    </header>
  );
}

const styles = stylex.create({
  scroller: {
    width: "100%",
    height: "100%",
    minHeight: 0,
    overflowY: "auto",
    overflowX: "hidden",
    overscrollBehavior: "contain",
    scrollbarWidth: "thin"
  },
  listContent: {
    minWidth: 0,
    padding: "var(--spacing-4)",
    "@media (max-width: 760px)": {
      padding: "var(--spacing-3)"
    }
  },
  toolbar: { position: "relative", flexShrink: 0 },
  desktopAction: {
    position: "absolute",
    top: "var(--spacing-3)",
    right: "var(--spacing-4)",
    zIndex: 4,
    "@media (max-width: 760px)": { display: "none" }
  },
  mobileAction: {
    display: "none",
    "@media (max-width: 760px)": {
      display: "inline-flex",
      position: "fixed",
      right: "max(var(--spacing-4), env(safe-area-inset-right))",
      bottom: "max(var(--spacing-4), env(safe-area-inset-bottom))",
      zIndex: 5,
      width: "var(--spacing-12)",
      height: "var(--spacing-12)",
      "--_button-radius": "var(--radius-full)",
      boxShadow: "var(--shadow-med)"
    }
  },
  detailContent: {
    minWidth: 0,
    padding: "var(--spacing-4)"
  },
  drawerDetailContent: { padding: "var(--spacing-0)" },
  backLink: {
    width: "fit-content",
    color: "var(--text-accent)",
    fontSize: 13,
    fontWeight: 600,
    textDecoration: "none",
    "@media (min-width: 980px)": { display: "none" }
  }
});

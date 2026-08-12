import type { ReactNode } from "react";
import { Link, useNavigate } from "@tanstack/react-router";
import { VStack } from "@astryxdesign/core/VStack";
import * as stylex from "@stylexjs/stylex";
import { MasterDetailLayout } from "@/components/shell/MasterDetailLayout";
import { CapabilityConnectionDetail } from "./CapabilityConnectionDetail";

export function CapabilityManagementLayout({
  kind,
  connectionId,
  list,
  startPolicyEditing,
  sourceActions,
  definitionDetails,
  dangerAction
}: {
  kind: "API" | "MCP";
  connectionId?: string;
  list: ReactNode;
  startPolicyEditing?: boolean;
  sourceActions?: ReactNode;
  definitionDetails?: ReactNode;
  dangerAction?: ReactNode;
}) {
  const hasDetail = connectionId !== undefined;
  const navigate = useNavigate();
  const listRoute = kind === "API" ? "/settings/tools/apis" : "/settings/tools/mcps";

  return (
    <MasterDetailLayout
      detailOpen={hasDetail}
      detailLabel="Manage connection"
      onDetailOpenChange={(open) => {
        if (!open) void navigate({ to: listRoute });
      }}
      list={
        <VStack {...stylex.props(styles.scroller)}>
          <VStack {...stylex.props(styles.listContent)}>{list}</VStack>
        </VStack>
      }
      detail={connectionId ? (
        <VStack {...stylex.props(styles.scroller)}>
          <VStack gap={3} {...stylex.props(styles.detailContent)}>
            <Link
              to={listRoute}
              {...stylex.props(styles.backLink)}
            >
              Back to connections
            </Link>
            <CapabilityConnectionDetail
              kind={kind}
              connectionId={connectionId}
              startPolicyEditing={startPolicyEditing}
              sourceActions={sourceActions}
              definitionDetails={definitionDetails}
              dangerAction={dangerAction}
            />
          </VStack>
        </VStack>
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
    minWidth: 0,
    padding: "var(--spacing-4)",
    "@media (max-width: 760px)": {
      padding: "var(--spacing-3)"
    }
  },
  detailContent: {
    minWidth: 0,
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

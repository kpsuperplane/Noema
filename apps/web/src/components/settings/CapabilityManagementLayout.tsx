import type { ReactNode } from "react";
import { useEffect } from "react";
import { useNavigate } from "@tanstack/react-router";
import { useMediaQuery } from "@astryxdesign/core/hooks";
import { CapabilityConnectionDetail } from "./CapabilityConnectionDetail";
import { SettingsManagementLayout } from "./SettingsManagementLayout";

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
    <SettingsManagementLayout
      title={title}
      primaryAction={primaryAction}
      detailOpen={hasDetail}
      detailLabel="Manage connection"
      onDetailOpenChange={(open) => {
        if (!open) void navigate({ to: listRoute });
      }}
      list={list}
      detail={connectionId ? (
        <CapabilityConnectionDetail
          kind={kind}
          connectionId={connectionId}
          serviceName={serviceName}
          connectionName={connectionName}
          sourceActions={sourceActions}
          definitionDetails={definitionDetails}
          dangerAction={dangerAction}
        />
      ) : null}
    />
  );
}

import { Link } from "@tanstack/react-router";
import { Badge } from "@astryxdesign/core/Badge";
import { Button } from "@astryxdesign/core/Button";
import { HStack } from "@astryxdesign/core/HStack";
import { VStack } from "@astryxdesign/core/VStack";
import * as stylex from "@stylexjs/stylex";
import type { ReactNode } from "react";
import type { CapabilityIntegrationsQuery } from "@/generated/graphql";

export type CapabilityIntegration = CapabilityIntegrationsQuery["capabilityIntegrations"][number];

export function CapabilityIntegrationList({
  integrations,
  kind,
  selectedConnectionId,
  emptyMessage,
  primaryAction,
  integrationAction,
  isAddingConnection = false,
  onAddConnection
}: {
  integrations: readonly CapabilityIntegration[];
  kind: "API" | "MCP";
  selectedConnectionId?: string;
  emptyMessage: string;
  primaryAction?: { label: string; onClick: () => void };
  integrationAction?: (integration: CapabilityIntegration) => ReactNode;
  isAddingConnection?: boolean;
  onAddConnection: (integration: CapabilityIntegration) => void;
}) {
  return (
    <VStack gap={3}>
      {primaryAction ? (
        <HStack hAlign="end">
          <Button
            type="button"
            size="sm"
            variant="secondary"
            label={primaryAction.label}
            onClick={primaryAction.onClick}
          />
        </HStack>
      ) : null}
      {integrations.length === 0 ? (
        <VStack as="section" {...stylex.props(styles.group)}>
          <p {...stylex.props(styles.empty)}>{emptyMessage}</p>
        </VStack>
      ) : null}
      {integrations.map((integration) => (
        <VStack as="section" key={integration.definitionId} {...stylex.props(styles.group)}>
          <HStack wrap="wrap" gap={2} vAlign="center" {...stylex.props(styles.groupHeader)}>
            <div {...stylex.props(styles.groupCopy)}>
              <h2 {...stylex.props(styles.title)}>{integration.name}</h2>
              <p {...stylex.props(styles.summary)}>{integration.sourceSummary}</p>
            </div>
            <Badge
              variant="neutral"
              label={`${integration.connections.length} ${integration.connections.length === 1 ? "connection" : "connections"}`}
            />
            <Button
              type="button"
              size="sm"
              variant="secondary"
              label="Add connection"
              isLoading={isAddingConnection}
              onClick={() => onAddConnection(integration)}
            />
            {integrationAction?.(integration)}
          </HStack>
          {integration.connections.length === 0 ? (
            <p {...stylex.props(styles.empty)}>No connection added to this definition.</p>
          ) : (
            <VStack gap={0}>
              {integration.connections.map((connection) => (
                <Link
                  key={connection.connectionId}
                  to={
                    kind === "API"
                      ? "/settings/tools/apis/$connectionId"
                      : "/settings/tools/mcps/$connectionId"
                  }
                  params={{ connectionId: connection.connectionId }}
                  aria-current={selectedConnectionId === connection.connectionId ? "page" : undefined}
                  {...stylex.props(
                    styles.connection,
                    selectedConnectionId === connection.connectionId && styles.selectedConnection
                  )}
                >
                  <VStack as="span" gap={0.5} {...stylex.props(styles.connectionCopy)}>
                    <strong {...stylex.props(styles.connectionName)}>{connection.name}</strong>
                    <span {...stylex.props(styles.meta)}>
                      {connection.authStatus} · {connection.availableToolCount}/{connection.toolCount} tools
                    </span>
                  </VStack>
                  <span {...stylex.props(styles.manageLabel)}>Manage</span>
                </Link>
              ))}
            </VStack>
          )}
        </VStack>
      ))}
    </VStack>
  );
}

const styles = stylex.create({
  group: {
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border-subtle)",
    borderRadius: 6,
    backgroundColor: "var(--surface-raised)",
    overflow: "hidden"
  },
  groupHeader: {
    padding: "var(--spacing-3)",
    borderBottomWidth: 1,
    borderBottomStyle: "solid",
    borderBottomColor: "var(--border-subtle)"
  },
  groupCopy: { flex: "1 1 16rem", minWidth: 0 },
  title: { margin: "var(--spacing-0)", fontFamily: "var(--font-heading)", fontSize: 16, fontWeight: 600 },
  summary: { margin: "var(--spacing-1) 0 0", color: "var(--muted-foreground)", fontSize: 12 },
  connection: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    gap: "var(--spacing-2)",
    padding: "var(--spacing-2) var(--spacing-3)",
    borderBottomWidth: 1,
    borderBottomStyle: "solid",
    borderBottomColor: "var(--border-subtle)",
    color: "inherit",
    textDecoration: "none",
    cursor: "pointer",
    backgroundColor: {
      default: "transparent",
      ":hover": "var(--surface-hover)"
    },
    outline: {
      default: "none",
      ":focus-visible": "2px solid var(--text-accent)"
    },
    outlineOffset: -2,
    ":last-child": { borderBottomWidth: 0 },
    "@media (max-width: 640px)": { alignItems: "flex-start", flexDirection: "column" }
  },
  selectedConnection: { backgroundColor: "var(--surface-hover)" },
  connectionCopy: { minWidth: 0 },
  connectionName: { fontSize: 14, overflowWrap: "anywhere" },
  meta: { color: "var(--muted-foreground)", fontSize: 12 },
  manageLabel: {
    color: "var(--text-accent)",
    fontSize: 13,
    fontWeight: 600,
    flexShrink: 0
  },
  empty: { margin: "var(--spacing-0)", padding: "var(--spacing-3)", color: "var(--muted-foreground)", fontSize: 13 }
});

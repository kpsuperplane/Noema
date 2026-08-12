import { Link } from "@tanstack/react-router";
import { Button } from "@astryxdesign/core/Button";
import { HStack } from "@astryxdesign/core/HStack";
import { VStack } from "@astryxdesign/core/VStack";
import { Braces, Plug } from "lucide-react";
import * as stylex from "@stylexjs/stylex";
import type { ReactNode } from "react";
import type { CapabilityIntegrationsQuery } from "@/generated/graphql";
import { SettingsList, SettingsListItem, SettingsSection } from "./SettingsPrimitives";
import { settingsStatusLabel } from "./settingsStatus";

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
  onAddConnection?: (integration: CapabilityIntegration) => void;
}) {
  const IntegrationIcon = kind === "API" ? Braces : Plug;
  const connectionNoun = kind === "API" ? "account" : "connection";
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
        <p {...stylex.props(styles.empty)}>{emptyMessage}</p>
      ) : null}
      {integrations.map((integration) => (
        <SettingsSection
          key={integration.definitionId}
          aria-labelledby={`capability-${integration.definitionId}-title`}
        >
          <VStack gap={2}>
            <HStack wrap="wrap" gap={2} vAlign="center" hAlign="between">
              <HStack gap={2} vAlign="center" {...stylex.props(styles.groupCopy)}>
                <HStack vAlign="center" hAlign="center" {...stylex.props(styles.iconFrame)}>
                  <IntegrationIcon aria-hidden="true" {...stylex.props(styles.icon)} />
                </HStack>
                <VStack gap={0.5}>
                  <h2 id={`capability-${integration.definitionId}-title`} {...stylex.props(styles.title)}>
                    {integration.name}
                  </h2>
                  <p {...stylex.props(styles.summary)}>
                    {integration.connections.length} {integration.connections.length === 1 ? connectionNoun : `${connectionNoun}s`}
                  </p>
                </VStack>
              </HStack>
              <HStack wrap="wrap" gap={1} vAlign="center" {...stylex.props(styles.rowControl)}>
                {onAddConnection ? <Button
                  type="button"
                  size="sm"
                  variant="secondary"
                  label="Add connection"
                  isLoading={isAddingConnection}
                  onClick={() => onAddConnection(integration)}
                /> : null}
                {integrationAction?.(integration)}
              </HStack>
            </HStack>
            {integration.connections.length === 0 ? (
              <p {...stylex.props(styles.empty)}>No connection added to this definition.</p>
            ) : (
              <SettingsList density="compact" hasDividers>
                {integration.connections.map((connection) => {
                  const route = kind === "API"
                    ? "/settings/tools/apis/$connectionId"
                    : "/settings/tools/mcps/$connectionId";
                  return (
                    <SettingsListItem
                      key={connection.connectionId}
                      isSelected={selectedConnectionId === connection.connectionId}
                      label={
                        <Link
                          to={route}
                          params={{ connectionId: connection.connectionId }}
                          aria-current={selectedConnectionId === connection.connectionId ? "page" : undefined}
                          {...stylex.props(styles.connectionLink)}
                        >
                          {connection.name}
                        </Link>
                      }
                      description={connectionDescription(connection)}
                      endContent={
                        <Link
                          to={route}
                          params={{ connectionId: connection.connectionId }}
                          aria-current={selectedConnectionId === connection.connectionId ? "page" : undefined}
                          {...stylex.props(styles.manageLabel)}
                        >
                          {connectionNeedsAuthorization(connection.authStatus) ? "Authorize" : "Manage"}
                        </Link>
                      }
                    />
                  );
                })}
              </SettingsList>
            )}
          </VStack>
        </SettingsSection>
      ))}
    </VStack>
  );
}

function connectionDescription(connection: CapabilityIntegration["connections"][number]) {
  if (connectionNeedsAuthorization(connection.authStatus)) return undefined;
  if (!["active", "healthy", "ready"].includes(connection.healthStatus.toLowerCase())) {
    return settingsStatusLabel(connection.healthStatus);
  }
  if (connection.availableToolCount !== connection.toolCount) {
    return `${connection.availableToolCount} of ${connection.toolCount} tools available`;
  }
  return undefined;
}

function connectionNeedsAuthorization(authStatus: string) {
  return ["authentication_required", "needs_auth", "required"].includes(authStatus.toLowerCase());
}

const styles = stylex.create({
  groupCopy: { flex: "1 1 16rem", minWidth: 0 },
  iconFrame: {
    width: "var(--spacing-7)",
    height: "var(--spacing-7)",
    flexShrink: 0,
    borderRadius: "var(--radius-sm)",
    backgroundColor: "var(--noema-surface-subtle)",
    color: "var(--muted-foreground)"
  },
  icon: { width: "var(--spacing-4)", height: "var(--spacing-4)" },
  title: {
    margin: "var(--spacing-0)",
    fontFamily: "var(--font-heading)",
    fontSize: 16,
    fontWeight: 600,
    color: "var(--foreground)"
  },
  summary: { margin: "var(--spacing-0)", color: "var(--muted-foreground)", fontSize: 12, lineHeight: 1.5 },
  connectionLink: {
    color: "var(--foreground)",
    fontSize: 14,
    fontWeight: 650,
    textDecoration: "none",
    overflowWrap: "anywhere",
    ":hover": { color: "var(--text-accent)" }
  },
  manageLabel: {
    color: "var(--text-accent)",
    fontSize: 13,
    fontWeight: 600,
    textDecoration: "none",
    flexShrink: 0
  },
  rowControl: {
    justifyContent: "flex-end",
    "@media (max-width: 620px)": {
      width: "100%",
      justifyContent: "flex-start",
      marginInlineStart: "0"
    }
  },
  empty: { margin: "var(--spacing-0)", color: "var(--muted-foreground)", fontSize: 13, lineHeight: 1.5 }
});

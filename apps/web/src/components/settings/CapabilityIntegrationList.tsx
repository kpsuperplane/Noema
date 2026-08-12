import { Link } from "@tanstack/react-router";
import { Avatar } from "@astryxdesign/core/Avatar";
import { Button } from "@astryxdesign/core/Button";
import { HStack } from "@astryxdesign/core/HStack";
import { Section } from "@astryxdesign/core/Section";
import { VStack } from "@astryxdesign/core/VStack";
import {
  Braces,
  CalendarDays,
  ChevronRight,
  FileText,
  HardDrive,
  Mail,
  Plug,
  Wrench
} from "lucide-react";
import * as stylex from "@stylexjs/stylex";
import { useState, type ReactNode } from "react";
import type { CapabilityIntegrationsQuery } from "@/generated/graphql";
import { settingsStatusLabel } from "./settingsStatus";

export type CapabilityIntegration = CapabilityIntegrationsQuery["capabilityIntegrations"][number];

type CapabilityIntegrationListProps = {
  integrations: readonly CapabilityIntegration[];
  kind: "API" | "MCP";
  selectedConnectionId?: string;
  emptyMessage: string;
  integrationAction?: (integration: CapabilityIntegration) => ReactNode;
  isAddingConnection?: boolean;
  onAddConnection?: (integration: CapabilityIntegration) => void;
};

export function CapabilityIntegrationList(props: CapabilityIntegrationListProps) {
  return <CapabilityIntegrationListView key={props.selectedConnectionId ?? "unselected"} {...props} />;
}

function CapabilityIntegrationListView({
  integrations,
  kind,
  selectedConnectionId,
  emptyMessage,
  integrationAction,
  isAddingConnection = false,
  onAddConnection
}: CapabilityIntegrationListProps) {
  const connectionNoun = kind === "API" ? "account" : "connection";
  const [visibleSelectedConnectionId, setVisibleSelectedConnectionId] = useState(selectedConnectionId);

  return (
    <VStack gap={2}>
      {integrations.length === 0 ? (
        <p {...stylex.props(styles.empty)}>{emptyMessage}</p>
      ) : null}
      {integrations.map((integration) => {
        return (
          <Section
            key={integration.definitionId}
            variant="section"
            padding={0}
            aria-labelledby={`capability-${integration.definitionId}-title`}
            xstyle={styles.service}
          >
            <VStack gap={0}>
              <HStack gap={2} vAlign="center" {...stylex.props(styles.serviceHeader)}>
                <CapabilityIcon
                  kind={kind}
                  definitionId={integration.definitionId}
                />
                <VStack gap={0.5} {...stylex.props(styles.serviceCopy)}>
                  <h2 id={`capability-${integration.definitionId}-title`} {...stylex.props(styles.title)}>
                    {integration.name}
                  </h2>
                  <p {...stylex.props(styles.summary)}>
                    {integration.connections.length} {integration.connections.length === 1 ? connectionNoun : `${connectionNoun}s`}
                  </p>
                </VStack>
                <HStack gap={1} vAlign="center">
                  {onAddConnection ? (
                    <Button
                      type="button"
                      size="sm"
                      variant="secondary"
                      label="Add connection"
                      isLoading={isAddingConnection}
                      onClick={() => onAddConnection(integration)}
                    />
                  ) : null}
                  {integrationAction?.(integration)}
                </HStack>
              </HStack>
              {integration.connections.length === 0 ? (
                <p {...stylex.props(styles.emptyConnection)}>No connections</p>
              ) : integration.connections.map((connection) => {
                const route = kind === "API"
                  ? "/settings/tools/apis/$connectionId"
                  : "/settings/tools/mcps/$connectionId";
                const isSelected = visibleSelectedConnectionId === connection.connectionId;
                return (
                  <Link
                    key={connection.connectionId}
                    to={route}
                    params={{ connectionId: connection.connectionId }}
                    aria-current={selectedConnectionId === connection.connectionId ? "page" : undefined}
                    onClick={(event) => {
                      if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
                      setVisibleSelectedConnectionId(connection.connectionId);
                    }}
                    {...stylex.props(styles.connectionRow, isSelected && styles.selectedConnection)}
                  >
                    <Avatar name={connection.name} size="sm" tooltip={false} />
                    <VStack gap={0.5} {...stylex.props(styles.connectionCopy)}>
                      <strong {...stylex.props(styles.connectionName)}>{connection.name}</strong>
                      <HStack as="span" gap={1} vAlign="center" {...stylex.props(styles.connectionMeta)}>
                        <Wrench aria-hidden="true" {...stylex.props(styles.metaIcon)} />
                        {connectionDescription(connection)}
                      </HStack>
                    </VStack>
                    <ChevronRight aria-hidden="true" {...stylex.props(styles.chevron)} />
                  </Link>
                );
              })}
            </VStack>
          </Section>
        );
      })}
    </VStack>
  );
}

export function CapabilityIcon({
  kind,
  definitionId,
  emphasized = false
}: {
  kind: "API" | "MCP";
  definitionId: string;
  emphasized?: boolean;
}) {
  return (
    <HStack
      vAlign="center"
      hAlign="center"
      aria-hidden="true"
      {...stylex.props(styles.iconFrame, emphasized && styles.emphasizedIcon)}
    >
      <CapabilityGlyph kind={kind} definitionId={definitionId} />
    </HStack>
  );
}

function CapabilityGlyph({ kind, definitionId }: { kind: "API" | "MCP"; definitionId: string }) {
  const iconProps = stylex.props(styles.icon);
  if (kind === "MCP") return <Plug {...iconProps} />;
  if (definitionId === "definition:google_calendar") return <CalendarDays {...iconProps} />;
  if (["definition:google_docs", "definition:google_docs_read_edit"].includes(definitionId)) {
    return <FileText {...iconProps} />;
  }
  if (definitionId === "definition:google_drive") return <HardDrive {...iconProps} />;
  if (["definition:google_gmail", "definition:gmail_read_search_drafts"].includes(definitionId)) {
    return <Mail {...iconProps} />;
  }
  return <Braces {...iconProps} />;
}

function connectionDescription(connection: CapabilityIntegration["connections"][number]) {
  if (connectionNeedsAuthorization(connection.authStatus)) return "Authorization required";
  if (!["active", "healthy", "ready"].includes(connection.healthStatus.toLowerCase())) {
    return settingsStatusLabel(connection.healthStatus);
  }
  return `${connection.availableToolCount} / ${connection.toolCount} tools`;
}

function connectionNeedsAuthorization(authStatus: string) {
  return ["authentication_required", "needs_auth", "required"].includes(authStatus.toLowerCase());
}

const styles = stylex.create({
  service: {
    overflow: "hidden",
    borderWidth: "var(--border-width)",
    borderStyle: "solid",
    borderColor: "var(--border-subtle)",
    borderRadius: "var(--radius-container)"
  },
  serviceHeader: { minHeight: "var(--spacing-10)", padding: "var(--spacing-2)" },
  serviceCopy: { minWidth: 0, flex: 1 },
  iconFrame: {
    width: "var(--spacing-8)",
    height: "var(--spacing-8)",
    flexShrink: 0,
    borderRadius: "var(--radius-sm)",
    backgroundColor: "var(--noema-surface-subtle)",
    color: "var(--muted-foreground)"
  },
  emphasizedIcon: { backgroundColor: "var(--color-success-muted)", color: "var(--primary)" },
  icon: { width: "var(--spacing-4)", height: "var(--spacing-4)" },
  title: {
    margin: "var(--spacing-0)",
    color: "var(--foreground)",
    fontFamily: "var(--font-heading)",
    fontSize: 14,
    fontWeight: 650,
    lineHeight: 1.3,
    overflowWrap: "anywhere"
  },
  summary: { margin: "var(--spacing-0)", color: "var(--muted-foreground)", fontSize: 12 },
  connectionRow: {
    display: "grid",
    gridTemplateColumns: "auto minmax(0, 1fr) auto",
    alignItems: "center",
    gap: "var(--spacing-2)",
    minHeight: "var(--spacing-12)",
    padding: "var(--spacing-1-5) var(--spacing-2)",
    borderBlockStartWidth: "var(--border-width)",
    borderBlockStartStyle: "solid",
    borderBlockStartColor: "var(--border-subtle)",
    backgroundColor: "var(--noema-surface-subtle)",
    color: "var(--foreground)",
    textDecoration: "none",
    ":hover": { backgroundColor: "var(--surface-raised)" },
    ":focus-visible": { outlineWidth: 2, outlineStyle: "solid", outlineColor: "var(--ring)", outlineOffset: -2 }
  },
  selectedConnection: { boxShadow: "inset 0 0 0 2px var(--primary)" },
  connectionCopy: { minWidth: 0 },
  connectionName: { fontSize: 13, fontWeight: 650, overflowWrap: "anywhere" },
  connectionMeta: { color: "var(--muted-foreground)", fontSize: 12 },
  metaIcon: { width: 12, height: 12 },
  chevron: { width: "var(--spacing-4)", height: "var(--spacing-4)", color: "var(--muted-foreground)" },
  empty: { margin: "var(--spacing-0)", color: "var(--muted-foreground)", fontSize: 13, lineHeight: 1.5 },
  emptyConnection: { margin: 0, padding: "var(--spacing-2)", borderBlockStartWidth: "var(--border-width)", borderBlockStartStyle: "solid", borderBlockStartColor: "var(--border-subtle)", color: "var(--muted-foreground)", fontSize: 12 }
});

import { Avatar } from "@astryxdesign/core/Avatar";
import { Button } from "@astryxdesign/core/Button";
import { HStack } from "@astryxdesign/core/HStack";
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
import { useState } from "react";
import type { CapabilityIntegrationsQuery } from "@/generated/graphql";
import { ListCardLink } from "@/components/ListCardLink";
import { settingsStatusLabel } from "./settingsStatus";

export type CapabilityIntegration = CapabilityIntegrationsQuery["capabilityIntegrations"][number];
type CapabilityConnection = CapabilityIntegration["connections"][number];
type CapabilityAccount = { id: string; name: string };

type CapabilityIntegrationListProps = {
  integrations: readonly CapabilityIntegration[];
  kind: "API" | "MCP";
  selectedConnectionId?: string;
  emptyMessage: string;
  accountFor?: (connection: CapabilityConnection) => CapabilityAccount;
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
  accountFor,
  isAddingConnection = false,
  onAddConnection
}: CapabilityIntegrationListProps) {
  const [visibleSelectedConnectionId, setVisibleSelectedConnectionId] = useState(selectedConnectionId);
  const selectConnection = (connectionId: string) => setVisibleSelectedConnectionId(connectionId);

  if (accountFor) {
    return (
      <VStack gap={4}>
        {integrations.length === 0 ? <p {...stylex.props(styles.empty)}>{emptyMessage}</p> : null}
        {groupConnectionsByAccount(integrations, accountFor).map(({ account, items }, index) => {
          const titleId = `capability-account-${index}-title`;
          return (
            <VStack as="section" key={account.id} gap={1.5} aria-labelledby={titleId} {...stylex.props(styles.service)}>
              <HStack gap={2} vAlign="center" {...stylex.props(styles.serviceHeader)}>
                <Avatar name={account.name} size="sm" tooltip={false} />
                <h2 id={titleId} {...stylex.props(styles.title)}>{account.name}</h2>
              </HStack>
              {items.map(({ integration, connection }) => (
                <CapabilityConnectionCard
                  key={connection.connectionId}
                  kind={kind}
                  integration={integration}
                  connection={connection}
                  showIntegration
                  selectedConnectionId={selectedConnectionId}
                  visibleSelectedConnectionId={visibleSelectedConnectionId}
                  onSelect={selectConnection}
                />
              ))}
            </VStack>
          );
        })}
      </VStack>
    );
  }

  return (
    <VStack gap={4}>
      {integrations.length === 0 ? (
        <p {...stylex.props(styles.empty)}>{emptyMessage}</p>
      ) : null}
      {integrations.map((integration) => {
        return (
          <VStack
            as="section"
            key={integration.definitionId}
            gap={1.5}
            aria-labelledby={`capability-${integration.definitionId}-title`}
            {...stylex.props(styles.service)}
          >
            <HStack gap={2} vAlign="center" {...stylex.props(styles.serviceHeader)}>
              <CapabilityIcon kind={kind} definitionId={integration.definitionId} />
              <h2 id={`capability-${integration.definitionId}-title`} {...stylex.props(styles.title)}>
                {integration.name}
              </h2>
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
            </HStack>
            {integration.connections.length === 0 ? (
              <p {...stylex.props(styles.emptyConnection)}>No connections</p>
            ) : integration.connections.map((connection) => (
              <CapabilityConnectionCard
                key={connection.connectionId}
                kind={kind}
                integration={integration}
                connection={connection}
                selectedConnectionId={selectedConnectionId}
                visibleSelectedConnectionId={visibleSelectedConnectionId}
                onSelect={selectConnection}
              />
            ))}
          </VStack>
        );
      })}
    </VStack>
  );
}

function CapabilityConnectionCard({
  kind,
  integration,
  connection,
  showIntegration = false,
  selectedConnectionId,
  visibleSelectedConnectionId,
  onSelect
}: {
  kind: "API" | "MCP";
  integration: CapabilityIntegration;
  connection: CapabilityConnection;
  showIntegration?: boolean;
  selectedConnectionId?: string;
  visibleSelectedConnectionId?: string;
  onSelect: (connectionId: string) => void;
}) {
  const route = kind === "API"
    ? "/settings/tools/apis/$connectionId"
    : "/settings/tools/mcps/$connectionId";
  return (
    <ListCardLink
      to={route}
      params={{ connectionId: connection.connectionId }}
      selected={visibleSelectedConnectionId === connection.connectionId}
      xstyle={styles.connectionRow}
      aria-current={selectedConnectionId === connection.connectionId ? "page" : undefined}
      onClick={(event) => {
        if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
        onSelect(connection.connectionId);
      }}
    >
      {showIntegration
        ? <CapabilityIcon kind={kind} definitionId={integration.definitionId} />
        : <Avatar name={connection.name} size="sm" tooltip={false} />}
      <VStack gap={0.5} {...stylex.props(styles.connectionCopy)}>
        <strong {...stylex.props(styles.connectionName)}>
          {showIntegration ? integration.name : connection.name}
        </strong>
        <HStack as="span" gap={1} vAlign="center" {...stylex.props(styles.connectionMeta)}>
          <Wrench aria-hidden="true" {...stylex.props(styles.metaIcon)} />
          {connectionDescription(connection)}
        </HStack>
      </VStack>
      <ChevronRight aria-hidden="true" {...stylex.props(styles.chevron)} />
    </ListCardLink>
  );
}

function groupConnectionsByAccount(
  integrations: readonly CapabilityIntegration[],
  accountFor: (connection: CapabilityConnection) => CapabilityAccount
) {
  const groups = new Map<string, { account: CapabilityAccount; items: Array<{
    integration: CapabilityIntegration;
    connection: CapabilityConnection;
  }> }>();
  for (const integration of integrations) {
    for (const connection of integration.connections) {
      const account = accountFor(connection);
      const group = groups.get(account.id) ?? { account, items: [] };
      group.items.push({ integration, connection });
      groups.set(account.id, group);
    }
  }
  return [...groups.values()];
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
    minWidth: 0
  },
  serviceHeader: { minHeight: "var(--spacing-8)", paddingInline: "var(--spacing-1)" },
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
    minWidth: 0,
    flex: 1,
    overflowWrap: "anywhere"
  },
  connectionRow: {
    display: "grid",
    gridTemplateColumns: "auto minmax(0, 1fr) auto",
    alignItems: "center",
    gap: "var(--spacing-2)",
    minHeight: "var(--spacing-12)",
    color: "var(--foreground)"
  },
  connectionCopy: { minWidth: 0 },
  connectionName: { fontSize: 13, fontWeight: 650, overflowWrap: "anywhere" },
  connectionMeta: { color: "var(--muted-foreground)", fontSize: 12 },
  metaIcon: { width: 12, height: 12 },
  chevron: { width: "var(--spacing-4)", height: "var(--spacing-4)", color: "var(--muted-foreground)" },
  empty: { margin: "var(--spacing-0)", color: "var(--muted-foreground)", fontSize: 13, lineHeight: 1.5 },
  emptyConnection: { margin: 0, paddingInline: "var(--spacing-1)", color: "var(--muted-foreground)", fontSize: 12 }
});

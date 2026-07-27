import type { ReactNode } from "react";
import { Link } from "@tanstack/react-router";
import { Badge } from "@astryxdesign/core/Badge";
import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import type { CapabilityIntegrationsQuery } from "@/generated/graphql";

export type CapabilityIntegration = CapabilityIntegrationsQuery["capabilityIntegrations"][number];
export type CapabilityConnection = CapabilityIntegration["connections"][number];
export type CapabilityConnectionAction = {
  label: string;
  icon?: ReactNode;
  tone?: "danger";
  isDisabled?: boolean;
  onClick: () => void;
};

export function CapabilityIntegrationList({
  integrations,
  kind,
  emptyMessage,
  primaryAction,
  isAddingConnection = false,
  onAddConnection,
  connectionActions
}: {
  integrations: readonly CapabilityIntegration[];
  kind: "API" | "MCP";
  emptyMessage: string;
  primaryAction?: { label: string; onClick: () => void };
  isAddingConnection?: boolean;
  onAddConnection: (integration: CapabilityIntegration) => void;
  connectionActions?: (connection: CapabilityConnection) => readonly CapabilityConnectionAction[];
}) {
  return (
    <div {...stylex.props(styles.list)}>
      {primaryAction ? (
        <div {...stylex.props(styles.toolbar)}>
          <Button
            type="button"
            size="sm"
            variant="secondary"
            label={primaryAction.label}
            onClick={primaryAction.onClick}
          />
        </div>
      ) : null}
      {integrations.length === 0 ? (
        <div {...stylex.props(styles.group)}>
          <p {...stylex.props(styles.empty)}>{emptyMessage}</p>
        </div>
      ) : null}
      {integrations.map((integration) => (
        <section key={integration.definitionId} {...stylex.props(styles.group)}>
          <div {...stylex.props(styles.groupHeader)}>
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
          </div>
          {integration.connections.length === 0 ? (
            <p {...stylex.props(styles.empty)}>No account connected to this definition.</p>
          ) : (
            <div {...stylex.props(styles.connections)}>
              {integration.connections.map((connection) => (
                <div key={connection.connectionId} {...stylex.props(styles.connection)}>
                  <div {...stylex.props(styles.connectionCopy)}>
                    <strong {...stylex.props(styles.connectionName)}>{connection.name}</strong>
                    <span {...stylex.props(styles.meta)}>
                      {connection.authStatus} · {connection.availableToolCount}/{connection.toolCount} tools
                    </span>
                  </div>
                  <div {...stylex.props(styles.actions)}>
                    <Link
                      to={
                        kind === "API"
                          ? "/settings/tools/apis/$connectionId"
                          : "/settings/tools/mcps/$connectionId"
                      }
                      params={{ connectionId: connection.connectionId }}
                      {...stylex.props(styles.manageLink)}
                    >
                      Manage
                    </Link>
                    {connectionActions?.(connection).map((action) => (
                      <Button
                        key={action.label}
                        type="button"
                        size="sm"
                        variant="ghost"
                        label={action.label}
                        icon={action.icon}
                        isDisabled={action.isDisabled}
                        {...stylex.props(action.tone === "danger" && styles.dangerAction)}
                        onClick={action.onClick}
                      />
                    ))}
                  </div>
                </div>
              ))}
            </div>
          )}
        </section>
      ))}
    </div>
  );
}

const styles = stylex.create({
  list: { display: "grid", gap: "var(--spacing-3)" },
  toolbar: { display: "flex", justifyContent: "flex-end" },
  group: {
    display: "grid",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border-subtle)",
    borderRadius: 6,
    backgroundColor: "var(--surface-raised)",
    overflow: "hidden"
  },
  groupHeader: {
    display: "flex",
    flexWrap: "wrap",
    alignItems: "center",
    gap: "var(--spacing-2)",
    padding: "var(--spacing-3)",
    borderBottomWidth: 1,
    borderBottomStyle: "solid",
    borderBottomColor: "var(--border-subtle)"
  },
  groupCopy: { flex: "1 1 16rem", minWidth: 0 },
  title: { margin: 0, fontFamily: "var(--font-heading)", fontSize: 16, fontWeight: 600 },
  summary: { margin: "var(--spacing-1) 0 0", color: "var(--muted-foreground)", fontSize: 12 },
  connections: { display: "grid" },
  connection: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    gap: "var(--spacing-2)",
    padding: "var(--spacing-2) var(--spacing-3)",
    borderBottomWidth: 1,
    borderBottomStyle: "solid",
    borderBottomColor: "var(--border-subtle)",
    ":last-child": { borderBottomWidth: 0 },
    "@media (max-width: 640px)": { alignItems: "flex-start", flexDirection: "column" }
  },
  connectionCopy: { display: "grid", gap: 2, minWidth: 0 },
  connectionName: { fontSize: 14, overflowWrap: "anywhere" },
  meta: { color: "var(--muted-foreground)", fontSize: 12 },
  actions: { display: "flex", alignItems: "center", gap: "var(--spacing-1)", flexWrap: "wrap" },
  manageLink: {
    display: "inline-flex",
    alignItems: "center",
    minHeight: "var(--size-element-sm)",
    padding: "var(--spacing-1) var(--spacing-2)",
    borderRadius: 4,
    color: "var(--text-accent)",
    fontSize: 13,
    fontWeight: 600,
    textDecoration: "none",
    ":hover": { backgroundColor: "var(--surface-hover)" }
  },
  dangerAction: { color: "var(--destructive)" },
  empty: { margin: 0, padding: "var(--spacing-3)", color: "var(--muted-foreground)", fontSize: 13 }
});

import type { ReactNode } from "react";
import { Link } from "@tanstack/react-router";
import { Badge } from "@astryxdesign/core/Badge";
import * as stylex from "@stylexjs/stylex";
import type { CapabilityIntegrationsQuery } from "@/generated/graphql";

export type CapabilityIntegration = CapabilityIntegrationsQuery["capabilityIntegrations"][number];
export type CapabilityConnection = CapabilityIntegration["connections"][number];

export function CapabilityIntegrationList({
  integrations,
  kind,
  empty,
  renderGroupAction,
  renderActions
}: {
  integrations: readonly CapabilityIntegration[];
  kind: "API" | "MCP";
  empty: ReactNode;
  renderGroupAction?: (integration: CapabilityIntegration) => ReactNode;
  renderActions?: (connection: CapabilityConnection) => ReactNode;
}) {
  if (integrations.length === 0) return <>{empty}</>;

  return (
    <div {...stylex.props(styles.list)}>
      {integrations.map((integration) => (
        <section key={integration.definitionId} {...stylex.props(styles.group)}>
          <div {...stylex.props(styles.groupHeader)}>
            <div>
              <h2 {...stylex.props(styles.title)}>{integration.name}</h2>
              <p {...stylex.props(styles.summary)}>{integration.sourceSummary}</p>
            </div>
            <Badge
              variant="neutral"
              label={`${integration.connections.length} ${integration.connections.length === 1 ? "connection" : "connections"}`}
            />
            {renderGroupAction?.(integration)}
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
                    {renderActions?.(connection)}
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
    alignItems: "center",
    justifyContent: "space-between",
    gap: "var(--spacing-2)",
    padding: "var(--spacing-3)",
    borderBottomWidth: 1,
    borderBottomStyle: "solid",
    borderBottomColor: "var(--border-subtle)"
  },
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
    padding: "var(--spacing-1) var(--spacing-2)",
    borderRadius: 4,
    color: "var(--text-accent)",
    fontSize: 13,
    fontWeight: 600,
    textDecoration: "none",
    ":hover": { backgroundColor: "var(--surface-hover)" }
  },
  empty: { margin: 0, padding: "var(--spacing-3)", color: "var(--muted-foreground)", fontSize: 13 }
});

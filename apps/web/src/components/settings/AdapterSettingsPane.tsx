import { useMutation, useQuery } from "@apollo/client/react";
import { Badge } from "@astryxdesign/core/Badge";
import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import type { ReactNode } from "react";
import {
  AdapterDefinitionsDocument,
  ApproveAdapterDefinitionDocument,
  type AdapterDefinitionsQuery,
  type ApproveAdapterDefinitionMutation
} from "@/generated/graphql";

export function AdapterSettingsPane() {
  const result = useQuery<AdapterDefinitionsQuery>(AdapterDefinitionsDocument, {
    fetchPolicy: "cache-and-network"
  });
  const [approve, approval] = useMutation<ApproveAdapterDefinitionMutation>(
    ApproveAdapterDefinitionDocument
  );
  const definitions = (result.data?.adapterDefinitions ?? []).filter(
    (definition) => !definition.superseded
  );

  if (result.loading && !result.data) {
    return <p {...stylex.props(styles.muted)}>Loading discovered definitions...</p>;
  }
  if (result.error) {
    return (
      <div {...stylex.props(styles.stack)}>
        <p {...stylex.props(styles.muted)}>Couldn't load discovered definitions.</p>
        <Button
          type="button"
          variant="secondary"
          label="Retry"
          {...stylex.props(styles.fit)}
          onClick={() => void result.refetch()}
        />
      </div>
    );
  }
  if (definitions.length === 0) {
    return (
      <div {...stylex.props(styles.card)}>
        <p {...stylex.props(styles.muted)}>
          No API definitions discovered yet. Ask Momo to connect a service and it can research the
          official API.
        </p>
      </div>
    );
  }

  async function approveDefinition(semanticDigest: string) {
    await approve({ variables: { input: { semanticDigest } } });
    await result.refetch();
  }

  return (
    <div {...stylex.props(styles.stack)}>
      {definitions.map((definition) => (
        <article key={definition.semanticDigest} {...stylex.props(styles.card)}>
          <div {...stylex.props(styles.titleRow)}>
            <h2 {...stylex.props(styles.title)}>{definition.displayName}</h2>
            <Badge
              variant="neutral"
              label={definition.reviewed ? "Reviewed" : "Pending review"}
            />
          </div>

          <dl {...stylex.props(styles.details)}>
            <DefinitionRow label="Source">
              {isHttpsUrl(definition.sourceReference) ? (
                <a
                  href={definition.sourceReference}
                  target="_blank"
                  rel="noreferrer"
                  {...stylex.props(styles.link)}
                >
                  {definition.sourceReference}
                </a>
              ) : (
                definition.sourceReference
              )}
            </DefinitionRow>
            <DefinitionRow label="API origin">{definition.origin}</DefinitionRow>
            <DefinitionRow label="Authentication">
              {humanize(definition.authenticationMode)}
            </DefinitionRow>
            <DefinitionRow label="Scopes">
              {definition.scopes.length > 0 ? definition.scopes.join(", ") : "None"}
            </DefinitionRow>
            <DefinitionRow label="Revision">{definition.definitionRevision}</DefinitionRow>
          </dl>

          <div {...stylex.props(styles.operations)}>
            <h3 {...stylex.props(styles.subheading)}>Proposed tools</h3>
            {definition.operations.map((operation) => (
              <div key={operation.operationId} {...stylex.props(styles.operation)}>
                <code {...stylex.props(styles.code)}>
                  {operation.method} {operation.path}
                </code>
                <span {...stylex.props(styles.muted)}>
                  {operation.operationId} · {operation.readOnly ? "Read only" : "Can make changes"} ·{" "}
                  {operation.idempotent ? "Safe to repeat" : "Do not retry"}
                </span>
              </div>
            ))}
          </div>

          {definition.reviewed ? (
            <p {...stylex.props(styles.muted)}>
              Definition approved. Account setup will use these exact operations and scopes.
            </p>
          ) : (
            <Button
              type="button"
              label="Approve definition"
              isDisabled={approval.loading}
              {...stylex.props(styles.fit)}
              onClick={() => void approveDefinition(definition.semanticDigest)}
            />
          )}
          {approval.error ? (
            <p role="alert" {...stylex.props(styles.error)}>
              {approval.error.message}
            </p>
          ) : null}
        </article>
      ))}
    </div>
  );
}

function DefinitionRow({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div {...stylex.props(styles.definitionRow)}>
      <dt {...stylex.props(styles.term)}>{label}</dt>
      <dd {...stylex.props(styles.value)}>{children}</dd>
    </div>
  );
}

function humanize(value: string) {
  return value.replaceAll("_", " ");
}

function isHttpsUrl(value: string) {
  try {
    return new URL(value).protocol === "https:";
  } catch {
    return false;
  }
}

const styles = stylex.create({
  stack: {
    display: "grid",
    gap: "var(--spacing-3)"
  },
  card: {
    display: "grid",
    gap: "var(--spacing-3)",
    padding: "var(--spacing-3)",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border-subtle)",
    borderRadius: 6,
    backgroundColor: "var(--surface-raised)"
  },
  titleRow: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    gap: "var(--spacing-2)"
  },
  title: {
    margin: 0,
    fontFamily: "var(--font-heading)",
    fontSize: 18,
    fontWeight: 600
  },
  details: {
    display: "grid",
    margin: 0
  },
  definitionRow: {
    display: "grid",
    gridTemplateColumns: "9rem minmax(0, 1fr)",
    gap: "var(--spacing-2)",
    paddingBlock: "var(--spacing-1)",
    borderBottomWidth: 1,
    borderBottomStyle: "solid",
    borderBottomColor: "var(--border-subtle)",
    "@media (max-width: 640px)": {
      gridTemplateColumns: "1fr",
      gap: 0
    }
  },
  term: {
    color: "var(--muted-foreground)",
    fontSize: 13
  },
  value: {
    minWidth: 0,
    margin: 0,
    overflowWrap: "anywhere",
    color: "var(--foreground)",
    fontSize: 13,
    lineHeight: 1.5
  },
  operations: {
    display: "grid",
    gap: "var(--spacing-2)"
  },
  subheading: {
    margin: 0,
    fontSize: 13,
    fontWeight: 600
  },
  operation: {
    display: "grid",
    gap: "var(--spacing-1)"
  },
  code: {
    overflowWrap: "anywhere",
    fontFamily: "var(--font-mono)",
    fontSize: 13
  },
  muted: {
    margin: 0,
    color: "var(--muted-foreground)",
    fontSize: 13,
    lineHeight: 1.5
  },
  link: {
    color: "var(--text-accent)"
  },
  error: {
    margin: 0,
    color: "var(--destructive)",
    fontSize: 13
  },
  fit: {
    width: "fit-content"
  }
});

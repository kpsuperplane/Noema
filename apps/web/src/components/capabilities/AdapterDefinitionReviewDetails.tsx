import { HStack } from "@astryxdesign/core/HStack";
import { List, ListItem } from "@astryxdesign/core/List";
import { MetadataList, MetadataListItem } from "@astryxdesign/core/MetadataList";
import { VStack } from "@astryxdesign/core/VStack";
import * as stylex from "@stylexjs/stylex";

type ReviewOperation = {
  operationId: string;
  method: string;
  path: string;
  readOnly?: boolean | null;
  idempotent?: boolean | null;
  destructive?: boolean | null;
  openWorld?: boolean | null;
  argumentNames: readonly string[];
  responseTransform?: {
    language: string;
    sourceDigest: string;
    source: string;
    acceptedContentTypes: readonly string[];
    outputSchemaJson: string;
  } | null;
};

type ReviewDefinition = {
  semanticDigest: string;
  definitionId: string;
  adapterId: string;
  definitionRevision: string;
  sourceReference: string;
  origin: string;
  authenticationMode: string;
  scopes: readonly string[];
  credentialSetup?: {
    credentialType: string;
    setupUrl: string;
  } | null;
  accountIdentityOperationId?: string | null;
  reviewed: boolean;
  operations: readonly ReviewOperation[];
};

export function AdapterDefinitionReviewDetails({ definition }: { definition: ReviewDefinition }) {
  const identityOperation = definition.operations.find(
    (operation) => operation.operationId === definition.accountIdentityOperationId
  );
  const transformedOperations = definition.operations.filter(
    (operation) => operation.responseTransform != null
  );
  const setup = definition.credentialSetup;
  const sourceIsHttps = definition.sourceReference.startsWith("https://");

  return (
    <VStack gap={4}>
      <MetadataList title={<strong {...stylex.props(styles.heading)}>Authentication</strong>}>
        <MetadataListItem label="Method">{authenticationLabel(definition.authenticationMode)}</MetadataListItem>
        <MetadataListItem label="Credential">{credentialLabel(definition.authenticationMode, setup?.credentialType)}</MetadataListItem>
        {setup ? (
          <MetadataListItem label="Setup page">
            <a href={setup.setupUrl} target="_blank" rel="noreferrer" {...stylex.props(styles.link)}>{setup.setupUrl}</a>
          </MetadataListItem>
        ) : null}
        <MetadataListItem label="Scopes">
          {definition.scopes.length > 0 ? (
            <VStack gap={1}>
              {definition.scopes.map((scope) => <code key={scope} {...stylex.props(styles.monospace)}>{scope}</code>)}
            </VStack>
          ) : "No scopes requested"}
        </MetadataListItem>
        <MetadataListItem label="Account label">
          {identityOperation
            ? `${identityOperation.method} ${identityOperation.path}`
            : definition.authenticationMode === "oauth2_authorization_code_pkce"
            ? "Generated connection label"
            : "Not configured"}
        </MetadataListItem>
      </MetadataList>

      <List
        density="compact"
        hasDividers
        header={(
          <VStack gap={1}>
            <strong {...stylex.props(styles.heading)}>API operations</strong>
            <span {...stylex.props(styles.supporting)}>{countLabel(definition.operations.length, "operation")}</span>
          </VStack>
        )}
      >
        {definition.operations.map((operation) => (
          <ListItem
            key={operation.operationId}
            label={(
              <HStack gap={2} wrap="wrap">
                <strong {...stylex.props(styles.operationName)}>{humanize(operation.operationId)}</strong>
                <code {...stylex.props(styles.operationId)}>{operation.operationId}</code>
              </HStack>
            )}
            description={(
              <VStack gap={1}>
                <code {...stylex.props(styles.endpoint)}>{operation.method} {operation.path}</code>
                <span {...stylex.props(styles.supporting)}>{behaviorSummary(operation)}</span>
                <span {...stylex.props(styles.supporting)}>
                  {operation.argumentNames.length > 0
                    ? `${countLabel(operation.argumentNames.length, "argument")}: ${operation.argumentNames.join(", ")}`
                    : "No arguments"}
                </span>
                {operation.responseTransform ? (
                  <span {...stylex.props(styles.supporting)}>Response is normalized before it reaches the agent</span>
                ) : null}
              </VStack>
            )}
          />
        ))}
      </List>

      {transformedOperations.length > 0 ? (
        <List
          density="compact"
          hasDividers
          header={<strong {...stylex.props(styles.heading)}>Response handling</strong>}
        >
          {transformedOperations.map((operation) => {
            const transform = operation.responseTransform;
            if (!transform) return null;
            return (
              <ListItem
                key={operation.operationId}
                label={humanize(operation.operationId)}
                description={(
                  <VStack gap={2}>
                    <MetadataList>
                      <MetadataListItem label="Language">{humanize(transform.language)}</MetadataListItem>
                      <MetadataListItem label="Accepted responses">
                        {transform.acceptedContentTypes.join(", ")}
                      </MetadataListItem>
                      <MetadataListItem label="Source SHA-256">
                        <code {...stylex.props(styles.monospace)}>{transform.sourceDigest}</code>
                      </MetadataListItem>
                    </MetadataList>
                    <VStack gap={1}>
                      <span {...stylex.props(styles.fieldLabel)}>Transform source</span>
                      <pre {...stylex.props(styles.code)}>{transform.source}</pre>
                    </VStack>
                    <VStack gap={1}>
                      <span {...stylex.props(styles.fieldLabel)}>Output schema</span>
                      <pre {...stylex.props(styles.code)}>{transform.outputSchemaJson}</pre>
                    </VStack>
                  </VStack>
                )}
              />
            );
          })}
        </List>
      ) : null}

      <MetadataList title={<strong {...stylex.props(styles.heading)}>Definition</strong>}>
        <MetadataListItem label="Review status">{definition.reviewed ? "Reviewed" : "Pending review"}</MetadataListItem>
        <MetadataListItem label="API origin"><code {...stylex.props(styles.monospace)}>{definition.origin}</code></MetadataListItem>
        <MetadataListItem label="Revision"><code {...stylex.props(styles.monospace)}>{definition.definitionRevision}</code></MetadataListItem>
        <MetadataListItem label="Adapter ID"><code {...stylex.props(styles.monospace)}>{definition.adapterId}</code></MetadataListItem>
        <MetadataListItem label="Definition ID"><code {...stylex.props(styles.monospace)}>{definition.definitionId}</code></MetadataListItem>
        <MetadataListItem label="Definition SHA-256"><code {...stylex.props(styles.monospace)}>{definition.semanticDigest}</code></MetadataListItem>
        <MetadataListItem label="Source">
          {sourceIsHttps ? (
            <a href={definition.sourceReference} target="_blank" rel="noreferrer" {...stylex.props(styles.link)}>
              {definition.sourceReference}
            </a>
          ) : definition.sourceReference}
        </MetadataListItem>
      </MetadataList>
    </VStack>
  );
}

function authenticationLabel(mode: string) {
  if (mode === "oauth2_authorization_code_pkce") return "OAuth 2.0 authorization code with PKCE";
  if (mode === "credential") return "Provider credential";
  if (mode === "none") return "No authentication";
  return humanize(mode);
}

function credentialLabel(mode: string, credentialType?: string) {
  if (credentialType) return credentialType;
  if (mode === "oauth2_authorization_code_pkce") return "OAuth client";
  if (mode === "credential") return "Provider credential";
  return "None";
}

function behaviorSummary(operation: ReviewOperation) {
  return [
    hintLabel(operation.readOnly, "Read only", "Can change data", "Read behavior unknown"),
    hintLabel(operation.idempotent, "Idempotent", "Not idempotent", "Retry behavior unknown"),
    hintLabel(operation.destructive, "Destructive", "Non-destructive", "Destructive behavior unknown"),
    hintLabel(operation.openWorld, "External interaction", "No external interaction", "External behavior unknown")
  ].join(" · ");
}

function hintLabel(value: boolean | null | undefined, yes: string, no: string, unknown: string) {
  return value === true ? yes : value === false ? no : unknown;
}

function countLabel(count: number, singular: string) {
  return `${count} ${singular}${count === 1 ? "" : "s"}`;
}

function humanize(value: string) {
  const words = value.replace(/[_\-.:\s]+/g, " ").trim().toLowerCase();
  return words.replace(/^./, (character) => character.toUpperCase());
}

const styles = stylex.create({
  heading: {
    color: "var(--noema-text-primary)",
    fontSize: 12
  },
  operationName: {
    color: "var(--noema-text-primary)",
    fontSize: 12
  },
  operationId: {
    color: "var(--noema-text-muted)",
    fontSize: 12,
    overflowWrap: "anywhere"
  },
  endpoint: {
    color: "var(--noema-text-primary)",
    fontFamily: "var(--noema-font-mono)",
    fontSize: 12,
    overflowWrap: "anywhere"
  },
  supporting: {
    color: "var(--noema-text-muted)",
    fontSize: 12,
    lineHeight: 1.4,
    overflowWrap: "anywhere"
  },
  fieldLabel: {
    color: "var(--noema-text-secondary)",
    fontSize: 12,
    fontWeight: 600
  },
  monospace: {
    fontFamily: "var(--noema-font-mono)",
    fontSize: 12,
    overflowWrap: "anywhere"
  },
  link: {
    color: "var(--noema-text-link)",
    textDecoration: "underline",
    overflowWrap: "anywhere"
  },
  code: {
    margin: "var(--spacing-0)",
    padding: "var(--spacing-2)",
    borderRadius: "var(--radius)",
    backgroundColor: "var(--noema-surface-subtle)",
    color: "var(--noema-text-primary)",
    fontFamily: "var(--noema-font-mono)",
    fontSize: 12,
    whiteSpace: "pre-wrap",
    overflowWrap: "anywhere",
    cursor: "text"
  }
});

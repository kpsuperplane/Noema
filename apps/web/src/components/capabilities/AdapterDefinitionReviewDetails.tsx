import { VStack } from "@astryxdesign/core/VStack";
import * as stylex from "@stylexjs/stylex";
type ReviewOperation = {
  operationId: string;
  method: string;
  path: string;
  responseTransform?: {
    language: string;
    sourceDigest: string;
    source: string;
    acceptedContentTypes: string[];
    outputSchemaJson: string;
  } | null;
};

export function AdapterDefinitionReviewDetails({
  operations,
  accountIdentityOperationId,
  authenticationMode
}: {
  operations: readonly ReviewOperation[];
  accountIdentityOperationId?: string | null;
  authenticationMode?: string | null;
}) {
  const identityOperation = operations.find(
    (operation) => operation.operationId === accountIdentityOperationId
  );
  const transformedOperations = operations.filter(
    (operation) => operation.responseTransform != null
  );
  const identityMissing = authenticationMode === "oauth2_authorization_code_pkce" && !identityOperation;
  if (!identityOperation && !identityMissing && transformedOperations.length === 0) return null;
  return (
    <VStack gap={2}>
      {identityOperation ? (
        <VStack gap={1}>
          <strong {...stylex.props(styles.heading)}>Account identification</strong>
          <span>{identityOperation.method} {identityOperation.path}</span>
        </VStack>
      ) : identityMissing ? (
        <VStack gap={1}>
          <strong {...stylex.props(styles.heading)}>Account identification</strong>
          <span>No recognizable account identifier is configured. Connections use a generated label.</span>
        </VStack>
      ) : null}
      {transformedOperations.length > 0 ? (
        <VStack gap={1}>
          <strong {...stylex.props(styles.heading)}>Response transforms</strong>
          {transformedOperations.map((operation) => {
            const transform = operation.responseTransform;
            if (!transform) return null;
            return (
              <details key={operation.operationId}>
                <summary>{operation.operationId} · {transform.language}</summary>
                <VStack gap={2} {...stylex.props(styles.transform)}>
                  <span><b>Source SHA-256</b><br />{transform.sourceDigest}</span>
                  <span><b>Accepted media types</b><br />{transform.acceptedContentTypes.join("\n")}</span>
                  <span><b>Exact source</b></span>
                  <pre {...stylex.props(styles.code)}>{transform.source}</pre>
                  <span><b>Output schema</b></span>
                  <pre {...stylex.props(styles.code)}>{transform.outputSchemaJson}</pre>
                </VStack>
              </details>
            );
          })}
        </VStack>
      ) : null}
    </VStack>
  );
}

const styles = stylex.create({
  heading: {
    fontSize: 12
  },
  transform: {
    marginTop: "var(--spacing-2)",
    overflowWrap: "anywhere"
  },
  code: {
    maxHeight: 180,
    margin: "var(--spacing-0)",
    padding: "var(--spacing-2)",
    overflow: "auto",
    borderRadius: 8,
    backgroundColor: "var(--noema-surface-subtle)",
    fontSize: 11,
    whiteSpace: "pre-wrap",
    overflowWrap: "anywhere",
    cursor: "text"
  }
});

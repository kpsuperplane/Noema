import { useApolloClient, useMutation, useQuery, useSubscription } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import {
  MemoryEventsDocument,
  MemoryTreeDocument,
  UpdateMemoryDocument,
  type MemoryTreeQuery,
  type UpdateMemoryMutation
} from "@/generated/graphql";
import { styles } from "@/pages/memoryPageStyles";

export function MemoryUpdateControl() {
  const client = useApolloClient();
  const treeResult = useQuery<MemoryTreeQuery>(MemoryTreeDocument, { fetchPolicy: "cache-only" });
  const subscription = useSubscription(MemoryEventsDocument, {
    onData: ({ data }) => {
      const memoryTree = data.data?.memoryEvents;
      if (memoryTree) client.writeQuery<MemoryTreeQuery>({ query: MemoryTreeDocument, data: { memoryTree } });
    }
  });
  const [updateMemory, updateResult] = useMutation<UpdateMemoryMutation>(UpdateMemoryDocument);
  const tree = treeResult.data?.memoryTree;
  const status = tree?.updateStatus;
  const updating = Boolean(updateResult.loading || status?.active);
  const pendingCount = tree?.pendingCount ?? 0;
  const retryable = status?.state === "error" || Boolean(updateResult.error);
  const tooltip = updateTooltip(
    pendingCount,
    status?.updatedAt,
    updating ? "Updating now" : retryable ? "Last update failed" : subscription.error ? "Reconnecting" : null
  );

  return (
    <div {...stylex.props(styles.shellUpdate)}>
      <Button
        type="button"
        size="sm"
        variant="primary"
        label={retryable ? "Retry" : "Update"}
        tooltip={tooltip}
        isLoading={updating}
        isDisabled={updating || !tree || (pendingCount === 0 && !retryable)}
        onClick={() => void updateMemory()}
      />
    </div>
  );
}

function updateTooltip(pendingCount: number, updatedAt: string | null | undefined, state: string | null): string {
  const pending = `${pendingCount} ${pendingCount === 1 ? "message" : "messages"} pending`;
  if (!updatedAt) return [pending, "Not updated yet", state].filter(Boolean).join(" · ");

  const timestamp = Date.parse(updatedAt);
  const formatted = Number.isNaN(timestamp)
    ? updatedAt
    : new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(timestamp);
  return [pending, `Last updated ${formatted}`, state].filter(Boolean).join(" · ");
}

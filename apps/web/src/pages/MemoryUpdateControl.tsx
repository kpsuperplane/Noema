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

  return (
    <div {...stylex.props(styles.shellUpdate)}>
      <span role="status" title={status?.error ?? updateResult.error?.message ?? subscription.error?.message} {...stylex.props(styles.status)}>
        {statusLabel(Boolean(status?.active), status?.state, pendingCount, Boolean(updateResult.error), Boolean(subscription.error))}
      </span>
      <Button
        type="button"
        size="sm"
        variant="primary"
        label={retryable ? "Retry memory" : "Update memory"}
        isLoading={updating}
        isDisabled={updating || !tree || (pendingCount === 0 && !retryable)}
        onClick={() => void updateMemory()}
      />
    </div>
  );
}

function statusLabel(active: boolean, state: string | null | undefined, pendingCount: number, launchFailed: boolean, reconnecting: boolean): string {
  if (active) return "Updating…";
  if (state === "error" || launchFailed) return "Update failed";
  if (reconnecting) return "Reconnecting…";
  if (pendingCount > 0) return `${pendingCount} pending`;
  return "Up to date";
}

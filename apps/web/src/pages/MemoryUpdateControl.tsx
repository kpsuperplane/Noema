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
  const updatedAt = formatUpdatedAt(status?.updatedAt);
  const title = updateNoticeTitle({ pendingCount, retryable, updatedAt, updating });
  const detail =
    updateResult.error?.message ??
    status?.error ??
    (subscription.error
      ? "Update status is reconnecting. The article will refresh when the connection returns."
      : updatedAt
        ? `Last updated ${updatedAt}.`
        : "No memory update has completed yet.");

  return (
    <aside
      role={retryable ? "alert" : "status"}
      {...stylex.props(styles.updateNotice, retryable && styles.updateNoticeError)}
    >
      <div {...stylex.props(styles.updateNoticeCopy)}>
        <strong {...stylex.props(styles.updateNoticeTitle)}>{title}</strong>
        <span {...stylex.props(styles.updateNoticeDetail)}>{detail}</span>
      </div>
      <Button
        type="button"
        size="sm"
        variant="secondary"
        label={retryable ? "Retry" : "Update"}
        isLoading={updating}
        isDisabled={updating || !tree || (pendingCount === 0 && !retryable)}
        onClick={() => void updateMemory()}
      />
    </aside>
  );
}

function formatUpdatedAt(updatedAt: string | null | undefined): string | null {
  if (!updatedAt) return null;
  const timestamp = Date.parse(updatedAt);
  return Number.isNaN(timestamp)
    ? updatedAt
    : new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(timestamp);
}

function updateNoticeTitle({
  pendingCount,
  retryable,
  updatedAt,
  updating
}: {
  pendingCount: number;
  retryable: boolean;
  updatedAt: string | null;
  updating: boolean;
}): string {
  if (updating) return "This article is being updated.";
  if (retryable) return "This article could not be updated.";
  if (pendingCount > 0) {
    return `This article may not include ${pendingCount} recent ${pendingCount === 1 ? "message" : "messages"}.`;
  }
  return updatedAt ? "This article is up to date." : "This article has not been updated yet.";
}

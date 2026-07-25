import { useApolloClient, useMutation, useQuery, useSubscription } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import type { CSSProperties } from "react";
import {
  MemoryEventsDocument,
  MemoryTreeDocument,
  UpdateMemoryDocument,
  type MemoryTreeQuery,
  type UpdateMemoryMutation
} from "@/generated/graphql";
import { styles } from "@/pages/memoryPageStyles";

const COMPACTION_IMMINENT_PENDING_COUNT = 100;

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
  const noticeStyle = updateNoticeStyle({ pendingCount, retryable, updatedAt });
  const detail =
    updateResult.error?.message ??
    status?.error ??
    (subscription.error
      ? "Update status is reconnecting. The article will refresh when the connection returns."
      : [
          pendingCount > 0
            ? `${pendingCount} pending`
            : null,
          updatedAt
        ].filter(Boolean).join(" · "));

  return (
    <aside
      role={retryable ? "alert" : "status"}
      {...stylex.props(styles.updateNotice, retryable && styles.updateNoticeError)}
      style={noticeStyle}
    >
      <div {...stylex.props(styles.updateNoticeCopy)}>
        <strong {...stylex.props(styles.updateNoticeTitle)}>{title}</strong>
        {detail ? <span {...stylex.props(styles.updateNoticeDetail)}>{detail}</span> : null}
      </div>
      <Button
        type="button"
        size="sm"
        variant="secondary"
        label={retryable ? "Retry" : "Update"}
        {...stylex.props(styles.updateNoticeAction)}
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
  if (Number.isNaN(timestamp)) return "Unknown";

  const elapsedMilliseconds = Date.now() - timestamp;
  const future = elapsedMilliseconds < 0;
  const elapsedMinutes = Math.round(Math.abs(elapsedMilliseconds) / 60_000);
  if (elapsedMinutes < 1) return "Now";

  const [amount, unit] = elapsedMinutes < 60
    ? [elapsedMinutes, "m"]
    : elapsedMinutes < 1_440
      ? [Math.round(elapsedMinutes / 60), "h"]
      : elapsedMinutes < 43_200
        ? [Math.round(elapsedMinutes / 1_440), "d"]
        : elapsedMinutes < 525_600
          ? [Math.round(elapsedMinutes / 43_200), "mo"]
          : [Math.round(elapsedMinutes / 525_600), "y"];
  return future ? `in ${amount}${unit}` : `${amount}${unit} ago`;
}

function updateNoticeStyle({
  pendingCount,
  retryable,
  updatedAt
}: {
  pendingCount: number;
  retryable: boolean;
  updatedAt: string | null;
}): CSSProperties {
  const stalePercent = retryable || !updatedAt
    ? 100
    : Math.min(100, (pendingCount / COMPACTION_IMMINENT_PENDING_COUNT) * 100);
  return {
    backgroundColor: `color-mix(in oklch, var(--noema-pine-50) ${100 - stalePercent}%, var(--noema-red-100) ${stalePercent}%)`
  };
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
  if (updating) return "Updating";
  if (retryable) return "Update failed";
  if (pendingCount > 0) return "Not up to date";
  return updatedAt ? "Up to date" : "Not updated yet";
}

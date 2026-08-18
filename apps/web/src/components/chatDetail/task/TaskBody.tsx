import * as React from "react";
import { Tab, TabList } from "@astryxdesign/core/TabList";
import * as stylex from "@stylexjs/stylex";
import type { ChatDetailTarget } from "@/components/chatDetail/chatDetailTypes";
import { ProviderCitationMarkdown } from "@/components/transcript/ProviderCitationSources";
import type { TaskDetail, TaskRunItem } from "./taskTypes";
import { TaskTranscript } from "./TaskTranscript";

type TaskTab = "task" | "transcript";
export function TaskBody({
  detail,
  contextCard,
  liveRunItems,
  onOpenDetail,
  onLatestRunItemChange
}: {
  detail: TaskDetail;
  contextCard: React.ReactNode;
  liveRunItems?: ReadonlyMap<string, readonly TaskRunItem[]>;
  onOpenDetail: (target: ChatDetailTarget) => void;
  onLatestRunItemChange?: (runId: string, item: TaskRunItem | null) => void;
}) {
  const [activeTab, setActiveTab] = React.useState<TaskTab>("task");
  const swipeOriginRef = React.useRef<{ x: number; y: number } | null>(null);

  function startSwipe(event: React.TouchEvent) {
    if (event.touches.length !== 1) return;
    const touch = event.touches[0];
    swipeOriginRef.current = { x: touch.clientX, y: touch.clientY };
  }

  function finishSwipe(event: React.TouchEvent) {
    const origin = swipeOriginRef.current;
    swipeOriginRef.current = null;
    const touch = event.changedTouches[0];
    if (!origin || !touch) return;

    const horizontalDistance = touch.clientX - origin.x;
    const verticalDistance = touch.clientY - origin.y;
    if (Math.abs(horizontalDistance) < 56 || Math.abs(horizontalDistance) < Math.abs(verticalDistance) * 1.25) {
      return;
    }

    if (horizontalDistance < 0 && activeTab === "task") {
      setActiveTab("transcript");
    } else if (horizontalDistance > 0 && activeTab === "transcript") {
      setActiveTab("task");
    }
  }

  return (
    <section aria-label="Task detail" {...stylex.props(styles.root)}>
      <div {...stylex.props(styles.tabBar)}>
        <TabList
          aria-label="Task detail view"
          hasDivider
          onChange={(value) => setActiveTab(value as TaskTab)}
          size="sm"
          value={activeTab}
        >
          <Tab label="Task" value="task" />
          <Tab label="Transcript" value="transcript" />
        </TabList>
      </div>
      <div
        {...stylex.props(styles.content)}
        onTouchStart={startSwipe}
        onTouchEnd={finishSwipe}
        onTouchCancel={() => { swipeOriginRef.current = null; }}
      >
        {activeTab === "task" ? (
          <TaskDocument detail={detail} />
        ) : (
          <div {...stylex.props(styles.transcript)}>
            <TaskTranscript
              detail={detail}
              liveRunItems={liveRunItems}
              onOpenDetail={onOpenDetail}
              onLatestRunItemChange={onLatestRunItemChange}
            />
          </div>
        )}
      </div>
      {contextCard}
    </section>
  );
}

function TaskDocument({ detail }: { detail: TaskDetail }) {
  const response = detail.taskDocument.trim() || undefined;

  return (
    <div data-slot="task-document" {...stylex.props(styles.taskScroller)}>
      <div {...stylex.props(styles.taskContent)}>
        {response ? (
          <ProviderCitationMarkdown
            contentAlign="center"
            contentWidth="min(760px, calc(100% - var(--spacing-6) - var(--spacing-6)))"
            density="default"
            headingLevelStart={1}
            citations={[]}
            text={response}
            xstyle={styles.markdown}
          />
        ) : (
          <p {...stylex.props(styles.empty)}>TASK.md has no text content.</p>
        )}
        {detail.reviewDocument?.trim() ? (
          <section aria-label="Review feedback" {...stylex.props(styles.review)}>
            <ProviderCitationMarkdown
              contentAlign="center"
              contentWidth="min(760px, calc(100% - var(--spacing-6) - var(--spacing-6)))"
              density="default"
              headingLevelStart={2}
              citations={[]}
              text={detail.reviewDocument}
              xstyle={styles.markdown}
            />
          </section>
        ) : null}
      </div>
    </div>
  );
}

const styles = stylex.create({
  root: {
    display: "grid",
    gridTemplateRows: "auto minmax(0, 1fr) auto",
    minWidth: 0,
    minHeight: 0
  },
  tabBar: {
    minWidth: 0,
    paddingInline: "var(--spacing-1)"
  },
  content: {
    minWidth: 0,
    minHeight: 0,
    overflow: "hidden"
  },
  transcript: {
    minWidth: 0,
    minHeight: 0,
    height: "100%",
    "--chat-transcript-top-fade": "var(--spacing-4)",
    "--task-transcript-bottom-inset": "var(--spacing-3)"
  },
  taskScroller: {
    height: "100%",
    overflowX: "hidden",
    overflowY: "auto",
    paddingBlock: "var(--spacing-4) var(--spacing-6)"
  },
  taskContent: {
    display: "grid",
    gap: "var(--spacing-4)",
    width: "100%"
  },
  markdown: {
    color: "var(--noema-text-primary)",
    fontSize: 14,
    lineHeight: 1.6
  },
  review: { borderTop: "1px solid var(--noema-border-subtle)", paddingBlockStart: "var(--spacing-4)" },
  empty: {
    width: "calc(100% - var(--spacing-6) - var(--spacing-6))",
    maxWidth: 760,
    marginInline: "auto",
    marginBlock: "var(--spacing-0)",
    color: "var(--noema-text-secondary)",
    fontSize: 13
  }
});

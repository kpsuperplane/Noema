import * as React from "react";
import { Tab, TabList } from "@astryxdesign/core/TabList";
import * as stylex from "@stylexjs/stylex";
import type { ChatDetailTarget } from "@/components/chatDetail/chatDetailTypes";
import { ProviderCitationMarkdown, taskResultCitationContent } from "@/components/transcript/ProviderCitationSources";
import type { TaskDetail, TaskRunItem } from "./taskTypes";
import type { TaskRunLatestEntryChange } from "./TaskRunTranscript";
import { TaskTranscript } from "./TaskTranscript";

type TaskTab = "result" | "task" | "transcript";
export function TaskBody({
  detail,
  liveRunItems,
  onOpenDetail,
  onLatestRunEntryChange,
  renderContextCard
}: {
  detail: TaskDetail;
  liveRunItems?: ReadonlyMap<string, readonly TaskRunItem[]>;
  onOpenDetail: (target: ChatDetailTarget) => void;
  onLatestRunEntryChange?: TaskRunLatestEntryChange;
  renderContextCard: (showStatus: boolean) => React.ReactNode;
}) {
  const hasResult = Boolean(detail.resultDocument?.trim());
  const tabs = React.useMemo<readonly TaskTab[]>(
    () => hasResult ? ["result", "task", "transcript"] : ["task", "transcript"],
    [hasResult]
  );
  const [tabState, setTabState] = React.useState(() => ({
    taskId: detail.taskId,
    hasResult,
    activeTab: (hasResult ? "result" : "task") as TaskTab
  }));
  let activeTab = tabState.activeTab;
  const swipeOriginRef = React.useRef<{ x: number; y: number } | null>(null);

  if (tabState.taskId !== detail.taskId || tabState.hasResult !== hasResult) {
    activeTab = tabState.taskId !== detail.taskId
      ? hasResult ? "result" : "task"
      : activeTab === "result" && !hasResult ? "task" : activeTab;
    setTabState({ taskId: detail.taskId, hasResult, activeTab });
  }

  function selectTab(tab: TaskTab) {
    setTabState((current) => ({ ...current, activeTab: tab }));
  }

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

    const currentIndex = tabs.indexOf(activeTab);
    const nextIndex = horizontalDistance < 0 ? currentIndex + 1 : currentIndex - 1;
    const nextTab = tabs[nextIndex];
    if (nextTab) {
      selectTab(nextTab);
    }
  }

  return (
    <section aria-label="Task detail" {...stylex.props(styles.root)}>
      <div {...stylex.props(styles.tabBar)}>
        <TabList
          aria-label="Task detail view"
          hasDivider
          onChange={(value) => selectTab(value as TaskTab)}
          size="sm"
          value={activeTab}
        >
          {hasResult ? <Tab label="Result" value="result" /> : null}
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
        {activeTab === "result" ? (
          <TaskDocument fileName="RESULT.md" text={detail.resultDocument ?? ""} />
        ) : activeTab === "task" ? (
          <TaskDocument fileName="TASK.md" text={detail.taskDocument} />
        ) : (
          <div {...stylex.props(styles.transcript)}>
            <TaskTranscript
              detail={detail}
              liveRunItems={liveRunItems}
              onOpenDetail={onOpenDetail}
              onLatestRunEntryChange={onLatestRunEntryChange}
            />
          </div>
        )}
      </div>
      {renderContextCard(activeTab !== "transcript")}
    </section>
  );
}

function TaskDocument({ fileName, text }: { fileName: string; text: string }) {
  const response = text.trim() || undefined;
  const content = response && fileName === "RESULT.md"
    ? taskResultCitationContent(response)
    : { text: response ?? "", citations: [] };

  return (
    <div data-slot="task-document" {...stylex.props(styles.taskScroller)}>
      <div {...stylex.props(styles.taskContent)}>
        {response ? (
          <ProviderCitationMarkdown
            contentAlign="center"
            contentWidth="min(760px, calc(100% - var(--spacing-6) - var(--spacing-6)))"
            density="default"
            headingLevelStart={1}
            citations={content.citations}
            text={content.text}
            xstyle={styles.markdown}
          />
        ) : (
          <p {...stylex.props(styles.empty)}>{fileName} has no text content.</p>
        )}
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
  empty: {
    width: "calc(100% - var(--spacing-6) - var(--spacing-6))",
    maxWidth: 760,
    marginInline: "auto",
    marginBlock: "var(--spacing-0)",
    color: "var(--noema-text-secondary)",
    fontSize: 13
  }
});

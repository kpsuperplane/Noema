import * as React from "react";
import { Tab, TabList } from "@astryxdesign/core/TabList";
import { IconButton } from "@astryxdesign/core/IconButton";
import * as stylex from "@stylexjs/stylex";
import { Check, Pencil, RefreshCcw, X } from "lucide-react";
import type { ChatDetailTarget } from "@/components/chatDetail/chatDetailTypes";
import { ProviderCitationMarkdown, taskResultCitationContent } from "@/components/transcript/ProviderCitationSources";
import type { TaskDetail, TaskRunItem } from "./taskTypes";
import { taskStageLabel } from "./TaskOverview";
import type { TaskRunLatestEntryChange } from "./TaskRunTranscript";
import { TaskTranscript } from "./TaskTranscript";
import { TaskMarkdownEditor } from "@/components/tasks/TaskMarkdownEditor";
import type { TaskInlineEditController } from "@/components/tasks/TaskActions";

type TaskTab = "result" | "task" | "transcript";
export function TaskBody({
  detail,
  edit,
  liveRunItems,
  onOpenDetail,
  onLatestRunEntryChange,
  renderContextCard
}: {
  detail: TaskDetail;
  edit?: TaskInlineEditController;
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
  if (edit?.field === "DOCUMENT") activeTab = "task";

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
          <TaskDocument detail={detail} edit={edit} fileName="TASK.md" text={detail.taskDocument} />
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

function TaskDocument({ detail, edit, fileName, text }: { detail?: TaskDetail; edit?: TaskInlineEditController; fileName: string; text: string }) {
  if (detail && edit?.field === "DOCUMENT") {
    return <TaskDocumentEditor key={`${detail.taskId}:document`} detail={detail} edit={edit} />;
  }
  const response = text.trim() || undefined;
  const content = response && fileName === "RESULT.md"
    ? taskResultCitationContent(response)
    : { text: response ?? "", citations: [] };

  return (
    <div data-slot="task-document" {...stylex.props(styles.taskScroller, !detail && styles.resultScroller)}>
      {detail ? <TaskDocumentBar edit={edit} /> : null}
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
        {detail ? <TaskMetadata detail={detail} /> : null}
      </div>
    </div>
  );
}

function TaskDocumentEditor({ detail, edit }: { detail: TaskDetail; edit: TaskInlineEditController }) {
  const [draft, setDraft] = React.useState(detail.taskDocument);
  return (
    <div data-slot="task-document" {...stylex.props(styles.taskScroller)}>
      <TaskDocumentBar edit={edit} onSave={() => edit.saveDocument(draft)} />
      <div {...stylex.props(styles.editorContent, Boolean(edit.error || edit.actionUnavailable) && styles.editorError)}>
        <TaskMarkdownEditor key={edit.task.taskDocumentDigest} value={draft} onChange={setDraft} label="Task description" />
        {edit.error ? <span role="alert" {...stylex.props(styles.srOnly)}>{edit.error}</span> : null}
      </div>
      <TaskMetadata detail={detail} />
    </div>
  );
}

function TaskDocumentBar({ edit, onSave }: { edit?: TaskInlineEditController; onSave?: () => Promise<void> }) {
  const editing = edit?.field === "DOCUMENT";
  return (
    <div {...stylex.props(styles.documentBar)}>
      <strong {...stylex.props(styles.documentLabel)}>Description</strong>
      <span {...stylex.props(styles.editControls)}>
        {!edit?.canEdit ? null : editing ? (
          <>
            {edit.requiresAcknowledgement ? <IconButton type="button" size="sm" variant="ghost" label="Use latest task" tooltip="Use latest task" icon={<RefreshCcw aria-hidden="true" size={14} />} isDisabled={edit.busy} onClick={() => void edit.acknowledge().catch(() => undefined)} /> : <IconButton type="button" size="sm" variant="ghost" label="Save description" tooltip="Save description" icon={<Check aria-hidden="true" size={14} />} isDisabled={edit.busy || edit.actionUnavailable} onClick={() => void onSave?.().catch(() => undefined)} />}
            <IconButton type="button" size="sm" variant="ghost" label="Cancel description edit" tooltip="Cancel" icon={<X aria-hidden="true" size={14} />} isDisabled={edit.busy} onClick={edit.cancel} />
          </>
        ) : (
          <IconButton type="button" size="sm" variant="ghost" label="Edit description" tooltip="Edit description" icon={<Pencil aria-hidden="true" size={14} />} isDisabled={edit.busy || !edit.canStart} onClick={() => void edit.start("DOCUMENT")} />
        )}
      </span>
    </div>
  );
}

function TaskMetadata({ detail }: { detail: TaskDetail }) {
  const revision = detail.currentRevision ?? latestRevision(detail);
  const provenance = [detail.createdBy, detail.sourceLabel].filter(Boolean).join(" · ");
  return (
    <dl {...stylex.props(styles.metadata)}>
      <MetadataRow label="Stage" value={taskStageLabel(detail)} />
      {revision > 0 ? <MetadataRow label="Revision" value={`${revision}`} /> : null}
      {detail.createdAt ? <MetadataRow label="Created" value={formatDate(detail.createdAt)} /> : null}
      {provenance ? <MetadataRow label="From" value={provenance} /> : null}
    </dl>
  );
}

function MetadataRow({ label, value }: { label: string; value: string }) {
  return <div {...stylex.props(styles.metadataRow)}><dt {...stylex.props(styles.metadataKey)}>{label}</dt><dd {...stylex.props(styles.metadataValue)}>{value}</dd></div>;
}

function latestRevision(detail: TaskDetail): number {
  return detail.revisions.reduce((latest, revision) => Math.max(latest, revision.revision), 0);
}

function formatDate(value: string): string {
  const timestamp = Date.parse(value);
  return Number.isNaN(timestamp)
    ? value
    : new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(timestamp);
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
    paddingBlockEnd: "var(--spacing-6)"
  },
  resultScroller: { paddingBlockStart: "var(--spacing-4)" },
  documentBar: { display: "grid", gridTemplateColumns: "minmax(0, 1fr) 64px", alignItems: "center", width: "calc(100% - var(--spacing-6) - var(--spacing-6))", maxWidth: 760, minHeight: 36, marginInline: "auto", marginBlockEnd: "var(--spacing-2)" },
  documentLabel: { color: "var(--noema-text-muted)", fontSize: 12, fontWeight: 650 },
  editControls: { display: "inline-flex", width: 64, minHeight: 28, alignItems: "center", justifyContent: "flex-end", gap: "var(--spacing-1)" },
  editorContent: { width: "calc(100% - var(--spacing-6) - var(--spacing-6))", maxWidth: 760, marginInline: "auto", borderRadius: "var(--radius-element)" },
  editorError: { outlineWidth: 1, outlineStyle: "solid", outlineColor: "var(--destructive)" },
  taskContent: {
    display: "grid",
    gap: "var(--spacing-4)",
    width: "100%"
  },
  metadata: {
    display: "grid",
    gap: "var(--spacing-1-5)",
    width: "calc(100% - var(--spacing-6) - var(--spacing-6))",
    maxWidth: 760,
    marginInline: "auto",
    marginBlock: "var(--spacing-0)",
    paddingBlockStart: "var(--spacing-3)",
    borderTopWidth: 1,
    borderTopStyle: "solid",
    borderTopColor: "var(--noema-border-subtle)"
  },
  metadataRow: { display: "grid", gridTemplateColumns: "minmax(88px, 0.42fr) minmax(0, 1fr)", gap: "var(--spacing-3)", alignItems: "baseline" },
  metadataKey: { color: "var(--noema-text-muted)", fontSize: 12 },
  metadataValue: { minWidth: 0, margin: "var(--spacing-0)", color: "var(--noema-text-secondary)", fontSize: 12, overflowWrap: "anywhere" },
  srOnly: { position: "absolute", width: 1, height: 1, overflow: "hidden", clip: "rect(0 0 0 0)", whiteSpace: "nowrap" },
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

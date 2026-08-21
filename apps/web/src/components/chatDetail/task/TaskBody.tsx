import * as React from "react";
import { useQuery } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import { HStack } from "@astryxdesign/core/HStack";
import { MoreMenu } from "@astryxdesign/core/MoreMenu";
import { Tab, TabList } from "@astryxdesign/core/TabList";
import { IconButton } from "@astryxdesign/core/IconButton";
import * as stylex from "@stylexjs/stylex";
import { Pencil } from "lucide-react";
import { MarkdownContent } from "@/components/MarkdownContent";
import type { ChatDetailTarget } from "@/components/chatDetail/chatDetailTypes";
import { ProviderCitationMarkdown, providerCitationsFromMetadata } from "@/components/transcript/ProviderCitationSources";
import { TasksTaskWorkspaceFileDocument } from "@/generated/graphql";
import type { TaskDetail, TaskRunItem, TaskWorkspaceFile } from "./taskTypes";
import { taskStageLabel } from "./TaskOverview";
import type { TaskRunLatestEntryChange } from "./TaskRunTranscript";
import { TaskTranscript } from "./TaskTranscript";
import { TaskDocumentInlineEditor } from "@/components/tasks/TaskMarkdownEditor";
import type { TaskInlineEditController } from "@/components/tasks/TaskActions";

const TASK_DOCUMENT_PATH = "TASK.md";
const TASK_RESULT_PATH = "RESULT.md";
const TASK_REVIEW_PATH = "REVIEW.md";
const TASK_TABS = ["workspace", "transcript"] as const;
type TaskTab = (typeof TASK_TABS)[number];

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
  const [tabState, setTabState] = React.useState(() => ({
    taskId: detail.taskId,
    activeTab: "workspace" as TaskTab
  }));
  const [workspaceState, setWorkspaceState] = React.useState(() => initialWorkspaceState(detail));
  let activeTab = tabState.activeTab;
  let selectedPath = workspaceState.selectedPath;
  const swipeOriginRef = React.useRef<{ x: number; y: number } | null>(null);
  const selectWorkspacePath = React.useCallback((path: string) => {
    setWorkspaceState((current) => ({ ...current, selectedPath: path }));
  }, []);

  if (tabState.taskId !== detail.taskId) {
    activeTab = "workspace";
    setTabState({ taskId: detail.taskId, activeTab });
  }
  if (edit?.field === "DOCUMENT") activeTab = "workspace";

  const nextWorkspaceState = reconcileWorkspaceState(workspaceState, detail, edit?.field === "DOCUMENT");
  if (nextWorkspaceState !== workspaceState) {
    selectedPath = nextWorkspaceState.selectedPath;
    setWorkspaceState(nextWorkspaceState);
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

    const currentIndex = TASK_TABS.indexOf(activeTab);
    const nextIndex = horizontalDistance < 0 ? currentIndex + 1 : currentIndex - 1;
    const nextTab = TASK_TABS[nextIndex];
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
          <Tab label="Workspace" value="workspace" />
          <Tab label="Transcript" value="transcript" />
        </TabList>
      </div>
      <div
        {...stylex.props(styles.content)}
        onTouchStart={startSwipe}
        onTouchEnd={finishSwipe}
        onTouchCancel={() => { swipeOriginRef.current = null; }}
      >
        {activeTab === "workspace" ? (
          <TaskWorkspace
            detail={detail}
            edit={edit}
            selectedPath={selectedPath}
            onSelectPath={selectWorkspacePath}
          />
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

function TaskWorkspace({
  detail,
  edit,
  selectedPath,
  onSelectPath
}: {
  detail: TaskDetail;
  edit?: TaskInlineEditController;
  selectedPath: string;
  onSelectPath: (path: string) => void;
}) {
  const files = React.useMemo(
    () => detail.workspaceFiles
      .filter((file) => !file.isDirectory)
      .sort((left, right) => left.path.localeCompare(right.path)),
    [detail.workspaceFiles]
  );
  const directFiles = [TASK_RESULT_PATH, TASK_DOCUMENT_PATH]
    .flatMap((path) => files.filter((file) => file.path === path));
  const otherFiles = files.filter((file) => file.path !== TASK_RESULT_PATH && file.path !== TASK_DOCUMENT_PATH);
  return (
    <section aria-label="Task workspace" {...stylex.props(styles.workspace)}>
      <section aria-label={selectedPath} {...stylex.props(styles.fileViewer)}>
        <nav aria-label="Task files" {...stylex.props(styles.fileBar)}>
          <HStack
            gap={1}
            width="100%"
            xstyle={styles.fileActions}
          >
            {directFiles.map((file) => (
              <Button
                key={file.path}
                aria-pressed={file.path === selectedPath}
                label={workspaceFileButtonLabel(file)}
                onClick={() => onSelectPath(file.path)}
                size="sm"
                tooltip={file.path}
                variant={file.path === selectedPath ? "secondary" : "ghost"}
              />
            ))}
            {otherFiles.length > 0 ? (
              <MoreMenu
                items={otherFiles.map((file) => ({
                  label: workspaceFileButtonLabel(file),
                  onClick: () => onSelectPath(file.path)
                }))}
                label="More files"
                size="sm"
                variant={otherFiles.some((file) => file.path === selectedPath) ? "secondary" : "ghost"}
              />
            ) : null}
          </HStack>
          {detail.workspaceFilesTruncated ? (
            <p role="status" {...stylex.props(styles.fileNotice)}>Some files are not shown.</p>
          ) : null}
        </nav>
        <TaskWorkspaceFileViewer detail={detail} edit={edit} path={selectedPath} />
      </section>
    </section>
  );
}

function TaskWorkspaceFileViewer({ detail, edit, path }: { detail: TaskDetail; edit?: TaskInlineEditController; path: string }) {
  if (path === TASK_DOCUMENT_PATH) {
    return <TaskDocument detail={detail} edit={edit} fileName={path} text={detail.taskDocument} />;
  }
  if (path === TASK_RESULT_PATH) {
    return <TaskDocument citations={providerCitationsFromMetadata(detail.resultMetadata)} fileName={path} text={detail.resultDocument ?? ""} />;
  }
  if (path === TASK_REVIEW_PATH && detail.reviewDocument != null) {
    return <WorkspaceTextFile fileName={path} text={detail.reviewDocument} />;
  }
  return <TaskWorkspaceSupportFile path={path} taskId={detail.taskId} />;
}

function TaskWorkspaceSupportFile({ path, taskId }: { path: string; taskId: string }) {
  const result = useQuery(TasksTaskWorkspaceFileDocument, {
    variables: { taskId, path },
    fetchPolicy: "cache-and-network"
  });
  const file = result.data?.taskWorkspaceFile;
  if (!file || file.path !== path) {
    return <p role="status" {...stylex.props(styles.workspaceStatus)}>{result.error ? "This file cannot be previewed." : "Loading file..."}</p>;
  }
  return <WorkspaceTextFile fileName={path} markdown={path.toLowerCase().endsWith(".md")} text={file.content} />;
}

function WorkspaceTextFile({ fileName, markdown = true, text }: { fileName: string; markdown?: boolean; text: string }) {
  const content = text.trim() || undefined;
  return (
    <div data-slot="task-document" {...stylex.props(styles.taskScroller)}>
      {content ? markdown ? (
        <MarkdownContent density="compact" className={stylex.props(styles.taskDescription).className}>{content}</MarkdownContent>
      ) : (
        <pre {...stylex.props(styles.plainText)}>{text}</pre>
      ) : (
        <p {...stylex.props(styles.empty)}>{fileName} has no text content.</p>
      )}
    </div>
  );
}

function initialWorkspaceState(detail: TaskDetail) {
  return {
    taskId: detail.taskId,
    resultAvailable: detail.status === "done" && Boolean(detail.resultDocument?.trim()),
    selectedPath: defaultWorkspacePath(detail)
  };
}

function reconcileWorkspaceState(
  current: ReturnType<typeof initialWorkspaceState>,
  detail: TaskDetail,
  editingTaskDocument: boolean
) {
  if (current.taskId !== detail.taskId) return initialWorkspaceState(detail);
  const resultAvailable = detail.status === "done" && Boolean(detail.resultDocument?.trim());
  const resultArrived = resultAvailable && !current.resultAvailable;
  const selectedPath = editingTaskDocument
    ? TASK_DOCUMENT_PATH
    : resultArrived && current.selectedPath === TASK_DOCUMENT_PATH
      ? TASK_RESULT_PATH
      : detail.workspaceFiles.some((file) => !file.isDirectory && file.path === current.selectedPath)
        ? current.selectedPath
        : defaultWorkspacePath(detail);
  if (current.resultAvailable === resultAvailable && current.selectedPath === selectedPath) return current;
  return { ...current, resultAvailable, selectedPath };
}

function defaultWorkspacePath(detail: TaskDetail): string {
  return detail.status === "done" && detail.resultDocument?.trim()
    ? TASK_RESULT_PATH
    : TASK_DOCUMENT_PATH;
}

function workspaceFileLabel(file: TaskWorkspaceFile): string {
  const name = file.path.slice(file.path.lastIndexOf("/") + 1);
  const withoutMarkdownExtension = !file.isDirectory && name.toLowerCase().endsWith(".md")
    ? name.slice(0, -3)
    : name;
  return withoutMarkdownExtension === withoutMarkdownExtension.toUpperCase()
    ? `${withoutMarkdownExtension.slice(0, 1).toUpperCase()}${withoutMarkdownExtension.slice(1).toLowerCase()}`
    : withoutMarkdownExtension;
}

function workspaceFileButtonLabel(file: TaskWorkspaceFile): string {
  const separator = file.path.lastIndexOf("/");
  return separator < 0
    ? workspaceFileLabel(file)
    : `${file.path.slice(0, separator + 1)}${workspaceFileLabel(file)}`;
}

function TaskDocument({ citations = [], detail, edit, fileName, text }: { citations?: Parameters<typeof ProviderCitationMarkdown>[0]["citations"]; detail?: TaskDetail; edit?: TaskInlineEditController; fileName: string; text: string }) {
  if (detail && edit?.field === "DOCUMENT") {
    return <div data-slot="task-document" {...stylex.props(styles.taskScroller)}>
      <TaskDocumentInlineEditor key={`${detail.taskId}:document`} className={stylex.props(styles.editorContent).className} document={detail.taskDocument} digest={edit.task.taskDocumentDigest} edit={edit} label="Task description" onSave={edit.saveDocument} />
      <TaskMetadata detail={detail} />
    </div>;
  }
  const response = text.trim() || undefined;

  return (
    <div data-slot="task-document" {...stylex.props(styles.taskScroller)}>
      {detail ? <TaskDocumentBar edit={edit} /> : null}
      <div {...stylex.props(styles.taskContent)}>
        {response && detail ? (
          <MarkdownContent density="compact" className={stylex.props(styles.taskDescription).className}>{response}</MarkdownContent>
        ) : response ? (
          <ProviderCitationMarkdown
            className={stylex.props(styles.taskDescription).className}
            contentWidth="100%"
            density="default"
            headingLevelStart={1}
            citations={citations}
            text={response}
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

function TaskDocumentBar({ edit }: { edit?: TaskInlineEditController }) {
  if (!edit?.canEdit) return null;

  return (
    <div {...stylex.props(styles.documentBar)}>
      <span {...stylex.props(styles.editControls)}>
        <IconButton type="button" size="sm" variant="ghost" label="Edit description" tooltip="Edit description" icon={<Pencil aria-hidden="true" size={14} />} isDisabled={edit.busy || !edit.canStart} onClick={() => void edit.start("DOCUMENT")} />
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
    containerType: "inline-size",
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
  workspace: {
    minWidth: 0,
    minHeight: 0,
    height: "100%",
    "--task-workspace-content-width": "max(0px, min(760px, calc(100cqw - var(--spacing-6) - var(--spacing-6))))",
    "--task-workspace-table-left-bleed": "max(0px, calc((100cqw - var(--task-workspace-content-width)) / 2))",
    "--task-workspace-table-right-bleed": "calc(100cqw - var(--task-workspace-table-left-bleed) - var(--task-workspace-content-width))",
  },
  fileBar: {
    width: "calc(100% - var(--spacing-6) - var(--spacing-6))",
    maxWidth: 760,
    minWidth: 0,
    marginInline: "auto",
    paddingBlockStart: "var(--spacing-4)",
  },
  fileActions: { minWidth: 0 },
  fileNotice: { margin: "var(--spacing-1) var(--spacing-0) var(--spacing-0)", color: "var(--noema-text-muted)", fontSize: 12 },
  fileViewer: {
    width: "100%",
    minWidth: 0,
    minHeight: 0,
    height: "100%",
    overflowX: "hidden",
    overflowY: "auto"
  },
  workspaceStatus: { margin: "var(--spacing-0)", padding: "var(--spacing-4)", color: "var(--noema-text-secondary)", fontSize: 13 },
  taskScroller: {
    overflowX: "hidden",
    paddingBlockStart: "var(--spacing-2)",
    paddingBlockEnd: "var(--spacing-6)"
  },
  documentBar: { display: "flex", justifyContent: "flex-end", alignItems: "center", width: "calc(100% - var(--spacing-6) - var(--spacing-6))", maxWidth: 760, minHeight: 36, marginInline: "auto", marginBlockEnd: "var(--spacing-2)" },
  editControls: { display: "inline-flex", width: "100%", minHeight: 28, alignItems: "center", justifyContent: "flex-end", gap: "var(--spacing-1)" },
  editorContent: { width: "calc(100% - var(--spacing-6) - var(--spacing-6))", maxWidth: 760, marginInline: "auto", borderRadius: "var(--radius-element)" },
  taskDescription: { width: "calc(100% - var(--spacing-6) - var(--spacing-6))", maxWidth: 760, marginInline: "auto" },
  plainText: { width: "calc(100% - var(--spacing-6) - var(--spacing-6))", maxWidth: 760, margin: "var(--spacing-0) auto", color: "var(--noema-text-primary)", fontFamily: "var(--noema-font-mono)", fontSize: 13, lineHeight: 1.55, whiteSpace: "pre-wrap", overflowWrap: "anywhere" },
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

import { TaskDocumentLayout } from "@/components/tasks/TaskDocumentLayout";
import * as React from "react";
import { useQuery } from "@apollo/client/react";
import { Grid } from "@astryxdesign/core/Grid";
import { HStack } from "@astryxdesign/core/HStack";
import { Tab, TabList } from "@astryxdesign/core/TabList";
import { VStack } from "@astryxdesign/core/VStack";
import { Link } from "@tanstack/react-router";
import { normalizeTasksSearch } from "@/components/tasks/tasksTypes";
import { TaskScheduleSummary } from "@/components/tasks/TaskScheduleSummary";
import { TaskInstructionsField } from "@/components/tasks/TaskDocumentFields";
import * as stylex from "@stylexjs/stylex";
import { skeletonGlimmerStyles } from "@/components/skeletonGlimmerStyles";
import { MarkdownContent } from "@/components/MarkdownContent";
import type { ChatDetailTarget } from "@/components/chatDetail/chatDetailTypes";
import { ProviderCitationMarkdown, providerCitationsFromMetadata } from "@/components/transcript/ProviderCitationSources";
import { TasksTaskWorkspaceFileDocument } from "@/generated/graphql";
import type { TaskDetail, TaskRunItem, TaskWorkspaceFile } from "./taskTypes";
import type { TaskRunLatestEntryChange } from "./TaskRunTranscript";
import { TaskTranscript } from "./TaskTranscript";
import type { TaskInlineEditController } from "@/components/tasks/TaskActions";

const TASK_DOCUMENT_PATH = "TASK.md";
const TASK_RESULT_PATH = "RESULT.md";
const TASK_REVIEW_PATH = "REVIEW.md";
type TaskTab = "workspace" | "transcript";

export function TaskBody({
  detail,
  header,
  edit,
  liveRunItems,
  onOpenDetail,
  onLatestRunEntryChange,
  renderContextCard
}: {
  detail: TaskDetail;
  header?: React.ReactNode;
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
  const rootRef = React.useRef<HTMLElement>(null);
  const [wideLayout, setWideLayout] = React.useState(false);
  React.useLayoutEffect(() => {
    const root = rootRef.current;
    if (!root) return;
    const updateLayout = () => setWideLayout(root.clientWidth > 1200);
    updateLayout();
    const observer = new ResizeObserver(updateLayout);
    observer.observe(root);
    return () => observer.disconnect();
  }, []);
  const files = React.useMemo(() => {
    const files = detail.workspaceFiles.filter((file) => !file.isDirectory)
      .sort((left, right) => left.path.localeCompare(right.path));
    return [
      ...[TASK_RESULT_PATH, TASK_DOCUMENT_PATH].flatMap((path) => files.filter((file) => file.path === path)),
      ...files.filter((file) => file.path !== TASK_RESULT_PATH && file.path !== TASK_DOCUMENT_PATH)
    ];
  }, [detail.workspaceFiles]);
  const [workspaceState, setWorkspaceState] = React.useState(() => initialWorkspaceState(detail));
  let activeTab = tabState.activeTab;
  let selectedPath = workspaceState.selectedPath;
  const swipeOriginRef = React.useRef<{ x: number; y: number } | null>(null);

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

  const selectedTab = !wideLayout && activeTab === "transcript" ? "transcript" : selectedPath;
  const tabs = [...files.map((file) => file.path), ...(!wideLayout ? ["transcript"] : [])];

  function selectTab(value: string) {
    setTabState((current) => ({ ...current, activeTab: value === "transcript" ? "transcript" : "workspace" }));
    if (value !== "transcript") {
      setWorkspaceState((current) => ({ ...current, selectedPath: value }));
    }
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

    const currentIndex = tabs.indexOf(selectedTab);
    const nextIndex = horizontalDistance < 0 ? currentIndex + 1 : currentIndex - 1;
    const nextTab = tabs[nextIndex];
    if (nextTab) {
      selectTab(nextTab);
    }
  }

  function cancelSwipe() {
    swipeOriginRef.current = null;
  }

  return (
    <section ref={rootRef} aria-label="Task detail" {...stylex.props(styles.root)}>
      <Grid
        height="100%"
        xstyle={styles.frame}
      >
        <VStack gap={0} xstyle={styles.heading}>
          {header}
          <HStack xstyle={styles.tabBar}>
            <TabList
              aria-label="Task detail view"
              onChange={selectTab}
              size="sm"
              value={selectedTab}
            >
              {files.map((file) => <Tab key={file.path} label={workspaceFileButtonLabel(file)} value={file.path} />)}
              {!wideLayout ? <Tab label="Transcript" value="transcript" /> : null}
            </TabList>
          </HStack>
        </VStack>
        <section
          aria-label="Workspace"
          {...stylex.props(styles.workspacePane, activeTab !== "workspace" && styles.inactivePane)}
          onTouchStart={startSwipe}
          onTouchEnd={finishSwipe}
          onTouchCancel={cancelSwipe}
        >
          <TaskWorkspace
            detail={detail}
            edit={edit}
            selectedPath={selectedPath}
          />
        </section>
        <section
          aria-label="Transcript"
          {...stylex.props(styles.transcript, activeTab !== "transcript" && styles.inactivePane)}
          onTouchStart={startSwipe}
          onTouchEnd={finishSwipe}
          onTouchCancel={cancelSwipe}
        >
          <TaskTranscript
            detail={detail}
            liveRunItems={liveRunItems}
            onOpenDetail={onOpenDetail}
            onLatestRunEntryChange={onLatestRunEntryChange}
          />
        </section>
        <section aria-label="Task context" {...stylex.props(styles.contextPane)}>
          {renderContextCard(activeTab !== "transcript")}
        </section>
      </Grid>
    </section>
  );
}

export function TaskLoadingSkeleton({ header, animateGlimmer = true }: { header?: React.ReactNode; animateGlimmer?: boolean }) {
  const lines = <VStack gap={3} aria-hidden="true" xstyle={styles.loadingLines}>
    {["72%", "100%", "92%", "64%"].map((width) => <HStack key={width} style={{ width }} xstyle={[styles.loadingLine, animateGlimmer && skeletonGlimmerStyles.animated]} />)}
  </VStack>;
  return <section aria-label="Loading task details" aria-busy="true" {...stylex.props(styles.root)}>
    <Grid height="100%" xstyle={styles.frame}>
      <VStack gap={0} xstyle={styles.heading}>
        {header ?? <VStack aria-hidden="true" xstyle={styles.loadingHeader}><HStack xstyle={[styles.loadingLine, styles.loadingTitle, animateGlimmer && skeletonGlimmerStyles.animated]} /></VStack>}
        <HStack inert xstyle={styles.tabBar}>
          <TabList aria-label="Task detail view" size="sm" value={TASK_DOCUMENT_PATH} onChange={() => undefined}>
            <Tab label="Task" value={TASK_DOCUMENT_PATH} /><Tab label="Transcript" value="transcript" xstyle={styles.narrowTab} />
          </TabList>
        </HStack>
      </VStack>
      <section aria-label="Workspace" {...stylex.props(styles.workspacePane)}>{lines}</section>
      <section aria-label="Transcript" {...stylex.props(styles.transcript, styles.inactivePane)}>{lines}</section>
      <section aria-label="Task context" {...stylex.props(styles.contextPane)}>
        <HStack aria-hidden="true" xstyle={[styles.loadingContext, animateGlimmer && skeletonGlimmerStyles.animated]} />
      </section>
    </Grid>
  </section>;
}

function TaskWorkspace({ detail, edit, selectedPath }: {
  detail: TaskDetail;
  edit?: TaskInlineEditController;
  selectedPath: string;
}) {
  return (
    <section aria-label="Task workspace" {...stylex.props(styles.workspace)}>
      <section aria-label={selectedPath} {...stylex.props(styles.fileViewer)}>
        {detail.workspaceFilesTruncated ? (
          <p role="status" {...stylex.props(styles.fileNotice)}>Some files are not shown.</p>
        ) : null}
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
    <TaskDocumentLayout>
      {content ? markdown ? (
        <MarkdownContent density="compact">{content}</MarkdownContent>
      ) : (
        <pre {...stylex.props(styles.plainText)}>{text}</pre>
      ) : (
        <p {...stylex.props(styles.empty)}>{fileName} has no text content.</p>
      )}
    </TaskDocumentLayout>
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
  if (detail) return <TaskDocumentLayout>
    {edit?.options}
    {detail.schedule?.recurrenceId ? <Link to="/tasks/recurrences/$recurrenceId" params={{ recurrenceId: detail.schedule.recurrenceId }} search={(current) => normalizeTasksSearch(current)}>View recurring task</Link> : detail.schedule || detail.stageBehavior === "INTAKE" ? <TaskScheduleSummary schedule={detail.schedule}>{edit?.timing}</TaskScheduleSummary> : null}
    <TaskInstructionsField value={detail.taskDocument} edit={edit} />
  </TaskDocumentLayout>;
  const response = text.trim() || undefined;

  return (
    <TaskDocumentLayout>
        {response ? (
          <ProviderCitationMarkdown
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
    </TaskDocumentLayout>
  );
}

const styles = stylex.create({
  root: {
    containerType: "inline-size",
    minWidth: 0,
    minHeight: 0,
    height: "100%"
  },
  loadingHeader: { width: "calc(100% - var(--spacing-6) - var(--spacing-6))", maxWidth: 760, marginInline: "auto", paddingBlockStart: "var(--spacing-3)", paddingBlockEnd: "var(--spacing-1)" },
  loadingTitle: { width: "60%", height: "calc(var(--text-heading-3-size) * var(--text-heading-3-leading))" },
  loadingLine: { height: "var(--spacing-3)", borderRadius: "var(--radius-element)", backgroundColor: "var(--skeleton-glimmer-line)" },
  loadingLines: { width: "calc(100% - var(--spacing-6) - var(--spacing-6))", maxWidth: 760, marginInline: "auto", paddingBlock: "var(--spacing-4)" },
  loadingContext: { height: "var(--spacing-12)", margin: "var(--spacing-4)", borderRadius: "var(--radius-page)", backgroundColor: "var(--skeleton-glimmer-line)" },
  heading: { minWidth: 0, borderBottomWidth: "var(--border-width)", borderBottomStyle: "solid", borderBottomColor: "var(--noema-border-subtle)", "@container (width > 1200px)": { gridColumn: "1", gridRow: "1" } },
  tabBar: {
    // Extend by the small tab's inline padding so its label aligns with the title.
    width: "calc(100% - var(--spacing-6))",
    maxWidth: "calc(760px + var(--spacing-6))",
    marginInline: "auto",
    minWidth: 0,
    // Astryx places one pixel of the selection indicator below the tab.
    paddingBlockEnd: "var(--border-width)",
    overflowX: "auto",
    overscrollBehaviorX: "contain",
    scrollbarWidth: "thin"
  },
  narrowTab: { display: { default: "inline-flex", "@container (width > 1200px)": "none" } },
  frame: {
    gridTemplateColumns: "minmax(0, 1fr)",
    gridTemplateRows: "auto minmax(0, 1fr) auto",
    minWidth: 0,
    minHeight: 0,
    overflow: "hidden",
    "@container (width > 1200px)": {
      gridTemplateColumns: "minmax(0, 1fr) 600px",
      gridTemplateRows: "auto minmax(0, 1fr) auto"
    }
  },
  inactivePane: { display: { default: "none", "@container (width > 1200px)": "block" } },
  workspacePane: {
    containerType: "inline-size",
    gridRow: "2",
    minWidth: 0,
    minHeight: 0,
    height: "100%",
    "@container (width > 1200px)": { gridColumn: "1", gridRow: "2 / -1" }
  },
  transcript: {
    gridRow: "2",
    minWidth: 0,
    minHeight: 0,
    height: "100%",
    "--chat-transcript-top-fade": "var(--spacing-4)",
    "--task-transcript-bottom-inset": "var(--spacing-3)",
    "@container (width > 1200px)": {
      gridColumn: "2",
      gridRow: "1 / 3",
      borderInlineStartWidth: "var(--border-width)",
      borderInlineStartStyle: "solid",
      borderInlineStartColor: "var(--noema-border-subtle)"
    }
  },
  contextPane: {
    gridRow: "3",
    minWidth: 0,
    minHeight: 0,
    "@container (width > 1200px)": {
      gridColumn: "2",
      gridRow: "3",
      borderInlineStartWidth: "var(--border-width)",
      borderInlineStartStyle: "solid",
      borderInlineStartColor: "var(--noema-border-subtle)",
      "--task-context-card-margin-block-start": "var(--spacing-0)",
      "--task-context-status-display": "none"
    }
  },
  workspace: {
    minWidth: 0,
    minHeight: 0,
    height: "100%",
    "--task-workspace-content-width": "max(0px, min(760px, calc(100cqw - var(--spacing-6) - var(--spacing-6))))",
    "--task-workspace-table-left-bleed": "max(0px, calc((100cqw - var(--task-workspace-content-width)) / 2))",
    "--task-workspace-table-right-bleed": "calc(100cqw - var(--task-workspace-table-left-bleed) - var(--task-workspace-content-width))",
  },
  fileNotice: { width: "calc(100% - var(--spacing-6) - var(--spacing-6))", maxWidth: 760, margin: "var(--spacing-3) auto var(--spacing-0)", color: "var(--noema-text-muted)", fontSize: 12 },
  fileViewer: {
    width: "100%",
    minWidth: 0,
    minHeight: 0,
    height: "100%",
    overflowX: "hidden",
    overflowY: "auto"
  },
  workspaceStatus: { margin: "var(--spacing-0)", padding: "var(--spacing-4)", color: "var(--noema-text-secondary)", fontSize: 13 },
  plainText: { margin: "var(--spacing-0)", color: "var(--noema-text-primary)", fontFamily: "var(--noema-font-mono)", fontSize: 13, lineHeight: 1.55, whiteSpace: "pre-wrap", overflowWrap: "anywhere" },
  markdown: {
    color: "var(--noema-text-primary)",
    fontSize: 14,
    lineHeight: 1.6
  },
  empty: {
    margin: "var(--spacing-0)",
    color: "var(--noema-text-secondary)",
    fontSize: 13
  }
});

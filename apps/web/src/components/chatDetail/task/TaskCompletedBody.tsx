import * as React from "react";
import { Markdown } from "@astryxdesign/core/Markdown";
import { Tab, TabList } from "@astryxdesign/core/TabList";
import * as stylex from "@stylexjs/stylex";
import type { ChatDetailTarget } from "@/components/chatDetail/chatDetailTypes";
import { ArtifactReferenceCard } from "@/components/transcript/ArtifactReferenceCard";
import {
  ProviderCitationTags,
  providerCitationContent
} from "@/components/transcript/ProviderCitationSources";
import type { TaskArtifact, TaskDetail, TaskRunItem } from "./taskTypes";
import { TaskTranscript } from "./TaskTranscript";

type CompletedTaskTab = "final-response" | "transcript";
export function TaskCompletedBody({
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
  const [activeTab, setActiveTab] = React.useState<CompletedTaskTab>("final-response");

  return (
    <section aria-label="Completed task result" {...stylex.props(styles.root)}>
      <div {...stylex.props(styles.tabBar)}>
        <TabList
          aria-label="Task detail view"
          hasDivider
          onChange={(value) => setActiveTab(value as CompletedTaskTab)}
          size="sm"
          value={activeTab}
        >
          <Tab label="Result" value="final-response" />
          <Tab label="Transcript" value="transcript" />
        </TabList>
      </div>
      <div {...stylex.props(styles.content)}>
        {activeTab === "final-response" ? (
          <FinalResponse detail={detail} onOpenDetail={onOpenDetail} />
        ) : (
          <div {...stylex.props(styles.transcriptContent)}>
            <div {...stylex.props(styles.transcript)}>
              <TaskTranscript
                detail={detail}
                liveRunItems={liveRunItems}
                onOpenDetail={onOpenDetail}
                onLatestRunItemChange={onLatestRunItemChange}
              />
            </div>
            {contextCard}
          </div>
        )}
      </div>
    </section>
  );
}

function FinalResponse({
  detail,
  onOpenDetail
}: {
  detail: TaskDetail;
  onOpenDetail: (target: ChatDetailTarget) => void;
}) {
  const submission = detail.completedResult;
  if (!submission) return null;
  const response = submission.result?.trim()
    ? submission.result
    : submission.summary?.trim()
      ? submission.summary
      : undefined;
  const citationContent = providerCitationContent(response ?? "", submission.citations ?? []);

  return (
    <div data-slot="task-final-response" {...stylex.props(styles.finalScroller)}>
      <div {...stylex.props(styles.finalContent)}>
        {response ? (
          <Markdown
            autolink="gfm"
            contentAlign="center"
            contentWidth="min(760px, calc(100% - var(--spacing-6) - var(--spacing-6)))"
            density="default"
            headingLevelStart={1}
            sources={citationContent.sources}
            xstyle={styles.markdown}
          >
            {citationContent.text}
          </Markdown>
        ) : (
          <p {...stylex.props(styles.empty)}>The accepted response has no text content.</p>
        )}
        <ProviderCitationTags citations={citationContent.fallbackCitations} />
        {(submission.artifacts ?? []).length > 0 ? (
          <div aria-label="Final response artifacts" {...stylex.props(styles.artifacts)}>
            {(submission.artifacts ?? []).map((artifact) => (
              <ArtifactReferenceCard
                key={artifact.id}
                item={artifactReferenceItem(artifact)}
                onOpenDetail={onOpenDetail}
              />
            ))}
          </div>
        ) : null}
      </div>
    </div>
  );
}

function artifactReferenceItem(artifact: TaskArtifact) {
  return {
    kind: "artifact_reference" as const,
    artifact_id: artifact.id,
    artifact_version_id: artifact.versionId ?? null,
    title: artifact.title,
    artifact_kind: artifact.kind ?? "artifact",
    storage_kind: artifact.storageKind ?? "local_file",
    external_url: artifact.externalUrl ?? null,
    download_url: artifact.downloadUrl ?? null,
    media_type: artifact.mediaType ?? null
  };
}

const styles = stylex.create({
  root: {
    display: "grid",
    gridTemplateRows: "auto minmax(0, 1fr)",
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
  transcriptContent: {
    display: "grid",
    gridTemplateRows: "minmax(0, 1fr) auto",
    minWidth: 0,
    minHeight: 0,
    height: "100%"
  },
  transcript: {
    minWidth: 0,
    minHeight: 0,
    height: "100%",
    "--chat-transcript-top-fade": "var(--spacing-4)",
    "--task-transcript-bottom-inset": "var(--spacing-3)"
  },
  finalScroller: {
    height: "100%",
    overflowX: "hidden",
    overflowY: "auto",
    paddingBlock: "var(--spacing-4) var(--spacing-6)"
  },
  finalContent: {
    display: "grid",
    gap: "var(--spacing-4)",
    width: "100%"
  },
  markdown: {
    color: "var(--noema-text-primary)",
    fontSize: 14,
    lineHeight: 1.6
  },
  artifacts: {
    display: "grid",
    gap: "var(--spacing-2)",
    width: "calc(100% - var(--spacing-6) - var(--spacing-6))",
    maxWidth: 760,
    marginInline: "auto",
    justifyItems: "start"
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

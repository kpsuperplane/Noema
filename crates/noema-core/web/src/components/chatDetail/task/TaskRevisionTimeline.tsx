import * as React from "react";
import { Badge } from "@astryxdesign/core/Badge";
import { Markdown, type MarkdownProps } from "@astryxdesign/core/Markdown";
import * as stylex from "@stylexjs/stylex";
import { ChevronDown, ChevronRight, FileText, Wrench } from "lucide-react";
import type { TaskCriterion, TaskRevision, TaskReview, TaskRun, TaskRunItem } from "./taskTypes";
import { SectionHeading } from "./TaskCriteria";

type MarkdownXStyle = MarkdownProps["xstyle"];

export function TaskRevisionTimeline({
  revisions,
  criteria,
  onExpandRevision
}: {
  revisions: readonly TaskRevision[];
  criteria: readonly TaskCriterion[];
  onExpandRevision?: (revision: number) => void;
}) {
  return (
    <section aria-labelledby="task-revisions-title" {...stylex.props(styles.section)}>
      <SectionHeading id="task-revisions-title" title="Revision timeline" count={revisions.length} />
      {revisions.length === 0 ? (
        <p {...stylex.props(styles.empty)}>Executor and reviewer activity will appear here.</p>
      ) : (
        <div {...stylex.props(styles.timeline)}>
          {revisions.map((revision) => (
            <TaskRunCycle
              key={revision.revision}
              criteria={criteria}
              revision={revision}
              onExpand={onExpandRevision}
            />
          ))}
        </div>
      )}
    </section>
  );
}

export function TaskRunCycle({
  revision,
  criteria,
  onExpand
}: {
  revision: TaskRevision;
  criteria: readonly TaskCriterion[];
  onExpand?: (revision: number) => void;
}) {
  const active = revisionIsActive(revision);
  const [expanded, setExpanded] = React.useState(active);

  const toggle = () => {
    setExpanded((open) => !open);
    if (!expanded) {
      onExpand?.(revision.revision);
    }
  };
  const verdict = revision.review?.verdict;

  return (
    <article {...stylex.props(styles.cycle, expanded && styles.cycleExpanded)}>
      <button
        type="button"
        aria-controls={`task-revision-${revision.revision}`}
        aria-expanded={expanded}
        onClick={toggle}
        {...stylex.props(styles.cycleButton)}
      >
        <span {...stylex.props(styles.chevron)} aria-hidden="true">
          {expanded ? <ChevronDown size={15} /> : <ChevronRight size={15} />}
        </span>
        <span {...stylex.props(styles.cycleTitle)}>Revision {revision.revision}</span>
        <span {...stylex.props(styles.cycleSummary)}>{revisionSummary(revision)}</span>
        {verdict ? <ReviewBadge review={revision.review} /> : null}
      </button>
      {expanded ? (
        <div id={`task-revision-${revision.revision}`} {...stylex.props(styles.cycleBody)}>
          <RunSummary label="Executor" run={revision.executor} />
          {revision.submission ? (
            <OutputBlock label="Submission" text={revision.submission.summary || revision.submission.result} />
          ) : null}
          {revision.review ? <ReviewBlock criteria={criteria} review={revision.review} /> : null}
          <AgentTranscript revision={revision} />
          {revision.itemsLoading ? <p {...stylex.props(styles.loading)}>Loading run activity...</p> : null}
          {revision.itemsError ? <p role="alert" {...stylex.props(styles.error)}>{revision.itemsError}</p> : null}
          <RunSummary label="Reviewer" run={revision.reviewer} />
        </div>
      ) : null}
    </article>
  );
}

function RunSummary({ label, run }: { label: string; run?: TaskRun | null }) {
  if (!run) {
    return null;
  }
  const output = run.output?.trim();
  return (
    <section {...stylex.props(styles.run)}>
      <div {...stylex.props(styles.subheading)}>
        <h4 {...stylex.props(styles.subheadingTitle)}>{label}</h4>
        <Badge variant={run.status === "failed" ? "error" : run.status === "completed" ? "success" : "neutral"} label={run.status} {...stylex.props(styles.smallBadge)} />
      </div>
      {run.model ? (
        <p {...stylex.props(styles.model)}>
          {run.model.modelLabel || run.model.modelProfile}
          {run.model.reasoningEffort ? ` · ${run.model.reasoningEffort}` : ""}
        </p>
      ) : null}
      {output ? <OutputBlock label="Output" text={output} /> : null}
      {run.error ? <p role="alert" {...stylex.props(styles.error)}>{run.error}</p> : null}
      {run.toolActivities?.length ? <ToolActivityList activities={run.toolActivities} /> : null}
    </section>
  );
}

function OutputBlock({ label, text }: { label: string; text?: string | null }) {
  if (!text?.trim()) {
    return null;
  }
  return (
    <div {...stylex.props(styles.output)}>
      <span {...stylex.props(styles.outputLabel)}>{label}</span>
      <Markdown
        autolink="gfm"
        contentWidth="100%"
        density="compact"
        headingLevelStart={4}
        xstyle={markdownXStyle(styles.markdown)}
      >
        {text}
      </Markdown>
    </div>
  );
}

function ReviewBlock({ criteria, review }: { criteria: readonly TaskCriterion[]; review: TaskReview }) {
  return (
    <section {...stylex.props(styles.review)}>
      <div {...stylex.props(styles.subheading)}>
        <h4 {...stylex.props(styles.subheadingTitle)}>Adversarial review</h4>
        <ReviewBadge review={review} />
      </div>
      {review.summary ? <OutputBlock label="Reviewer summary" text={review.summary} /> : null}
      {review.criteria.length > 0 ? (
        <ul {...stylex.props(styles.reviewList)}>
          {review.criteria.map((criterion) => {
            const criterionLabel = criteria.find((candidate) => candidate.id === criterion.criterionId)?.text;
            return (
              <li key={criterion.criterionId} {...stylex.props(styles.reviewItem)}>
                <div {...stylex.props(styles.reviewItemHeader)}>
                  <Badge
                    variant={criterion.verdict === "pass" ? "success" : criterion.verdict === "fail" ? "error" : "warning"}
                    label={criterion.verdict}
                    {...stylex.props(styles.smallBadge)}
                  />
                  <span {...stylex.props(styles.reviewCriterion)}>
                    {criterionLabel || `Criterion ${criterion.criterionId}`}
                  </span>
                </div>
                {criterion.feedback ? <p {...stylex.props(styles.reviewFeedback)}>{criterion.feedback}</p> : null}
                {criterion.evidence ? <p {...stylex.props(styles.reviewEvidence)}>{criterion.evidence}</p> : null}
              </li>
            );
          })}
        </ul>
      ) : null}
    </section>
  );
}

function AgentTranscript({ revision }: { revision: TaskRevision }) {
  if (!revision.items?.length) {
    return null;
  }
  return (
    <section {...stylex.props(styles.transcript)}>
      <div {...stylex.props(styles.subheading)}>
        <h4 {...stylex.props(styles.subheadingTitle)}>Agent transcript</h4>
        <span {...stylex.props(styles.activityHint)}>Full run exchange</span>
      </div>
      <div role="log" aria-live="polite" {...stylex.props(styles.transcriptList)}>
        {revision.items.map((item) => <TranscriptItem key={item.id} item={item} />)}
      </div>
    </section>
  );
}

function TranscriptItem({ item }: { item: TaskRunItem }) {
  const role = item.role === "reviewer" ? "Reviewer" : item.role === "completion_delivery" ? "Delivery" : "Executor";
  const isMessage = item.kind === "message";
  const isInput = item.kind === "input";
  const content = item.details || item.summary;
  return (
    <article {...stylex.props(styles.transcriptItem, isMessage && styles.transcriptMessage)}>
      <div {...stylex.props(styles.transcriptHeader)}>
        <span {...stylex.props(styles.transcriptRole)}>{role}</span>
        <span {...stylex.props(styles.transcriptTitle)}>
          {item.kind === "tool" ? <Wrench aria-hidden="true" size={12} /> : <FileText aria-hidden="true" size={12} />}
          {item.title}
        </span>
      </div>
      {isMessage && item.summary ? (
        <Markdown
          autolink="gfm"
          contentWidth="100%"
          density="compact"
          headingLevelStart={5}
          xstyle={markdownXStyle(styles.transcriptMarkdown)}
        >
          {item.summary}
        </Markdown>
      ) : content ? (
        <pre {...stylex.props(styles.transcriptPre)}>{content}</pre>
      ) : isInput ? (
        <p {...stylex.props(styles.activitySummary)}>No input text recorded.</p>
      ) : null}
    </article>
  );
}

function revisionIsActive(revision: TaskRevision): boolean {
  return [revision.executor, revision.reviewer].some(
    (run) => run?.status === "queued" || run?.status === "leased" || run?.status === "running"
  );
}

function ToolActivityList({ activities }: { activities: NonNullable<TaskRun["toolActivities"]> }) {
  return (
    <div {...stylex.props(styles.tools)}>
      <span {...stylex.props(styles.outputLabel)}>Tool activity</span>
      <ul {...stylex.props(styles.activityList)}>
        {activities.map((activity) => (
          <li key={activity.id} {...stylex.props(styles.activityItem)}>
            <span {...stylex.props(styles.activityIcon)} aria-hidden="true"><Wrench size={12} /></span>
            <span {...stylex.props(styles.activityText)}>
              <span {...stylex.props(styles.activityTitle)}>{activity.title}</span>
              {activity.summary ? <span {...stylex.props(styles.activitySummary)}>{activity.summary}</span> : null}
            </span>
            {activity.status ? <span {...stylex.props(styles.activityStatus)}>{activity.status}</span> : null}
          </li>
        ))}
      </ul>
    </div>
  );
}

function ReviewBadge({ review }: { review: TaskReview | null | undefined }) {
  if (!review) {
    return null;
  }
  const variant = review.verdict === "approve" ? "success" : review.verdict === "request_changes" ? "warning" : "neutral";
  const label = review.verdict === "request_changes" ? "Changes requested" : review.verdict === "approve" ? "Approved" : "Needs you";
  return <Badge variant={variant} label={label} {...stylex.props(styles.smallBadge)} />;
}

function revisionSummary(revision: TaskRevision): string {
  if (revision.review?.verdict === "approve") return "Approved";
  if (revision.review?.verdict === "request_changes") return "Feedback returned";
  if (revision.review?.verdict === "needs_human") return "Needs attention";
  if (revision.executor?.status === "running") return "Executor working";
  return "Activity recorded";
}

function markdownXStyle(...xstyle: unknown[]): MarkdownXStyle {
  return xstyle as unknown as MarkdownXStyle;
}

const styles = stylex.create({
  section: { display: "grid", gap: 10, paddingBlock: 2 },
  timeline: { display: "grid", gap: 8 },
  cycle: {
    minWidth: 0,
    overflow: "hidden",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--noema-border-subtle)",
    borderRadius: 8,
    backgroundColor: "var(--noema-surface-card)"
  },
  cycleExpanded: { backgroundColor: "var(--noema-surface-sunken)" },
  cycleButton: {
    display: "grid",
    gridTemplateColumns: "18px auto minmax(0, 1fr) auto",
    alignItems: "center",
    width: "100%",
    gap: 7,
    border: 0,
    backgroundColor: "transparent",
    padding: 10,
    color: "var(--noema-text-primary)",
    font: "inherit",
    textAlign: "left",
    cursor: "pointer",
    ":hover": { backgroundColor: "var(--noema-surface-hover)" },
    ":focus-visible": {
      outlineWidth: 3,
      outlineStyle: "solid",
      outlineColor: "color-mix(in srgb, var(--noema-pine-500) 24%, transparent)",
      outlineOffset: -2
    }
  },
  chevron: { display: "inline-flex", color: "var(--noema-text-muted)" },
  cycleTitle: { fontSize: 12, fontWeight: 700, whiteSpace: "nowrap" },
  cycleSummary: { minWidth: 0, color: "var(--noema-text-secondary)", fontSize: 11, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" },
  cycleBody: { display: "grid", gap: 12, borderTopWidth: 1, borderTopStyle: "solid", borderTopColor: "var(--noema-border-subtle)", padding: 11 },
  run: { display: "grid", gap: 7, minWidth: 0 },
  review: { display: "grid", gap: 8, minWidth: 0, borderRadius: 7, backgroundColor: "color-mix(in srgb, var(--noema-yellow-100) 25%, transparent)", padding: 9 },
  subheading: { display: "flex", flexWrap: "wrap", alignItems: "center", justifyContent: "space-between", gap: 7 },
  subheadingTitle: { margin: 0, color: "var(--noema-text-primary)", fontSize: 11, fontWeight: 700 },
  smallBadge: { flexShrink: 0, fontSize: 9 },
  model: { margin: 0, color: "var(--noema-text-muted)", fontFamily: "var(--noema-font-mono)", fontSize: 10, overflowWrap: "anywhere" },
  output: { display: "grid", gap: 4, minWidth: 0 },
  outputLabel: { color: "var(--noema-text-muted)", fontSize: 10, fontWeight: 650, letterSpacing: "0.06em", textTransform: "uppercase" },
  markdown: { color: "var(--noema-text-secondary)", fontSize: 12, lineHeight: 1.45 },
  reviewList: { display: "grid", gap: 7, margin: 0, padding: 0, listStyle: "none" },
  reviewItem: { display: "grid", gap: 4, minWidth: 0 },
  reviewItemHeader: { display: "flex", alignItems: "start", gap: 7, minWidth: 0 },
  reviewCriterion: { minWidth: 0, color: "var(--noema-text-secondary)", fontSize: 11, lineHeight: 1.35, overflowWrap: "anywhere" },
  reviewFeedback: { margin: 0, paddingInlineStart: 4, color: "var(--noema-text-secondary)", fontSize: 11, lineHeight: 1.4, overflowWrap: "anywhere" },
  reviewEvidence: { margin: 0, paddingInlineStart: 4, color: "var(--noema-text-muted)", fontSize: 10, lineHeight: 1.4, overflowWrap: "anywhere", whiteSpace: "pre-wrap" },
  tools: { display: "grid", gap: 5 },
  transcript: { display: "grid", gap: 6, minWidth: 0 },
  transcriptList: { display: "grid", gap: 6, maxHeight: 520, overflowY: "auto", minWidth: 0, padding: 1 },
  transcriptItem: { display: "grid", gap: 6, minWidth: 0, borderRadius: 7, backgroundColor: "var(--noema-surface-card)", padding: 8 },
  transcriptMessage: { backgroundColor: "color-mix(in srgb, var(--noema-pine-100) 24%, var(--noema-surface-card))" },
  transcriptHeader: { display: "flex", alignItems: "center", gap: 7, minWidth: 0 },
  transcriptRole: { flexShrink: 0, color: "var(--noema-text-muted)", fontSize: 9, fontWeight: 700, letterSpacing: "0.06em", textTransform: "uppercase" },
  transcriptTitle: { display: "inline-flex", alignItems: "center", gap: 5, minWidth: 0, color: "var(--noema-text-secondary)", fontSize: 11, overflowWrap: "anywhere" },
  transcriptMarkdown: { color: "var(--noema-text-primary)", fontSize: 12, lineHeight: 1.5 },
  transcriptPre: { maxWidth: "100%", margin: 0, overflowX: "auto", whiteSpace: "pre-wrap", overflowWrap: "anywhere", color: "var(--noema-text-secondary)", fontFamily: "var(--noema-font-mono)", fontSize: 10, lineHeight: 1.45 },
  activity: { display: "grid", gap: 6 },
  activityHint: { color: "var(--noema-text-muted)", fontSize: 10 },
  activityList: { display: "grid", gap: 4, margin: 0, padding: 0, listStyle: "none" },
  activityItem: { display: "flex", alignItems: "start", gap: 7, minWidth: 0, borderRadius: 6, backgroundColor: "var(--noema-surface-card)", padding: 6 },
  activityIcon: { display: "inline-flex", flexShrink: 0, color: "var(--noema-text-muted)" },
  activityText: { display: "grid", flex: 1, minWidth: 0, gap: 1 },
  activityTitle: { minWidth: 0, color: "var(--noema-text-secondary)", fontSize: 11, overflowWrap: "anywhere" },
  activitySummary: { minWidth: 0, color: "var(--noema-text-muted)", fontSize: 10, overflowWrap: "anywhere" },
  activityStatus: { flexShrink: 0, color: "var(--noema-text-muted)", fontSize: 10 },
  loading: { margin: 0, color: "var(--noema-text-muted)", fontSize: 11 },
  error: { margin: 0, color: "var(--noema-red-700)", fontSize: 11, lineHeight: 1.35 },
  empty: { margin: 0, color: "var(--noema-text-secondary)", fontSize: 12, lineHeight: 1.45 }
});

import { Button } from "@astryxdesign/core/Button";
import { Markdown, type MarkdownProps } from "@astryxdesign/core/Markdown";
import * as stylex from "@stylexjs/stylex";
import { ExternalLink, FileText } from "lucide-react";
import { resolveTaskArtifactLink } from "@/shared/artifactLinks";
import { TaskExpandableContent, TaskStaticSection } from "./TaskSection";
import type { TaskArtifact, TaskFinalResult } from "./taskTypes";

type MarkdownXStyle = MarkdownProps["xstyle"];

export function TaskResult({
  result,
  artifacts,
  embedded = false
}: {
  result?: TaskFinalResult | null;
  artifacts?: readonly TaskArtifact[];
  embedded?: boolean;
}) {
  if (!result && (!artifacts || artifacts.length === 0)) {
    return null;
  }
  const title = result?.approvedAt ? "Approved result" : "Proposed result";
  const content = (
    <div {...stylex.props(styles.content)}>
      {result ? (
        embedded ? (
          <div {...stylex.props(styles.resultCopy)}>
            {result.summary ? <p {...stylex.props(styles.summary)}>{result.summary}</p> : null}
            {result.body ? (
              <TaskExpandableContent id="task-result-content">
                <Markdown
                  autolink="gfm"
                  contentWidth="100%"
                  density="default"
                  headingLevelStart={4}
                  xstyle={markdownXStyle(styles.markdown)}
                >
                  {result.body}
                </Markdown>
              </TaskExpandableContent>
            ) : null}
            {result.approvedAt ? (
              <p {...stylex.props(styles.approvedAt)}>Approved {formatDate(result.approvedAt)}</p>
            ) : null}
          </div>
        ) : (
          <TaskExpandableContent id="task-result-content">
            <div {...stylex.props(styles.resultCopy)}>
              {result.summary ? <p {...stylex.props(styles.summary)}>{result.summary}</p> : null}
              {result.body ? (
                <Markdown
                  autolink="gfm"
                  contentWidth="100%"
                  density="default"
                  headingLevelStart={3}
                  xstyle={markdownXStyle(styles.markdown)}
                >
                  {result.body}
                </Markdown>
              ) : null}
              {result.approvedAt ? (
                <p {...stylex.props(styles.approvedAt)}>Approved {formatDate(result.approvedAt)}</p>
              ) : null}
            </div>
          </TaskExpandableContent>
        )
      ) : null}
      {artifacts?.length ? (
        <div {...stylex.props(styles.artifacts)}>
          <span {...stylex.props(styles.label)}>Task artifacts</span>
          <ul {...stylex.props(styles.artifactList)}>
            {artifacts.map((artifact) => <ArtifactRow key={artifact.id} artifact={artifact} />)}
          </ul>
        </div>
      ) : null}
    </div>
  );

  if (embedded) {
    return (
      <section aria-labelledby="task-result-title" {...stylex.props(styles.embedded)}>
        <h4 id="task-result-title" {...stylex.props(styles.embeddedTitle)}>{title}</h4>
        {content}
      </section>
    );
  }

  return (
    <TaskStaticSection id="task-result-title" title={title}>
      {content}
    </TaskStaticSection>
  );
}

function ArtifactRow({ artifact }: { artifact: TaskArtifact }) {
  const link = resolveTaskArtifactLink(artifact.href ?? null);
  return (
    <li {...stylex.props(styles.artifactRow)}>
      <span {...stylex.props(styles.artifactIcon)} aria-hidden="true"><FileText size={14} /></span>
      <span {...stylex.props(styles.artifactText)}>
        <span {...stylex.props(styles.artifactTitle)}>{artifact.title}</span>
        {artifact.kind || artifact.mediaType ? (
          <span {...stylex.props(styles.artifactMeta)}>{[artifact.kind, artifact.mediaType].filter(Boolean).join(" · ")}</span>
        ) : null}
      </span>
      {link ? (
        <Button
          href={link.href}
          icon={<ExternalLink aria-hidden="true" size={13} />}
          isIconOnly
          label={`Open ${artifact.title}`}
          rel={link.external ? "noreferrer" : undefined}
          size="sm"
          target="_blank"
          variant="ghost"
        />
      ) : null}
    </li>
  );
}

function markdownXStyle(...xstyle: unknown[]): MarkdownXStyle {
  return xstyle as unknown as MarkdownXStyle;
}

function formatDate(value: string): string {
  const timestamp = Date.parse(value);
  return Number.isNaN(timestamp)
    ? value
    : new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(timestamp);
}

const styles = stylex.create({
  embedded: { display: "grid", gap: "var(--spacing-1-5)", minWidth: 0 },
  embeddedTitle: { margin: 0, color: "var(--noema-text-secondary)", fontSize: 11, fontWeight: 650 },
  content: { display: "grid", gap: "var(--spacing-2)", minWidth: 0 },
  resultCopy: { display: "grid", gap: "var(--spacing-2)", minWidth: 0 },
  summary: { margin: 0, color: "var(--noema-text-primary)", fontSize: 13, lineHeight: 1.45, overflowWrap: "anywhere" },
  markdown: { color: "var(--noema-text-secondary)", fontSize: 13, lineHeight: 1.55 },
  approvedAt: { margin: 0, color: "var(--noema-text-muted)", fontSize: 10 },
  artifacts: { display: "grid", gap: "var(--spacing-1-5)" },
  label: { color: "var(--noema-text-muted)", fontSize: 10, fontWeight: 650, letterSpacing: "0.06em", textTransform: "uppercase" },
  artifactList: { display: "grid", gap: "var(--spacing-1)", margin: 0, padding: 0, listStyle: "none" },
  artifactRow: { display: "flex", alignItems: "center", gap: "var(--spacing-2)", minWidth: 0, borderRadius: 7, backgroundColor: "var(--noema-surface-sunken)", padding: "var(--spacing-2)" },
  artifactIcon: { display: "inline-flex", flexShrink: 0, color: "var(--noema-pine-700)" },
  artifactText: { display: "grid", flex: 1, minWidth: 0, gap: "var(--spacing-0-5)" },
  artifactTitle: { minWidth: 0, color: "var(--noema-text-primary)", fontSize: 12, overflowWrap: "anywhere" },
  artifactMeta: { minWidth: 0, color: "var(--noema-text-muted)", fontSize: 10, overflowWrap: "anywhere" }
});

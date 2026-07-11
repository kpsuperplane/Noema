import { Button } from "@astryxdesign/core/Button";
import { Markdown, type MarkdownProps } from "@astryxdesign/core/Markdown";
import * as stylex from "@stylexjs/stylex";
import { ExternalLink, FileText } from "lucide-react";
import type { TaskArtifact, TaskFinalResult } from "./taskTypes";
import { SectionHeading } from "./TaskCriteria";

type MarkdownXStyle = MarkdownProps["xstyle"];

export function TaskResult({
  result,
  artifacts
}: {
  result?: TaskFinalResult | null;
  artifacts?: readonly TaskArtifact[];
}) {
  if (!result && (!artifacts || artifacts.length === 0)) {
    return null;
  }
  return (
    <section aria-labelledby="task-result-title" {...stylex.props(styles.section)}>
      <SectionHeading id="task-result-title" title="Approved result" />
      {result?.summary ? <p {...stylex.props(styles.summary)}>{result.summary}</p> : null}
      {result?.body ? (
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
      {result?.approvedAt ? (
        <p {...stylex.props(styles.approvedAt)}>Approved {formatDate(result.approvedAt)}</p>
      ) : null}
      {artifacts?.length ? (
        <div {...stylex.props(styles.artifacts)}>
          <span {...stylex.props(styles.label)}>Task artifacts</span>
          <ul {...stylex.props(styles.artifactList)}>
            {artifacts.map((artifact) => <ArtifactRow key={artifact.id} artifact={artifact} />)}
          </ul>
        </div>
      ) : null}
    </section>
  );
}

function ArtifactRow({ artifact }: { artifact: TaskArtifact }) {
  return (
    <li {...stylex.props(styles.artifactRow)}>
      <span {...stylex.props(styles.artifactIcon)} aria-hidden="true"><FileText size={14} /></span>
      <span {...stylex.props(styles.artifactText)}>
        <span {...stylex.props(styles.artifactTitle)}>{artifact.title}</span>
        {artifact.kind || artifact.mediaType ? (
          <span {...stylex.props(styles.artifactMeta)}>{[artifact.kind, artifact.mediaType].filter(Boolean).join(" · ")}</span>
        ) : null}
      </span>
      {artifact.href ? (
        <Button
          href={trustedHref(artifact.href) ?? undefined}
          icon={<ExternalLink aria-hidden="true" size={13} />}
          isIconOnly
          label={`Open ${artifact.title}`}
          rel="noreferrer"
          size="sm"
          target="_blank"
          variant="ghost"
        />
      ) : null}
    </li>
  );
}

function trustedHref(value: string): string | null {
  try {
    const url = new URL(value, window.location.origin);
    if (url.origin === window.location.origin) {
      return url.pathname.startsWith("/artifacts/") ? url.toString() : null;
    }
    return url.protocol === "http:" || url.protocol === "https:" ? url.toString() : null;
  } catch {
    return null;
  }
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
  section: { display: "grid", gap: 10, paddingBlock: 2 },
  summary: { margin: 0, color: "var(--noema-text-primary)", fontSize: 13, lineHeight: 1.45, overflowWrap: "anywhere" },
  markdown: { color: "var(--noema-text-secondary)", fontSize: 13, lineHeight: 1.55 },
  approvedAt: { margin: 0, color: "var(--noema-text-muted)", fontSize: 10 },
  artifacts: { display: "grid", gap: 6 },
  label: { color: "var(--noema-text-muted)", fontSize: 10, fontWeight: 650, letterSpacing: "0.06em", textTransform: "uppercase" },
  artifactList: { display: "grid", gap: 5, margin: 0, padding: 0, listStyle: "none" },
  artifactRow: { display: "flex", alignItems: "center", gap: 8, minWidth: 0, borderRadius: 7, backgroundColor: "var(--noema-surface-sunken)", padding: 7 },
  artifactIcon: { display: "inline-flex", flexShrink: 0, color: "var(--noema-pine-700)" },
  artifactText: { display: "grid", flex: 1, minWidth: 0, gap: 2 },
  artifactTitle: { minWidth: 0, color: "var(--noema-text-primary)", fontSize: 12, overflowWrap: "anywhere" },
  artifactMeta: { minWidth: 0, color: "var(--noema-text-muted)", fontSize: 10, overflowWrap: "anywhere" }
});

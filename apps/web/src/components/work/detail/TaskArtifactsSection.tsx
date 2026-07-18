import * as React from "react";
import { useLazyQuery } from "@apollo/client/react";
import { Markdown } from "@astryxdesign/core/Markdown";
import * as stylex from "@stylexjs/stylex";
import { Download, ExternalLink } from "lucide-react";
import type { WorkTaskDetail } from "../workTypes";
import { sentenceCase } from "../workModel";
import { WorkTaskArtifactsPageDocument } from "@/generated/graphql";
import { WorkHistoryLoadMore } from "./WorkHistoryLoadMore";

export function TaskArtifactsSection({ task }: { task: WorkTaskDetail }) {
  type Artifact = WorkTaskDetail["artifacts"]["edges"][number]["node"];
  const [older, setOlder] = React.useState<Artifact[]>([]);
  const [pageInfoOverride, setPageInfo] = React.useState<typeof task.artifacts.pageInfo | null>(null);
  const pageInfo = pageInfoOverride ?? task.artifacts.pageInfo;
  const [load, { loading, error }] = useLazyQuery(WorkTaskArtifactsPageDocument, { fetchPolicy: "network-only" });
  const initial = task.artifacts.edges.map((edge) => edge.node);
  const artifacts = [...new Map([...initial, ...older].map((item) => [item.artifactId, item])).values()];
  const loadMore = async () => {
    const result = await load({ variables: { taskId: task.taskId, after: pageInfo.endCursor, first: 20 } });
    const next = result.data?.task?.artifacts;
    if (!next) return;
    setOlder((current) => [...new Map([...current, ...initial, ...next.edges.map((edge) => edge.node)].map((item) => [item.artifactId, item])).values()]);
    setPageInfo(next.pageInfo);
  };
  return (
    <section aria-labelledby="task-artifacts-title" {...stylex.props(styles.section)}>
      <h2 id="task-artifacts-title" {...stylex.props(styles.title)}>Artifacts and result</h2>
      {task.acceptedResult ? (
        <div {...stylex.props(styles.result)}>
          <strong>Accepted result</strong>
          <Markdown density="default" headingLevelStart={3}>{task.acceptedResult.resultMarkdown}</Markdown>
        </div>
      ) : <p {...stylex.props(styles.empty)}>No result has been accepted yet.</p>}
      {artifacts.length > 0 ? (
        <ul {...stylex.props(styles.grid)}>
          {artifacts.map((artifact) => {
            const version = artifact.currentVersion;
            return (
              <li key={artifact.artifactId} {...stylex.props(styles.artifact)}>
                <div {...stylex.props(styles.artifactHeading)}>
                  <strong>{artifact.title}</strong>
                  <span>{sentenceCase(artifact.artifactKind)}</span>
                </div>
                {artifact.description ? <p {...stylex.props(styles.description)}>{artifact.description}</p> : null}
                <div {...stylex.props(styles.links)}>
                  {version?.downloadUrl ? <a href={version.downloadUrl}><Download aria-hidden="true" size={13} />Download</a> : null}
                  {version?.externalUrl ? <a href={version.externalUrl} target="_blank" rel="noreferrer"><ExternalLink aria-hidden="true" size={13} />Open</a> : null}
                  {!version?.downloadUrl && !version?.externalUrl ? <span>No link available</span> : null}
                </div>
              </li>
            );
          })}
        </ul>
      ) : null}
      {pageInfo.hasNextPage ? <WorkHistoryLoadMore label="Load older artifacts" loading={loading} error={Boolean(error)} onClick={() => { void loadMore().catch(() => undefined); }} /> : null}
    </section>
  );
}

const styles = stylex.create({
  section: { display: "grid", gap: 12, borderTopWidth: 1, borderTopStyle: "solid", borderTopColor: "var(--border-subtle)", paddingTop: 20 },
  title: { margin: 0, fontFamily: "var(--font-heading)", fontSize: 18 },
  result: { display: "grid", gap: 8, maxWidth: 880, color: "var(--text-secondary)", fontSize: 13 },
  grid: { display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))", gap: 9, margin: 0, padding: 0, listStyle: "none" },
  artifact: { display: "grid", alignContent: "start", gap: 8, minWidth: 0, borderWidth: 1, borderStyle: "solid", borderColor: "var(--border-subtle)", borderRadius: 11, padding: 12 },
  artifactHeading: { display: "grid", gap: 2, fontSize: 12, color: "var(--muted-foreground)" },
  description: { margin: 0, color: "var(--text-secondary)", fontSize: 12 },
  links: { display: "flex", flexWrap: "wrap", gap: 10, color: "var(--muted-foreground)", fontSize: 11 },
  empty: { margin: 0, color: "var(--muted-foreground)", fontSize: 13 }
});

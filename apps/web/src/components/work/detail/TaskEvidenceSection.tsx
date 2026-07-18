import * as React from "react";
import { useLazyQuery } from "@apollo/client/react";
import { Markdown } from "@astryxdesign/core/Markdown";
import * as stylex from "@stylexjs/stylex";
import type { WorkTaskDetail } from "../workTypes";
import { sentenceCase, timestampLabel } from "../workModel";
import { WorkTaskReviewsPageDocument, WorkTaskSubmissionsPageDocument } from "@/generated/graphql";
import { WorkHistoryLoadMore } from "./WorkHistoryLoadMore";

export function TaskEvidenceSection({ task }: { task: WorkTaskDetail }) {
  type Submission = WorkTaskDetail["submissions"]["edges"][number]["node"];
  type Review = WorkTaskDetail["reviews"]["edges"][number]["node"];
  const [olderSubmissions, setOlderSubmissions] = React.useState<Submission[]>([]);
  const [olderReviews, setOlderReviews] = React.useState<Review[]>([]);
  const [submissionPageOverride, setSubmissionPage] = React.useState<typeof task.submissions.pageInfo | null>(null);
  const [reviewPageOverride, setReviewPage] = React.useState<typeof task.reviews.pageInfo | null>(null);
  const submissionPage = submissionPageOverride ?? task.submissions.pageInfo;
  const reviewPage = reviewPageOverride ?? task.reviews.pageInfo;
  const [loadSubmissions, submissionLoad] = useLazyQuery(WorkTaskSubmissionsPageDocument, { fetchPolicy: "network-only" });
  const [loadReviews, reviewLoad] = useLazyQuery(WorkTaskReviewsPageDocument, { fetchPolicy: "network-only" });
  const initialSubmissions = task.submissions.edges.map((edge) => edge.node);
  const initialReviews = task.reviews.edges.map((edge) => edge.node);
  const submissions = [...new Map([...initialSubmissions, ...olderSubmissions].map((item) => [item.submissionId, item])).values()];
  const reviews = [...new Map([...initialReviews, ...olderReviews].map((item) => [item.reviewId, item])).values()];
  const reviewsBySubmission = new Map<string, Review[]>();
  const reviewAttemptById = new Map(reviews.map((review) => [review.reviewId, review.reviewAttemptIndex]));
  for (const review of reviews) {
    const group = reviewsBySubmission.get(review.reviewedSubmissionId) ?? [];
    group.push(review);
    reviewsBySubmission.set(review.reviewedSubmissionId, group);
  }
  for (const group of reviewsBySubmission.values()) {
    group.sort((left, right) => left.reviewAttemptIndex - right.reviewAttemptIndex || left.createdAt.localeCompare(right.createdAt));
  }
  const loadMoreSubmissions = async () => {
    const result = await loadSubmissions({ variables: { taskId: task.taskId, after: submissionPage.endCursor, first: 20 } });
    const next = result.data?.task?.submissions;
    if (!next) return;
    setOlderSubmissions((current) => [...new Map([...current, ...initialSubmissions, ...next.edges.map((edge) => edge.node)].map((item) => [item.submissionId, item])).values()]);
    setSubmissionPage(next.pageInfo);
  };
  const loadMoreReviews = async () => {
    const result = await loadReviews({ variables: { taskId: task.taskId, after: reviewPage.endCursor, first: 20 } });
    const next = result.data?.task?.reviews;
    if (!next) return;
    setOlderReviews((current) => [...new Map([...current, ...initialReviews, ...next.edges.map((edge) => edge.node)].map((item) => [item.reviewId, item])).values()]);
    setReviewPage(next.pageInfo);
  };

  return (
    <section aria-labelledby="task-evidence-title" {...stylex.props(styles.section)}>
      <h2 id="task-evidence-title" {...stylex.props(styles.title)}>Evidence and review</h2>
      {submissions.length === 0 ? (
        <p {...stylex.props(styles.empty)}>No result has been submitted for review.</p>
      ) : (
        <ol {...stylex.props(styles.list)}>
          {submissions.map((submission) => {
            const submissionReviews = reviewsBySubmission.get(submission.submissionId) ?? [];
            return (
              <li key={submission.submissionId} {...stylex.props(styles.card)}>
                <div {...stylex.props(styles.heading)}>
                  <strong>Review round {submission.reviewRound}</strong>
                  <span>{timestampLabel(submission.createdAt)}</span>
                </div>
                <p {...stylex.props(styles.summary)}>{submission.summary}</p>
                {submission.criteria.length > 0 ? (
                  <ol {...stylex.props(styles.criteria)}>
                    {submission.criteria.map((criterion) => {
                      return (
                        <li key={criterion.criterionId}>
                          <div {...stylex.props(styles.outcome)}>
                            <strong>{criterion.criterionId}</strong>
                            <span>Submitted evidence</span>
                          </div>
                          <Markdown density="compact" headingLevelStart={4}>{criterion.evidenceMarkdown}</Markdown>
                        </li>
                      );
                    })}
                  </ol>
                ) : null}
                {submissionReviews.length > 0 ? <ol aria-label={`Review attempts for round ${submission.reviewRound}`} {...stylex.props(styles.reviews)}>{submissionReviews.map((review) => (
                  <li key={review.reviewId} {...stylex.props(styles.review)}>
                    <div {...stylex.props(styles.reviewHeading)}><strong>Review attempt {review.reviewAttemptIndex}: {sentenceCase(review.verdict)}</strong><span>{timestampLabel(review.createdAt)}</span></div>
                    <small {...stylex.props(styles.feedback)}>{review.supersedesReviewId ? reviewAttemptById.has(review.supersedesReviewId) ? `Follows review attempt ${reviewAttemptById.get(review.supersedesReviewId)}.` : "Follows an earlier review attempt." : "Initial review attempt."}</small>
                    <Markdown density="compact" headingLevelStart={4}>{review.feedback}</Markdown>
                    {review.criteria.length > 0 ? <ul {...stylex.props(styles.reviewCriteria)}>{review.criteria.map((criterion) => <li key={criterion.criterionId}><div {...stylex.props(styles.outcome)}><strong>{criterion.criterionId}</strong><span>{sentenceCase(criterion.outcome)}</span></div>{criterion.feedback ? <small {...stylex.props(styles.feedback)}>{criterion.feedback}</small> : null}</li>)}</ul> : null}
                  </li>
                ))}</ol> : <p {...stylex.props(styles.awaiting)}>Awaiting review.</p>}
              </li>
            );
          })}
        </ol>
      )}
      {submissionPage.hasNextPage ? <WorkHistoryLoadMore label="Load older submissions" loading={submissionLoad.loading} error={Boolean(submissionLoad.error)} onClick={() => { void loadMoreSubmissions().catch(() => undefined); }} /> : null}
      {reviewPage.hasNextPage ? <WorkHistoryLoadMore label="Load older reviews" loading={reviewLoad.loading} error={Boolean(reviewLoad.error)} onClick={() => { void loadMoreReviews().catch(() => undefined); }} /> : null}
    </section>
  );
}

const styles = stylex.create({
  section: { display: "grid", gap: 12, borderTopWidth: 1, borderTopStyle: "solid", borderTopColor: "var(--border-subtle)", paddingTop: 20 },
  title: { margin: 0, fontFamily: "var(--font-heading)", fontSize: 18 },
  list: { display: "grid", gap: 10, margin: 0, padding: 0, listStyle: "none" },
  card: { display: "grid", gap: 10, borderWidth: 1, borderStyle: "solid", borderColor: "var(--border-subtle)", borderRadius: 12, padding: 13 },
  heading: { display: "flex", flexWrap: "wrap", justifyContent: "space-between", gap: 8, fontSize: 12, color: "var(--muted-foreground)" },
  summary: { margin: 0, color: "var(--text-secondary)", fontSize: 13 },
  criteria: { display: "grid", gap: 8, margin: 0, padding: 0, listStyle: "none" },
  outcome: { display: "flex", flexWrap: "wrap", justifyContent: "space-between", gap: 8, fontSize: 11 },
  feedback: { color: "var(--muted-foreground)", fontSize: 11 },
  reviews: { display: "grid", gap: 8, margin: 0, padding: 0, listStyle: "none" },
  review: { display: "grid", gap: 6, borderRadius: 9, backgroundColor: "var(--paper-100)", padding: 10, fontSize: 12 },
  reviewHeading: { display: "flex", flexWrap: "wrap", justifyContent: "space-between", gap: 8 },
  reviewCriteria: { display: "grid", gap: 6, margin: 0, padding: 0, listStyle: "none" },
  awaiting: { margin: 0, color: "var(--muted-foreground)", fontSize: 12 },
  empty: { margin: 0, color: "var(--muted-foreground)", fontSize: 13 }
});

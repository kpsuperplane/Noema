import * as React from "react";
import type { ApolloClient } from "@apollo/client";
import type { DocumentNode } from "graphql";
import {
  WorkActivityDocument,
  WorkCompletedTasksDocument,
  WorkNeedsYouDocument,
  WorkOverviewDocument,
  WorkTasksDocument,
  type WorkEventsSubscription
} from "@/generated/graphql";
import type { WorkView } from "./workTypes";

type WorkEvent = WorkEventsSubscription["workEvents"];

export function useWorkEventInvalidation({
  client,
  view,
  refetchProjects
}: {
  client: ApolloClient;
  view: WorkView;
  refetchProjects: () => Promise<void>;
}) {
  const timerRef = React.useRef<number | null>(null);
  const documentsRef = React.useRef(new Set<DocumentNode>());
  const refetchProjectsRef = React.useRef(false);

  const flush = React.useCallback(() => {
    timerRef.current = null;
    const documents = [...documentsRef.current];
    const shouldRefetchProjects = refetchProjectsRef.current;
    documentsRef.current.clear();
    refetchProjectsRef.current = false;
    if (documents.length > 0) void client.refetchQueries({ include: documents }).catch(() => undefined);
    if (shouldRefetchProjects) void refetchProjects().catch(() => undefined);
  }, [client, refetchProjects]);

  React.useEffect(() => () => {
    if (timerRef.current !== null) window.clearTimeout(timerRef.current);
  }, []);

  return React.useCallback((event: WorkEvent) => {
    for (const document of documentsForView(view)) documentsRef.current.add(document);
    if (!event.taskId) refetchProjectsRef.current = true;
    if (timerRef.current === null) timerRef.current = window.setTimeout(flush, 75);
  }, [flush, view]);
}

function documentsForView(view: WorkView): readonly DocumentNode[] {
  switch (view) {
    case "board":
      return [WorkOverviewDocument, WorkTasksDocument];
    case "list":
      return [WorkTasksDocument];
    case "needs-you":
      return [WorkNeedsYouDocument];
    case "activity":
      return [WorkActivityDocument];
    case "completed":
      return [WorkCompletedTasksDocument];
  }
}

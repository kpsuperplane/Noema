import * as React from "react";
import type { ApolloClient } from "@apollo/client";
import type { DocumentNode } from "graphql";
import {
  TasksHistoryDocument,
  TasksNeedsYouDocument,
  TasksListDocument,
  PendingHumanInterventionsDocument,
  type TasksEventsSubscription
} from "@/generated/graphql";

type TasksEvent = TasksEventsSubscription["tasksEvents"];
const tasksDocuments: readonly DocumentNode[] = [
  TasksListDocument,
  TasksNeedsYouDocument,
  PendingHumanInterventionsDocument,
  TasksHistoryDocument
];

export function useTasksEventInvalidation({
  client,
  refetchProjects
}: {
  client: ApolloClient;
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

  return React.useCallback((event: TasksEvent) => {
    for (const document of tasksDocuments) documentsRef.current.add(document);
    if (!event.taskId) refetchProjectsRef.current = true;
    if (timerRef.current === null) timerRef.current = window.setTimeout(flush, 75);
  }, [flush]);
}

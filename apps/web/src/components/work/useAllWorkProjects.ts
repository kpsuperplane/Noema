import * as React from "react";
import { useQuery } from "@apollo/client/react";
import { WorkProjectsDocument, type WorkProjectsQuery } from "@/generated/graphql";
import { PERSONAL_WORKSPACE_ID, type WorkProject } from "./workTypes";

type ProjectEdge = WorkProjectsQuery["projects"]["edges"][number];

export function useAllWorkProjects({ skip = false }: { skip?: boolean } = {}) {
  const result = useQuery(WorkProjectsDocument, {
    variables: {
      workspaceId: PERSONAL_WORKSPACE_ID,
      includeArchived: true,
      first: 100
    },
    fetchPolicy: "cache-and-network",
    notifyOnNetworkStatusChange: true,
    skip
  });
  const [pagingError, setPagingError] = React.useState<Error | null>(null);
  const [retryGeneration, setRetryGeneration] = React.useState(0);
  const loadingCursorRef = React.useRef<string | null>(null);
  const failedCursorRef = React.useRef<string | null>(null);
  const connection = result.data?.projects;
  const endCursor = connection?.pageInfo.endCursor ?? null;
  const hasNextPage = connection?.pageInfo.hasNextPage ?? false;

  React.useEffect(() => {
    if (skip || !hasNextPage || !endCursor) return;
    if (loadingCursorRef.current === endCursor || failedCursorRef.current === endCursor) return;

    loadingCursorRef.current = endCursor;
    setPagingError(null);
    void result.fetchMore({
      variables: { after: endCursor },
      updateQuery: (previous, { fetchMoreResult }) => ({
        ...fetchMoreResult,
        projects: {
          ...fetchMoreResult.projects,
          edges: mergeProjectEdges(previous.projects.edges, fetchMoreResult.projects.edges)
        }
      })
    }).catch((caught: unknown) => {
      failedCursorRef.current = endCursor;
      setPagingError(caught instanceof Error ? caught : new Error("Projects could not be loaded."));
    }).finally(() => {
      loadingCursorRef.current = null;
    });
  }, [endCursor, hasNextPage, result, retryGeneration, skip]);

  const projects = React.useMemo(
    () => mergeProjects(connection?.edges.map((edge) => edge.node) ?? []),
    [connection?.edges]
  );
  const retry = React.useCallback(async () => {
    failedCursorRef.current = null;
    setPagingError(null);
    if (result.error) {
      try {
        await result.refetch();
      } catch (caught) {
        setPagingError(caught instanceof Error ? caught : new Error("Projects could not be refreshed."));
      }
    }
    setRetryGeneration((current) => current + 1);
  }, [result]);
  const refetch = React.useCallback(async () => {
    failedCursorRef.current = null;
    setPagingError(null);
    try {
      await result.refetch();
    } catch (caught) {
      setPagingError(caught instanceof Error ? caught : new Error("Projects could not be refreshed."));
    }
    setRetryGeneration((current) => current + 1);
  }, [result]);

  return {
    projects,
    loading: result.loading,
    error: result.error ?? pagingError,
    retry,
    refetch
  };
}

function mergeProjectEdges(previous: readonly ProjectEdge[], next: readonly ProjectEdge[]): ProjectEdge[] {
  const byId = new Map<string, ProjectEdge>();
  for (const edge of [...previous, ...next]) byId.set(edge.node.projectId, edge);
  return [...byId.values()];
}

function mergeProjects(projects: readonly WorkProject[]): WorkProject[] {
  const byId = new Map<string, WorkProject>();
  for (const project of projects) byId.set(project.projectId, project);
  return [...byId.values()];
}

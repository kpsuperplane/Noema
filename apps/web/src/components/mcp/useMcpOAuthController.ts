import * as React from "react";
import { useApolloClient } from "@apollo/client/react";
import {
  McpOauthSetupAttemptDocument,
  type McpOauthSetupAttemptQuery
} from "@/generated/graphql";
import type { ReservedExternalAuthNavigation } from "@/graphql/externalUrls";

type OAuthAttempt = NonNullable<McpOauthSetupAttemptQuery["mcpOauthSetupAttempt"]>;
type StartAttempt = Pick<OAuthAttempt, "attemptId" | "authorizationUrl">;

export function useMcpOAuthController<Context>({
  onCompleted,
  onFailed
}: {
  onCompleted: (attempt: OAuthAttempt, context: Context) => void | Promise<void>;
  onFailed: (message: string, context: Context) => void;
}) {
  const client = useApolloClient();
  const handlers = React.useRef({ onCompleted, onFailed });
  React.useEffect(() => {
    handlers.current = { onCompleted, onFailed };
  }, [onCompleted, onFailed]);
  const [active, setActive] = React.useState<{
    attemptId: string;
    context: Context;
  } | null>(null);

  const begin = React.useCallback(async (
    attempt: StartAttempt,
    context: Context,
    navigation: ReservedExternalAuthNavigation
  ) => {
    if (!attempt.authorizationUrl) {
      navigation.cancel();
      return;
    }
    setActive({ attemptId: attempt.attemptId, context });
    await navigation.open(attempt.authorizationUrl);
  }, []);

  React.useEffect(() => {
    if (!active) return;
    let cancelled = false;
    let timeout: number | null = null;
    const poll = () => {
      timeout = window.setTimeout(() => {
        client
          .query<McpOauthSetupAttemptQuery>({
            query: McpOauthSetupAttemptDocument,
            variables: { attemptId: active.attemptId },
            fetchPolicy: "network-only"
          })
          .then(async ({ data }) => {
            if (cancelled) return;
            const attempt = data?.mcpOauthSetupAttempt;
            if (!attempt) {
              setActive(null);
              handlers.current.onFailed("Noema could not find that MCP OAuth attempt.", active.context);
            } else if (attempt.status === "completed") {
              setActive(null);
              await handlers.current.onCompleted(attempt, active.context);
            } else if (attempt.status === "failed") {
              setActive(null);
              handlers.current.onFailed(
                attempt.errorMessage ?? "Noema could not complete MCP OAuth.",
                active.context
              );
            } else {
              poll();
            }
          })
          .catch((error: unknown) => {
            if (cancelled) return;
            setActive(null);
            handlers.current.onFailed(
              error instanceof Error ? error.message : "MCP OAuth failed.",
              active.context
            );
          });
      }, 1500);
    };
    poll();
    return () => {
      cancelled = true;
      if (timeout !== null) window.clearTimeout(timeout);
    };
  }, [active, client]);

  return { active, begin };
}

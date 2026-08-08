const HELP = `Usage: bun run scripts/run-live-noema-case.ts [options] -- <prompt>

Submit one prompt to Noema's primary conversation, wait for its exact turn, and
follow any directly delegated task until completion or human intervention.

Options:
  --origin <url>       Noema origin (default: http://localhost:3737)
  --timezone <zone>    IANA client timezone (default: Etc/UTC)
  --timeout-ms <ms>    Turn timeout (default: 600000)
  --help               Show this help
`;

type Options = {
  origin: string;
  timezone: string;
  timeoutMs: number;
  prompt: string;
};

type GraphqlEnvelope<T> = {
  data?: T;
  errors?: Array<{ message: string }>;
};

type DelegatedTaskState = {
  task: {
    taskId: string;
    title: string;
    completedAt: string | null;
    stage: { key: string; name: string; behavior: string };
    currentRun: {
      runId: string;
      kind: string;
      status: string;
      attemptIndex: number;
      updatedAt: string;
      activityLabel: string;
    } | null;
    activeGate: {
      gateId: string;
      kind: string;
      state: string;
      prompt: string;
    } | null;
    completedResult: {
      submissionId: string;
      executorRunId: string;
      summary: string;
      resultMarkdown: string;
      criteria: Array<{ criterionId: string; evidenceMarkdown: string }>;
      artifacts: Array<{
        artifactId: string;
        artifactVersionId: string;
        title: string;
        artifactKind: string;
        externalUrl: string | null;
      }>;
    } | null;
  } | null;
  pendingHumanInterventions: Array<Record<string, unknown>>;
};

function parseArgs(args: string[]): Options {
  let origin = "http://localhost:3737";
  let timezone = "Etc/UTC";
  let timeoutMs = 600_000;
  const prompt: string[] = [];

  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index];
    if (argument === "--help") {
      console.log(HELP);
      process.exit(0);
    }
    if (argument === "--") {
      prompt.push(...args.slice(index + 1));
      break;
    }
    if (argument === "--origin" || argument === "--timezone" || argument === "--timeout-ms") {
      const value = args[index + 1];
      if (!value) throw new Error(`${argument} requires a value`);
      index += 1;
      if (argument === "--origin") origin = value;
      if (argument === "--timezone") timezone = value;
      if (argument === "--timeout-ms") timeoutMs = Number(value);
      continue;
    }
    prompt.push(argument);
  }

  if (!prompt.length) throw new Error("a prompt is required");
  if (!Number.isSafeInteger(timeoutMs) || timeoutMs <= 0) {
    throw new Error("--timeout-ms must be a positive integer");
  }
  return { origin: origin.replace(/\/$/, ""), timezone, timeoutMs, prompt: prompt.join(" ") };
}

async function graphql<T>(origin: string, query: string, variables = {}): Promise<T> {
  const response = await fetch(`${origin}/graphql`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ query, variables }),
  });
  if (!response.ok) throw new Error(`GraphQL HTTP ${response.status}`);
  const envelope = (await response.json()) as GraphqlEnvelope<T>;
  if (envelope.errors?.length) {
    throw new Error(envelope.errors.map((error) => error.message).join("; "));
  }
  if (!envelope.data) throw new Error("GraphQL response omitted data");
  return envelope.data;
}

async function findTurnId(options: Options, conversationId: string, clientMessageId: string) {
  const page = await graphql<{ conversationTranscriptPage: { items: Array<{ turnId: string | null; metadata: unknown }> } }>(
    options.origin,
    `query SubmittedTurn($conversationId: String!) {
      conversationTranscriptPage(input: { conversationId: $conversationId, limit: 200 }) {
        items { turnId metadata }
      }
    }`,
    { conversationId },
  );
  return page.conversationTranscriptPage.items.find((item) =>
    typeof item.metadata === "object"
      && item.metadata !== null
      && (item.metadata as Record<string, unknown>).client_message_id === clientMessageId
  )?.turnId ?? undefined;
}

async function turnIsTerminal(options: Options, turnId: string) {
  const result = await graphql<{ runtimeDebugProfile: { status: string } | null }>(
    options.origin,
    `query TurnStatus($turnId: String!) {
      runtimeDebugProfile(input: { kind: CONVERSATION_TURN, scopeId: $turnId }) { status }
    }`,
    { turnId },
  );
  return result.runtimeDebugProfile !== null && result.runtimeDebugProfile.status !== "RUNNING";
}

async function waitForTurn(options: Options, conversationId: string, clientMessageId: string) {
  const wsUrl = `${options.origin.replace(/^http/, "ws")}/graphql/ws`;
  const itemIds = new Set<string>();
  let turnId: string | undefined;
  const deadline = Date.now() + options.timeoutMs;

  const subscriptionCompleted = await new Promise<boolean>((resolve, reject) => {
    const socket = new WebSocket(wsUrl, "graphql-transport-ws");
    let sent = false;
    let settled = false;
    const timeout = setTimeout(() => {
      finish(() => reject(
        new Error(`turn timed out after ${options.timeoutMs} ms; no action was retried`),
      ));
    }, options.timeoutMs);

    const finish = (result: () => void) => {
      if (settled) return;
      settled = true;
      clearTimeout(timeout);
      socket.close();
      result();
    };

    socket.addEventListener("open", () => socket.send(JSON.stringify({ type: "connection_init" })));
    socket.addEventListener("error", () => finish(() => {
      if (sent) resolve(false);
      else reject(new Error("subscription disconnected before the turn was submitted"));
    }));
    socket.addEventListener("close", () => finish(() => {
      if (sent) resolve(false);
      else reject(new Error("subscription closed before the turn was submitted"));
    }));
    socket.addEventListener("message", async (message) => {
      const frame = JSON.parse(String(message.data));
      if (frame.type === "ping") {
        socket.send(JSON.stringify({ type: "pong", payload: frame.payload }));
        return;
      }
      if (frame.type === "connection_ack") {
        socket.send(JSON.stringify({
          id: "conversation-events",
          type: "subscribe",
          payload: {
            query: `subscription CaseEvents($conversationId: String!) {
              conversationEvents(conversationId: $conversationId) {
                __typename
                ... on SubscriptionReadyEvent { conversationId }
                ... on ConversationItemEvent { clientMessageId itemId turnId }
                ... on TurnCompletedEvent { clientMessageId }
              }
            }`,
            variables: { conversationId },
          },
        }));
        return;
      }
      if (frame.type === "error") {
        finish(() => reject(new Error(`subscription failed: ${JSON.stringify(frame.payload)}`)));
        return;
      }
      const event = frame.payload?.data?.conversationEvents;
      if (!event) return;
      if (event.__typename === "SubscriptionReadyEvent" && !sent) {
        sent = true;
        try {
          await graphql(options.origin, `mutation RunCase($input: SendConversationTurnInput!) {
            sendConversationTurn(input: $input) { conversationId clientMessageId }
          }`, {
            input: {
              conversationId,
              input: options.prompt,
              clientMessageId,
              clientTimeZone: options.timezone,
            },
          });
        } catch (error) {
          finish(() => reject(error));
        }
        return;
      }
      if (event.clientMessageId === clientMessageId && event.itemId) {
        itemIds.add(event.itemId);
        turnId ??= event.turnId ?? undefined;
      }
      if (event.__typename === "TurnCompletedEvent" && event.clientMessageId === clientMessageId) {
        finish(() => resolve(true));
      }
    });
  });

  if (subscriptionCompleted) return { itemIds, turnId };

  while (Date.now() < deadline) {
    turnId ??= await findTurnId(options, conversationId, clientMessageId);
    if (turnId && await turnIsTerminal(options, turnId)) return { itemIds, turnId };
    await Bun.sleep(Math.min(2_000, deadline - Date.now()));
  }

  throw new Error(`turn timed out after ${options.timeoutMs} ms; no action was retried`);
}

async function delegatedTaskState(options: Options, taskId: string) {
  return graphql<DelegatedTaskState>(options.origin, `query DelegatedTask($taskId: String!) {
    task(taskId: $taskId) {
      taskId title completedAt
      stage { key name behavior }
      currentRun { runId kind status attemptIndex updatedAt activityLabel }
      activeGate { gateId kind state prompt }
      completedResult {
        submissionId executorRunId summary resultMarkdown
        criteria { criterionId evidenceMarkdown }
        artifacts { artifactId artifactVersionId title artifactKind externalUrl }
      }
    }
    pendingHumanInterventions(taskId: $taskId, first: 50) {
      __typename
      ... on GovernedAction { actionId revision capabilityName safeSummary state }
      ... on McpAuthenticationIntervention { requestId revision capabilityName failureCode }
      ... on AdapterAuthenticationIntervention { requestId revision capabilityName state failureCode }
      ... on TaskAttention { kind title summary validActions }
    }
  }`, { taskId });
}

function delegatedTaskReachedBoundary(state: DelegatedTaskState) {
  return state.task === null
    || state.task.completedAt !== null
    || state.task.activeGate !== null
    || state.pendingHumanInterventions.length > 0;
}

async function waitForDelegatedTask(options: Options, taskId: string) {
  const initial = await delegatedTaskState(options, taskId);
  if (delegatedTaskReachedBoundary(initial)) return initial;

  const wsUrl = `${options.origin.replace(/^http/, "ws")}/graphql/ws`;
  const deadline = Date.now() + options.timeoutMs;
  const subscribed = await new Promise<DelegatedTaskState | null>((resolve, reject) => {
    const socket = new WebSocket(wsUrl, "graphql-transport-ws");
    let settled = false;
    let checking = false;
    let timeout: ReturnType<typeof setTimeout>;
    const finish = (result: () => void) => {
      if (settled) return;
      settled = true;
      clearTimeout(timeout);
      socket.close();
      result();
    };
    timeout = setTimeout(() => finish(() => reject(
      new Error(`delegated task ${taskId} timed out; no action was retried`),
    )), options.timeoutMs);
    const check = async () => {
      if (checking || settled) return;
      checking = true;
      try {
        const state = await delegatedTaskState(options, taskId);
        if (delegatedTaskReachedBoundary(state)) finish(() => resolve(state));
      } catch (error) {
        finish(() => reject(error));
      } finally {
        checking = false;
      }
    };

    socket.addEventListener("open", () => socket.send(JSON.stringify({ type: "connection_init" })));
    socket.addEventListener("error", () => finish(() => resolve(null)));
    socket.addEventListener("close", () => finish(() => resolve(null)));
    socket.addEventListener("message", (message) => {
      const frame = JSON.parse(String(message.data));
      if (frame.type === "ping") {
        socket.send(JSON.stringify({ type: "pong", payload: frame.payload }));
      } else if (frame.type === "connection_ack") {
        socket.send(JSON.stringify({
          id: `task-${taskId}`,
          type: "subscribe",
          payload: {
            query: `subscription DelegatedTaskEvents($taskId: String!) {
              taskRuntimeEvents(taskId: $taskId) { taskId runId }
            }`,
            variables: { taskId },
          },
        }));
        void check();
      } else if (frame.type === "next") {
        void check();
      } else if (frame.type === "error") {
        finish(() => reject(new Error(`task subscription failed: ${JSON.stringify(frame.payload)}`)));
      } else if (frame.type === "complete") {
        finish(() => resolve(null));
      }
    });
  });
  if (subscribed) return subscribed;

  while (Date.now() < deadline) {
    const state = await delegatedTaskState(options, taskId);
    if (delegatedTaskReachedBoundary(state)) return state;
    await Bun.sleep(Math.min(2_000, deadline - Date.now()));
  }
  throw new Error(`delegated task ${taskId} timed out; no action was retried`);
}

const options = parseArgs(process.argv.slice(2));
const primary = await graphql<{ primaryConversation: { conversationId: string } | null }>(
  options.origin,
  "query PrimaryConversation { primaryConversation { conversationId } }",
);
if (!primary.primaryConversation) throw new Error("Noema has no primary conversation");

const conversationId = primary.primaryConversation.conversationId;
const clientMessageId = `live-eval:${crypto.randomUUID()}`;
const completed = await waitForTurn(options, conversationId, clientMessageId);
const result = await graphql<{
  conversationTranscriptPage: { items: Array<Record<string, unknown>> };
  pendingGovernedActions: Array<Record<string, unknown>>;
  pendingHumanInterventions: Array<Record<string, unknown>>;
}>(options.origin, `query CaseResult($conversationId: String!) {
  conversationTranscriptPage(input: { conversationId: $conversationId, limit: 200 }) {
    items {
      itemId cursor turnId metadata
      item {
        __typename
        ... on UserText { text }
        ... on AssistantText { text }
        ... on Activity { id activityKind status title summary metadata }
        ... on ErrorNotice { message recoverable }
        ... on MultipleChoicePrompt { prompt selectionMode options { id label } }
        ... on ArtifactReference { artifactId artifactVersionId title artifactKind storageKind externalUrl }
        ... on TaskReference { taskId }
      }
    }
  }
  pendingGovernedActions(conversationId: $conversationId, first: 50) {
    actionId revision capabilityName reviewRoute safeSummary destination state
    behavior { readOnly idempotent destructive openWorld }
    assessment { status authorization risk reasonCodes explanation }
  }
  pendingHumanInterventions(conversationId: $conversationId, first: 50) {
    __typename
    ... on GovernedAction { actionId revision capabilityName safeSummary state }
    ... on McpAuthenticationIntervention { requestId revision capabilityName failureCode }
    ... on AdapterAuthenticationIntervention { requestId revision capabilityName state failureCode }
    ... on McpSetupIntervention { itemId setupStatus displayName }
  }
}`, { conversationId });

const transcriptItems = result.conversationTranscriptPage.items.filter((item) =>
  item.turnId === completed.turnId || completed.itemIds.has(String(item.itemId)),
);
const taskIds = [...new Set(transcriptItems.flatMap((entry) => {
  const item = entry.item as {
    __typename?: string;
    taskId?: string;
    metadata?: { action?: {
      provider_name?: string;
      success?: boolean;
      payload?: { task?: { task_id?: string } };
    } };
  } | undefined;
  if (item?.__typename === "TaskReference" && item.taskId) return [item.taskId];
  const action = item?.__typename === "Activity" ? item.metadata?.action : undefined;
  const delegatedTaskId = action?.provider_name === "delegate" && action.success === true
    ? action.payload?.task?.task_id
    : undefined;
  return delegatedTaskId ? [delegatedTaskId] : [];
}))];
const delegatedTasks = await Promise.all(taskIds.map(async (taskId) => ({
  taskId,
  state: await waitForDelegatedTask(options, taskId),
})));
console.log(JSON.stringify({
  conversationId,
  clientMessageId,
  transcriptItems,
  delegatedTasks,
  pendingGovernedActions: result.pendingGovernedActions,
  pendingHumanInterventions: result.pendingHumanInterventions,
}, null, 2));

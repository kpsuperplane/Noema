const HELP = `Usage: bun run scripts/run-live-noema-case.ts [options] -- <prompt>

Submit one prompt to Noema's primary conversation and wait for its exact turn.

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
console.log(JSON.stringify({
  conversationId,
  clientMessageId,
  transcriptItems,
  pendingGovernedActions: result.pendingGovernedActions,
  pendingHumanInterventions: result.pendingHumanInterventions,
}, null, 2));

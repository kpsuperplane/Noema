import * as React from "react";
import { useMutation } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import { VStack } from "@astryxdesign/core/VStack";
import * as stylex from "@stylexjs/stylex";
import {
  CreateMcpServerDocument,
  ResolveMcpSetupInterventionDocument,
  SaveCapabilityConnectionPolicyDocument,
  StartMcpServerOauthSetupDocument,
  type CreateMcpServerInput,
  type CreateMcpServerMutation,
  type PendingHumanInterventionsQuery,
  type ResolveMcpSetupInterventionMutation,
  type SaveCapabilityConnectionPolicyMutation,
  type StartMcpServerOauthSetupMutation
} from "@/generated/graphql";
import {
  CapabilityPolicyChoices,
  type CapabilityDataSharingPolicy,
  type CapabilityUnsafeActionPolicy
} from "@/components/capabilities/CapabilityPolicyChoices";
import { HumanInterventionCard } from "@/components/actions/HumanInterventionCard";
import { mcpOAuthRedirectUri } from "@/graphql/mcpOAuthCallback";
import { reserveExternalAuthNavigation } from "@/graphql/externalUrls";
import { useMcpOAuthController } from "./useMcpOAuthController";

type SetupStatus = "needs_auth" | "authentication_available" | "ready_for_policy";
type McpChatSetup = Extract<
  PendingHumanInterventionsQuery["pendingHumanInterventions"][number],
  { __typename: "McpSetupIntervention" }
>;

type SetupServer = {
  mcpServerId: string;
  connectionRevision: string;
  policyRevision: number;
  toolCount: number;
};

export function McpChatSetupCard({
  setup,
  onResolved
}: {
  setup: McpChatSetup;
  onResolved?: () => void;
}) {
  const setupInput = createSetupInput(setup);
  const [status, setStatus] = React.useState<SetupStatus>(() => setupStatus(setup.setupStatus));
  const [server, setServer] = React.useState<SetupServer | null>(() => setupServerFromIntervention(setup));
  const [sharing, setSharing] = React.useState<CapabilityDataSharingPolicy>("allow_automatically");
  const [unsafeActions, setUnsafeActions] = React.useState<CapabilityUnsafeActionPolicy>("reviewer_may_approve");
  const [policyStep, setPolicyStep] = React.useState<"sharing" | "unsafe_actions">("sharing");
  const [connected, setConnected] = React.useState(false);
  const [policySaved, setPolicySaved] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);
  const [startOAuth, oauthStart] = useMutation<StartMcpServerOauthSetupMutation>(
    StartMcpServerOauthSetupDocument
  );
  const [createServer, createState] = useMutation<CreateMcpServerMutation>(CreateMcpServerDocument);
  const [savePolicy, policyState] = useMutation<SaveCapabilityConnectionPolicyMutation>(
    SaveCapabilityConnectionPolicyDocument
  );
  const [resolveSetup] = useMutation<ResolveMcpSetupInterventionMutation>(
    ResolveMcpSetupInterventionDocument
  );
  const oauth = useMcpOAuthController<{ setupId: string }>({
    onCompleted: (attempt) => {
      const completed = setupServerFromGraphql(attempt.setupResult?.server);
      if (!completed || attempt.setupResult?.setupStatus !== "ready_for_policy") {
        setError("Noema signed in, but MCP tool discovery did not finish.");
        return;
      }
      setServer(completed);
      setStatus("ready_for_policy");
      setError(null);
    },
    onFailed: (message) => setError(message)
  });

  const beginOAuth = async () => {
    const navigation = reserveExternalAuthNavigation();
    setError(null);
    try {
      const redirectUri = await mcpOAuthRedirectUri();
      const response = await startOAuth({
        variables: { input: { server: setupInput, redirectUri } }
      });
      const attempt = response.data?.startMcpServerOauthSetup;
      if (!attempt) throw new Error("Noema did not return an MCP OAuth attempt.");
      await oauth.begin(attempt, { setupId: setup.endpointUrl }, navigation);
    } catch (caught: unknown) {
      navigation.cancel();
      setError(caught instanceof Error ? caught.message : "MCP OAuth setup failed.");
    }
  };

  const connectAnonymously = async () => {
    setError(null);
    try {
      const response = await createServer({
        variables: {
          input: { ...setupInput, authPreference: "USE_ANONYMOUS" }
        }
      });
      const result = response.data?.createMcpServer;
      const created = setupServerFromGraphql(result?.server);
      if (!created || result?.setupStatus !== "ready_for_policy") {
        throw new Error("Noema could not finish public MCP discovery.");
      }
      setServer(created);
      setStatus("ready_for_policy");
    } catch (caught: unknown) {
      setError(caught instanceof Error ? caught.message : "MCP setup failed.");
    }
  };

  const submitPolicy = async () => {
    if (!server) return;
    setError(null);
    try {
      if (!policySaved) {
        await savePolicy({
          variables: {
            input: {
              kind: "MCP",
              connectionId: server.mcpServerId,
              expectedConnectionRevision: server.connectionRevision,
              expectedPolicyRevision: server.policyRevision,
              dataSharingPolicy: sharing,
              unsafeActionPolicy: unsafeActions
            }
          }
        });
        setPolicySaved(true);
      }
      await resolveSetup({
        variables: {
          input: { itemId: setup.itemId, mcpServerId: server.mcpServerId }
        }
      });
      setConnected(true);
      onResolved?.();
    } catch (caught: unknown) {
      setError(caught instanceof Error ? caught.message : "The MCP policy could not be saved.");
    }
  };

  const waitingForOAuth = oauthStart.loading || oauth.active !== null;
  const title = connected
    ? `${setup.displayName} is connected`
    : status === "ready_for_policy"
      ? `Enable ${setup.displayName}`
      : `Connect ${setup.displayName}`;
  const description = connected
    ? `${server?.toolCount ?? setup.discoveredToolCount} MCP tools are ready for Noema to use under your policy.`
    : status === "ready_for_policy"
      ? policyStep === "sharing"
        ? "Your account is connected. Choose when Noema may share relevant conversation details."
        : "Choose who may approve calls that can change, delete, or send information."
      : status === "authentication_available"
        ? `${setup.discoveredToolCount} tools are public. Sign in for full access or continue with the public tools only.`
        : "This service requires browser sign-in before Noema can discover its tools.";

  return (
    <HumanInterventionCard
      label={connected ? "MCP connected" : status === "ready_for_policy" ? `Tool permissions · ${policyStep === "sharing" ? "1" : "2"} of 2` : "MCP setup"}
      title={title}
      description={description}
      error={error}
      actions={connected ? null : policyActions({
        status,
        displayName: setup.displayName,
        oauthSupported: setup.oauthSupported,
        waitingForOAuth,
        creating: createState.loading,
        saving: policyState.loading,
        policyStep,
        onBack: () => setPolicyStep("sharing"),
        onContinue: () => setPolicyStep("unsafe_actions"),
        onOAuth: () => void beginOAuth(),
        onAnonymous: () => void connectAnonymously(),
        onSave: () => void submitPolicy()
      })}
    >
      <VStack gap={3}>
        {status === "ready_for_policy" && !connected ? (
          <CapabilityPolicyChoices
            serviceName={setup.displayName}
            step={policyStep}
            dataSharingPolicy={sharing}
            unsafeActionPolicy={unsafeActions}
            onChange={(policy) => {
              setSharing(policy.dataSharingPolicy);
              setUnsafeActions(policy.unsafeActionPolicy);
            }}
          />
        ) : null}
        <details {...stylex.props(styles.details)}>
          <summary>Connection details</summary>
          <VStack gap={1}>
            {setup.description ? <span>{setup.description}</span> : null}
            <a href={setup.serviceUrl} target="_blank" rel="noreferrer">Official website</a>
            <span {...stylex.props(styles.endpoint)}>{setup.endpointUrl}</span>
          </VStack>
        </details>
      </VStack>
    </HumanInterventionCard>
  );
}

function policyActions({
  status,
  displayName,
  oauthSupported,
  waitingForOAuth,
  creating,
  saving,
  policyStep,
  onBack,
  onContinue,
  onOAuth,
  onAnonymous,
  onSave
}: {
  status: SetupStatus;
  displayName: string;
  oauthSupported: boolean;
  waitingForOAuth: boolean;
  creating: boolean;
  saving: boolean;
  policyStep: "sharing" | "unsafe_actions";
  onBack: () => void;
  onContinue: () => void;
  onOAuth: () => void;
  onAnonymous: () => void;
  onSave: () => void;
}) {
  if (status === "ready_for_policy") {
    return policyStep === "sharing" ? (
      <Button size="sm" variant="primary" label="Continue" onClick={onContinue} />
    ) : (
      <>
        <Button size="sm" variant="ghost" label="Back" isDisabled={saving} onClick={onBack} />
        <Button size="sm" variant="primary" label={`Enable ${displayName}`} isLoading={saving} isDisabled={saving} onClick={onSave} />
      </>
    );
  }
  if (!oauthSupported) return null;
  return (
    <>
      {status === "authentication_available" ? (
        <Button size="sm" variant="ghost" label="Use public tools only" isLoading={creating} isDisabled={creating || waitingForOAuth} onClick={onAnonymous} />
      ) : null}
      <Button size="sm" variant="primary" label="Continue in browser" isLoading={waitingForOAuth} isDisabled={waitingForOAuth || creating} onClick={onOAuth} />
    </>
  );
}

function createSetupInput(setup: McpChatSetup): CreateMcpServerInput {
  return {
    displayName: setup.displayName,
    transportKind: "streamable_http",
    stdio: null,
    http: {
      url: setup.endpointUrl,
      headers: {},
      secretHeaders: {},
      oauthClientCredentials: null
    }
  };
}

function setupServerFromIntervention(setup: McpChatSetup): SetupServer | null {
  if (!setup.setupMcpServerId || !setup.connectionRevision || setup.policyRevision === null) return null;
  return {
    mcpServerId: setup.setupMcpServerId,
    connectionRevision: setup.connectionRevision,
    policyRevision: setup.policyRevision,
    toolCount: setup.toolCount ?? 0
  };
}

function setupServerFromGraphql(
  value: NonNullable<CreateMcpServerMutation["createMcpServer"]["server"]> | null | undefined
): SetupServer | null {
  if (!value) return null;
  return {
    mcpServerId: value.mcpServerId,
    connectionRevision: value.connectionRevision,
    policyRevision: value.policyRevision,
    toolCount: value.toolCount
  };
}

function setupStatus(value: string): SetupStatus {
  return value === "authentication_available" || value === "ready_for_policy"
    ? value
    : "needs_auth";
}

const styles = stylex.create({
  details: {
    color: "var(--noema-text-muted)",
    fontSize: "var(--text-supporting-size)",
    lineHeight: "var(--text-supporting-leading)",
    ":is([open])": {
      color: "var(--noema-text-secondary)"
    }
  },
  endpoint: {
    color: "var(--noema-text-muted)",
    fontFamily: "var(--font-family-code)",
    overflowWrap: "anywhere"
  }
});

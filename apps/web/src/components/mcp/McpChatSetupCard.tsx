import * as React from "react";
import { useMutation } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import { VStack } from "@astryxdesign/core/VStack";
import * as stylex from "@stylexjs/stylex";
import {
  CreateMcpServerDocument,
  SaveCapabilityConnectionPolicyDocument,
  StartMcpServerOauthSetupDocument,
  type CreateMcpServerInput,
  type CreateMcpServerMutation,
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
import { useMcpOAuthController } from "./useMcpOAuthController";
import type { ToolMarkerGroup } from "@/components/transcript/renderModel";

const CONNECT_SERVICE_TOOL = "mcp.connect_service";

type SetupStatus = "needs_auth" | "authentication_available" | "ready_for_policy";

type SetupServer = {
  mcpServerId: string;
  connectionRevision: string;
  policyRevision: number;
  toolCount: number;
};

type McpChatSetup = {
  status: SetupStatus;
  displayName: string;
  description: string | null;
  serviceUrl: string;
  endpointUrl: string;
  setupInput: CreateMcpServerInput;
  oauthSupported: boolean;
  discoveredToolCount: number;
  server: SetupServer | null;
};

export function mcpChatSetupFromMarker(marker: ToolMarkerGroup): McpChatSetup | null {
  const action = recordValue(marker.result?.item.metadata, "action");
  if (stringValue(action?.name) !== CONNECT_SERVICE_TOOL || action?.success !== true) return null;
  const payload = recordValue(action, "payload");
  const status = stringValue(payload?.status);
  if (!isSetupStatus(status)) return null;
  const displayName = stringValue(payload?.display_name);
  const serviceUrl = stringValue(payload?.service_url);
  const endpointUrl = stringValue(payload?.endpoint_url);
  const setupInput = createSetupInput(payload?.setup_input);
  if (!displayName || !serviceUrl || !endpointUrl || !setupInput) return null;
  const setupResult = recordValue(payload, "setup_result");
  const auth = recordValue(setupResult, "auth");
  return {
    status,
    displayName,
    description: stringValue(payload?.description),
    serviceUrl,
    endpointUrl,
    setupInput,
    oauthSupported: auth?.oauth_authorization_supported === true,
    discoveredToolCount: numberValue(setupResult?.discovered_tool_count) ?? 0,
    server: setupServerFromToolResult(recordValue(setupResult, "server"))
  };
}

export function McpChatSetupCard({ setup }: { setup: McpChatSetup }) {
  const [status, setStatus] = React.useState<SetupStatus>(setup.status);
  const [server, setServer] = React.useState<SetupServer | null>(setup.server);
  const [sharing, setSharing] = React.useState<CapabilityDataSharingPolicy>("allow_automatically");
  const [unsafeActions, setUnsafeActions] = React.useState<CapabilityUnsafeActionPolicy>("reviewer_may_approve");
  const [policyStep, setPolicyStep] = React.useState<"sharing" | "unsafe_actions">("sharing");
  const [connected, setConnected] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);
  const [startOAuth, oauthStart] = useMutation<StartMcpServerOauthSetupMutation>(
    StartMcpServerOauthSetupDocument
  );
  const [createServer, createState] = useMutation<CreateMcpServerMutation>(CreateMcpServerDocument);
  const [savePolicy, policyState] = useMutation<SaveCapabilityConnectionPolicyMutation>(
    SaveCapabilityConnectionPolicyDocument
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
    setError(null);
    try {
      const redirectUri = await mcpOAuthRedirectUri();
      const response = await startOAuth({
        variables: { input: { server: setup.setupInput, redirectUri } }
      });
      const attempt = response.data?.startMcpServerOauthSetup;
      if (!attempt) throw new Error("Noema did not return an MCP OAuth attempt.");
      await oauth.begin(attempt, { setupId: setup.endpointUrl });
    } catch (caught: unknown) {
      setError(caught instanceof Error ? caught.message : "MCP OAuth setup failed.");
    }
  };

  const connectAnonymously = async () => {
    setError(null);
    try {
      const response = await createServer({
        variables: {
          input: { ...setup.setupInput, authPreference: "USE_ANONYMOUS" }
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
      setConnected(true);
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

function createSetupInput(value: unknown): CreateMcpServerInput | null {
  if (!isRecord(value)) return null;
  const displayName = stringValue(value.displayName);
  const transportKind = stringValue(value.transportKind);
  const http = recordValue(value, "http");
  const url = stringValue(http?.url);
  if (!displayName || transportKind !== "streamable_http" || !url) return null;
  return {
    displayName,
    transportKind,
    stdio: null,
    http: {
      url,
      headers: isRecord(http?.headers) ? http.headers : {},
      secretHeaders: {},
      oauthClientCredentials: null
    }
  };
}

function setupServerFromToolResult(value: Record<string, unknown> | null): SetupServer | null {
  const mcpServerId = stringValue(value?.mcp_server_id);
  const connectionRevision = stringValue(value?.connection_revision);
  const policyRevision = numberValue(value?.policy_revision);
  if (!mcpServerId || !connectionRevision || policyRevision === null) return null;
  return {
    mcpServerId,
    connectionRevision,
    policyRevision,
    toolCount: numberValue(value?.tool_count) ?? 0
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

function isSetupStatus(value: string | null): value is SetupStatus {
  return value === "needs_auth" || value === "authentication_available" || value === "ready_for_policy";
}

function recordValue(value: unknown, key: string): Record<string, unknown> | null {
  return isRecord(value) && isRecord(value[key]) ? value[key] : null;
}

function stringValue(value: unknown): string | null {
  return typeof value === "string" && value.trim() ? value : null;
}

function numberValue(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
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

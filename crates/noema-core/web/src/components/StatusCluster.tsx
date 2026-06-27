import type { LocalStatusQuery } from "../generated/graphql";
import type { ConversationAgentStatus, SocketState } from "../types";

export function StatusCluster({
  status,
  socketState,
  agentStatus
}: {
  status: LocalStatusQuery["localStatus"] | null;
  socketState: SocketState;
  agentStatus: ConversationAgentStatus;
}) {
  const localService = status?.localService === "RUNNING" ? "Ready" : "Checking";
  const memory = status?.memoryStorage === "READY" ? "Memory ready" : "Memory starting";
  const socket = socketState === "ready" ? "Chat live" : socketState === "connecting" ? "Connecting" : "Disconnected";
  const agent = agentLabel[agentStatus];

  return (
    <div className="status-cluster" aria-label="Local status">
      <StatusPill tone={status?.localService === "RUNNING" ? "good" : "neutral"}>{localService}</StatusPill>
      <StatusPill tone={status?.memoryStorage === "READY" ? "good" : "neutral"}>{memory}</StatusPill>
      <StatusPill tone={socketState === "ready" ? "good" : socketState === "closed" ? "bad" : "neutral"}>
        {socket}
      </StatusPill>
      <StatusPill tone={agentStatus === "ERROR" || agentStatus === "closed" ? "bad" : "neutral"}>
        {agent}
      </StatusPill>
    </div>
  );
}

function StatusPill({ tone, children }: { tone: "good" | "bad" | "neutral"; children: React.ReactNode }) {
  return <span className={`status-pill status-pill--${tone}`}>{children}</span>;
}

const agentLabel: Record<ConversationAgentStatus, string> = {
  IDLE: "Idle",
  INPUT_RECEIVED: "Input received",
  THINKING: "Thinking",
  TOOL_RUNNING: "Tool running",
  WAITING_FOR_PREVIOUS_TURN_COMPLETION: "Waiting",
  INTERRUPTING: "Interrupting",
  ERROR: "Error",
  connecting: "Connecting",
  closed: "Disconnected"
};

import type { WebStatus } from "../generated/noema";
import type { ConversationAgentStatus, SocketState } from "../types";

export function StatusCluster({
  status,
  socketState,
  agentStatus
}: {
  status: WebStatus | null;
  socketState: SocketState;
  agentStatus: ConversationAgentStatus;
}) {
  const localService = status?.local_service === "running" ? "Ready" : "Checking";
  const memory = status?.memory_storage === "ready" ? "Memory ready" : "Memory starting";
  const socket = socketState === "ready" ? "Chat live" : socketState === "connecting" ? "Connecting" : "Disconnected";
  const agent = agentLabel[agentStatus];

  return (
    <div className="status-cluster" aria-label="Local status">
      <StatusPill tone={status?.local_service === "running" ? "good" : "neutral"}>{localService}</StatusPill>
      <StatusPill tone={status?.memory_storage === "ready" ? "good" : "neutral"}>{memory}</StatusPill>
      <StatusPill tone={socketState === "ready" ? "good" : socketState === "closed" ? "bad" : "neutral"}>
        {socket}
      </StatusPill>
      <StatusPill tone={agentStatus === "error" || agentStatus === "closed" ? "bad" : "neutral"}>{agent}</StatusPill>
    </div>
  );
}

function StatusPill({ tone, children }: { tone: "good" | "bad" | "neutral"; children: React.ReactNode }) {
  return <span className={`status-pill status-pill--${tone}`}>{children}</span>;
}

const agentLabel: Record<ConversationAgentStatus, string> = {
  idle: "Idle",
  input_received: "Input received",
  thinking: "Thinking",
  tool_running: "Tool running",
  waiting_for_previous_turn_completion: "Waiting",
  interrupting: "Interrupting",
  error: "Error",
  connecting: "Connecting",
  closed: "Disconnected"
};

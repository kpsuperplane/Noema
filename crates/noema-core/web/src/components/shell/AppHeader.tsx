import { StatusCluster } from "@/components/StatusCluster";
import type { LocalStatusQuery } from "@/generated/graphql";
import type { ConversationAgentStatus, SocketState } from "@/types";

export function AppHeader({
  status,
  socketState,
  agentStatus
}: {
  status: LocalStatusQuery["localStatus"] | null;
  socketState: SocketState;
  agentStatus: ConversationAgentStatus;
}) {
  return (
    <header className="topbar">
      <div className="brand">
        <img src="/assets/noema-mark.svg" width="34" height="34" alt="" />
        <div>
          <strong>Noema</strong>
          <span>Local chat</span>
        </div>
      </div>
      <StatusCluster status={status} socketState={socketState} agentStatus={agentStatus} />
    </header>
  );
}

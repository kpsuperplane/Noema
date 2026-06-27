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
    <header className="flex min-h-[68px] items-center justify-between gap-[18px] border-b border-[var(--border-subtle)] bg-white/90 px-7 max-[760px]:flex-col max-[760px]:items-start max-[760px]:px-5 max-[760px]:py-4">
      <div className="flex min-w-0 items-center gap-[11px]">
        <img src="/assets/noema-mark.svg" width="34" height="34" alt="" />
        <div>
          <strong className="block font-heading text-lg tracking-normal">Noema</strong>
          <span className="block text-xs text-muted-foreground">Local chat</span>
        </div>
      </div>
      <StatusCluster status={status} socketState={socketState} agentStatus={agentStatus} />
    </header>
  );
}

import { cn } from "@/lib/utils";
import { IdentityAvatar, LOCAL_AGENT_AVATAR_ID, LOCAL_HUMAN_AVATAR_ID } from "../IdentityAvatar";
import {
  Message as MessagePrimitive,
  MessageAvatar,
  MessageContent
} from "@/components/ui/message";
import type { TranscriptLane } from "./renderModel";

export function TranscriptRow({
  lane,
  showAvatar = true,
  children
}: {
  lane: TranscriptLane;
  showAvatar?: boolean;
  children: React.ReactNode;
}) {
  const actorId = lane === "human" ? LOCAL_HUMAN_AVATAR_ID : LOCAL_AGENT_AVATAR_ID;
  const actorType = lane === "human" ? "human" : "agent";

  return (
    <MessagePrimitive align={lane === "human" ? "end" : "start"} className="max-w-[760px]">
      <MessageAvatar aria-hidden={!showAvatar} className={cn(!showAvatar && "invisible")}>
        <IdentityAvatar actorId={actorId} actorType={actorType} size="sm" />
      </MessageAvatar>
      <MessageContent>{children}</MessageContent>
    </MessagePrimitive>
  );
}

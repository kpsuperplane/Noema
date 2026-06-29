import BoringAvatar from "boring-avatars";

import { Avatar } from "@/components/ui/avatar";
import { cn } from "@/lib/utils";

const NOEMA_AVATAR_COLORS = ["#5b4b8a", "#ff7a59", "#ffd166", "#8bd3ff", "#2b2d42"];

export const LOCAL_HUMAN_AVATAR_ID = "human:local";
export const LOCAL_AGENT_AVATAR_ID = "agent:local";

export function IdentityAvatar({
  actorId,
  className,
  size = "default"
}: {
  actorId: string;
  className?: string;
  size?: "default" | "sm" | "lg";
}) {
  return (
    <Avatar className={className} size={size}>
      <BoringAvatar
        aria-hidden="true"
        className={cn("size-full rounded-full")}
        colors={NOEMA_AVATAR_COLORS}
        data-avatar-seed={actorId}
        focusable="false"
        name={actorId}
        size="100%"
        title={false}
        variant="beam"
      />
    </Avatar>
  );
}

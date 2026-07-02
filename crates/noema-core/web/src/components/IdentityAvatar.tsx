import { Avatar } from "@astryxdesign/core/Avatar";

export const NOEMA_AVATAR_COLORS = ["#3b4a6b", "#7d6a91", "#b9786d", "#d6ad6b", "#e6d8c4", "#2f3440"];

export const LOCAL_HUMAN_AVATAR_ID = "human:local";
export const LOCAL_AGENT_AVATAR_ID = "agent:local";

export type IdentityAvatarActorType = "agent" | "human";

export function avatarSeedForActorId(actorId: string): string {
  let hash = 0x811c9dc5;
  for (let index = 0; index < actorId.length; index += 1) {
    hash ^= actorId.charCodeAt(index);
    hash = Math.imul(hash, 0x01000162);
  }
  return `actor-${(hash >>> 0).toString(16).padStart(8, "0")}`;
}

function avatarVariantForActorType(actorType: IdentityAvatarActorType) {
  return actorType === "human" ? "marble" : "beam";
}

export function IdentityAvatar({
  actorId,
  actorType,
  className,
  size = "default"
}: {
  actorId: string;
  actorType: IdentityAvatarActorType;
  className?: string;
  size?: "default" | "sm" | "lg";
}) {
  const avatarSeed = avatarSeedForActorId(actorId);
  const avatarSize = size === "sm" ? "xsmall" : size === "lg" ? "medium" : "small";
  const avatarProps = className ? { className } : {};

  return (
    <Avatar
      {...avatarProps}
      alt={actorType === "human" ? "Human avatar" : "Agent avatar"}
      data-avatar-seed={avatarSeed}
      data-avatar-variant={avatarVariantForActorType(actorType)}
      name={avatarSeed}
      size={avatarSize}
    />
  );
}

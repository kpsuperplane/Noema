import * as stylex from "@stylexjs/stylex";
import BoringAvatar from "boring-avatars";

export const NOEMA_AVATAR_COLORS = ["#3b4a6b", "#7d6a91", "#b9786d", "#d6ad6b", "#e6d8c4", "#2f3440"];

export const LOCAL_HUMAN_AVATAR_ID = "human:local";
export const LOCAL_AGENT_AVATAR_ID = "agent:local";

export type IdentityAvatarActorType = "agent" | "human";
const CLASS_NAME_PROP = "className";

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

const styles = stylex.create({
  root: {
    display: "inline-flex",
    alignItems: "center",
    justifyContent: "center",
    flexShrink: 0,
    overflow: "hidden",
    borderRadius: 999,
    backgroundColor: "var(--noema-surface-sunken)"
  },
  defaultSize: {
    width: 36,
    height: 36
  },
  smSize: {
    width: 28,
    height: 28
  },
  lgSize: {
    width: 44,
    height: 44
  },
  avatar: {
    display: "block",
    width: "100%",
    height: "100%",
    borderRadius: 999
  }
});

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
  const avatarVariant = avatarVariantForActorType(actorType);
  const rootProps = stylex.props(
    styles.root,
    size === "sm" && styles.smSize,
    size === "lg" && styles.lgSize,
    size === "default" && styles.defaultSize
  );
  const avatarProps = stylex.props(styles.avatar);
  const rootClassName = [rootProps.className, className].filter(Boolean).join(" ");
  const rootClassNameProp = rootClassName ? { [CLASS_NAME_PROP]: rootClassName } : {};
  const avatarClassNameProp = avatarProps.className ? { [CLASS_NAME_PROP]: avatarProps.className } : {};

  return (
    <span
      {...rootProps}
      {...rootClassNameProp}
      aria-label={actorType === "human" ? "Human avatar" : "Agent avatar"}
      data-avatar-seed={avatarSeed}
      data-avatar-variant={avatarVariant}
      role="img"
    >
      <BoringAvatar
        {...avatarClassNameProp}
        aria-hidden="true"
        colors={NOEMA_AVATAR_COLORS}
        focusable="false"
        name={avatarSeed}
        size="100%"
        title={false}
        variant={avatarVariant}
      />
    </span>
  );
}

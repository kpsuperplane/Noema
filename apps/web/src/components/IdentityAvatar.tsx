import * as React from "react";
import * as stylex from "@stylexjs/stylex";
import Avatar, { type AvatarActivity } from "@kpsuperplane/boring-avatars";
import type { ConversationAgentStatus } from "@/shared/types";

export const NOEMA_AVATAR_COLORS = ["#3b4a6b", "#7d6a91", "#b9786d", "#d6ad6b", "#e6d8c4", "#2f3440"];

export const LOCAL_HUMAN_AVATAR_ID = "human:local";
export const LOCAL_AGENT_AVATAR_ID = "agent:local";

export type IdentityAvatarActorType = "agent" | "human";
export type IdentityAvatarActivity = AvatarActivity;
const CLASS_NAME_PROP = "className";

export function avatarActivityForAgentStatus(
  status: ConversationAgentStatus
): IdentityAvatarActivity {
  switch (status) {
    case "INPUT_RECEIVED":
    case "WAITING_FOR_PREVIOUS_TURN_COMPLETION":
    case "INTERRUPTING":
      return "listening";
    case "THINKING":
    case "TOOL_RUNNING":
      return "thinking";
    case "IDLE":
    case "ERROR":
    case "connecting":
    case "closed":
      return "idle";
  }
}

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
    cornerShape: "var(--corner-shape-full)",
    backgroundColor: "var(--noema-surface-sunken)"
  },
  xsSize: {
    width: 16,
    height: 16
  },
  navSize: {
    width: 24,
    height: 24
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
    borderRadius: 999,
    cornerShape: "var(--corner-shape-full)"
  }
});

export function IdentityAvatar({
  activity = "idle",
  actorId,
  actorType,
  animated = false,
  audioLevel,
  className,
  focusable = true,
  label,
  size = "default"
}: {
  activity?: IdentityAvatarActivity;
  actorId: string;
  actorType: IdentityAvatarActorType;
  animated?: boolean;
  audioLevel?: number;
  className?: string;
  focusable?: boolean;
  label?: string;
  size?: "xs" | "nav" | "default" | "sm" | "lg";
}) {
  const [focused, setFocused] = React.useState(false);
  const [hovered, setHovered] = React.useState(false);
  const avatarSeed = avatarSeedForActorId(actorId);
  const avatarVariant = avatarVariantForActorType(actorType);
  const interactionAnimated = focused || hovered;
  const rootProps = stylex.props(
    styles.root,
    size === "xs" && styles.xsSize,
    size === "nav" && styles.navSize,
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
      aria-label={label ?? (actorType === "human" ? "Human avatar" : "Agent avatar")}
      data-avatar-seed={avatarSeed}
      data-avatar-variant={avatarVariant}
      onBlur={() => setFocused(false)}
      onFocus={() => setFocused(true)}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      onPointerDown={focusable ? (event) => {
        if (event.pointerType === "touch") event.currentTarget.focus({ preventScroll: true });
      } : undefined}
      role="img"
      tabIndex={focusable ? -1 : undefined}
      title={label}
    >
      <Avatar
        {...avatarClassNameProp}
        activity={animated ? activity : "idle"}
        animated={animated || interactionAnimated}
        aria-hidden="true"
        audioLevel={audioLevel}
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

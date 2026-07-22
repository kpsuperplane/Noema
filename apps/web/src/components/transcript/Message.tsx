import * as React from "react";
import { ContextMenu } from "@astryxdesign/core/ContextMenu";
import { Markdown, type MarkdownProps } from "@astryxdesign/core/Markdown";
import * as stylex from "@stylexjs/stylex";
import type { ReactNode } from "react";
import type { IdentityAvatarActivity } from "../IdentityAvatar";
import type { ProviderUsageDebug } from "./debugUsage";
import { ExpandableTextBubbleContent } from "./ExpandableTextBubble";
import { TranscriptChatBubble } from "./TranscriptChatBubble";
import type { ChatBubbleGroup } from "./renderModel";
import { TypingMessageContent } from "./TypingMessage";

type MarkdownXStyle = MarkdownProps["xstyle"];
type MarkdownComponents = NonNullable<MarkdownProps["components"]>;

const styles = stylex.create({
  markdown: {
    color: "inherit",
    fontFamily: "inherit",
    fontSize: "inherit",
    lineHeight: "inherit"
  },
  userMarkdown: {
    "--color-text-primary": "currentColor",
    "--color-text-secondary": "color-mix(in srgb, currentColor 80%, transparent)",
    "--color-text-disabled": "color-mix(in srgb, currentColor 62%, transparent)",
    "--color-text-accent": "currentColor",
    "--color-border": "color-mix(in srgb, currentColor 24%, transparent)",
    "--color-border-emphasized": "color-mix(in srgb, currentColor 48%, transparent)",
    "--color-background-muted": "color-mix(in srgb, currentColor 12%, transparent)",
    "--color-background-surface": "color-mix(in srgb, black 10%, transparent)",
    "--color-accent-muted": "color-mix(in srgb, currentColor 16%, transparent)",
    "--color-overlay-hover": "color-mix(in srgb, currentColor 14%, transparent)",
    "--color-syntax-background": "color-mix(in srgb, black 16%, transparent)",
    "--color-syntax-keyword": "currentColor",
    "--color-syntax-string": "color-mix(in srgb, currentColor 88%, transparent)",
    "--color-syntax-comment": "color-mix(in srgb, currentColor 66%, transparent)",
    "--color-syntax-number": "color-mix(in srgb, currentColor 88%, transparent)",
    "--color-syntax-function": "currentColor",
    "--color-syntax-type": "currentColor",
    "--color-syntax-variable": "currentColor",
    "--color-syntax-operator": "color-mix(in srgb, currentColor 74%, transparent)",
    "--color-syntax-constant": "color-mix(in srgb, currentColor 88%, transparent)",
    "--color-syntax-tag": "currentColor",
    "--color-syntax-attribute": "color-mix(in srgb, currentColor 88%, transparent)",
    "--color-syntax-property": "color-mix(in srgb, currentColor 88%, transparent)",
    "--color-syntax-punctuation": "color-mix(in srgb, currentColor 66%, transparent)"
  },
  userLink: {
    color: "currentColor",
    fontWeight: 600,
    textDecoration: "underline",
    textDecorationColor: "color-mix(in srgb, currentColor 76%, transparent)",
    textDecorationThickness: 1,
    textUnderlineOffset: 3
  },
  assistantContextMenu: {
    width: "100%",
    maxWidth: "100%",
    minWidth: 0
  },
  content: {
    display: "grid",
    minWidth: 0
  },
  attachment: {
    display: "flex",
    alignItems: "flex-start",
    flexDirection: "column",
    gap: "var(--spacing-1)",
    minWidth: 0,
    paddingBlockStart: "var(--spacing-1)"
  }
});

const userMarkdownComponents: MarkdownComponents = {
  link: UserBubbleMarkdownLink
};

export function Message({
  animate,
  avatarActivity = "idle",
  avatarAnimated = false,
  group,
  role,
  text,
  showAvatar,
  reserveAvatarSpace = true,
  variant = "message",
  debugUsage = null,
  onDebug,
  attachment
}: {
  animate: boolean;
  avatarActivity?: IdentityAvatarActivity;
  avatarAnimated?: boolean;
  group?: ChatBubbleGroup;
  role: "user" | "assistant";
  text: string;
  showAvatar: boolean;
  reserveAvatarSpace?: boolean;
  variant?: "message" | "typing";
  debugUsage?: ProviderUsageDebug | null;
  onDebug?: () => void;
  attachment?: ReactNode;
}) {
  const [expanded, setExpanded] = React.useState(false);
  const [overflowing, setOverflowing] = React.useState(false);
  const bubble = (
    <TranscriptChatBubble
      avatarActivity={avatarActivity}
      avatarAnimated={avatarAnimated}
      group={group}
      interactive={variant === "message" && overflowing && !expanded}
      reserveAvatarSpace={reserveAvatarSpace}
      role={role}
      showAvatar={showAvatar}
      variant={variant}
    >
      {variant === "typing" ? (
        <TypingMessageContent />
      ) : (
        <div {...stylex.props(styles.content)}>
          <ExpandableTextBubbleContent onExpandedChange={setExpanded} onOverflowChange={setOverflowing}>
            <MessageMarkdown animate={animate} role={role} text={text} />
          </ExpandableTextBubbleContent>
          {attachment ? <div {...stylex.props(styles.attachment)}>{attachment}</div> : null}
        </div>
      )}
    </TranscriptChatBubble>
  );

  if (role !== "assistant") {
    return bubble;
  }

  return (
    <div {...stylex.props(styles.assistantContextMenu)}>
      <ContextMenu items={[{ label: "Debug", isDisabled: !onDebug && !debugUsage, onClick: onDebug }]}>
        {bubble}
      </ContextMenu>
    </div>
  );
}

function MessageMarkdown({ animate, role, text }: { animate: boolean; role: "user" | "assistant"; text: string }) {
  return (
    <Markdown
      autolink="gfm"
      components={role === "user" ? userMarkdownComponents : undefined}
      contentWidth="100%"
      density="compact"
      headingLevelStart={3}
      isStreaming={animate}
      xstyle={markdownXStyle(styles.markdown, role === "user" && styles.userMarkdown)}
    >
      {text}
    </Markdown>
  );
}

function markdownXStyle(...xstyle: unknown[]): MarkdownXStyle {
  return xstyle as unknown as MarkdownXStyle;
}

function UserBubbleMarkdownLink({ href, children }: { href: string; children: ReactNode }) {
  const isExternal = /^https?:\/\//i.test(href);

  return (
    <a
      href={href}
      {...(isExternal ? { target: "_blank", rel: "noopener noreferrer" } : {})}
      {...stylex.props(styles.userLink)}
    >
      {children}
    </a>
  );
}

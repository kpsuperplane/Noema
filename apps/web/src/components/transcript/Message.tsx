import * as React from "react";
import { ContextMenu } from "@astryxdesign/core/ContextMenu";
import { parseMarkdown } from "@astryxdesign/core/Markdown";
import { VStack } from "@astryxdesign/core/Stack";
import * as stylex from "@stylexjs/stylex";
import type { ReactNode } from "react";
import {
  MarkdownContent,
  type MarkdownComponents
} from "@/components/MarkdownContent";
import { RollingSwap } from "@/components/RollingText";
import type { IdentityAvatarActivity } from "../IdentityAvatar";
import type { ProviderUsageDebug } from "./debugUsage";
import { ExpandableTextBubbleContent } from "./ExpandableTextBubble";
import { TranscriptChatBubble } from "./TranscriptChatBubble";
import type { ChatBubbleGroup } from "./renderModel";
import { TypingMessageContent } from "./TypingMessage";
import {
  ProviderCitationMarkdown,
  type ProviderCitation
} from "./ProviderCitationSources";

const styles = stylex.create({
  markdown: {
    color: "inherit",
    fontFamily: "inherit",
    fontSize: "inherit",
    lineHeight: "inherit"
  },
  userMarkdown: {
    "--noema-markdown-bullet-accent": "currentColor",
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
  progress: {
    color: "var(--color-text-secondary)",
    fontSize: "var(--font-size-sm)",
    opacity: 0.82
  },
  content: {
    display: "grid",
    minWidth: 0
  },
  singleLine: {
    maxHeight: "1.7em",
    minWidth: 0,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap"
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

export function Message(props: React.ComponentProps<typeof MessageBubble>) {
  const paragraphs = React.useMemo(() => {
    // Structured Markdown and cited answers keep their shared document context.
    if (props.role !== "assistant" || props.progress || props.singleLine ||
      props.variant === "typing" || props.citations?.length ||
      parseMarkdown(props.text).some((block) => block.type !== "paragraph")) {
      return [props.text];
    }
    return props.text.split(/\r?\n[ \t]*\r?\n/).map((text) => text.trim()).filter(Boolean);
  }, [props.role, props.progress, props.singleLine, props.variant, props.citations, props.text]);
  if (paragraphs.length <= 1) return <MessageBubble {...props} />;
  return (
    <VStack gap={1}>
      {paragraphs.map((text, index) => {
        const last = index === paragraphs.length - 1;
        const previous = index > 0 || props.group === "middle" || props.group === "last";
        const next = !last || props.group === "first" || props.group === "middle";
        return (
          <MessageBubble
            {...props}
            key={index}
            text={text}
            group={previous ? (next ? "middle" : "last") : "first"}
            showAvatar={last && props.showAvatar}
            attachment={last ? props.attachment : undefined}
          />
        );
      })}
    </VStack>
  );
}

function MessageBubble({
  animate,
  avatarActivity = "idle",
  avatarAnimated = false,
  group,
  role,
  text,
  showAvatar,
  reserveAvatarSpace = true,
  rollingText = false,
  singleLine = false,
  variant = "message",
  debugUsage = null,
  onDebug,
  attachment,
  citations = [],
  progress = false
}: {
  animate: boolean;
  avatarActivity?: IdentityAvatarActivity;
  avatarAnimated?: boolean;
  group?: ChatBubbleGroup;
  role: "user" | "assistant";
  text: string;
  showAvatar: boolean;
  reserveAvatarSpace?: boolean;
  rollingText?: boolean;
  singleLine?: boolean;
  variant?: "message" | "typing";
  debugUsage?: ProviderUsageDebug | null;
  onDebug?: () => void;
  attachment?: ReactNode;
  citations?: readonly ProviderCitation[];
  progress?: boolean;
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
          {singleLine ? (
            <div {...stylex.props(styles.singleLine)}>
              {rollingText ? (
                <RollingSwap transitionKey={text}>
                  <MessageMarkdown animate={animate} role={role} citations={citations} text={text} />
                </RollingSwap>
              ) : (
                <MessageMarkdown animate={animate} role={role} citations={citations} text={text} />
              )}
            </div>
          ) : (
            <ExpandableTextBubbleContent onExpandedChange={setExpanded} onOverflowChange={setOverflowing}>
              <VStack gap={1}>
                <MessageMarkdown animate={animate} role={role} citations={citations} text={text} />
              </VStack>
            </ExpandableTextBubbleContent>
          )}
          {attachment ? <div {...stylex.props(styles.attachment)}>{attachment}</div> : null}
        </div>
      )}
    </TranscriptChatBubble>
  );

  if (role !== "assistant") {
    return bubble;
  }

  return (
    <div
      aria-label={progress ? "Assistant progress" : undefined}
      role={progress ? "group" : undefined}
      {...stylex.props(styles.assistantContextMenu, progress && styles.progress)}
    >
      <ContextMenu items={[{ label: "Debug", isDisabled: !onDebug && !debugUsage, onClick: onDebug }]}>
        {bubble}
      </ContextMenu>
    </div>
  );
}

function MessageMarkdown({
  animate,
  role,
  citations,
  text
}: {
  animate: boolean;
  role: "user" | "assistant";
  citations: readonly ProviderCitation[];
  text: string;
}) {
  return role === "assistant" ? (
    <ProviderCitationMarkdown
      density="compact"
      headingLevelStart={3}
      isStreaming={animate}
      citations={citations}
      sourcesOnOwnLine
      text={text}
      xstyle={styles.markdown}
    />
  ) : (
    <MarkdownContent
      className={stylex.props(styles.userMarkdown).className}
      components={userMarkdownComponents}
      density="compact"
      headingLevelStart={3}
      isStreaming={animate}
      xstyle={styles.markdown}
    >
      {text}
    </MarkdownContent>
  );
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

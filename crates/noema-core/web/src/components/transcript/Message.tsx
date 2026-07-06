import { Markdown, type MarkdownProps } from "@astryxdesign/core/Markdown";
import * as stylex from "@stylexjs/stylex";
import type { ReactNode } from "react";
import { TranscriptChatBubble } from "./TranscriptChatBubble";

type MarkdownXStyle = MarkdownProps["xstyle"];
type MarkdownComponents = NonNullable<MarkdownProps["components"]>;

const styles = stylex.create({
  markdown: {
    color: "inherit",
    fontFamily: "inherit",
    fontSize: "inherit",
    lineHeight: "inherit"
  },
  userLink: {
    color: "currentColor",
    fontWeight: 600,
    textDecoration: "underline",
    textDecorationColor: "color-mix(in srgb, currentColor 76%, transparent)",
    textDecorationThickness: 1,
    textUnderlineOffset: 3
  }
});

const userMarkdownComponents: MarkdownComponents = {
  link: UserBubbleMarkdownLink
};

export function Message({
  animate,
  role,
  text,
  showAvatar
}: {
  animate: boolean;
  role: "user" | "assistant";
  text: string;
  showAvatar: boolean;
}) {
  return (
    <TranscriptChatBubble role={role} showAvatar={showAvatar}>
      <Markdown
        autolink="gfm"
        components={role === "user" ? userMarkdownComponents : undefined}
        contentWidth="100%"
        density="compact"
        headingLevelStart={3}
        isStreaming={animate}
        xstyle={markdownXStyle(styles.markdown)}
      >
        {text}
      </Markdown>
    </TranscriptChatBubble>
  );
}

function markdownXStyle(xstyle: unknown): MarkdownXStyle {
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

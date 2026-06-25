import React from 'react';

/**
 * Props for a single conversation turn.
 * @startingPoint section="Agent" subtitle="Conversation turn (user + agent)" viewport="700x360"
 */
export interface ChatMessageProps extends React.HTMLAttributes<HTMLDivElement> {
  /** `agent` = full-width prose, `user` = soft pine bubble. @default 'agent' */
  role?: 'agent' | 'user';
  name?: React.ReactNode;
  /** Timestamp, rendered in mono. */
  time?: React.ReactNode;
  /** Avatar node (use <Avatar/>). */
  avatar?: React.ReactNode;
  /** Show the animated typing indicator instead of content. */
  typing?: boolean;
  children?: React.ReactNode;
}

/** A single turn in an agent conversation. */
export function ChatMessage(props: ChatMessageProps): JSX.Element;

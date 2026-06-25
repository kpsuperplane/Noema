import React from 'react';

export interface TooltipProps extends React.HTMLAttributes<HTMLSpanElement> {
  /** Tooltip text. */
  label: React.ReactNode;
  /** Optional keyboard shortcut hint shown in mono. */
  kbd?: string;
  side?: 'top' | 'bottom';
  /** The trigger element. */
  children: React.ReactNode;
}

/** Lightweight CSS hover/focus tooltip wrapping a trigger. */
export function Tooltip(props: TooltipProps): JSX.Element;

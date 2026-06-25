import React from 'react';

export type BadgeTone =
  | 'neutral' | 'brand' | 'accent' | 'success' | 'warning' | 'danger' | 'info' | 'solid' | 'outline';

export interface BadgeProps extends React.HTMLAttributes<HTMLSpanElement> {
  tone?: BadgeTone;
  /** Leading status dot. */
  dot?: boolean;
  /** Monospace label — for versions, IDs, statuses. */
  mono?: boolean;
  children?: React.ReactNode;
}

/** Small pill for status, counts and labels. */
export function Badge(props: BadgeProps): JSX.Element;

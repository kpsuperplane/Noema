import React from 'react';

export interface AvatarProps extends React.HTMLAttributes<HTMLSpanElement> {
  /** Display name — used for initials fallback and tooltip. */
  name?: string;
  /** Image URL; falls back to initials when absent. */
  src?: string;
  /** `agent` = pine gradient, `person` = clay. @default 'person' */
  kind?: 'agent' | 'person';
  size?: 'xs' | 'sm' | 'md' | 'lg';
  /** Rounded-square instead of circle (used for agents). */
  square?: boolean;
  /** Decorative focus ring. */
  ring?: boolean;
  /** Show an online status dot. */
  status?: boolean;
}

/** User or agent identity chip — image, initials, or agent mark. */
export function Avatar(props: AvatarProps): JSX.Element;

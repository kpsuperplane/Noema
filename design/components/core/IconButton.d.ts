import React from 'react';

export type IconButtonVariant = 'ghost' | 'soft' | 'outline' | 'primary';
export type IconButtonSize = 'sm' | 'md' | 'lg';

export interface IconButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  /** The icon node (e.g. an SVG or <i data-lucide>). */
  icon?: React.ReactNode;
  variant?: IconButtonVariant;
  size?: IconButtonSize;
  /** Fully circular instead of rounded-square. */
  round?: boolean;
  /** Required for accessibility — describes the action. */
  'aria-label': string;
  children?: React.ReactNode;
}

/** Compact square/circular control holding a single icon. */
export function IconButton(props: IconButtonProps): JSX.Element;

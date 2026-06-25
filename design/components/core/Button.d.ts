import React from 'react';

export type ButtonVariant = 'primary' | 'accent' | 'secondary' | 'ghost' | 'danger';
export type ButtonSize = 'sm' | 'md' | 'lg';

/**
 * Props for the primary interactive button.
 * @startingPoint section="Core" subtitle="All button variants & sizes" viewport="700x300"
 */
export interface ButtonProps extends React.HTMLAttributes<HTMLElement> {
  /** Visual emphasis. `primary` = pine, `accent` = clay, `danger` = destructive. */
  variant?: ButtonVariant;
  /** Control height. @default 'md' */
  size?: ButtonSize;
  /** Stretch to fill the container width. */
  fullWidth?: boolean;
  /** Show a spinner and block interaction. */
  loading?: boolean;
  disabled?: boolean;
  /** Icon node rendered before the label. */
  leftIcon?: React.ReactNode;
  /** Icon node rendered after the label. */
  rightIcon?: React.ReactNode;
  /** Render as a different element/component (e.g. 'a'). Defaults to 'a' if `href` is set. */
  as?: any;
  href?: string;
  children?: React.ReactNode;
}

/** Primary interactive button for Noema interfaces. */
export function Button(props: ButtonProps): JSX.Element;

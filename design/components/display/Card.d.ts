import React from 'react';

export type CardVariant = 'default' | 'flat' | 'raised' | 'sunken' | 'brand';

/**
 * Props for the primary surface container.
 * @startingPoint section="Core" subtitle="Content container variants" viewport="700x280"
 */
export interface CardProps extends React.HTMLAttributes<HTMLElement> {
  /** Inner padding. @default 'md' */
  padding?: 'sm' | 'md' | 'lg';
  variant?: CardVariant;
  /** Adds hover-lift + pointer for clickable cards. */
  interactive?: boolean;
  as?: any;
  children?: React.ReactNode;
}

/** Primary surface container — white, hairline border, soft warm shadow. */
export function Card(props: CardProps): JSX.Element;

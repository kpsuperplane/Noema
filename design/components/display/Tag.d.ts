import React from 'react';

export interface TagProps extends React.HTMLAttributes<HTMLSpanElement> {
  tone?: 'neutral' | 'brand';
  /** Leading icon node. */
  icon?: React.ReactNode;
  mono?: boolean;
  /** Adds hover affordance for filter chips. */
  selectable?: boolean;
  /** Filled selected state. */
  selected?: boolean;
  /** When provided, renders a remove (×) button. */
  onRemove?: (e: React.MouseEvent) => void;
  children?: React.ReactNode;
}

/** Input chip / filter token, optionally removable or selectable. */
export function Tag(props: TagProps): JSX.Element;

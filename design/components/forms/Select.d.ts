import React from 'react';

export interface SelectOption { value: string; label: string; }

export interface SelectProps extends React.SelectHTMLAttributes<HTMLSelectElement> {
  label?: React.ReactNode;
  hint?: React.ReactNode;
  error?: React.ReactNode;
  required?: boolean;
  size?: 'sm' | 'md';
  /** Convenience: pass options instead of <option> children. */
  options?: Array<SelectOption | string>;
}

/** Styled native select with custom chevron. */
export function Select(props: SelectProps): JSX.Element;

import React from 'react';

export interface CheckboxProps extends React.InputHTMLAttributes<HTMLInputElement> {
  label?: React.ReactNode;
  /** Secondary description shown under the label. */
  description?: React.ReactNode;
}

/** Custom-styled checkbox with optional label and description. */
export function Checkbox(props: CheckboxProps): JSX.Element;

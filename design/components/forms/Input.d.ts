import React from 'react';

export interface InputProps extends React.InputHTMLAttributes<HTMLInputElement> {
  /** Field label rendered above the control. */
  label?: React.ReactNode;
  /** Helper text below the field. */
  hint?: React.ReactNode;
  /** Error message — turns the field red and overrides `hint`. */
  error?: React.ReactNode;
  required?: boolean;
  size?: 'sm' | 'md' | 'lg';
  /** Icon node inside the field, leading edge. */
  leftIcon?: React.ReactNode;
  /** Icon node inside the field, trailing edge. */
  rightIcon?: React.ReactNode;
}

/** Single-line text field with label, hint, error and optional inline icons. */
export function Input(props: InputProps): JSX.Element;

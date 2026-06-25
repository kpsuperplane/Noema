import React from 'react';

export interface TextareaProps extends React.TextareaHTMLAttributes<HTMLTextAreaElement> {
  label?: React.ReactNode;
  hint?: React.ReactNode;
  error?: React.ReactNode;
  required?: boolean;
  /** Render in monospace — for prompts, system messages, JSON. */
  mono?: boolean;
}

/** Multi-line text field. Set `mono` for prompt/code entry. */
export function Textarea(props: TextareaProps): JSX.Element;

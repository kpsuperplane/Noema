import React from 'react';

export interface DialogProps {
  open?: boolean;
  onClose?: () => void;
  title?: React.ReactNode;
  description?: React.ReactNode;
  size?: 'sm' | 'md' | 'lg';
  /** Optional leading icon in the header. */
  icon?: React.ReactNode;
  /** Footer content — typically action Buttons. */
  footer?: React.ReactNode;
  children?: React.ReactNode;
  className?: string;
}

/** Centered modal dialog with scrim, header, body and footer slots. */
export function Dialog(props: DialogProps): JSX.Element | null;

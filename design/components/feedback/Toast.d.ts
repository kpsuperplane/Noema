import React from 'react';

export interface ToastProps extends React.HTMLAttributes<HTMLDivElement> {
  title?: React.ReactNode;
  message?: React.ReactNode;
  tone?: 'success' | 'warning' | 'danger' | 'info';
  onClose?: () => void;
}

/** Transient notification card. Place inside a fixed `.noema-toast__stack` container (bottom-right). */
export function Toast(props: ToastProps): JSX.Element;

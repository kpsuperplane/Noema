import React from 'react';

export interface SwitchProps extends React.InputHTMLAttributes<HTMLInputElement> {
  label?: React.ReactNode;
}

/** On/off toggle for settings and feature flags. */
export function Switch(props: SwitchProps): JSX.Element;

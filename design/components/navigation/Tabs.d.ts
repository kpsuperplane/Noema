import React from 'react';

export interface TabItem {
  value: string;
  label: React.ReactNode;
  /** Optional count shown after the label (e.g. unread). */
  count?: number;
}

export interface TabsProps {
  items: TabItem[];
  /** Controlled active value. */
  value?: string;
  /** Initial value when uncontrolled. */
  defaultValue?: string;
  onChange?: (value: string) => void;
  variant?: 'underline' | 'segmented';
  className?: string;
}

/** Tab bar in `underline` (page nav) or `segmented` (compact switch) styles. */
export function Tabs(props: TabsProps): JSX.Element;

import React from 'react';

export type ToolStatus = 'running' | 'success' | 'error' | 'pending';

/**
 * Props for the tool-invocation record.
 * @startingPoint section="Agent" subtitle="Expandable tool invocation record" viewport="700x240"
 */
export interface ToolCallProps extends React.HTMLAttributes<HTMLDivElement> {
  /** Tool name, e.g. "web_search". */
  tool: string;
  status?: ToolStatus;
  /** Invocation arguments — object (pretty-printed) or string. */
  args?: any;
  /** Tool result — object or string. */
  result?: any;
  /** Expand the args/result on mount. */
  defaultOpen?: boolean;
  /** Custom leading glyph. */
  icon?: React.ReactNode;
}

/** Collapsible record of an agent calling a tool — Noema's "show your work" primitive. */
export function ToolCall(props: ToolCallProps): JSX.Element;

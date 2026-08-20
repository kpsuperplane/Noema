import React from "react";

export type RenderErrorFallbackProps = {
  error: Error;
  retry: () => void;
};

type RenderErrorBoundaryProps = {
  children: React.ReactNode;
  errorScope: string;
  fallback: (props: RenderErrorFallbackProps) => React.ReactNode;
  resetKey?: unknown;
};

type RenderErrorBoundaryState = {
  error: Error | null;
  resetKey: unknown;
};

export class RenderErrorBoundary extends React.Component<
  RenderErrorBoundaryProps,
  RenderErrorBoundaryState
> {
  state: RenderErrorBoundaryState;

  constructor(props: RenderErrorBoundaryProps) {
    super(props);
    this.state = { error: null, resetKey: props.resetKey };
  }

  static getDerivedStateFromProps(
    props: RenderErrorBoundaryProps,
    state: RenderErrorBoundaryState
  ): RenderErrorBoundaryState | null {
    return Object.is(props.resetKey, state.resetKey)
      ? null
      : { error: null, resetKey: props.resetKey };
  }

  static getDerivedStateFromError(
    error: unknown
  ): Pick<RenderErrorBoundaryState, "error"> {
    return {
      error: error instanceof Error ? error : new Error("Noema could not display this content.")
    };
  }

  render() {
    if (this.state.error) {
      return this.props.fallback({
        error: this.state.error,
        retry: () => this.setState({ error: null })
      });
    }

    return this.props.children;
  }
}

type ReactErrorInfo = {
  componentStack?: string;
  errorBoundary?: React.Component<unknown>;
};

export function reportCaughtReactError(error: unknown, info: ReactErrorInfo) {
  const props = info.errorBoundary?.props;
  const errorScope = isRecord(props) && typeof props.errorScope === "string"
    ? props.errorScope
    : "react.caught";
  reportReactError(errorScope, error, info.componentStack);
}

export function reportReactError(
  errorScope: string,
  error: unknown,
  componentStack?: string
) {
  console.error(`[Noema render error: ${errorScope}]`, error, componentStack ?? "");
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

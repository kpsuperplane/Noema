import React from "react";
import * as stylex from "@stylexjs/stylex";
import { ErrorMarker } from "@/components/ErrorMarker";

export function AppBootBoundary({ children }: { children: React.ReactNode }) {
  return (
    <AppBootErrorBoundary>
      <React.Suspense fallback={<AppBootSkeleton />}>
        {children}
      </React.Suspense>
    </AppBootErrorBoundary>
  );
}

type AppBootErrorBoundaryState = {
  error: Error | null;
};

class AppBootErrorBoundary extends React.Component<
  { children: React.ReactNode },
  AppBootErrorBoundaryState
> {
  state: AppBootErrorBoundaryState = {
    error: null
  };

  static getDerivedStateFromError(error: unknown): AppBootErrorBoundaryState {
    return {
      error: error instanceof Error ? error : new Error("Noema could not load.")
    };
  }

  render() {
    if (this.state.error) {
      return (
        <main {...stylex.props(styles.root)} aria-label="Noema status">
          <div {...stylex.props(styles.errorFrame)}>
            <img src="/assets/noema-mark.svg" width="36" height="36" alt="" />
            <ErrorMarker message={this.state.error.message} />
          </div>
        </main>
      );
    }

    return this.props.children;
  }
}

function AppBootSkeleton() {
  return (
    <main {...stylex.props(styles.root)} aria-label="Loading Noema">
      <div {...stylex.props(styles.bootFrame)}>
        <img src="/assets/noema-mark.svg" width="36" height="36" alt="" />
        <div {...stylex.props(styles.bootContent)}>
          <div data-slot="skeleton-glimmer" {...stylex.props(styles.bootLine, styles.bootLineShort)} />
          <div data-slot="skeleton-glimmer" {...stylex.props(styles.bootTitle)} />
          <div data-slot="skeleton-glimmer" {...stylex.props(styles.bootLine)} />
          <div data-slot="skeleton-glimmer" {...stylex.props(styles.bootLine, styles.bootLineMedium)} />
        </div>
      </div>
    </main>
  );
}

const styles = stylex.create({
  root: {
    display: "grid",
    minHeight: "100dvh",
    placeItems: "center",
    backgroundColor: "var(--background)",
    padding: 24
  },
  bootFrame: {
    display: "grid",
    width: "min(520px, 100%)",
    justifyItems: "center",
    gap: 22
  },
  bootContent: {
    display: "grid",
    width: "100%",
    justifyItems: "center",
    gap: 12
  },
  bootLine: {
    width: "min(420px, 82%)",
    height: 14,
    borderRadius: 7,
    backgroundColor: "var(--color-skeleton)"
  },
  bootLineShort: {
    width: 108
  },
  bootLineMedium: {
    width: "min(300px, 62%)"
  },
  bootTitle: {
    width: "min(360px, 74%)",
    height: 34,
    borderRadius: 8,
    backgroundColor: "var(--color-skeleton)"
  },
  errorFrame: {
    display: "grid",
    width: "min(520px, 100%)",
    justifyItems: "center",
    gap: 18
  }
});

import type { ReactNode } from "react";
import { VStack } from "@astryxdesign/core/Stack";
import * as stylex from "@stylexjs/stylex";
import { isTauriRuntime } from "@/graphql/transportMode";

export function SetupFrame({ children }: { children: ReactNode }) {
  const desktop = isTauriRuntime();

  return (
    <VStack as="main" height="100dvh" {...stylex.props(styles.root)}>
      {desktop ? (
        <header aria-hidden="true" data-tauri-drag-region {...stylex.props(styles.dragRegion)} />
      ) : null}
      <VStack as="div" {...stylex.props(styles.body, desktop && styles.desktopBody)}>
        {children}
      </VStack>
    </VStack>
  );
}

const styles = stylex.create({
  root: {
    position: "relative",
    overflow: "hidden",
    backgroundColor: "var(--background)",
    "::after": {
      content: "''",
      position: "absolute",
      right: 0,
      bottom: 0,
      left: 0,
      zIndex: 2,
      height: "var(--spacing-12)",
      pointerEvents: "none",
      backgroundImage:
        "linear-gradient(to bottom, rgb(255 255 255 / 0), rgb(255 255 255 / 0.38) 50%, var(--background) 100%)"
    }
  },
  dragRegion: {
    position: "absolute",
    top: 0,
    right: 0,
    left: 0,
    zIndex: 3,
    height: "calc(var(--spacing-12) + var(--spacing-1))",
    backgroundImage:
      "linear-gradient(to bottom, var(--background) 0%, 80%, rgb(255 255 255 / 0) 100%)"
  },
  body: {
    flex: 1,
    minHeight: 0,
    boxSizing: "border-box",
    overflow: "auto"
  },
  desktopBody: {
    paddingBlockStart: "calc(var(--spacing-12) + var(--spacing-1))"
  }
});

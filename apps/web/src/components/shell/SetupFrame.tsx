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
      <VStack as="div" {...stylex.props(styles.body)}>{children}</VStack>
    </VStack>
  );
}

const styles = stylex.create({
  root: {
    overflow: "hidden",
    backgroundColor: "var(--background)"
  },
  dragRegion: {
    position: "relative",
    zIndex: 1,
    width: "100%",
    height: 52,
    boxSizing: "border-box",
    flexShrink: 0,
    borderBlockEndWidth: 1,
    borderBlockEndStyle: "solid",
    borderBlockEndColor: "var(--border-subtle)",
    backgroundColor: "var(--background)"
  },
  body: {
    flex: 1,
    minHeight: 0,
    overflow: "auto"
  }
});

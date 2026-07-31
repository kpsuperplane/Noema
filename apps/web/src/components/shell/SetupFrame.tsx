import type { ReactNode } from "react";
import { VStack } from "@astryxdesign/core/Stack";
import * as stylex from "@stylexjs/stylex";

export function SetupFrame({ children }: { children: ReactNode }) {
  return (
    <VStack as="main" height="100dvh" {...stylex.props(styles.root)}>
      <VStack as="div" {...stylex.props(styles.body)}>{children}</VStack>
    </VStack>
  );
}

const styles = stylex.create({
  root: {
    overflow: "hidden",
    backgroundColor: "var(--background)"
  },
  body: {
    flex: 1,
    minHeight: 0,
    overflow: "auto"
  }
});

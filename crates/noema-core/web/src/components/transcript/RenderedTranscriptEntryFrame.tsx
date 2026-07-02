import * as React from "react";
import * as stylex from "@stylexjs/stylex";

const styles = stylex.create({
  content: {
    width: "100%",
    maxWidth: 760,
    minWidth: 0
  },
  inner: {
    minHeight: 0
  }
});

export function RenderedTranscriptEntryFrame({
  animateArrival,
  children
}: {
  animateArrival: boolean;
  children: React.ReactNode;
}) {
  if (!animateArrival) {
    return <>{children}</>;
  }

  return (
    <div {...stylex.props(styles.content)} data-slot="message-arrival-content">
      <div {...stylex.props(styles.inner)} data-slot="message-arrival-inner">
        {children}
      </div>
    </div>
  );
}

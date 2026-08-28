import * as React from "react";
import { TextArea } from "@astryxdesign/core/TextArea";
import { VStack } from "@astryxdesign/core/VStack";
import * as stylex from "@stylexjs/stylex";
import { RenderErrorBoundary } from "@/components/errors/RenderErrorBoundary";

export type MarkdownEditorProps = {
  value: string; onChange: (value: string) => void;
  label?: string; density?: "default" | "inline"; sourceMode?: boolean;
  onSourceModeChange?: (sourceMode: boolean) => void; onReady?: () => void;
};

const MarkdownEditorImpl = React.lazy(() => import("./MarkdownEditorImpl"));

export function MarkdownEditor(props: MarkdownEditorProps) {
  return (
    <RenderErrorBoundary
      errorScope="markdown.editor"
      fallback={() => <MarkdownEditorFallback {...props} />}
    >
      <React.Suspense fallback={<p role="status">Loading Markdown editor…</p>}>
        <MarkdownEditorImpl {...props} />
      </React.Suspense>
    </RenderErrorBoundary>
  );
}

function MarkdownEditorFallback({
  value,
  onChange,
  onReady,
  label = "Markdown document",
  density = "default"
}: MarkdownEditorProps) {
  React.useLayoutEffect(() => onReady?.(), [onReady]);
  return (
    <VStack gap={2}>
      <p role="status" {...stylex.props(styles.notice)}>
        Rich editing is unavailable. Markdown source remains editable.
      </p>
      <TextArea
        isLabelHidden
        label={`${label} Markdown source`}
        rows={density === "inline" ? 6 : 14}
        value={value}
        width="100%"
        onChange={onChange}
      />
    </VStack>
  );
}

const styles = stylex.create({
  notice: { margin: "var(--spacing-0)", color: "var(--muted-foreground)", fontSize: 12, lineHeight: 1.4 }
});

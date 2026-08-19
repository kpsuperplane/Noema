import * as React from "react";

export type TaskMarkdownEditorProps = {
  value: string;
  onChange: (value: string) => void;
  label?: string;
  density?: "default" | "inline";
  sourceMode?: boolean;
  onSourceModeChange?: (sourceMode: boolean) => void;
};

const TaskMarkdownEditorImpl = React.lazy(() => import("./TaskMarkdownEditorImpl"));

export function TaskMarkdownEditor(props: TaskMarkdownEditorProps) {
  return (
    <React.Suspense fallback={<p role="status">Loading Markdown editor…</p>}>
      <TaskMarkdownEditorImpl {...props} />
    </React.Suspense>
  );
}

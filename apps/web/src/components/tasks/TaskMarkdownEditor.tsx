import * as React from "react";
import { HStack } from "@astryxdesign/core/HStack";
import { IconButton } from "@astryxdesign/core/IconButton";
import { TextArea } from "@astryxdesign/core/TextArea";
import { VStack } from "@astryxdesign/core/VStack";
import * as stylex from "@stylexjs/stylex";
import { Check, Code2, RefreshCcw, X } from "lucide-react";
import { MarkdownContent } from "@/components/MarkdownContent";
import { RenderErrorBoundary } from "@/components/errors/RenderErrorBoundary";

export type TaskMarkdownEditorProps = {
  value: string;
  onChange: (value: string) => void;
  label?: string;
  density?: "default" | "inline";
  sourceMode?: boolean;
  onSourceModeChange?: (sourceMode: boolean) => void;
  onReady?: () => void;
};

const TaskMarkdownEditorImpl = React.lazy(() => import("./TaskMarkdownEditorImpl"));

export function TaskMarkdownEditor(props: TaskMarkdownEditorProps) {
  return (
    <RenderErrorBoundary
      errorScope="task.markdown_editor"
      fallback={() => <TaskMarkdownEditorFallback {...props} />}
    >
      <React.Suspense fallback={<p role="status">Loading Markdown editor…</p>}>
        <TaskMarkdownEditorImpl {...props} />
      </React.Suspense>
    </RenderErrorBoundary>
  );
}

function TaskMarkdownEditorFallback({
  value,
  onChange,
  onReady,
  label = "Task document",
  density = "default"
}: TaskMarkdownEditorProps) {
  React.useLayoutEffect(() => onReady?.(), [onReady]);
  return (
    <VStack gap={2}>
      <p role="status" {...stylex.props(styles.editorNotice)}>
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

type TaskDocumentEditController = {
  busy: boolean; error: string | null;
  requiresAcknowledgement: boolean; actionUnavailable: boolean;
  cancel: () => void; acknowledge: () => Promise<void>;
};

export function TaskDocumentInlineEditor({ document, digest, edit, label, scope, className, onSave }: {
  document: string; digest?: string; edit: TaskDocumentEditController;
  label: string; scope?: string; className?: string;
  onSave: (document: string) => Promise<void>;
}) {
  const [draft, setDraft] = React.useState(document);
  const [sourceMode, setSourceMode] = React.useState(false);
  const [editorReady, setEditorReady] = React.useState(false);
  const saveLabel = scope ? "Save description for future occurrences" : "Save description";
  return (
    <VStack as="section" aria-labelledby="task-document-editor-title" gap={2} className={className}>
      <HStack justify="between" align="center" gap={2} className={stylex.props(styles.bar).className}>
        <HStack align="center" gap={1}>
          <strong id="task-document-editor-title" {...stylex.props(styles.label)}>Description</strong>
          {scope ? <span {...stylex.props(styles.scope)}>{scope}</span> : null}
        </HStack>
        <span {...stylex.props(styles.controls)}>
          <IconButton type="button" size="sm" variant="ghost" label={sourceMode ? "Use rich editor" : "Edit source"} tooltip={sourceMode ? "Use rich editor" : "Edit source"} icon={<Code2 aria-hidden="true" size={14} />} isDisabled={edit.busy} onClick={() => setSourceMode((current) => !current)} />
          {edit.requiresAcknowledgement ? <IconButton type="button" size="sm" variant="ghost" label="Reload latest" tooltip="Reload latest" icon={<RefreshCcw aria-hidden="true" size={14} />} isDisabled={edit.busy} onClick={() => void edit.acknowledge().catch(() => undefined)} /> : <IconButton type="button" size="sm" variant="ghost" label={saveLabel} tooltip={scope ? "Save for future occurrences" : saveLabel} icon={<Check aria-hidden="true" size={14} />} isLoading={edit.busy} isDisabled={edit.actionUnavailable} onClick={() => void onSave(draft).catch(() => undefined)} />}
          <IconButton type="button" size="sm" variant="ghost" label="Cancel description edit" tooltip="Cancel" icon={<X aria-hidden="true" size={14} />} isDisabled={edit.busy} onClick={edit.cancel} />
        </span>
      </HStack>
      <VStack className={stylex.props(styles.editor, Boolean(edit.error || edit.actionUnavailable) && styles.editorError).className}>
        <div {...stylex.props(styles.editorFrame)}>
          {!editorReady ? <MarkdownContent density="compact" className={stylex.props(styles.preview).className}>{draft}</MarkdownContent> : null}
          <div {...stylex.props(!editorReady && styles.editorLoading)}>
            <TaskMarkdownEditor key={digest} value={draft} onChange={setDraft} label={label} density="inline" sourceMode={sourceMode} onSourceModeChange={setSourceMode} onReady={() => setEditorReady(true)} />
          </div>
        </div>
      </VStack>
      {edit.error ? <span role="alert" {...stylex.props(styles.error)}>{edit.error}</span> : null}
    </VStack>
  );
}

const styles = stylex.create({
  bar: { minHeight: 36 },
  label: { color: "var(--noema-text-muted)", fontSize: 12, fontWeight: 650 },
  scope: { color: "var(--noema-text-muted)", fontSize: 11 },
  controls: { display: "inline-flex", width: 96, minHeight: 28, alignItems: "center", justifyContent: "flex-end", gap: "var(--spacing-1)" },
  editor: { display: "grid", gap: "var(--spacing-2)", borderRadius: "var(--radius-element)" },
  editorFrame: { position: "relative", minHeight: "var(--spacing-5)" },
  editorLoading: { position: "absolute", inset: 0, visibility: "hidden", pointerEvents: "none" },
  preview: { paddingInline: "var(--spacing-2)" },
  editorError: { outlineWidth: 1, outlineStyle: "solid", outlineColor: "var(--destructive)" },
  error: { color: "var(--destructive)", fontSize: 12, lineHeight: 1.4 },
  editorNotice: {
    margin: "var(--spacing-0)",
    color: "var(--muted-foreground)",
    fontSize: 12,
    lineHeight: 1.4
  }
});

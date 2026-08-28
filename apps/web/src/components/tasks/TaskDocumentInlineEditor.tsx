import * as React from "react";
import { HStack } from "@astryxdesign/core/HStack";
import { IconButton } from "@astryxdesign/core/IconButton";
import { VStack } from "@astryxdesign/core/VStack";
import * as stylex from "@stylexjs/stylex";
import { Check, Code2, RefreshCcw, X } from "lucide-react";
import { MarkdownInlineEditor } from "@/components/MarkdownEditor";

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
        <MarkdownInlineEditor key={digest} value={draft} onChange={setDraft} label={label} sourceMode={sourceMode} onSourceModeChange={setSourceMode} />
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
  editorError: { outlineWidth: 1, outlineStyle: "solid", outlineColor: "var(--destructive)" },
  error: { color: "var(--destructive)", fontSize: 12, lineHeight: 1.4 }
});

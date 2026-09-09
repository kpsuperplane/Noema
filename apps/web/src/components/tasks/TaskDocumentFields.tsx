import * as React from "react";
import { Button } from "@astryxdesign/core/Button";
import { HStack } from "@astryxdesign/core/HStack";
import { VStack } from "@astryxdesign/core/VStack";
import { useBlocker } from "@tanstack/react-router";
import * as stylex from "@stylexjs/stylex";
import { MarkdownInlineEditor } from "@/components/MarkdownEditor";
import { MarkdownContent } from "@/components/MarkdownContent";

export type TaskFieldEdit = {
  flush?: () => Promise<boolean>;
  registerFlush?: (flush: (() => Promise<boolean>) | null) => void;
  field: "TITLE" | "DOCUMENT" | null;
  canEdit: boolean;
  canStart: boolean;
  busy: boolean;
  error: string | null;
  requiresAcknowledgement: boolean;
  actionUnavailable: boolean;
  start: (field: "TITLE" | "DOCUMENT") => Promise<void>;
  cancel: () => void;
  acknowledge: () => Promise<void>;
  saveTitle: (value: string) => Promise<void>;
  saveDocument: (value: string) => Promise<void>;
};

export function useTaskEditFlush() {
  const pending = React.useRef<(() => Promise<boolean>) | null>(null);
  const flush = React.useCallback(() => pending.current?.() ?? Promise.resolve(true), []);
  const registerFlush = React.useCallback((value: (() => Promise<boolean>) | null) => { pending.current = value; }, []);
  return { flush, registerFlush };
}

// Capture and existing tasks use the same chromeless title field.
export const TaskTitleField = React.forwardRef<HTMLTextAreaElement, React.TextareaHTMLAttributes<HTMLTextAreaElement>>(
  function TaskTitleField(props, ref) {
    const inputRef = React.useRef<HTMLTextAreaElement>(null);
    React.useImperativeHandle(ref, () => inputRef.current!, []);
    React.useLayoutEffect(() => {
      const input = inputRef.current;
      if (!input) return;
      const resize = () => {
        input.style.height = "0px";
        input.style.height = `${input.scrollHeight}px`;
      };
      resize();
      const observer = new ResizeObserver(resize);
      observer.observe(input);
      return () => observer.disconnect();
    }, [props.value]);
    return <textarea ref={inputRef} rows={1} aria-label="Task title" autoComplete="off" placeholder="What needs to be done?" {...props} {...stylex.props(styles.title)} />;
  }
);

function useTaskField(value: string, field: "TITLE" | "DOCUMENT", edit?: TaskFieldEdit, readValueRef?: React.RefObject<(() => string) | null>) {
  const [draft, setDraft] = React.useState(value);
  const [failed, setFailed] = React.useState(false);
  const [saved, setSaved] = React.useState(false);
  const [localError, setLocalError] = React.useState<string | null>(null);
  const dirty = React.useRef(false);
  const baseline = React.useRef<string | null>(null);
  const saving = React.useRef<Promise<boolean> | null>(null);
  React.useEffect(() => {
    if (!dirty.current) setDraft(value);
  }, [value]);
  const change = (next: string) => { dirty.current = true; setSaved(false); setLocalError(null); setDraft(next); };
  const begin = () => {
    if (edit?.canEdit && !edit.busy && edit.field !== field) {
      baseline.current = readValueRef?.current?.() ?? draft;
      void edit.start(field).catch(() => setLocalError("Task editing could not be opened. Try again."));
    }
  };
  const finish = React.useCallback(async (next = readValueRef?.current?.() ?? draft): Promise<boolean> => {
    if (saving.current) return saving.current;
    if (!edit || edit.field !== field) return true;
    if (next === value || !dirty.current && next === baseline.current) { edit.cancel(); return true; }
    dirty.current = true;
    setDraft(next);
    if (field === "TITLE" && !next.trim()) { setLocalError("Enter a task title."); return false; }
    if (edit.busy || edit.requiresAcknowledgement || edit.actionUnavailable) return false;
    const persist = field === "TITLE" ? edit.saveTitle : edit.saveDocument;
    saving.current = persist(field === "TITLE" ? next.trim() : next).then(() => {
      dirty.current = false; setSaved(true); return true;
    }).catch(() => { setFailed(true); return false; }).finally(() => { saving.current = null; });
    return saving.current;
  }, [draft, edit, field, readValueRef, value]);
  React.useEffect(() => {
    if (edit?.field !== field || !edit.registerFlush) return;
    edit.registerFlush(finish);
    return () => edit.registerFlush?.(null);
  }, [edit, field, finish]);
  useBlocker({ shouldBlockFn: async () => !(await finish()), enableBeforeUnload: () => dirty.current });
  const error = localError ?? (edit?.field === field ? edit.actionUnavailable ? "This task is no longer editable. Your draft is still here." : edit.error : null);
  const discard = () => { dirty.current = false; setDraft(value); setFailed(false); setLocalError(null); edit?.cancel(); };
  const discardButton = <Button size="sm" variant="ghost" label="Discard draft" onClick={discard} />;
  const feedback = edit?.field === field && edit.requiresAcknowledgement
    ? <HStack gap={2} wrap="wrap"><span role="alert">Changed elsewhere. Your draft is safe.</span><Button size="sm" variant="secondary" label="Use latest version" onClick={() => void edit.acknowledge().then(() => setFailed(true)).catch(() => undefined)} />{discardButton}</HStack>
    : error ? <HStack gap={2} wrap="wrap"><span role="alert">{error}</span>{!localError && !edit?.actionUnavailable ? <Button size="sm" variant="secondary" label="Retry" onClick={() => void finish()} /> : null}{discardButton}</HStack>
    : failed && edit?.field === field ? <Button size="sm" variant="secondary" label="Retry" onClick={() => void finish()} /> : <span role="status">{edit?.field === field && edit.busy ? "Saving…" : saved ? "Saved" : null}</span>;
  return { draft, change, begin, finish, feedback, error };
}

export function EditableTaskTitle({ title, edit }: { title: string; edit?: TaskFieldEdit }) {
  const field = useTaskField(title, "TITLE", edit);
  return <VStack gap={0} width="100%">
    <TaskTitleField value={field.draft} readOnly={!edit?.canEdit} disabled={edit?.busy} aria-invalid={Boolean(field.error)} onFocus={field.begin} onChange={(event) => field.change(event.currentTarget.value)} onBlur={() => { void field.finish(); }} onKeyDown={(event) => { if (event.key === "Enter" && !event.nativeEvent.isComposing) { event.preventDefault(); event.currentTarget.blur(); } }} />
    <section {...stylex.props(styles.feedback)}>{field.feedback}</section>
  </VStack>;
}

export function TaskInstructionsField({ value, edit, scope }: { value: string; edit?: TaskFieldEdit; scope?: string }) {
  const readValueRef = React.useRef<(() => string) | null>(null);
  const field = useTaskField(value, "DOCUMENT", edit, readValueRef);
  const [sourceMode, setSourceMode] = React.useState(false);
  const root = React.useRef<HTMLElement>(null);
  const read = () => readValueRef.current?.() ?? field.draft;
  const changeSource = (next: boolean) => { field.change(read()); setSourceMode(next); };
  return <VStack ref={root} as="section" gap={2} aria-label="Task instructions" onFocus={field.begin} onBlur={(event) => {
    if (event.currentTarget.contains(event.relatedTarget as Node | null)) return;
    void field.finish(read());
  }}>
    {edit && (edit.canEdit || edit.field === "DOCUMENT") ? <section inert={edit.busy || !edit.canEdit} {...stylex.props(styles.instructions)}>
      <MarkdownInlineEditor readValueRef={readValueRef} value={field.draft} onChange={field.change} label="Task instructions" placeholder="Add details or instructions…" sourceMode={sourceMode} onSourceModeChange={changeSource} />
    </section> : <MarkdownContent density="compact">{value || "No instructions added."}</MarkdownContent>}
    <HStack justify="between" align="center" gap={2} wrap="wrap">
      <VStack gap={1} className={stylex.props(styles.feedback).className}>{scope ? <span>{scope}</span> : null}{field.feedback}</VStack>
      {edit?.canEdit && edit.field === "DOCUMENT" ? <Button size="sm" variant="ghost" label={sourceMode ? "Use rich editor" : "Markdown source"} isDisabled={edit.busy} onClick={() => changeSource(!sourceMode)} /> : null}
    </HStack>
  </VStack>;
}

const styles = stylex.create({
  title: { width: "100%", minWidth: 0, resize: "none", overflow: "hidden", lineHeight: "var(--text-heading-3-leading)", padding: "var(--spacing-0)", borderWidth: 0, backgroundColor: "transparent", color: "var(--noema-text-primary)", fontFamily: "var(--font-family-heading)", fontSize: "var(--text-heading-3-size)", fontWeight: "var(--text-heading-3-weight)", outline: "none", ":focus-visible": { boxShadow: "0 1px var(--ring)" }, "::placeholder": { color: "var(--muted-foreground)" } },
  instructions: { width: "100%", minWidth: 0 },
  feedback: { color: "var(--noema-text-secondary)", fontSize: "var(--text-supporting-size)", textWrap: "pretty", ":empty": { display: "none" } }
});

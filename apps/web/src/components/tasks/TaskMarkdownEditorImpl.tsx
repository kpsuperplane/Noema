import * as React from "react";
import { Button } from "@astryxdesign/core/Button";
import { HStack } from "@astryxdesign/core/HStack";
import { TextArea } from "@astryxdesign/core/TextArea";
import { Crepe } from "@milkdown/crepe";
import "@milkdown/crepe/theme/common/style.css";
import "@milkdown/crepe/theme/frame.css";
import * as stylex from "@stylexjs/stylex";
import type { TaskMarkdownEditorProps } from "./TaskMarkdownEditor";

export default function TaskMarkdownEditorImpl({ value, onChange, label = "Task document", density = "default" }: TaskMarkdownEditorProps) {
  const [sourceMode, setSourceMode] = React.useState(false);
  const [richFailed, setRichFailed] = React.useState(false);
  return (
    <section data-slot="task-markdown-editor" data-density={density} aria-label={label} {...stylex.props(styles.root, density === "inline" && styles.inlineRoot)}>
      <HStack justify="between" align="center" gap={2} className={stylex.props(styles.modeBar, density === "inline" && styles.inlineModeBar).className}>
        <span {...stylex.props(styles.modeLabel)}>{sourceMode ? "Markdown source" : "Rich text"}</span>
        <Button
          type="button"
          size="sm"
          variant="ghost"
          label={sourceMode ? "Use rich editor" : "Edit source"}
          isDisabled={richFailed && sourceMode}
          onClick={() => setSourceMode((current) => !current)}
        />
      </HStack>
      {sourceMode ? (
        <TextArea
          isLabelHidden
          label={`${label} Markdown source`}
          rows={14}
          value={value}
          width="100%"
          className={stylex.props(styles.source, density === "inline" && styles.inlineSource).className}
          onChange={onChange}
        />
      ) : (
        <MilkdownCrepe
          initialValue={value}
          inline={density === "inline"}
          onChange={onChange}
          onFailure={() => {
            setRichFailed(true);
            setSourceMode(true);
          }}
        />
      )}
      {richFailed ? <p role="status" {...stylex.props(styles.notice)}>Rich editing is unavailable for this Markdown. Source mode keeps the original text.</p> : null}
    </section>
  );
}

function MilkdownCrepe({ initialValue, inline, onChange, onFailure }: { initialValue: string; inline: boolean; onChange: (value: string) => void; onFailure: () => void }) {
  const root = React.useRef<HTMLDivElement>(null);
  const initialValueRef = React.useRef(initialValue);
  const onChangeRef = React.useRef(onChange);
  const onFailureRef = React.useRef(onFailure);
  React.useEffect(() => { onChangeRef.current = onChange; }, [onChange]);
  React.useEffect(() => { onFailureRef.current = onFailure; }, [onFailure]);
  React.useEffect(() => {
    if (!root.current) return;
    const crepe = new Crepe({
      root: root.current,
      defaultValue: initialValueRef.current,
      features: {
        [Crepe.Feature.AI]: false,
        [Crepe.Feature.ImageBlock]: false,
        [Crepe.Feature.Latex]: false,
        [Crepe.Feature.TopBar]: true
      }
    });
    crepe.on((listener) => {
      listener.markdownUpdated((_context, markdown, previousMarkdown) => {
        if (markdown !== previousMarkdown) onChangeRef.current(markdown);
      });
    });
    let active = true;
    void crepe.create().catch(() => { if (active) onFailureRef.current(); });
    return () => {
      active = false;
      void crepe.destroy().catch(() => undefined);
    };
  }, []);
  return <div ref={root} {...stylex.props(styles.editor, inline && styles.inlineEditor)} />;
}

const styles = stylex.create({
  root: { minWidth: 0, minHeight: 320, borderWidth: 1, borderStyle: "solid", borderColor: "var(--border)", borderRadius: "var(--radius-element)", overflow: "hidden", backgroundColor: "var(--background)" },
  inlineRoot: { minHeight: 0 },
  modeBar: { minHeight: 38, borderBottomWidth: 1, borderBottomStyle: "solid", borderBottomColor: "var(--border)", paddingInline: "var(--spacing-2)" },
  inlineModeBar: { minHeight: 32 },
  modeLabel: { color: "var(--muted-foreground)", fontSize: 12, fontWeight: 600 },
  editor: { minHeight: 280, color: "var(--foreground)", fontFamily: "var(--font-family-body)", "--crepe-base-font-size": "var(--text-body-size)", "--crepe-font-default": "var(--font-family-body)", "--crepe-font-title": "var(--font-family-heading)", "--crepe-font-code": "var(--font-family-code)", "--crepe-color-background": "var(--background)", "--crepe-color-on-background": "var(--foreground)", "--crepe-color-surface": "var(--card)", "--crepe-color-on-surface": "var(--card-foreground)", "--crepe-color-outline": "var(--border)", "--crepe-color-primary": "var(--primary)", "--crepe-color-on-primary": "var(--primary-foreground)" },
  inlineEditor: { minHeight: 96 },
  source: { minHeight: 280, borderWidth: 0, borderRadius: 0, fontFamily: "var(--noema-font-mono)", fontSize: 13, lineHeight: 1.55, resize: "vertical" },
  inlineSource: { minHeight: 96 },
  notice: { margin: 0, borderTopWidth: 1, borderTopStyle: "solid", borderTopColor: "var(--border)", padding: "var(--spacing-2)", color: "var(--muted-foreground)", fontSize: 12 }
});

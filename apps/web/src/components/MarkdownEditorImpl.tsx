import * as React from "react";
import { Button } from "@astryxdesign/core/Button";
import { HStack } from "@astryxdesign/core/HStack";
import { TextArea } from "@astryxdesign/core/TextArea";
import { Crepe } from "@milkdown/crepe";
import "@milkdown/crepe/theme/common/style.css";
import "@milkdown/crepe/theme/frame.css";
import * as stylex from "@stylexjs/stylex";
import type { MarkdownEditorProps } from "./MarkdownEditor";

export default function MarkdownEditorImpl({ value, onChange, label = "Markdown document", placeholder, density = "default", sourceMode: controlledSourceMode, onSourceModeChange, onReady }: MarkdownEditorProps) {
  const [internalSourceMode, setInternalSourceMode] = React.useState(false);
  const [richFailed, setRichFailed] = React.useState(false);
  const sourceMode = controlledSourceMode ?? internalSourceMode;
  const setSourceMode = (next: boolean) => onSourceModeChange ? onSourceModeChange(next) : setInternalSourceMode(next);
  React.useLayoutEffect(() => { if (sourceMode) onReady?.(); }, [onReady, sourceMode]);
  const modeLabel = sourceMode ? "Use rich editor" : "Edit source";
  const modeButton = <Button type="button" size="sm" variant="ghost" label={modeLabel} isDisabled={richFailed && sourceMode} onClick={() => setSourceMode(!sourceMode)} />;
  return (
    <section data-slot="markdown-editor" data-density={density} aria-label={label} {...stylex.props(styles.root, density === "inline" && styles.inlineRoot)}>
      {density === "default" ? <HStack justify="between" align="center" gap={2} className={stylex.props(styles.modeBar).className}>
        <span {...stylex.props(styles.modeLabel)}>{sourceMode ? "Markdown source" : "Rich text"}</span>
        {modeButton}
      </HStack> : null}
      {sourceMode ? (
        <TextArea
          isLabelHidden
          label={`${label} Markdown source`}
          rows={density === "inline" ? 6 : 14}
          value={value}
          placeholder={placeholder}
          width="100%"
          className={stylex.props(styles.source, density === "inline" && styles.inlineSource).className}
          onChange={onChange}
        />
      ) : (
        <MilkdownCrepe
          initialValue={value}
          inline={density === "inline"}
          placeholder={placeholder}
          onChange={onChange}
          onReady={onReady}
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

function MilkdownCrepe({ initialValue, inline, placeholder, onChange, onFailure, onReady }: { initialValue: string; inline: boolean; placeholder?: string; onChange: (value: string) => void; onFailure: () => void; onReady?: () => void }) {
  const root = React.useRef<HTMLDivElement>(null);
  const initialValueRef = React.useRef(initialValue);
  const onChangeRef = React.useRef(onChange);
  const onFailureRef = React.useRef(onFailure);
  const onReadyRef = React.useRef(onReady);
  React.useEffect(() => { onChangeRef.current = onChange; }, [onChange]);
  React.useEffect(() => { onFailureRef.current = onFailure; }, [onFailure]);
  React.useEffect(() => { onReadyRef.current = onReady; }, [onReady]);
  React.useLayoutEffect(() => {
    if (!root.current) return;
    const crepe = new Crepe({
      root: root.current,
      defaultValue: initialValueRef.current,
      featureConfigs: {
        // Keep the native caret and avoid virtual-cursor layout work on selection changes.
        [Crepe.Feature.Cursor]: { virtual: false },
        ...(placeholder ? {
          [Crepe.Feature.Placeholder]: { text: placeholder, mode: "doc" as const }
        } : {})
      },
      features: {
        [Crepe.Feature.AI]: false,
        [Crepe.Feature.ImageBlock]: false,
        [Crepe.Feature.Latex]: false,
        [Crepe.Feature.TopBar]: !inline
      }
    });
    crepe.on((listener) => {
      listener.markdownUpdated((_context, markdown, previousMarkdown) => {
        if (markdown !== previousMarkdown) onChangeRef.current(markdown);
      });
    });
    let active = true;
    void crepe.create().then(() => { if (active) onReadyRef.current?.(); }).catch(() => { if (active) { onFailureRef.current(); onReadyRef.current?.(); } });
    return () => {
      active = false;
      void crepe.destroy().catch(() => undefined);
    };
  }, [inline, placeholder]);
  return <div ref={root} {...stylex.props(styles.editor, inline && styles.inlineEditor)} />;
}

const styles = stylex.create({
  root: { position: "relative", minWidth: 0, minHeight: 320, borderWidth: 1, borderStyle: "solid", borderColor: "var(--border)", borderRadius: "var(--radius-element)", overflow: "hidden", backgroundColor: "var(--background)" },
  inlineRoot: { minHeight: 0, overflow: "visible", borderWidth: 0, borderRadius: 0 },
  modeBar: { minHeight: 38, borderBottomWidth: 1, borderBottomStyle: "solid", borderBottomColor: "var(--border)", paddingInline: "var(--spacing-2)" },
  modeLabel: { color: "var(--muted-foreground)", fontSize: 12, fontWeight: 600 },
  editor: { minHeight: 280, color: "var(--foreground)", fontFamily: "var(--font-family-body)" },
  inlineEditor: { minHeight: 0 },
  source: { minHeight: 280, borderWidth: 0, borderRadius: 0, fontFamily: "var(--noema-font-mono)", fontSize: 13, lineHeight: 1.55, resize: "vertical" },
  inlineSource: { minHeight: 96 },
  notice: { margin: 0, borderTopWidth: 1, borderTopStyle: "solid", borderTopColor: "var(--border)", padding: "var(--spacing-2)", color: "var(--muted-foreground)", fontSize: 12 }
});

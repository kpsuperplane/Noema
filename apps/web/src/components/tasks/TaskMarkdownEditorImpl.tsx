import * as React from "react";
import {
  BlockTypeSelect,
  BoldItalicUnderlineToggles,
  CreateLink,
  DiffSourceToggleWrapper,
  InsertCodeBlock,
  InsertTable,
  InsertThematicBreak,
  ListsToggle,
  MDXEditor,
  type MDXEditorMethods,
  codeBlockPlugin,
  codeMirrorPlugin,
  diffSourcePlugin,
  headingsPlugin,
  linkDialogPlugin,
  linkPlugin,
  listsPlugin,
  quotePlugin,
  tablePlugin,
  thematicBreakPlugin,
  toolbarPlugin
} from "@mdxeditor/editor";
import "@mdxeditor/editor/style.css";
import * as stylex from "@stylexjs/stylex";
import type { TaskMarkdownEditorProps } from "./TaskMarkdownEditor";

export default function TaskMarkdownEditorImpl({ value, onChange, label = "Task document" }: TaskMarkdownEditorProps) {
  const editor = React.useRef<MDXEditorMethods>(null);
  const [sourceOnly, setSourceOnly] = React.useState(false);
  React.useEffect(() => {
    if (editor.current?.getMarkdown() !== value) editor.current?.setMarkdown(value);
  }, [value]);
  const plugins = React.useMemo(() => [
    headingsPlugin(), listsPlugin(), quotePlugin(), thematicBreakPlugin(), linkPlugin(), linkDialogPlugin(),
    tablePlugin(), codeBlockPlugin({ defaultCodeBlockLanguage: "" }),
    codeMirrorPlugin({ codeBlockLanguages: { "": "Plain text", bash: "Bash", json: "JSON", markdown: "Markdown", rust: "Rust", typescript: "TypeScript" } }),
    diffSourcePlugin({ viewMode: sourceOnly ? "source" : "rich-text" }),
    toolbarPlugin({ toolbarContents: () => (
      <DiffSourceToggleWrapper options={["rich-text", "source"]}>
        <BlockTypeSelect />
        <BoldItalicUnderlineToggles options={["Bold", "Italic"]} />
        <ListsToggle />
        <CreateLink />
        <InsertCodeBlock />
        <InsertTable />
        <InsertThematicBreak />
      </DiffSourceToggleWrapper>
    ) })
  ], [sourceOnly]);
  return (
    <section aria-label={label} {...stylex.props(styles.root)}>
      <MDXEditor key={sourceOnly ? "source" : "rich"} ref={editor} markdown={value} plugins={plugins} className={stylex.props(styles.editor).className} contentEditableClassName={stylex.props(styles.content).className} onChange={(markdown, initialNormalize) => { if (!initialNormalize) onChange(markdown); }} onError={({ source }) => { onChange(source); setSourceOnly(true); }} />
      {sourceOnly ? <p role="status" {...stylex.props(styles.notice)}>Rich editing is unavailable for this Markdown. Source mode keeps the original text.</p> : null}
    </section>
  );
}

const styles = stylex.create({
  root: { minWidth: 0, minHeight: 320, borderWidth: 1, borderStyle: "solid", borderColor: "var(--border)", borderRadius: "var(--radius-element)", overflow: "hidden", backgroundColor: "var(--background)" },
  editor: { minHeight: 320 },
  content: { minHeight: 260, maxWidth: 880, marginInline: "auto", padding: "var(--spacing-4)", color: "var(--foreground)", fontFamily: "var(--font-family-sans)", fontSize: 15, lineHeight: 1.6 },
  notice: { margin: 0, borderTopWidth: 1, borderTopStyle: "solid", borderTopColor: "var(--border)", padding: "var(--spacing-2)", color: "var(--muted-foreground)", fontSize: 12 }
});

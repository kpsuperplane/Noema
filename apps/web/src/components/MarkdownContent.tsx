import { Markdown, type MarkdownProps } from "@astryxdesign/core/Markdown";

export type { MarkdownSource } from "@astryxdesign/core/Markdown";
export type MarkdownContentProps = MarkdownProps;
export type MarkdownComponents = NonNullable<MarkdownContentProps["components"]>;

export function MarkdownContent({
  autolink = "gfm",
  className,
  contentWidth = "100%",
  ...props
}: MarkdownContentProps) {
  const rootClassName = className ? `noema-markdown ${className}` : "noema-markdown";
  return (
    <Markdown
      autolink={autolink}
      className={rootClassName}
      contentWidth={contentWidth}
      {...props}
    />
  );
}

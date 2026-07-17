import * as stylex from "@stylexjs/stylex";
import type { MemoryArticleParagraph as MemoryArticleParagraphModel } from "@/pages/memoryPageModel";
import { styles as pageStyles } from "@/pages/memoryPageStyles";

export function MemoryArticleParagraph({
  paragraph,
  variant
}: {
  paragraph: MemoryArticleParagraphModel;
  variant: "lead" | "body";
}) {
  return (
    <p {...stylex.props(variant === "lead" ? pageStyles.lead : pageStyles.bodyText)}>
      {paragraph.parts.map((part, index) =>
        part.kind === "text" ? (
          <span key={`${paragraph.id}-text-${index}`}>{part.text}</span>
        ) : (
          <sup id={part.anchorId} key={part.anchorId} {...stylex.props(styles.footnote)}>
            <a
              href={`#reference-${part.number}`}
              aria-label={`Go to citation ${part.number}`}
              {...stylex.props(styles.footnoteLink)}
            >
              [{part.number}]
            </a>
          </sup>
        )
      )}
    </p>
  );
}

const styles = stylex.create({
  footnote: {
    marginLeft: 1,
    fontFamily: "-apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif",
    fontSize: 10,
    lineHeight: 0,
    scrollMarginTop: 16,
    verticalAlign: "super"
  },
  footnoteLink: {
    borderRadius: 2,
    color: "#36c",
    textDecoration: "none",
    ":hover": {
      color: "#233f8f",
      textDecoration: "underline"
    },
    ":focus-visible": {
      outlineWidth: 2,
      outlineStyle: "solid",
      outlineColor: "#36c",
      outlineOffset: 2
    }
  }
});

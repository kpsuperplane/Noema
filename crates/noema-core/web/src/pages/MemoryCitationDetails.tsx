import * as stylex from "@stylexjs/stylex";
import type { MemoryArticleReference } from "@/pages/memoryPageModel";

const wikiSerif = "Georgia, 'Times New Roman', serif";
const wikiSans = "-apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif";

export function MemoryCitationDetails({
  number,
  reference
}: {
  number: number;
  reference: MemoryArticleReference;
}) {
  return (
    <div {...stylex.props(styles.content)}>
      <div {...stylex.props(styles.kicker)}>Citation {number}</div>
      <h3 {...stylex.props(styles.title)}>{reference.label}</h3>

      {reference.sourceMessage ? (
        <section {...stylex.props(styles.section)}>
          <div {...stylex.props(styles.label)}>Source message</div>
          <blockquote {...stylex.props(styles.sourceQuote)}>{reference.sourceMessage}</blockquote>
        </section>
      ) : null}

      {reference.sourceMeta.length > 0 ? (
        <section {...stylex.props(styles.section)}>
          <div {...stylex.props(styles.label)}>Provenance</div>
          <div {...stylex.props(styles.sourceMeta)}>{reference.sourceMeta.join(" · ")}</div>
        </section>
      ) : null}

      {reference.citedFacts.length > 0 ? (
        <section {...stylex.props(styles.section)}>
          <div {...stylex.props(styles.label)}>Memories from this source</div>
          <ul {...stylex.props(styles.citedFacts)}>
            {reference.citedFacts.map((fact) => (
              <li key={fact.id}>{fact.displayText}</li>
            ))}
          </ul>
        </section>
      ) : null}
    </div>
  );
}

const styles = stylex.create({
  content: {
    boxSizing: "border-box",
    maxHeight: "min(420px, calc(100vh - 64px))",
    overflowY: "auto",
    color: "#202122",
    fontFamily: wikiSans,
    fontSize: 12,
    lineHeight: 1.5
  },
  kicker: {
    color: "#72777d",
    fontSize: 10,
    fontWeight: 700,
    letterSpacing: "0.06em",
    textTransform: "uppercase"
  },
  title: {
    margin: "2px 0 0",
    color: "#202122",
    fontFamily: wikiSerif,
    fontSize: 18,
    fontWeight: 400,
    lineHeight: 1.3
  },
  section: {
    marginTop: 14
  },
  label: {
    marginBottom: 4,
    color: "#54595d",
    fontSize: 10,
    fontWeight: 700,
    letterSpacing: "0.04em",
    textTransform: "uppercase"
  },
  sourceQuote: {
    margin: 0,
    borderLeftWidth: 3,
    borderLeftStyle: "solid",
    borderLeftColor: "#c8ccd1",
    padding: "3px 0 3px 9px",
    color: "#202122",
    fontFamily: wikiSerif,
    fontSize: 14,
    lineHeight: 1.45
  },
  sourceMeta: {
    color: "#54595d",
    overflowWrap: "anywhere"
  },
  citedFacts: {
    display: "grid",
    gap: 5,
    margin: 0,
    paddingLeft: 18,
    color: "#54595d"
  }
});

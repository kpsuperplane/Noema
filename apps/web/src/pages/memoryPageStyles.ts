import * as stylex from "@stylexjs/stylex";

const wikiSerif = "Georgia, 'Times New Roman', serif";
const wikiSans = "-apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif";

export const styles = stylex.create({
  surface: { display: "flex", flexDirection: "column", minWidth: 0, minHeight: 0, height: "100%", overflow: "hidden", backgroundColor: "var(--surface-base)" },
  updateNotice: { display: "flex", width: "100%", maxWidth: "100%", alignItems: "center", gap: "var(--spacing-2)", borderWidth: 1, borderStyle: "solid", borderColor: "var(--border-default)", backgroundColor: "var(--pine-50)", paddingBlock: "var(--spacing-1)", paddingInline: "var(--spacing-2)", fontFamily: wikiSans },
  updateNoticeError: { borderColor: "color-mix(in srgb, var(--destructive) 48%, var(--border-default))", backgroundColor: "var(--red-100)" },
  updateNoticeCopy: { display: "flex", flex: 1, minWidth: 0, alignItems: "baseline", flexWrap: "wrap", columnGap: "var(--spacing-1-5)", rowGap: "var(--spacing-0-5)" },
  updateNoticeAction: { flexShrink: 0, marginInlineStart: "auto" },
  updateNoticeTitle: { color: "var(--foreground)", fontSize: 12, lineHeight: 1.35 },
  updateNoticeDetail: { color: "var(--muted-foreground)", fontSize: 11, lineHeight: 1.35, overflowWrap: "anywhere" },
  notice: { flexShrink: 0, margin: "var(--spacing-3) var(--spacing-4) 0", borderRadius: 4, backgroundColor: "var(--surface-sunken)", padding: "var(--spacing-2) var(--spacing-3)", color: "var(--muted-foreground)", fontFamily: wikiSans, fontSize: 13, lineHeight: 1.45 },
  errorNotice: { backgroundColor: "color-mix(in srgb, var(--destructive) 8%, var(--surface-base))", color: "var(--destructive)" },
  pageTreeList: { margin: 0, paddingInline: 0, listStyle: "none" },
  pageTreeItem: { display: "grid", minWidth: 0, gap: "var(--spacing-1)" },
  pageTreeChildren: { display: "grid", gap: "var(--spacing-1)", margin: 0, padding: 0, listStyle: "none" },
  articleScroller: { boxSizing: "border-box", flex: 1, minWidth: 0, minHeight: 0, overflowY: "auto", backgroundColor: "var(--surface-base)" },
  article: { boxSizing: "border-box", width: "100%", minWidth: 0, color: "var(--foreground)" },
  articleContent: { boxSizing: "border-box", width: "100%", minWidth: 0, paddingBlock: "var(--spacing-4)", "::after": { content: "''", display: "block", clear: "both" }, "@media (max-width: 760px)": { paddingBlock: "var(--spacing-3)" } },
  contentsBox: { float: "left", width: 220, margin: "var(--spacing-4) var(--spacing-4) var(--spacing-3) 0", borderWidth: 1, borderStyle: "solid", borderColor: "var(--border-subtle)", backgroundColor: "var(--surface-sunken)", padding: "var(--spacing-3)", fontFamily: wikiSans, fontSize: 13, lineHeight: 1.6, "@media (max-width: 760px)": { float: "none", width: "auto", margin: "var(--spacing-4) 0" } },
  contentsTitle: { display: "block", marginBottom: "var(--spacing-1-5)", textAlign: "center" },
  contentsList: { display: "grid", gap: "var(--spacing-1)", margin: 0, paddingLeft: "var(--spacing-4)" },
  nestedContentsItem: { marginLeft: "var(--spacing-3)" },
  articleLink: { color: "var(--text-accent)", textDecoration: "none", ":hover": { textDecoration: "underline" } },
  articleBody: { color: "var(--foreground)", fontFamily: wikiSerif, fontSize: 15, lineHeight: 1.65 },
  citation: { position: "relative", top: "-0.15em", marginLeft: 2, fontFamily: wikiSans, fontSize: "0.72em", lineHeight: 0, verticalAlign: "baseline" },
  citationTrigger: { borderWidth: 0, backgroundColor: "transparent", padding: 0, color: "var(--text-accent)", font: "inherit", lineHeight: 1, cursor: "pointer", ":hover": { textDecoration: "underline" }, ":focus-visible": { outlineWidth: 2, outlineStyle: "solid", outlineColor: "var(--ring)", outlineOffset: 2 } },
  citationCard: { display: "grid", gap: "var(--spacing-2)", width: 320, maxWidth: "calc(100vw - var(--spacing-6))" },
  citationExcerpt: { color: "var(--foreground)", fontFamily: wikiSerif, fontSize: 13, lineHeight: 1.55, overflowWrap: "anywhere" },
  citationContext: { color: "var(--muted-foreground)", fontFamily: wikiSans, fontSize: 11, lineHeight: 1.4 },
  articleParagraph: { margin: "var(--spacing-4) 0 0", color: "var(--foreground)", fontFamily: wikiSerif, fontSize: 15, lineHeight: 1.65, overflowWrap: "anywhere" },
  articleHeading: { clear: "both", color: "var(--foreground)", fontFamily: wikiSerif, fontWeight: 400, overflowWrap: "anywhere", scrollMarginTop: "calc(var(--shell-deck-header-height, 44px) + var(--spacing-4))" },
  articleHeadingMajor: { margin: "var(--spacing-6) 0 var(--spacing-2)", borderBottomWidth: 1, borderBottomStyle: "solid", borderBottomColor: "var(--border-default)", paddingBottom: "var(--spacing-1)", fontSize: 24, lineHeight: 1.25 },
  articleHeadingMinor: { margin: "var(--spacing-4) 0 var(--spacing-2)", fontSize: 19, lineHeight: 1.3 },
  relatedArticlesSection: { clear: "both", paddingTop: "var(--spacing-1)" },
  relatedArticleList: { display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(min(260px, 100%), 300px))", gap: "var(--spacing-3)", margin: 0, padding: 0, listStyle: "none" },
  relatedArticleEntry: { minWidth: 0 },
  relatedArticleCardLink: { display: "block", width: "100%", maxWidth: 300, borderWidth: 1, borderStyle: "solid", borderColor: "var(--border-subtle)", borderRadius: 8, backgroundColor: "var(--surface-card)", boxShadow: "0 1px 0 color-mix(in srgb, black 4%, transparent)", color: "var(--foreground)", cursor: "pointer", textDecoration: "none", outlineColor: "var(--text-accent)", outlineOffset: 2, transitionProperty: "background-color, border-color, box-shadow", transitionDuration: "var(--motion-spring-micro-duration)", transitionTimingFunction: "var(--motion-spring-critical-easing)", ":hover": { borderColor: "var(--border-default)", backgroundColor: "var(--surface-sunken)", boxShadow: "0 2px 8px color-mix(in srgb, black 6%, transparent)" } },
  relatedArticleCard: { width: "100%", minHeight: 84, color: "inherit", cursor: "inherit" },
  stub: { marginTop: "var(--spacing-4)", borderLeftWidth: 4, borderLeftStyle: "solid", borderLeftColor: "var(--border-default)", backgroundColor: "var(--surface-sunken)", padding: "var(--spacing-2) var(--spacing-3)", color: "var(--muted-foreground)", fontFamily: wikiSans, fontSize: 13 },
  articleState: { margin: "var(--spacing-6)", color: "var(--muted-foreground)", fontFamily: wikiSans, fontSize: 13 }
});

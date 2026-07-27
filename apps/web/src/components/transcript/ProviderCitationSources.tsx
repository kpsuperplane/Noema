import * as stylex from "@stylexjs/stylex";

export type ProviderCitation = {
  title: string;
  url: string;
};

const styles = stylex.create({
  root: {
    display: "flex",
    alignItems: "baseline",
    flexWrap: "wrap",
    gap: "var(--spacing-1-5)",
    minWidth: 0,
    color: "var(--noema-text-secondary)",
    fontSize: 11,
    lineHeight: 1.4
  },
  label: {
    fontWeight: 600
  },
  links: {
    display: "inline-flex",
    flexWrap: "wrap",
    columnGap: "var(--spacing-1-5)",
    rowGap: "var(--spacing-1)"
  },
  link: {
    color: "var(--noema-text-link)",
    textDecoration: "none",
    ":hover": {
      textDecoration: "underline"
    }
  }
});

export function ProviderCitationSources({ citations }: { citations: readonly ProviderCitation[] }) {
  if (citations.length === 0) {
    return null;
  }

  return (
    <nav aria-label="Sources" {...stylex.props(styles.root)}>
      <span {...stylex.props(styles.label)}>Sources</span>
      <span {...stylex.props(styles.links)}>
        {citations.map((citation, index) => (
          <a
            key={citation.url}
            href={citation.url}
            target="_blank"
            rel="noopener noreferrer"
            {...stylex.props(styles.link)}
          >
            {index + 1}. {citation.title}
          </a>
        ))}
      </span>
    </nav>
  );
}

export function providerCitationsFromMetadata(metadata: unknown): ProviderCitation[] {
  if (!isRecord(metadata) || !Array.isArray(metadata.citations)) {
    return [];
  }

  const seen = new Set<string>();
  return metadata.citations.flatMap((citation) => {
    if (!isRecord(citation)) {
      return [];
    }
    const title = typeof citation.title === "string" ? citation.title.trim() : "";
    const url = typeof citation.url === "string" ? citation.url.trim() : "";
    if (!title || !/^https?:\/\//i.test(url) || seen.has(url)) {
      return [];
    }
    seen.add(url);
    return [{ title, url }];
  });
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

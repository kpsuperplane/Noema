import { RefreshCw } from "lucide-react";
import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import {
  trustedIdentityRows,
  type TrustedIdentitySelector
} from "./trustedIdentityMetadata";

export function TrustedIdentitiesSettingsPaneContent({
  selectors,
  loading,
  error,
  onRetry
}: {
  selectors: readonly TrustedIdentitySelector[];
  loading: boolean;
  error: string | null;
  onRetry: () => void;
}) {
  if (loading) {
    return <p {...stylex.props(styles.mutedText)}>Loading trusted identities...</p>;
  }

  if (error) {
    return (
      <div {...stylex.props(styles.card)}>
        <p {...stylex.props(styles.mutedText)}>
          Trusted identity selectors could not be loaded.
        </p>
        <Button
          type="button"
          variant="secondary"
          label="Retry"
          icon={<RefreshCw {...stylex.props(styles.icon)} aria-hidden="true" />}
          {...stylex.props(styles.fitButton)}
          onClick={onRetry}
        />
      </div>
    );
  }

  if (selectors.length === 0) {
    return (
      <div {...stylex.props(styles.card)}>
        <p {...stylex.props(styles.mutedText)}>
          No trusted identity selectors are configured.
        </p>
      </div>
    );
  }

  return (
    <div {...stylex.props(styles.panel)}>
      <div {...stylex.props(styles.list)}>
        {selectors.map((selector) => (
          <article key={selector.selectorId} {...stylex.props(styles.item)}>
            <div {...stylex.props(styles.minWidthZero)}>
              <h2 {...stylex.props(styles.title)}>
                {selector.normalizedValue}
              </h2>
            </div>
            <dl {...stylex.props(styles.definitionList)}>
              {trustedIdentityRows(selector).map((row) => (
                <div
                  key={row.label}
                  {...stylex.props(styles.definitionRow)}
                >
                  <dt {...stylex.props(styles.definitionTerm)}>{row.label}</dt>
                  <dd {...stylex.props(styles.definitionValue)}>
                    {row.value}
                  </dd>
                </div>
              ))}
            </dl>
          </article>
        ))}
      </div>
    </div>
  );
}

const styles = stylex.create({
  mutedText: {
    margin: 0,
    fontSize: 14,
    lineHeight: 1.5,
    color: "var(--muted-foreground)"
  },
  card: {
    display: "grid",
    gap: 12,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border-subtle)",
    borderRadius: 6,
    backgroundColor: "white",
    padding: 16
  },
  panel: {
    overflow: "hidden",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border-subtle)",
    borderRadius: 6,
    backgroundColor: "white"
  },
  list: {
    display: "grid",
    minWidth: 0
  },
  item: {
    display: "grid",
    gap: 12,
    padding: 16,
    borderTopWidth: 1,
    borderTopStyle: "solid",
    borderTopColor: "var(--border-subtle)",
    ":first-child": {
      borderTopWidth: 0
    }
  },
  minWidthZero: {
    minWidth: 0
  },
  title: {
    margin: 0,
    overflowWrap: "break-word",
    fontFamily: "var(--font-heading)",
    fontSize: 18,
    lineHeight: 1.25,
    letterSpacing: 0,
    color: "var(--foreground)"
  },
  definitionList: {
    display: "grid",
    gap: 8,
    margin: 0
  },
  definitionRow: {
    display: "grid",
    gridTemplateColumns: "minmax(110px, 160px) 1fr",
    gap: 16,
    "@media (max-width: 760px)": {
      gridTemplateColumns: "1fr",
      gap: 4
    }
  },
  definitionTerm: {
    fontSize: 14,
    fontWeight: 500,
    lineHeight: 1.5,
    color: "var(--muted-foreground)"
  },
  definitionValue: {
    minWidth: 0,
    margin: 0,
    overflowWrap: "break-word",
    fontFamily: "var(--font-mono)",
    fontSize: 14,
    lineHeight: 1.5,
    color: "var(--foreground)"
  },
  fitButton: {
    width: "fit-content"
  },
  icon: {
    width: 16,
    height: 16
  }
});

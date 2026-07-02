import { Badge } from "@astryxdesign/core/Badge";
import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import { RefreshCw } from "lucide-react";
import {
  providerStatusLabel,
  providerTechnicalRows,
  type ProviderSettingsAccount
} from "./providerMetadata";

export function ProvidersSettingsPaneContent({
  accounts,
  loading,
  error,
  onRetry
}: {
  accounts: readonly ProviderSettingsAccount[];
  loading: boolean;
  error: string | null;
  onRetry: () => void;
}) {
  if (loading) {
    return <p {...stylex.props(styles.mutedText)}>Loading provider metadata...</p>;
  }

  if (error) {
    return (
      <div {...stylex.props(styles.card)}>
        <p {...stylex.props(styles.mutedText)}>{error}</p>
        <Button
          {...stylex.props(styles.fitButton)}
          type="button"
          variant="secondary"
          label="Retry"
          icon={<RefreshCw {...stylex.props(styles.icon)} aria-hidden="true" />}
          onClick={onRetry}
        />
      </div>
    );
  }

  if (accounts.length === 0) {
    return (
      <div {...stylex.props(styles.card)}>
        <p {...stylex.props(styles.mutedText)}>
          No provider accounts are available. Settings is open, but Noema did not return a
          connected provider account.
        </p>
      </div>
    );
  }

  return (
    <div {...stylex.props(styles.list)}>
      {accounts.map((account) => {
        const rows = providerTechnicalRows(account);
        return (
          <article
            key={`${account.providerKind}:${account.accountKey}`}
            {...stylex.props(styles.card)}
          >
            <div {...stylex.props(styles.titleRow)}>
              <h2 {...stylex.props(styles.cardTitle)}>
                {account.displayName}
              </h2>
              <Badge variant="neutral" label={providerStatusLabel(account.status)} />
            </div>
            {account.providerKind === "foundation_local" ? (
              <p {...stylex.props(styles.mutedText)}>
                Local Apple model support is managed by this machine. Choose the model for each
                agent in Agents.
              </p>
            ) : null}
            <dl {...stylex.props(styles.definitionList)}>
              {rows.map((row) => (
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
        );
      })}
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
  list: {
    display: "grid",
    gap: 12
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
  titleRow: {
    display: "flex",
    flexWrap: "wrap",
    alignItems: "center",
    gap: 12
  },
  cardTitle: {
    margin: 0,
    fontFamily: "var(--font-heading)",
    fontSize: 20,
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
    gridTemplateColumns: "minmax(120px, 220px) 1fr",
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

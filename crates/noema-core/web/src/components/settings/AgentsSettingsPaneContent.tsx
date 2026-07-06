import { Badge } from "@astryxdesign/core/Badge";
import * as stylex from "@stylexjs/stylex";
import { ModelPreferenceSelect } from "./ModelPreferenceSelect";
import type {
  ModelPreference,
  ModelPreferenceSaveInput,
  ModelProviderOption
} from "./modelPreferenceTypes";
import {
  agentBadgeLabel,
  agentDisplayName,
  agentMetadataRows,
  selectedModelWarning
} from "./agentMetadata";

export type AgentSettingsAgent = {
  agentId: string;
  displayName?: string | null;
  isPrimary?: boolean;
  modelPreference?: ModelPreference | null;
  modelOptions?: readonly ModelProviderOption[];
};

type SaveAgentModelPreferenceInput = ModelPreferenceSaveInput & {
  agentId: string;
};

export function AgentsSettingsPaneContent({
  agents,
  loading,
  error,
  saving,
  saveError,
  onSaveModelPreference
}: {
  agents: readonly AgentSettingsAgent[];
  loading: boolean;
  error: string | null;
  saving: boolean;
  saveError: string | null;
  onSaveModelPreference: (input: SaveAgentModelPreferenceInput) => Promise<unknown>;
}) {
  if (loading) {
    return <p {...stylex.props(styles.mutedText)}>Loading agents...</p>;
  }

  return (
    <div {...stylex.props(styles.list)}>
      {saveError ? (
        <p {...stylex.props(styles.saveError)}>
          Noema could not save the model choice.
        </p>
      ) : null}
      {error ? (
        <div {...stylex.props(styles.card)}>
          <p {...stylex.props(styles.mutedText)}>
            Agent metadata could not be loaded.
          </p>
        </div>
      ) : agents.length === 0 ? (
        <div {...stylex.props(styles.card)}>
          <p {...stylex.props(styles.mutedText)}>No agents were found.</p>
        </div>
      ) : null}
      {error ? null : agents.map((agent) => {
        const displayName = agentDisplayName(agent);
        const badgeLabel = agentBadgeLabel(agent);
        const rows = agentMetadataRows(agent);
        const warning = selectedModelWarning(agent);
        return (
          <article
            key={agent.agentId}
            {...stylex.props(styles.card)}
          >
            <div {...stylex.props(styles.cardHeader)}>
              <div {...stylex.props(styles.titleRow)}>
                <h2 {...stylex.props(styles.cardTitle)}>
                  {displayName}
                </h2>
                {badgeLabel ? <Badge variant="neutral" label={badgeLabel} /> : null}
              </div>
              <ModelPreferenceSelect
                options={agent.modelOptions ?? []}
                preference={agent.modelPreference ?? null}
                saving={saving}
                ariaLabel={`Model settings for ${displayName}`}
                onSave={(input) =>
                  onSaveModelPreference({
                    agentId: agent.agentId,
                    ...input
                  })
                }
              />
            </div>
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
            {warning ? <p {...stylex.props(styles.warningText)}>{warning}</p> : null}
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
  saveError: {
    margin: 0,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "color-mix(in srgb, var(--destructive) 30%, transparent)",
    borderRadius: 6,
    backgroundColor: "color-mix(in srgb, var(--destructive) 5%, transparent)",
    padding: 12,
    fontSize: 14,
    lineHeight: 1.5,
    color: "var(--destructive)"
  },
  cardHeader: {
    display: "flex",
    flexWrap: "wrap",
    alignItems: "center",
    justifyContent: "space-between",
    gap: 12
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
    gridTemplateColumns: "minmax(120px, 180px) 1fr",
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
  warningText: {
    margin: 0,
    fontSize: 14,
    lineHeight: 1.5,
    color: "rgb(180, 83, 9)"
  }
});

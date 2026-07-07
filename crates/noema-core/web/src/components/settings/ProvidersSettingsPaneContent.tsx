import { Badge } from "@astryxdesign/core/Badge";
import { Button } from "@astryxdesign/core/Button";
import { Dialog, DialogHeader } from "@astryxdesign/core/Dialog";
import * as stylex from "@stylexjs/stylex";
import { AlertTriangle, KeyRound, Plus, RefreshCw, Trash2 } from "lucide-react";
import { useMemo, useState } from "react";
import {
  providerStatusLabel,
  providerTechnicalRows,
  type ProviderAccountCatalogEntry,
  type ProviderSettingsAccount
} from "./providerMetadata";

export function ProvidersSettingsPaneContent({
  catalog,
  accounts,
  loading,
  error,
  mutationSaving,
  mutationError,
  deleteError,
  onRetry,
  onCreateProviderAccount,
  onSaveProviderSecret,
  onClearProviderSecret,
  onDeleteProviderAccount
}: {
  catalog: readonly ProviderAccountCatalogEntry[];
  accounts: readonly ProviderSettingsAccount[];
  loading: boolean;
  error: string | null;
  mutationSaving: boolean;
  mutationError: string | null;
  deleteError: string | null;
  onRetry: () => void;
  onCreateProviderAccount: (input: {
    providerKind: string;
    displayName?: string | null;
    secret: string;
  }) => Promise<unknown>;
  onSaveProviderSecret: (input: {
    providerAccountId: string;
    secret: string;
  }) => Promise<unknown>;
  onClearProviderSecret: (input: { providerAccountId: string }) => Promise<unknown>;
  onDeleteProviderAccount: (input: { providerAccountId: string }) => Promise<unknown>;
}) {
  const [selectedProviderKind, setSelectedProviderKind] = useState(catalog[0]?.providerKind ?? "");
  const [deleteTargetId, setDeleteTargetId] = useState<string | null>(null);
  const selectedCatalogEntry = useMemo(
    () =>
      catalog.find((entry) => entry.providerKind === selectedProviderKind) ?? catalog[0] ?? null,
    [catalog, selectedProviderKind]
  );
  const deleteTarget =
    accounts.find((account) => account.providerAccountId === deleteTargetId) ?? null;

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

  return (
    <div {...stylex.props(styles.list)}>
      <AddProviderAccountCard
        catalog={catalog}
        selectedProviderKind={selectedProviderKind}
        selectedCatalogEntry={selectedCatalogEntry}
        mutationSaving={mutationSaving}
        mutationError={mutationError}
        onSelectProviderKind={setSelectedProviderKind}
        onCreateProviderAccount={onCreateProviderAccount}
      />
      {accounts.map((account) => (
        <ProviderAccountCard
          key={account.providerAccountId}
          account={account}
          mutationSaving={mutationSaving}
          onSaveProviderSecret={onSaveProviderSecret}
          onClearProviderSecret={onClearProviderSecret}
          onDeleteClick={() => setDeleteTargetId(account.providerAccountId)}
        />
      ))}
      {accounts.length === 0 ? (
        <div {...stylex.props(styles.card)}>
          <p {...stylex.props(styles.mutedText)}>
            No provider accounts have been added yet.
          </p>
        </div>
      ) : null}
      <DeleteProviderAccountDialog
        account={deleteTarget}
        open={deleteTarget !== null}
        submitting={mutationSaving}
        error={deleteError}
        onOpenChange={(open) => {
          if (!open && !mutationSaving) setDeleteTargetId(null);
        }}
        onConfirm={async () => {
          if (!deleteTarget) return;
          await onDeleteProviderAccount({ providerAccountId: deleteTarget.providerAccountId });
          setDeleteTargetId(null);
        }}
      />
    </div>
  );
}

function AddProviderAccountCard({
  catalog,
  selectedProviderKind,
  selectedCatalogEntry,
  mutationSaving,
  mutationError,
  onSelectProviderKind,
  onCreateProviderAccount
}: {
  catalog: readonly ProviderAccountCatalogEntry[];
  selectedProviderKind: string;
  selectedCatalogEntry: ProviderAccountCatalogEntry | null;
  mutationSaving: boolean;
  mutationError: string | null;
  onSelectProviderKind: (providerKind: string) => void;
  onCreateProviderAccount: (input: {
    providerKind: string;
    displayName?: string | null;
    secret: string;
  }) => Promise<unknown>;
}) {
  const [displayName, setDisplayName] = useState("");
  const [secret, setSecret] = useState("");
  const canSubmit = Boolean(selectedCatalogEntry) && secret.trim().length > 0;

  return (
    <form
      {...stylex.props(styles.card)}
      onSubmit={(event) => {
        event.preventDefault();
        if (!selectedCatalogEntry || !canSubmit) {
          return;
        }
        void onCreateProviderAccount({
          providerKind: selectedCatalogEntry.providerKind,
          displayName: displayName.trim() || null,
          secret
        }).then(() => {
          setDisplayName("");
          setSecret("");
        });
      }}
    >
      <div {...stylex.props(styles.titleRow)}>
        <h2 {...stylex.props(styles.cardTitle)}>Add provider account</h2>
        {selectedCatalogEntry ? (
          <Badge variant="neutral" label={providerAuthLabel(selectedCatalogEntry.authMethod)} />
        ) : null}
      </div>
      <div {...stylex.props(styles.formGrid)}>
        <label {...stylex.props(styles.field)}>
          <span {...stylex.props(styles.fieldLabel)}>Provider</span>
          <select
            {...stylex.props(styles.input)}
            value={selectedProviderKind}
            disabled={mutationSaving || catalog.length === 0}
            onChange={(event) => onSelectProviderKind(event.currentTarget.value)}
          >
            {catalog.map((entry) => (
              <option key={entry.providerKind} value={entry.providerKind}>
                {entry.displayName}
              </option>
            ))}
          </select>
        </label>
        <label {...stylex.props(styles.field)}>
          <span {...stylex.props(styles.fieldLabel)}>Account name</span>
          <input
            {...stylex.props(styles.input)}
            value={displayName}
            disabled={mutationSaving}
            placeholder={selectedCatalogEntry?.displayName ?? "Provider"}
            onChange={(event) => setDisplayName(event.currentTarget.value)}
          />
        </label>
        <label {...stylex.props(styles.field)}>
          <span {...stylex.props(styles.fieldLabel)}>API key</span>
          <input
            {...stylex.props(styles.input)}
            type="password"
            value={secret}
            disabled={mutationSaving}
            autoComplete="off"
            onChange={(event) => setSecret(event.currentTarget.value)}
          />
        </label>
      </div>
      <div {...stylex.props(styles.actionRow)}>
        <Button
          type="submit"
          label="Add account"
          icon={<Plus {...stylex.props(styles.icon)} aria-hidden="true" />}
          isDisabled={mutationSaving || !canSubmit}
        />
        {mutationError ? <p {...stylex.props(styles.saveError)}>{mutationError}</p> : null}
      </div>
    </form>
  );
}

function ProviderAccountCard({
  account,
  mutationSaving,
  onSaveProviderSecret,
  onClearProviderSecret,
  onDeleteClick
}: {
  account: ProviderSettingsAccount;
  mutationSaving: boolean;
  onSaveProviderSecret: (input: {
    providerAccountId: string;
    secret: string;
  }) => Promise<unknown>;
  onClearProviderSecret: (input: { providerAccountId: string }) => Promise<unknown>;
  onDeleteClick: () => void;
}) {
  const rows = providerTechnicalRows(account);
  const [replacementSecret, setReplacementSecret] = useState("");
  const canSaveSecret = replacementSecret.trim().length > 0;

  return (
    <article {...stylex.props(styles.card)}>
      <div {...stylex.props(styles.titleRow)}>
        <h2 {...stylex.props(styles.cardTitle)}>{account.displayName}</h2>
        <Badge variant="neutral" label={providerStatusLabel(account.status)} />
      </div>
      {account.providerKind === "foundation_local" ? (
        <p {...stylex.props(styles.mutedText)}>
          Local Apple model support is managed by this machine. Choose the model for each agent in
          Agents.
        </p>
      ) : null}
      <dl {...stylex.props(styles.definitionList)}>
        {rows.map((row) => (
          <div key={row.label} {...stylex.props(styles.definitionRow)}>
            <dt {...stylex.props(styles.definitionTerm)}>{row.label}</dt>
            <dd {...stylex.props(styles.definitionValue)}>{row.value}</dd>
          </div>
        ))}
      </dl>
      {account.authMethod === "secret_input" ? (
        <form
          {...stylex.props(styles.secretForm)}
          onSubmit={(event) => {
            event.preventDefault();
            if (!canSaveSecret) {
              return;
            }
            void onSaveProviderSecret({
              providerAccountId: account.providerAccountId,
              secret: replacementSecret
            }).then(() => setReplacementSecret(""));
          }}
        >
          <label {...stylex.props(styles.field)}>
            <span {...stylex.props(styles.fieldLabel)}>API key</span>
            <input
              {...stylex.props(styles.input)}
              type="password"
              value={replacementSecret}
              disabled={mutationSaving}
              autoComplete="off"
              onChange={(event) => setReplacementSecret(event.currentTarget.value)}
            />
          </label>
          <div {...stylex.props(styles.actionRow)}>
            <Button
              type="submit"
              label="Save key"
              icon={<KeyRound {...stylex.props(styles.icon)} aria-hidden="true" />}
              isDisabled={mutationSaving || !canSaveSecret}
            />
            <Button
              type="button"
              variant="secondary"
              label="Clear key"
              icon={<Trash2 {...stylex.props(styles.icon)} aria-hidden="true" />}
              isDisabled={mutationSaving}
              onClick={() =>
                void onClearProviderSecret({ providerAccountId: account.providerAccountId })
              }
            />
          </div>
        </form>
      ) : null}
      {!account.isDefault ? (
        <div {...stylex.props(styles.deleteSection)}>
          <Button
            type="button"
            variant="destructive"
            label="Delete account"
            icon={<Trash2 {...stylex.props(styles.icon)} aria-hidden="true" />}
            isDisabled={mutationSaving}
            onClick={onDeleteClick}
          />
        </div>
      ) : null}
    </article>
  );
}

function DeleteProviderAccountDialog({
  account,
  open,
  submitting,
  error,
  onOpenChange,
  onConfirm
}: {
  account: ProviderSettingsAccount | null;
  open: boolean;
  submitting: boolean;
  error: string | null;
  onOpenChange: (open: boolean) => void;
  onConfirm: () => void;
}) {
  const subtitle = account
    ? `Delete ${account.displayName}, its stored secrets, and any web tool selections using it.`
    : "Delete this provider account and its stored secrets.";

  return (
    <Dialog
      isOpen={open}
      onOpenChange={onOpenChange}
      purpose="form"
      width={520}
      aria-label="Delete provider account"
    >
      <div {...stylex.props(styles.dialog)}>
        <DialogHeader
          title="Delete provider account"
          subtitle={subtitle}
          onOpenChange={onOpenChange}
        />
        <div {...stylex.props(styles.dialogBody)}>
          <p {...stylex.props(styles.warningText)}>
            <AlertTriangle {...stylex.props(styles.warningIcon)} aria-hidden="true" />
            This cannot be undone from Settings.
          </p>
          {error ? <p {...stylex.props(styles.saveError)}>{error}</p> : null}
          <div {...stylex.props(styles.actionRow)}>
            <Button
              type="button"
              variant="destructive"
              label="Delete account"
              icon={<Trash2 {...stylex.props(styles.icon)} aria-hidden="true" />}
              isDisabled={submitting}
              isLoading={submitting}
              onClick={onConfirm}
            />
            <Button
              type="button"
              variant="secondary"
              label="Cancel"
              isDisabled={submitting}
              onClick={() => onOpenChange(false)}
            />
          </div>
        </div>
      </div>
    </Dialog>
  );
}

function providerAuthLabel(method: string) {
  if (method === "secret_input") {
    return "API key";
  }
  return method;
}

const styles = stylex.create({
  mutedText: {
    margin: 0,
    fontSize: 14,
    lineHeight: 1.5,
    color: "var(--muted-foreground)"
  },
  saveError: {
    margin: 0,
    fontSize: 14,
    lineHeight: 1.5,
    color: "var(--destructive)"
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
  formGrid: {
    display: "grid",
    gridTemplateColumns: "minmax(160px, 220px) minmax(180px, 1fr) minmax(220px, 1.2fr)",
    gap: 12,
    "@media (max-width: 900px)": {
      gridTemplateColumns: "1fr"
    }
  },
  field: {
    display: "grid",
    gap: 6
  },
  fieldLabel: {
    fontSize: 13,
    fontWeight: 500,
    lineHeight: 1.4,
    color: "var(--muted-foreground)"
  },
  input: {
    width: "100%",
    minHeight: 36,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border)",
    borderRadius: 6,
    backgroundColor: "white",
    paddingBlock: 6,
    paddingInline: 10,
    font: "inherit",
    color: "var(--foreground)"
  },
  actionRow: {
    display: "flex",
    flexWrap: "wrap",
    alignItems: "center",
    gap: 10
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
  secretForm: {
    display: "grid",
    gap: 10,
    borderTopWidth: 1,
    borderTopStyle: "solid",
    borderTopColor: "var(--border-subtle)",
    paddingTop: 12
  },
  deleteSection: {
    display: "flex",
    justifyContent: "flex-start",
    borderTopWidth: 1,
    borderTopStyle: "solid",
    borderTopColor: "var(--border-subtle)",
    paddingTop: 12
  },
  dialog: {
    display: "grid"
  },
  dialogBody: {
    display: "grid",
    gap: 16,
    padding: 20
  },
  warningText: {
    display: "flex",
    alignItems: "center",
    gap: 8,
    margin: 0,
    fontSize: 14,
    lineHeight: 1.5,
    color: "var(--foreground)"
  },
  warningIcon: {
    width: 16,
    height: 16,
    color: "var(--destructive)"
  },
  fitButton: {
    width: "fit-content"
  },
  icon: {
    width: 16,
    height: 16
  }
});

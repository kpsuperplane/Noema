import * as stylex from "@stylexjs/stylex";
import { Badge } from "@astryxdesign/core/Badge";
import { Button } from "@astryxdesign/core/Button";
import { Dialog, DialogHeader } from "@/components/ResponsiveDialog";
import { HStack } from "@astryxdesign/core/HStack";
import { Layout, LayoutContent } from "@astryxdesign/core/Layout";
import { Selector, type SelectorOptionType } from "@astryxdesign/core/Selector";
import { TextInput } from "@astryxdesign/core/TextInput";
import { VStack } from "@astryxdesign/core/VStack";
import { AlertTriangle, KeyRound, Plus, RefreshCw, Trash2 } from "lucide-react";
import { useMemo, useState } from "react";
import {
  providerAuthMethodLabel,
  providerStatusLabel,
  providerTechnicalRows,
  type ProviderAccountCatalogEntry,
  type ProviderSettingsAccount
} from "./providerMetadata";
import { SettingsEditDialog } from "./SettingsEditDialog";
import { AuthAttempt } from "../onboarding/AuthAttempt";
import type { ProviderAuthAttemptView } from "../onboarding/types";
import { SettingsList, SettingsListItem, SettingsSection } from "./SettingsPrimitives";

export function ProvidersSettingsPaneContent({
  catalog,
  accounts,
  loading,
  error,
  mutationSaving,
  mutationError,
  deleteError,
  authAttempt,
  onRetry,
  onCreateProviderAccount,
  onConnectProvider,
  onCancelProviderAuth,
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
  authAttempt: ProviderAuthAttemptView | null;
  onRetry: () => void;
  onCreateProviderAccount: (input: {
    providerKind: string;
    displayName?: string | null;
    secret: string;
    authMethod: "SECRET_INPUT";
  }) => Promise<unknown>;
  onConnectProvider: (
    providerKind: string,
    method: ProviderAccountCatalogEntry["preferredAuthMethod"]
  ) => Promise<unknown>;
  onCancelProviderAuth: () => Promise<unknown>;
  onSaveProviderSecret: (input: { providerAccountId: string; secret: string }) => Promise<unknown>;
  onClearProviderSecret: (input: { providerAccountId: string }) => Promise<unknown>;
  onDeleteProviderAccount: (input: { providerAccountId: string }) => Promise<unknown>;
}) {
  const [addOpen, setAddOpen] = useState(false);
  const [deleteTargetId, setDeleteTargetId] = useState<string | null>(null);
  const deleteTarget = accounts.find((account) => account.providerAccountId === deleteTargetId) ?? null;

  if (loading) {
    return <p {...stylex.props(styles.mutedText)}>Loading provider metadata...</p>;
  }

  if (error) {
    return (
      <SettingsSection aria-labelledby="provider-accounts-title">
        <VStack gap={2}>
          <h2 id="provider-accounts-title" {...stylex.props(styles.sectionTitle)}>Provider accounts</h2>
          <p {...stylex.props(styles.mutedText)}>{error}</p>
          <Button
            {...stylex.props(styles.fitButton)}
            type="button"
            variant="secondary"
            label="Retry"
            icon={<RefreshCw {...stylex.props(styles.icon)} aria-hidden="true" />}
            onClick={onRetry}
          />
        </VStack>
      </SettingsSection>
    );
  }

  return (
    <VStack gap={6} {...stylex.props(styles.stack)}>
      <SettingsSection aria-labelledby="provider-accounts-title">
        <VStack gap={2}>
          <HStack wrap="wrap" gap={3} vAlign="center" hAlign="between">
            <h2 id="provider-accounts-title" {...stylex.props(styles.sectionTitle)}>
              Provider accounts
            </h2>
            <Button
              type="button"
              label="Add provider"
              icon={<Plus {...stylex.props(styles.icon)} aria-hidden="true" />}
              isDisabled={mutationSaving || catalog.length === 0}
              onClick={() => setAddOpen(true)}
            />
          </HStack>
          {accounts.length > 0 ? (
            <SettingsList density="balanced" hasDividers>
              {accounts.map((account) => (
                <ProviderAccountRow
                  key={account.providerAccountId}
                  account={account}
                  mutationSaving={mutationSaving}
                  mutationError={mutationError}
                  onSaveProviderSecret={onSaveProviderSecret}
                  onClearProviderSecret={onClearProviderSecret}
                  onDeleteClick={() => setDeleteTargetId(account.providerAccountId)}
                />
              ))}
            </SettingsList>
          ) : (
            <p {...stylex.props(styles.mutedText)}>No provider accounts have been added yet.</p>
          )}
        </VStack>
      </SettingsSection>
      <AddProviderAccountDialog
        catalog={catalog}
        open={addOpen}
        mutationSaving={mutationSaving}
        mutationError={mutationError}
        authAttempt={authAttempt}
        onOpenChange={setAddOpen}
        onCreateProviderAccount={async (input) => {
          await onCreateProviderAccount(input);
          setAddOpen(false);
        }}
        onConnectProvider={onConnectProvider}
        onCancelProviderAuth={onCancelProviderAuth}
      />
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
    </VStack>
  );
}

function ProviderAccountRow({
  account,
  mutationSaving,
  mutationError,
  onSaveProviderSecret,
  onClearProviderSecret,
  onDeleteClick
}: {
  account: ProviderSettingsAccount;
  mutationSaving: boolean;
  mutationError: string | null;
  onSaveProviderSecret: (input: { providerAccountId: string; secret: string }) => Promise<unknown>;
  onClearProviderSecret: (input: { providerAccountId: string }) => Promise<unknown>;
  onDeleteClick: () => void;
}) {
  const rows = providerTechnicalRows(account);
  const [replaceOpen, setReplaceOpen] = useState(false);
  const [replacementSecret, setReplacementSecret] = useState("");
  const canSaveSecret = replacementSecret.trim().length > 0;
  const supportsSecret = account.authMethod === "secret_input" || account.providerKind === "openrouter";

  return (
    <SettingsListItem
      label={
        <HStack gap={2} vAlign="center" wrap="wrap">
          <span {...stylex.props(styles.rowLabel)}>{account.displayName}</span>
          <Badge variant="neutral" label={providerStatusLabel(account.status)} />
          {account.isDefault ? <Badge variant="neutral" label="Default" /> : null}
        </HStack>
      }
      description={
        <VStack gap={1}>
          <span>{account.providerKind} · {providerAuthMethodLabel(account.authMethod)}</span>
          {account.providerKind === "foundation_local" ? (
            <span>Local Apple model support is managed by this machine; agent model choices stay in Agents.</span>
          ) : null}
          <VStack as="details" gap={1} {...stylex.props(styles.details)}>
            <summary {...stylex.props(styles.detailsSummary)}>Technical details</summary>
            <SettingsList density="compact">
              {rows.map((row) => <SettingsListItem key={row.label} label={row.label} description={row.value} />)}
            </SettingsList>
          </VStack>
        </VStack>
      }
      endContent={
        <HStack wrap="wrap" gap={2} vAlign="center" {...stylex.props(styles.rowControl)}>
          {supportsSecret ? (
            <>
              <Button
                type="button"
                variant="secondary"
                size="sm"
                label="Replace key"
                icon={<KeyRound {...stylex.props(styles.icon)} aria-hidden="true" />}
                isDisabled={mutationSaving}
                onClick={() => setReplaceOpen(true)}
              />
              <Button
                type="button"
                variant="secondary"
                size="sm"
                label="Clear key"
                icon={<Trash2 {...stylex.props(styles.icon)} aria-hidden="true" />}
                isDisabled={mutationSaving}
                onClick={() => void onClearProviderSecret({ providerAccountId: account.providerAccountId })}
              />
            </>
          ) : null}
          {!account.isDefault ? (
            <Button
              type="button"
              variant="destructive"
              size="sm"
              label="Delete account"
              icon={<Trash2 {...stylex.props(styles.icon)} aria-hidden="true" />}
              isDisabled={mutationSaving}
              onClick={onDeleteClick}
            />
          ) : null}
          <SettingsEditDialog
            title={`Replace API key for ${account.displayName}`}
            open={replaceOpen}
            saving={mutationSaving}
            saveLabel="Save key"
            saveDisabled={!canSaveSecret}
            error={replaceOpen ? mutationError : null}
            onOpenChange={(open) => {
              setReplaceOpen(open);
              if (!open) setReplacementSecret("");
            }}
            onSave={async () => {
              if (!canSaveSecret) return;
              await onSaveProviderSecret({
                providerAccountId: account.providerAccountId,
                secret: replacementSecret
              });
              setReplacementSecret("");
              setReplaceOpen(false);
            }}
          >
            <TextInput
              hasAutoFocus
              label="API key"
              type="password"
              value={replacementSecret}
              onChange={setReplacementSecret}
            />
          </SettingsEditDialog>
        </HStack>
      }
    />
  );
}

function AddProviderAccountDialog({
  catalog,
  open,
  mutationSaving,
  mutationError,
  authAttempt,
  onOpenChange,
  onCreateProviderAccount,
  onConnectProvider,
  onCancelProviderAuth
}: {
  catalog: readonly ProviderAccountCatalogEntry[];
  open: boolean;
  mutationSaving: boolean;
  mutationError: string | null;
  authAttempt: ProviderAuthAttemptView | null;
  onOpenChange: (open: boolean) => void;
  onCreateProviderAccount: (input: {
    providerKind: string;
    displayName?: string | null;
    secret: string;
    authMethod: "SECRET_INPUT";
  }) => Promise<unknown>;
  onConnectProvider: (
    providerKind: string,
    method: ProviderAccountCatalogEntry["preferredAuthMethod"]
  ) => Promise<unknown>;
  onCancelProviderAuth: () => Promise<unknown>;
}) {
  const [selectedProviderKind, setSelectedProviderKind] = useState(catalog[0]?.providerKind ?? "");
  const [displayName, setDisplayName] = useState("");
  const [secret, setSecret] = useState("");
  const selectedCatalogEntry = useMemo(
    () => catalog.find((entry) => entry.providerKind === selectedProviderKind) ?? catalog[0] ?? null,
    [catalog, selectedProviderKind]
  );
  const acceptsApiKey = selectedCatalogEntry?.supportedAuthMethods.includes("SECRET_INPUT") ?? false;
  const canSubmit = acceptsApiKey && secret.trim().length > 0;
  const browserAuth = selectedCatalogEntry?.preferredAuthMethod === "OAUTH_PKCE" ||
    selectedCatalogEntry?.preferredAuthMethod === "OAUTH_DEVICE_CODE";
  const providerOptions = useMemo<SelectorOptionType[]>(
    () => catalog.map((entry) => ({ value: entry.providerKind, label: entry.displayName })),
    [catalog]
  );

  const submit = async () => {
    if (!selectedCatalogEntry || !canSubmit) return;
    try {
      await onCreateProviderAccount({
        providerKind: selectedCatalogEntry.providerKind,
        displayName: displayName.trim() || null,
        secret,
        authMethod: "SECRET_INPUT"
      });
      setDisplayName("");
      setSecret("");
    } catch {
      // The mutation error is rendered in this dialog by the owning pane.
    }
  };

  return (
    <Dialog isOpen={open} onOpenChange={onOpenChange} purpose="form" width={620} aria-label="Add provider account">
      <Layout
        height="auto"
        header={<DialogHeader title="Add provider account" onOpenChange={onOpenChange} />}
        content={
          <LayoutContent>
            <VStack
              as="form"
              gap={3}
              onSubmit={(event) => {
                event.preventDefault();
                void submit();
              }}
            >
              <VStack gap={2}>
                <label {...stylex.props(styles.selectorField)}>
                  <span {...stylex.props(styles.fieldLabel)}>Provider</span>
                  <Selector
                    isLabelHidden
                    label="Provider"
                    options={providerOptions}
                    placeholder="Select provider"
                    value={selectedProviderKind}
                    width="100%"
                    isDisabled={mutationSaving || catalog.length === 0}
                    onChange={setSelectedProviderKind}
                  />
                </label>
                <TextInput
                  label="Account name"
                  value={displayName}
                  isDisabled={mutationSaving}
                  placeholder={selectedCatalogEntry?.displayName ?? "Provider"}
                  onChange={setDisplayName}
                />
                <TextInput
                  label="API key"
                  type="password"
                  value={secret}
                  isDisabled={mutationSaving || !acceptsApiKey}
                  placeholder={acceptsApiKey ? undefined : "Use this provider's connect flow"}
                  onChange={setSecret}
                />
              </VStack>
              <HStack gap={2} wrap="wrap" vAlign="center" hAlign="end">
                {browserAuth && selectedCatalogEntry && !authAttempt ? (
                  <Button
                    type="button"
                    variant="secondary"
                    label={`Connect ${selectedCatalogEntry.displayName}`}
                    isDisabled={mutationSaving}
                    onClick={() => void onConnectProvider(selectedCatalogEntry.providerKind, selectedCatalogEntry.preferredAuthMethod)}
                  />
                ) : null}
                <Button
                  type="submit"
                  label={browserAuth ? "Use API key" : "Add account"}
                  icon={<Plus {...stylex.props(styles.icon)} aria-hidden="true" />}
                  isDisabled={mutationSaving || !canSubmit}
                  isLoading={mutationSaving}
                />
              </HStack>
              {mutationError ? <p role="alert" {...stylex.props(styles.saveError)}>{mutationError}</p> : null}
              {authAttempt ? <AuthAttempt attempt={authAttempt} onCancel={() => void onCancelProviderAuth()} /> : null}
            </VStack>
          </LayoutContent>
        }
      />
    </Dialog>
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
  const consequence = account
    ? `Delete ${account.displayName}, its stored secrets, and any web tool selections using it.`
    : "Delete this provider account and its stored secrets.";

  return (
    <Dialog
      isOpen={open}
      onOpenChange={onOpenChange}
      purpose="form"
      width={480}
      aria-label="Delete provider account"
    >
      <Layout
        height="auto"
        header={<DialogHeader title="Delete provider account?" onOpenChange={onOpenChange} />}
        content={
          <LayoutContent>
            <VStack gap={3}>
              <HStack as="p" gap={2} vAlign="start" {...stylex.props(styles.warningText)}>
                <AlertTriangle {...stylex.props(styles.warningIcon)} aria-hidden="true" />
                <span>{consequence} This cannot be undone from Settings.</span>
              </HStack>
              {error ? <p role="alert" {...stylex.props(styles.saveError)}>{error}</p> : null}
              <HStack gap={2} hAlign="end" wrap="wrap">
                <Button
                  type="button"
                  variant="secondary"
                  label="Cancel"
                  isDisabled={submitting}
                  onClick={() => onOpenChange(false)}
                />
                <Button
                  type="button"
                  variant="destructive"
                  label="Delete account"
                  icon={<Trash2 {...stylex.props(styles.icon)} aria-hidden="true" />}
                  isDisabled={submitting}
                  isLoading={submitting}
                  onClick={onConfirm}
                />
              </HStack>
            </VStack>
          </LayoutContent>
        }
      />
    </Dialog>
  );
}

const styles = stylex.create({
  stack: { minWidth: 0 },
  sectionTitle: {
    margin: "var(--spacing-0)",
    fontFamily: "var(--font-heading)",
    fontSize: 16,
    lineHeight: 1.3,
    color: "var(--foreground)"
  },
  rowLabel: { color: "var(--foreground)", fontWeight: 650, overflowWrap: "anywhere" },
  mutedText: {
    margin: "var(--spacing-0)",
    color: "var(--muted-foreground)",
    fontSize: 13,
    lineHeight: 1.5
  },
  saveError: {
    margin: "var(--spacing-0)",
    color: "var(--destructive)",
    fontSize: 13,
    lineHeight: 1.5
  },
  rowControl: {
    justifyContent: "flex-end",
    "@media (max-width: 620px)": {
      width: "100%",
      justifyContent: "flex-start",
      marginInlineStart: "0"
    }
  },
  details: { minWidth: 0 },
  detailsSummary: { width: "fit-content", color: "var(--muted-foreground)", fontSize: 12 },
  selectorField: { display: "grid", gap: "var(--spacing-1-5)" },
  fieldLabel: { color: "var(--foreground)", fontSize: 13, fontWeight: 500 },
  warningText: {
    margin: "var(--spacing-0)",
    color: "var(--foreground)",
    fontSize: 13,
    lineHeight: 1.5
  },
  warningIcon: { width: 16, height: 16, marginTop: "var(--spacing-0-5)", color: "var(--destructive)" },
  fitButton: { width: "fit-content" },
  icon: { width: 16, height: 16 }
});

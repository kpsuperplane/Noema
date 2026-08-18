import * as stylex from "@stylexjs/stylex";
import { useMutation, useQuery, useSubscription } from "@apollo/client/react";
import { Avatar } from "@astryxdesign/core/Avatar";
import { Badge } from "@astryxdesign/core/Badge";
import { Button } from "@astryxdesign/core/Button";
import { Dialog, DialogHeader } from "@/components/ResponsiveDialog";
import { HStack } from "@astryxdesign/core/HStack";
import { useMediaQuery } from "@astryxdesign/core/hooks";
import { Layout, LayoutContent } from "@astryxdesign/core/Layout";
import { MoreMenu } from "@astryxdesign/core/MoreMenu";
import { Selector, type SelectorOptionType } from "@astryxdesign/core/Selector";
import { Switch } from "@astryxdesign/core/Switch";
import { TextInput } from "@astryxdesign/core/TextInput";
import { VStack } from "@astryxdesign/core/VStack";
import { AlertTriangle, ChevronRight, KeyRound, Plus, Trash2 } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useNavigate } from "@tanstack/react-router";
import {
  CancelProviderAuthAttemptDocument,
  ClearProviderSecretDocument,
  CreateProviderAccountDocument,
  DeleteProviderAccountDocument,
  ProviderAccountsDocument,
  ProviderAuthAttemptEventsDocument,
  SaveProviderSecretInputDocument,
  SetProviderFastModeDocument,
  StartProviderAuthAttemptDocument,
  WebToolSettingsDocument,
  type ClearProviderSecretMutation,
  type ClearProviderSecretMutationVariables,
  type CreateProviderAccountMutation,
  type CreateProviderAccountMutationVariables,
  type DeleteProviderAccountMutation,
  type DeleteProviderAccountMutationVariables,
  type ProviderAccountsQuery,
  type SaveProviderSecretInputMutation,
  type SaveProviderSecretInputMutationVariables,
  type SetProviderFastModeMutation,
  type SetProviderFastModeMutationVariables,
  type StartProviderAuthAttemptMutation
} from "@/generated/graphql";
import {
  providerAuthMethodLabel,
  providerStatusLabel,
  providerTechnicalRows,
  type ProviderAccountCatalogEntry,
  type ProviderSettingsAccount
} from "./providerMetadata";
import { SettingsEditDialog } from "./SettingsEditDialog";
import { DeleteConfirmationDialog } from "./DeleteConnectionDialog";
import { AuthAttempt } from "../onboarding/AuthAttempt";
import type { ProviderAuthAttemptView } from "../onboarding/types";
import { ListCardLink } from "@/components/ListCardLink";
import { SettingsManagementLayout } from "./SettingsManagementLayout";
import {
  SettingsList,
  SettingsListItem,
  SettingsLocalFeedback,
  SettingsSection,
  SettingsSectionInset,
  SettingsTechnicalDetails
} from "./SettingsPrimitives";

export function ProvidersSettingsPane({ providerAccountId }: { providerAccountId?: string }) {
  const navigate = useNavigate();
  const desktop = useMediaQuery("(min-width: 980px)");
  const [authAttempt, setAuthAttempt] = useState<StartProviderAuthAttemptMutation["startProviderAuthAttempt"] | null>(null);
  const result = useQuery<ProviderAccountsQuery>(ProviderAccountsDocument, { fetchPolicy: "cache-and-network" });
  const refetchQueries = [{ query: ProviderAccountsDocument }, { query: WebToolSettingsDocument }];
  const mutationOptions = { refetchQueries, awaitRefetchQueries: true };
  const [createProviderAccount, createResult] = useMutation<CreateProviderAccountMutation, CreateProviderAccountMutationVariables>(CreateProviderAccountDocument, mutationOptions);
  const [saveProviderSecretInput, saveSecretResult] = useMutation<SaveProviderSecretInputMutation, SaveProviderSecretInputMutationVariables>(SaveProviderSecretInputDocument, mutationOptions);
  const [clearProviderSecret, clearSecretResult] = useMutation<ClearProviderSecretMutation, ClearProviderSecretMutationVariables>(ClearProviderSecretDocument, mutationOptions);
  const [deleteProviderAccount, deleteResult] = useMutation<DeleteProviderAccountMutation, DeleteProviderAccountMutationVariables>(DeleteProviderAccountDocument, mutationOptions);
  const [setProviderFastMode, fastModeResult] = useMutation<SetProviderFastModeMutation, SetProviderFastModeMutationVariables>(SetProviderFastModeDocument, mutationOptions);
  const [startProviderAuthAttempt, startAuthResult] = useMutation(StartProviderAuthAttemptDocument);
  const [cancelProviderAuthAttempt] = useMutation(CancelProviderAuthAttemptDocument);
  useSubscription(ProviderAuthAttemptEventsDocument, {
    variables: { attemptId: authAttempt?.attemptId ?? "" },
    skip: !authAttempt,
    onData: ({ data }) => {
      const next = data.data?.providerAuthAttemptEvents;
      if (!next) return;
      setAuthAttempt(next);
      if (next.status === "COMPLETED") void result.refetch();
    }
  });
  const catalog = result.data?.providerAccountCatalog ?? [];
  const accounts = result.data?.providerAccounts ?? [];
  const loading = result.loading && !result.data;
  const error = result.error?.message ?? null;
  const mutationSaving = createResult.loading || saveSecretResult.loading || clearSecretResult.loading || deleteResult.loading || startAuthResult.loading;
  const mutationError = createResult.error?.message ?? saveSecretResult.error?.message ?? clearSecretResult.error?.message ?? startAuthResult.error?.message ?? null;
  const deleteError = deleteResult.error?.message ?? null;
  const onRetry = () => void result.refetch();
  const onCreateProviderAccount = (input: { providerKind: string; displayName?: string | null; secret: string; authMethod: "SECRET_INPUT" }) => createProviderAccount({ variables: { input } });
  const onConnectProvider = async (providerKind: string, method: ProviderAccountCatalogEntry["preferredAuthMethod"]) => {
    const response = await startProviderAuthAttempt({ variables: { input: { providerKind, method } } });
    if (response.data?.startProviderAuthAttempt) setAuthAttempt(response.data.startProviderAuthAttempt);
  };
  const onCancelProviderAuth = async () => {
    if (!authAttempt) return;
    await cancelProviderAuthAttempt({ variables: { input: { attemptId: authAttempt.attemptId } } });
    setAuthAttempt(null);
  };
  const onSaveProviderSecret = (input: { providerAccountId: string; secret: string }) => saveProviderSecretInput({ variables: { input } });
  const onClearProviderSecret = (input: { providerAccountId: string }) => clearProviderSecret({ variables: { input } });
  const onDeleteProviderAccount = (input: { providerAccountId: string }) => deleteProviderAccount({ variables: { input } });
  const onSetProviderFastMode = (input: { providerAccountId: string; enabled: boolean }) => setProviderFastMode({ variables: { input } });
  const [addOpen, setAddOpen] = useState(false);
  const [replaceOpen, setReplaceOpen] = useState(false);
  const [clearOpen, setClearOpen] = useState(false);
  const [replacementSecret, setReplacementSecret] = useState("");
  const [deleteTargetId, setDeleteTargetId] = useState<string | null>(null);
  const deleteTarget = accounts.find((account) => account.providerAccountId === deleteTargetId) ?? null;
  const selectedAccount = accounts.find((account) => account.providerAccountId === providerAccountId) ?? null;
  const defaultAccount = accounts.find((account) => account.isDefault) ?? accounts[0];

  useEffect(() => {
    if (!desktop || providerAccountId || !defaultAccount) return;
    void navigate({
      to: "/settings/system/providers/$providerAccountId",
      params: { providerAccountId: defaultAccount.providerAccountId },
      replace: true
    });
  }, [defaultAccount, desktop, navigate, providerAccountId]);

  useEffect(() => {
    if (loading || !providerAccountId || selectedAccount) return;
    void navigate({ to: "/settings/system/providers", replace: true });
  }, [loading, navigate, providerAccountId, selectedAccount]);

  const catalogEntry = catalog.find((entry) => entry.providerKind === selectedAccount?.providerKind);
  const supportsSecret = selectedAccount?.authMethod === "secret_input"
    || selectedAccount?.providerKind === "openrouter";
  const canSaveSecret = replacementSecret.trim().length > 0;

  return (
    <>
      <SettingsManagementLayout
        title="Providers"
        primaryAction={catalog.length > 0 ? { label: "Add provider", onClick: () => setAddOpen(true) } : undefined}
        detailOpen={providerAccountId !== undefined}
        detailLabel="Manage provider account"
        onDetailOpenChange={(open) => {
          if (!open) void navigate({ to: "/settings/system/providers" });
        }}
        list={
          <ProviderAccountList
            accounts={accounts}
            catalog={catalog}
            selectedProviderAccountId={providerAccountId}
            loading={loading}
            error={error}
            onRetry={onRetry}
          />
        }
        detail={selectedAccount ? (
          <ProviderAccountDetail
            account={selectedAccount}
            catalogEntry={catalogEntry}
            busy={mutationSaving}
            supportsSecret={supportsSecret}
            fastModeSaving={fastModeResult.loading}
            fastModeError={fastModeResult.error?.message ?? null}
            onFastModeChange={(enabled) => onSetProviderFastMode({
              providerAccountId: selectedAccount.providerAccountId,
              enabled
            }).then(() => undefined)}
            onReplace={() => setReplaceOpen(true)}
            onClear={() => setClearOpen(true)}
            onDelete={() => setDeleteTargetId(selectedAccount.providerAccountId)}
          />
        ) : providerAccountId ? (
          <SettingsSectionInset>
            <p {...stylex.props(styles.mutedText)}>This provider account no longer exists.</p>
          </SettingsSectionInset>
        ) : undefined}
      />
      <AddProviderAccountDialog
        catalog={catalog}
        open={addOpen}
        mutationSaving={mutationSaving}
        mutationError={mutationError}
        authAttempt={authAttempt}
        onOpenChange={setAddOpen}
        onCreateProviderAccount={async (input) => {
          const response = await onCreateProviderAccount(input);
          setAddOpen(false);
          const created = response.data?.createProviderAccount;
          if (created) void navigate({
            to: "/settings/system/providers/$providerAccountId",
            params: { providerAccountId: created.providerAccountId }
          });
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
          void navigate({ to: "/settings/system/providers" });
        }}
      />
      <SettingsEditDialog
        title={`Replace API key for ${selectedAccount?.displayName ?? "provider"}`}
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
          if (!selectedAccount || !canSaveSecret) return;
          await onSaveProviderSecret({
            providerAccountId: selectedAccount.providerAccountId,
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
      <DeleteConfirmationDialog
        title={`Clear API key for ${selectedAccount?.displayName ?? "provider"}?`}
        message="Noema cannot use this account until you add a key again."
        confirmLabel="Clear key"
        open={clearOpen}
        submitting={mutationSaving}
        error={mutationError}
        onOpenChange={(open) => {
          if (!mutationSaving) setClearOpen(open);
        }}
        onConfirm={() => {
          if (!selectedAccount) return;
          void onClearProviderSecret({ providerAccountId: selectedAccount.providerAccountId })
            .then(() => setClearOpen(false))
            .catch(() => undefined);
        }}
      />
    </>
  );
}

function ProviderAccountList({
  accounts,
  catalog,
  selectedProviderAccountId,
  loading,
  error,
  onRetry
}: {
  accounts: readonly ProviderSettingsAccount[];
  catalog: readonly ProviderAccountCatalogEntry[];
  selectedProviderAccountId?: string;
  loading: boolean;
  error: string | null;
  onRetry: () => void;
}) {
  if (loading) return <p {...stylex.props(styles.mutedText)}>Loading provider accounts...</p>;
  if (error && accounts.length === 0) return (
    <SettingsSection title="Provider accounts" titleId="provider-load-error">
      <SettingsSectionInset>
        <p role="alert" {...stylex.props(styles.saveError)}>Provider accounts could not be loaded.</p>
        <Button type="button" size="sm" variant="secondary" label="Retry" onClick={onRetry} />
      </SettingsSectionInset>
    </SettingsSection>
  );
  if (accounts.length === 0) return (
    <SettingsSection title="No provider accounts" titleId="provider-empty">
      <SettingsSectionInset>
        <p {...stylex.props(styles.mutedText)}>Add a provider account to make models and web tools available.</p>
      </SettingsSectionInset>
    </SettingsSection>
  );
  return <VStack gap={4}>
    {error ? <SettingsSection title="Provider accounts" titleId="provider-stale">
      <SettingsSectionInset>
        <HStack gap={2} wrap="wrap" vAlign="center">
          <p role="alert" {...stylex.props(styles.saveError)}>Provider accounts could not refresh.</p>
          <Button type="button" size="sm" variant="secondary" label="Retry" onClick={onRetry} />
        </HStack>
      </SettingsSectionInset>
    </SettingsSection> : null}
    {groupProviderAccounts(accounts, catalog).map(({ providerKind, providerName, accounts: group }) => (
      <VStack as="section" key={providerKind} gap={1.5} aria-labelledby={`provider-${providerKind}`}>
        <h2 id={`provider-${providerKind}`} {...stylex.props(styles.groupTitle)}>{providerName}</h2>
        {group.map((account) => (
          <ListCardLink
            key={account.providerAccountId}
            to="/settings/system/providers/$providerAccountId"
            params={{ providerAccountId: account.providerAccountId }}
            selected={selectedProviderAccountId === account.providerAccountId}
            aria-current={selectedProviderAccountId === account.providerAccountId ? "page" : undefined}
            xstyle={styles.accountCard}
          >
            <Avatar name={account.displayName} size="sm" tooltip={false} />
            <VStack gap={0.5} {...stylex.props(styles.cardCopy)}>
              <HStack gap={1} wrap="wrap" vAlign="center">
                <strong {...stylex.props(styles.cardTitle)}>{account.displayName}</strong>
                {account.isDefault ? <Badge variant="neutral" label="Default" /> : null}
              </HStack>
              {providerStatusIssue(account) ? (
                <span {...stylex.props(styles.cardIssue)}>{providerStatusIssue(account)}</span>
              ) : null}
            </VStack>
            <ChevronRight aria-hidden="true" {...stylex.props(styles.chevron)} />
          </ListCardLink>
        ))}
      </VStack>
    ))}
  </VStack>;
}

function ProviderAccountDetail({
  account,
  catalogEntry,
  busy,
  supportsSecret,
  fastModeSaving,
  fastModeError,
  onFastModeChange,
  onReplace,
  onClear,
  onDelete
}: {
  account: ProviderSettingsAccount;
  catalogEntry?: ProviderAccountCatalogEntry;
  busy: boolean;
  supportsSecret: boolean;
  fastModeSaving: boolean;
  fastModeError: string | null;
  onFastModeChange: (enabled: boolean) => Promise<void>;
  onReplace: () => void;
  onClear: () => void;
  onDelete: () => void;
}) {
  const capabilities = account.capabilities.length > 0 ? account.capabilities : catalogEntry?.capabilities ?? [];
  const accessAction = supportsSecret ? (
    <MoreMenu
      label={`Access actions for ${account.displayName}`}
      size="sm"
      isDisabled={busy}
      items={[
        { label: "Replace key", icon: <KeyRound aria-hidden="true" size={16} />, onClick: onReplace },
        { label: "Clear key", icon: <Trash2 aria-hidden="true" size={16} />, onClick: onClear }
      ]}
    />
  ) : undefined;
  return <>
    <VStack gap={0.5} {...stylex.props(styles.detailHeader)}>
      <span {...stylex.props(styles.eyebrow)}>{catalogEntry?.displayName ?? account.providerKind}</span>
      <h1 {...stylex.props(styles.detailTitle)}>{account.displayName}</h1>
      {providerStatusIssue(account) ? <p {...stylex.props(styles.cardIssue)}>{providerStatusIssue(account)}</p> : null}
    </VStack>
    <SettingsSection title="Access" titleId="provider-access" action={accessAction}>
      <SettingsList density="balanced" hasDividers>
        <SettingsListItem label="Authentication" description={providerAuthMethodLabel(account.authMethod)} />
        <SettingsListItem label="Status" description={providerStatusLabel(account.status)} />
        <SettingsListItem label="Default account" description={account.isDefault ? "Yes" : "No"} />
      </SettingsList>
      {account.providerKind === "foundation_local" ? (
        <SettingsSectionInset divided>
          <p {...stylex.props(styles.mutedText)}>Apple model support is managed by this machine. Agent model choices remain in Agents.</p>
        </SettingsSectionInset>
      ) : null}
    </SettingsSection>
    {account.fastMode !== null ? (
      <SettingsSection title="Performance" titleId="provider-performance">
        <SettingsList density="balanced">
          <SettingsListItem
            label="Fast mode"
            description="Faster responses cost more."
            endContent={
              <Switch
                label="Fast mode"
                isLabelHidden
                value={account.fastMode}
                isLoading={fastModeSaving}
                changeAction={onFastModeChange}
              />
            }
          />
        </SettingsList>
        {fastModeError ? (
          <SettingsLocalFeedback>
            <p role="alert" {...stylex.props(styles.saveError)}>Fast mode could not be changed.</p>
          </SettingsLocalFeedback>
        ) : null}
      </SettingsSection>
    ) : null}
    <SettingsSection title="Capabilities" titleId="provider-capabilities">
      {capabilities.length > 0 ? <SettingsList density="balanced" hasDividers>
        {capabilities.map((capability) => (
          <SettingsListItem
            key={capability.capabilityId}
            label={capability.capabilityId}
            description={`${capability.reliabilityContract} · ${capability.dataFlowClass}`}
          />
        ))}
      </SettingsList> : <SettingsSectionInset>
        <p {...stylex.props(styles.mutedText)}>No provider capabilities are reported.</p>
      </SettingsSectionInset>}
      <SettingsTechnicalDetails>
        <SettingsList density="compact">
          {providerTechnicalRows(account).map((row) => (
            <SettingsListItem key={row.label} label={row.label} description={row.value} />
          ))}
        </SettingsList>
      </SettingsTechnicalDetails>
    </SettingsSection>
    {!account.isDefault ? (
      <SettingsSection
        title="Lifecycle"
        titleId="provider-lifecycle"
        action={<Button type="button" size="sm" variant="destructive" label="Delete account" isDisabled={busy} onClick={onDelete} />}
      >
        <SettingsSectionInset>
          <p {...stylex.props(styles.mutedText)}>Deleting this account removes its stored credentials and web tool selections.</p>
        </SettingsSectionInset>
      </SettingsSection>
    ) : null}
  </>;
}

function groupProviderAccounts(
  accounts: readonly ProviderSettingsAccount[],
  catalog: readonly ProviderAccountCatalogEntry[]
) {
  const groups = new Map<string, ProviderSettingsAccount[]>();
  for (const account of accounts) groups.set(account.providerKind, [...(groups.get(account.providerKind) ?? []), account]);
  return [...groups].map(([providerKind, group]) => ({
    providerKind,
    providerName: catalog.find((entry) => entry.providerKind === providerKind)?.displayName ?? providerKind,
    accounts: group
  }));
}

function providerStatusIssue(account: ProviderSettingsAccount) {
  return account.status === "AUTHENTICATED" ? null : providerStatusLabel(account.status);
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
  groupTitle: {
    margin: "var(--spacing-0) var(--spacing-1)",
    color: "var(--foreground)",
    fontFamily: "var(--font-heading)",
    fontSize: 15,
    fontWeight: 700,
    lineHeight: 1.3
  },
  accountCard: {
    gridTemplateColumns: "auto minmax(0, 1fr) auto",
    alignItems: "center",
    gap: "var(--spacing-2)",
    minHeight: "var(--spacing-12)"
  },
  cardCopy: { minWidth: 0 },
  cardTitle: { color: "var(--foreground)", fontSize: 13, fontWeight: 650, overflowWrap: "anywhere" },
  cardIssue: { margin: 0, color: "var(--destructive)", fontSize: 12, lineHeight: 1.35 },
  chevron: { width: "var(--spacing-4)", height: "var(--spacing-4)", color: "var(--muted-foreground)" },
  detailHeader: { minWidth: 0, paddingBlockEnd: "var(--spacing-3)" },
  eyebrow: { color: "var(--muted-foreground)", fontSize: 12, lineHeight: 1.2 },
  detailTitle: {
    margin: 0,
    color: "var(--foreground)",
    fontFamily: "var(--font-heading)",
    fontSize: 18,
    fontWeight: 650,
    lineHeight: 1.25,
    overflowWrap: "anywhere"
  },
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
  selectorField: { display: "grid", gap: "var(--spacing-1-5)" },
  fieldLabel: { color: "var(--foreground)", fontSize: 13, fontWeight: 500 },
  warningText: {
    margin: "var(--spacing-0)",
    color: "var(--foreground)",
    fontSize: 13,
    lineHeight: 1.5
  },
  warningIcon: { width: 16, height: 16, marginTop: "var(--spacing-0-5)", color: "var(--destructive)" },
  icon: { width: 16, height: 16 }
});

import { useMutation, useQuery } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import { FileInput } from "@astryxdesign/core/FileInput";
import { FormLayout } from "@astryxdesign/core/FormLayout";
import { HStack } from "@astryxdesign/core/HStack";
import { Switch } from "@astryxdesign/core/Switch";
import { TextInput } from "@astryxdesign/core/TextInput";
import { VStack } from "@astryxdesign/core/VStack";
import * as stylex from "@stylexjs/stylex";
import { KeyRound, RefreshCw, Trash2 } from "lucide-react";
import { useState } from "react";
import { useWebPush } from "@/pwa/WebPushContext";
import {
  ApnsProviderStatusDocument,
  ConfigureApnsProviderDocument,
  RemoveApnsProviderDocument,
  type ApnsProviderStatusQuery,
  type ConfigureApnsProviderInput,
  type ConfigureApnsProviderMutation,
  type ConfigureApnsProviderMutationVariables,
  type RemoveApnsProviderMutation,
  type RemoveApnsProviderMutationVariables
} from "@/generated/graphql";
import { DeleteConfirmationDialog } from "./DeleteConnectionDialog";
import { SettingsEditDialog } from "./SettingsEditDialog";
import {
  SettingsList,
  SettingsListItem,
  SettingsLocalFeedback,
  SettingsSection,
  SettingsSectionInset,
  SettingsTechnicalDetails
} from "./SettingsPrimitives";

type ApnsProviderStatus = NonNullable<ApnsProviderStatusQuery["apnsProviderStatus"]>;

const dateFormatter = new Intl.DateTimeFormat(undefined, {
  dateStyle: "medium",
  timeStyle: "short"
});

export function NotificationsSettingsPane() {
  return (
    <VStack gap={4}>
      <WebPushSettings />
      <ApnsProviderSettings />
    </VStack>
  );
}

function ApnsProviderSettings() {
  const result = useQuery<ApnsProviderStatusQuery>(ApnsProviderStatusDocument, {
    fetchPolicy: "cache-and-network"
  });
  const [configureApns, configureResult] = useMutation<
    ConfigureApnsProviderMutation,
    ConfigureApnsProviderMutationVariables
  >(ConfigureApnsProviderDocument, {
    update(cache, response) {
      const next = response.data?.configureApnsProvider;
      if (next) cache.writeQuery({
        query: ApnsProviderStatusDocument,
        data: { apnsProviderStatus: next }
      });
    }
  });
  const [removeApns, removeResult] = useMutation<
    RemoveApnsProviderMutation,
    RemoveApnsProviderMutationVariables
  >(RemoveApnsProviderDocument, {
    update(cache, response) {
      const next = response.data?.removeApnsProvider;
      if (next) cache.writeQuery({
        query: ApnsProviderStatusDocument,
        data: { apnsProviderStatus: next }
      });
    }
  });
  const [configureOpen, setConfigureOpen] = useState(false);
  const [removeOpen, setRemoveOpen] = useState(false);
  const status = result.data?.apnsProviderStatus ?? null;
  const loading = result.loading && !result.data;
  const busy = configureResult.loading || removeResult.loading;
  const queryError = result.error ? "Apple push settings could not be loaded." : null;
  const configureError = configureResult.error
    ? "The APNs provider could not be configured. Check the key and try again."
    : null;
  const removeError = removeResult.error
    ? "The APNs provider could not be removed. Refresh and try again."
    : null;
  const mutationError = configureError ?? removeError;

  const openConfigure = () => {
    configureResult.reset();
    removeResult.reset();
    setConfigureOpen(true);
  };

  const openRemove = () => {
    configureResult.reset();
    removeResult.reset();
    setRemoveOpen(true);
  };

  const configure = async (input: ConfigureApnsProviderInput) => {
    await configureApns({ variables: { input } });
    setConfigureOpen(false);
  };

  const remove = async () => {
    try {
      await removeApns({ variables: { expectedRevision: status?.revision ?? 0 } });
      setRemoveOpen(false);
    } catch {
      // Keep the confirmation open so the mutation error can be retried or dismissed.
    }
  };

  return (
    <SettingsSection
      title="Apple device delivery"
      titleId="apns-provider-settings-title"
      action={<Button
        type="button"
        variant="secondary"
        size="sm"
        label={status?.configured ? "Rotate key" : "Configure"}
        icon={<KeyRound {...stylex.props(styles.icon)} aria-hidden="true" />}
        isDisabled={busy || loading}
        onClick={openConfigure}
      />}
    >
        {loading ? (
          <SettingsSectionInset><p {...stylex.props(styles.mutedText)}>Loading Apple push settings...</p></SettingsSectionInset>
        ) : queryError && !status ? (
          <SettingsSectionInset>
            <p role="alert" {...stylex.props(styles.errorText)}>{queryError}</p>
            <Button
              type="button"
              variant="secondary"
              size="sm"
              label="Retry"
              icon={<RefreshCw {...stylex.props(styles.icon)} aria-hidden="true" />}
              onClick={() => void result.refetch()}
            />
          </SettingsSectionInset>
        ) : (
          <>
            {queryError && status ? <SettingsLocalFeedback>
              <HStack gap={2} wrap="wrap" vAlign="center">
                <p role="alert" {...stylex.props(styles.errorText)}>Apple push settings could not refresh.</p>
                <Button type="button" variant="secondary" size="sm" label="Retry" icon={<RefreshCw {...stylex.props(styles.icon)} aria-hidden="true" />} onClick={() => void result.refetch()} />
              </HStack>
            </SettingsLocalFeedback> : null}
            {mutationError && !configureOpen && !removeOpen ? (
              <SettingsLocalFeedback><p role="alert" {...stylex.props(styles.errorText)}>{mutationError}</p></SettingsLocalFeedback>
            ) : null}
            <ApnsProviderDetails status={status} busy={busy} onRemove={openRemove} />
          </>
        )}
        <ApnsConfigureDialog
          key={configureOpen ? "open" : "closed"}
          status={status}
          open={configureOpen}
          saving={configureResult.loading}
          error={configureError}
          onOpenChange={setConfigureOpen}
          onSave={configure}
        />
        <DeleteConfirmationDialog
          title="Remove Apple Push Notifications?"
          message="Removes the server APNs credential. Paired Apple clients stop receiving notifications until you configure it again."
          confirmLabel="Remove provider"
          open={removeOpen}
          submitting={removeResult.loading}
          error={removeError}
          onOpenChange={setRemoveOpen}
          onConfirm={() => void remove()}
        />
    </SettingsSection>
  );
}

function ApnsProviderDetails({
  status,
  busy,
  onRemove
}: {
  status: ApnsProviderStatus | null;
  busy: boolean;
  onRemove: () => void;
}) {
  const configured = status?.configured ?? false;
  if (!configured) return <SettingsSectionInset>
    <p {...stylex.props(styles.mutedText)}>Configure this server to send notifications to the Noema app on Apple devices.</p>
  </SettingsSectionInset>;
  return <>
      {status?.lastErrorCode || status?.lastErrorAt ? <SettingsList density="balanced" hasDividers>
        <MetadataRow
          label="Delivery needs attention"
          value={`${status.lastErrorCode ?? "Unknown error"}${status.lastErrorAt ? ` · ${formatDate(status.lastErrorAt)}` : ""}`}
          error
        />
      </SettingsList> : null}
      <SettingsSectionInset divided>
        <HStack gap={2} wrap="wrap" vAlign="center" hAlign="between">
          <p {...stylex.props(styles.mutedText)}>Removing this credential stops notifications for paired Apple clients.</p>
          <Button type="button" variant="destructive" size="sm" label="Remove" icon={<Trash2 {...stylex.props(styles.icon)} aria-hidden="true" />} isDisabled={busy} onClick={onRemove} />
        </HStack>
      </SettingsSectionInset>
      <SettingsTechnicalDetails>
        <SettingsList density="compact">
          <MetadataRow label="Team ID" value={status?.teamId} />
          <MetadataRow label="Key ID" value={status?.keyId} />
          <MetadataRow label="Topic" value={status?.topic} />
          <MetadataRow label="Key fingerprint" value={status?.keyFingerprint} />
          <MetadataRow label="Revision" value={String(status?.revision ?? 0)} />
          <MetadataRow label="Last updated" value={formatDate(status?.updatedAt)} />
        </SettingsList>
      </SettingsTechnicalDetails>
    </>;
}

function MetadataRow({
  label,
  value,
  error = false
}: {
  label: string;
  value: string | null | undefined;
  error?: boolean;
}) {
  return (
    <SettingsListItem
      label={label}
      description={
        <span {...stylex.props(styles.metadataValue, error && styles.errorValue)}>
          {value || "Not reported"}
        </span>
      }
    />
  );
}

function ApnsConfigureDialog({
  status,
  open,
  saving,
  error,
  onOpenChange,
  onSave
}: {
  status: ApnsProviderStatus | null;
  open: boolean;
  saving: boolean;
  error: string | null;
  onOpenChange: (open: boolean) => void;
  onSave: (input: ConfigureApnsProviderInput) => Promise<void>;
}) {
  const [teamId, setTeamId] = useState(status?.teamId ?? "");
  const [keyId, setKeyId] = useState(status?.keyId ?? "");
  const [keyFile, setKeyFile] = useState<File | null>(null);
  const [validationError, setValidationError] = useState<string | null>(null);

  const clearDraft = () => {
    setTeamId("");
    setKeyId("");
    setKeyFile(null);
  };

  const submit = async () => {
    if (!keyFile) {
      setValidationError("Choose the Apple .p8 key file before saving.");
      return;
    }

    let privateKeyPem = "";
    try {
      privateKeyPem = await keyFile.text();
    } catch {
      setValidationError("The .p8 key file could not be read.");
      clearDraft();
      return;
    }

    const input: ConfigureApnsProviderInput = {
      teamId: teamId.trim(),
      keyId: keyId.trim(),
      privateKeyPem,
      expectedRevision: status?.revision ?? 0
    };
    clearDraft();
    setValidationError(null);
    try {
      await onSave(input);
    } finally {
      privateKeyPem = "";
      clearDraft();
    }
  };

  return (
    <SettingsEditDialog
      title={status?.configured ? "Rotate APNs key" : "Configure APNs"}
      open={open}
      saving={saving}
      saveLabel={status?.configured ? "Rotate key" : "Configure provider"}
      saveDisabled={!teamId.trim() || !keyId.trim() || keyFile === null}
      error={validationError || error}
      width={560}
      onOpenChange={onOpenChange}
      onSave={submit}
    >
      <VStack gap={3}>
        <FormLayout>
          <TextInput
            label="Apple Team ID"
            value={teamId}
            hasAutoFocus
            isRequired
            isDisabled={saving}
            onChange={setTeamId}
          />
          <TextInput
            label="Key ID"
            value={keyId}
            isRequired
            isDisabled={saving}
            onChange={setKeyId}
          />
        </FormLayout>
        <FileInput
          label="APNs .p8 private key"
          description="Read as text for this request, then cleared from the form after submit."
          value={keyFile}
          onChange={(file) => setKeyFile(Array.isArray(file) ? file[0] ?? null : file)}
          accept=".p8,text/plain"
          maxSize={16 * 1024}
          isRequired
          isDisabled={saving}
          mode="dropzone"
        />
      </VStack>
    </SettingsEditDialog>
  );
}

function formatDate(value: string | null | undefined): string {
  if (!value) return "Not reported";
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? "Unknown date" : dateFormatter.format(date);
}

function WebPushSettings() {
  const webPush = useWebPush();
  const enabled = webPush.state === "enabled";
  const actionable = webPush.state === "enabled" || webPush.state === "disabled";
  const loading = webPush.state === "loading";

  return (
    <SettingsSection title="Device notifications" titleId="notification-settings-title">
        <SettingsList density="balanced" hasDividers>
          <SettingsListItem
            label={enabled ? "Notifications on" : "Notifications off"}
            endContent={
              <Switch
                label="Device notifications"
                isLabelHidden
                value={enabled}
                isDisabled={!actionable}
                isLoading={loading}
                disabledMessage={!actionable && !loading ? webPush.detail : undefined}
                changeAction={(checked) => checked ? webPush.enable() : webPush.disable()}
              />
            }
          />
        </SettingsList>
        <SettingsLocalFeedback><p {...stylex.props(styles.mutedText)}>{webPush.detail}</p></SettingsLocalFeedback>
        {webPush.error ? (
          <SettingsLocalFeedback><p role="alert" {...stylex.props(styles.errorText)}>{webPush.error}</p></SettingsLocalFeedback>
        ) : null}
    </SettingsSection>
  );
}

const styles = stylex.create({
  mutedText: {
    margin: "var(--spacing-0)",
    color: "var(--muted-foreground)",
    fontSize: 13,
    textWrap: "pretty",
    lineHeight: 1.5
  },
  errorText: {
    margin: "var(--spacing-0)",
    color: "var(--destructive)",
    fontSize: 13,
    lineHeight: 1.5
  },
  metadataValue: {
    color: "var(--foreground)",
    fontFamily: "var(--font-mono)",
    fontSize: 12,
    overflowWrap: "anywhere"
  },
  errorValue: {
    color: "var(--destructive)"
  },
  icon: {
    width: 14,
    height: 14
  }
});

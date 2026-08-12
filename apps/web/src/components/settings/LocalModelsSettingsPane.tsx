import * as stylex from "@stylexjs/stylex";
import { useMutation, useQuery, useSubscription } from "@apollo/client/react";
import { Avatar } from "@astryxdesign/core/Avatar";
import { Badge } from "@astryxdesign/core/Badge";
import { Button } from "@astryxdesign/core/Button";
import { HStack } from "@astryxdesign/core/HStack";
import { useMediaQuery } from "@astryxdesign/core/hooks";
import { Selector, type SelectorOptionType } from "@astryxdesign/core/Selector";
import { StatusDot } from "@astryxdesign/core/StatusDot";
import { TextInput } from "@astryxdesign/core/TextInput";
import { VStack } from "@astryxdesign/core/VStack";
import { useNavigate } from "@tanstack/react-router";
import { ChevronRight, CircleStop, Download, Play, RefreshCw, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { ListCardLink } from "@/components/ListCardLink";
import {
  ActivateLocalModelDocument,
  CancelLocalModelInstallDocument,
  ImportLocalModelDocument,
  InstallLocalModelDocument,
  LocalModelEventsDocument,
  LocalModelsSettingsDocument,
  RemoveLocalModelDocument,
  RetryLocalModelRuntimeDocument,
  type ImportLocalModelInput,
  type LocalModelsSettingsQuery
} from "@/generated/graphql";
import { DeleteConfirmationDialog } from "./DeleteConnectionDialog";
import {
  formatBytes,
  formatGigabytes,
  installationProgress,
  installationStatusLabel,
  isTransferActive,
  runtimeStatusLabel,
  type LocalModelCatalogItem,
  type LocalModelInstallationItem
} from "./localModelMetadata";
import { SettingsEditDialog } from "./SettingsEditDialog";
import { SettingsManagementLayout } from "./SettingsManagementLayout";
import {
  SettingsList,
  SettingsListItem,
  SettingsSection,
  SettingsSectionInset,
  SettingsTechnicalDetails
} from "./SettingsPrimitives";

type LocalModelSetupView = LocalModelsSettingsQuery["localModelSetup"];
type DefaultModelPreference = LocalModelsSettingsQuery["defaultModelPreference"];
type ImportKind = "LOCAL_FILE" | "PUBLIC_GGUF";

export function LocalModelsSettingsPane({
  installationId,
  modelId
}: {
  installationId?: string;
  modelId?: string;
}) {
  const navigate = useNavigate();
  const desktop = useMediaQuery("(min-width: 980px)");
  const result = useQuery<LocalModelsSettingsQuery>(LocalModelsSettingsDocument, {
    fetchPolicy: "cache-and-network"
  });
  const refetchQueries = [{ query: LocalModelsSettingsDocument }];
  const mutationOptions = { refetchQueries, awaitRefetchQueries: true };
  const [install, installResult] = useMutation(InstallLocalModelDocument, mutationOptions);
  const [importModel, importResult] = useMutation(ImportLocalModelDocument, mutationOptions);
  const [cancel, cancelResult] = useMutation(CancelLocalModelInstallDocument, mutationOptions);
  const [remove, removeResult] = useMutation(RemoveLocalModelDocument, mutationOptions);
  const [activate, activateResult] = useMutation(ActivateLocalModelDocument, mutationOptions);
  const [retryRuntime, retryResult] = useMutation(RetryLocalModelRuntimeDocument, mutationOptions);
  useSubscription(LocalModelEventsDocument, { onData: () => void result.refetch() });

  const setup = result.data?.localModelSetup ?? null;
  const catalog = result.data?.localModelCatalog ?? [];
  const installations = result.data?.localModelInstallations ?? [];
  const selectedInstallation = installations.find((item) => item.installationId === installationId) ?? null;
  const selectedCatalogModel = catalog.find((item) => item.modelId === modelId) ?? null;
  const selectedInstallationModel = catalog.find((item) => item.modelId === selectedInstallation?.modelId) ?? null;
  const mutationResults = [installResult, importResult, cancelResult, removeResult, activateResult, retryResult];
  const saving = mutationResults.some((mutation) => mutation.loading);
  const mutationError = mutationResults.find((mutation) => mutation.error)?.error?.message ?? null;
  const loading = result.loading && !result.data;
  const queryError = result.error ? "Local model settings could not be loaded." : null;
  const installedModelIds = new Set(installations.map((installation) => installation.modelId));
  const available = catalog.filter((model) => !installedModelIds.has(model.modelId));
  const activeInstallationId = installations.find((item) => item.isActive)?.installationId;
  const setupInstallationId = installations.find(
    (item) => item.installationId === setup?.installation?.installationId
  )?.installationId;
  const recommendedModelId = available.find(
    (item) => item.modelId === setup?.recommendedModel?.modelId
  )?.modelId;
  const defaultInstallationId = activeInstallationId
    ?? setupInstallationId
    ?? (recommendedModelId ? undefined : installations[0]?.installationId);
  const defaultModelId = activeInstallationId || setupInstallationId
    ? undefined
    : recommendedModelId ?? (installations[0] ? undefined : available[0]?.modelId);
  const [importOpen, setImportOpen] = useState(false);
  const [removeOpen, setRemoveOpen] = useState(false);

  useEffect(() => {
    if (!desktop || installationId || modelId) return;
    if (defaultInstallationId) {
      void navigate({
        to: "/settings/models/installations/$installationId",
        params: { installationId: defaultInstallationId },
        replace: true
      });
      return;
    }
    if (!defaultModelId) return;
    void navigate({
      to: "/settings/models/catalog/$modelId",
      params: { modelId: defaultModelId },
      replace: true
    });
  }, [defaultInstallationId, defaultModelId, desktop, installationId, modelId, navigate]);

  useEffect(() => {
    if (loading) return;
    const invalidInstallation = installationId && !selectedInstallation;
    const invalidCatalogModel = modelId && !selectedCatalogModel;
    if (invalidInstallation || invalidCatalogModel) {
      void navigate({ to: "/settings/models", replace: true });
    }
  }, [installationId, loading, modelId, navigate, selectedCatalogModel, selectedInstallation]);

  const installCatalogModel = async (model: LocalModelCatalogItem) => {
    try {
      const response = await install({ variables: { input: { modelId: model.modelId, file: model.selectedBuild?.file } } });
      const created = response.data?.installLocalModel;
      if (created) void navigate({
        to: "/settings/models/installations/$installationId",
        params: { installationId: created.installationId }
      });
    } catch {
      // Keep the selected model visible with its local mutation error.
    }
  };

  const importLocalModel = async (input: ImportLocalModelInput) => {
    const response = await importModel({ variables: { input } });
    const created = response.data?.importLocalModel;
    setImportOpen(false);
    if (created) void navigate({
      to: "/settings/models/installations/$installationId",
      params: { installationId: created.installationId }
    });
  };

  const removeInstallation = async () => {
    if (!selectedInstallation) return;
    try {
      await remove({ variables: { installationId: selectedInstallation.installationId } });
      setRemoveOpen(false);
      void navigate({ to: "/settings/models" });
    } catch {
      // Keep the confirmation open so the local error can be retried.
    }
  };

  return <>
    <SettingsManagementLayout
      title="Local Models"
      primaryAction={{ label: "Import model", onClick: () => setImportOpen(true) }}
      detailOpen={installationId !== undefined || modelId !== undefined}
      detailLabel="Manage local model"
      onDetailOpenChange={(open) => {
        if (!open) void navigate({ to: "/settings/models" });
      }}
      list={
        <LocalModelList
          installations={installations}
          available={available}
          selectedInstallationId={installationId}
          selectedModelId={modelId}
          loading={loading}
          error={queryError}
          onRetry={() => void result.refetch()}
        />
      }
      detail={selectedInstallation && setup ? (
        <InstallationDetail
          installation={selectedInstallation}
          model={selectedInstallationModel}
          setup={setup}
          defaultPreference={result.data?.defaultModelPreference ?? null}
          totalDiskBytes={totalDiskBytes(installations)}
          saving={saving}
          error={mutationError}
          onCancel={() => void cancel({ variables: { installationId: selectedInstallation.installationId } })}
          onActivate={() => void activate({ variables: { installationId: selectedInstallation.installationId } })}
          onRetryRuntime={() => void retryRuntime()}
          onRemove={() => setRemoveOpen(true)}
        />
      ) : selectedCatalogModel ? (
        <CatalogModelDetail
          model={selectedCatalogModel}
          saving={saving}
          error={installResult.error?.message ?? null}
          onInstall={() => void installCatalogModel(selectedCatalogModel)}
        />
      ) : installationId || modelId ? (
        <SettingsSectionInset>
          <p {...stylex.props(styles.mutedText)}>This local model no longer exists.</p>
        </SettingsSectionInset>
      ) : undefined}
    />
    <AdvancedImportDialog
      open={importOpen}
      saving={saving}
      error={importResult.error?.message ?? null}
      onOpenChange={setImportOpen}
      onImport={importLocalModel}
    />
    <DeleteConfirmationDialog
      title={selectedInstallation ? `Remove ${selectedInstallation.name}?` : "Remove model?"}
      message={selectedInstallation
        ? `This removes ${formatBytes(selectedInstallation.diskBytes)} from this device. You can install it again later.`
        : "This removes the model from this device."}
      confirmLabel="Remove model"
      open={removeOpen}
      submitting={removeResult.loading}
      error={removeOpen ? removeResult.error?.message ?? null : null}
      onOpenChange={(open) => {
        if (!removeResult.loading) setRemoveOpen(open);
      }}
      onConfirm={() => void removeInstallation()}
    />
  </>;
}

function LocalModelList({
  installations,
  available,
  selectedInstallationId,
  selectedModelId,
  loading,
  error,
  onRetry
}: {
  installations: readonly LocalModelInstallationItem[];
  available: readonly LocalModelCatalogItem[];
  selectedInstallationId?: string;
  selectedModelId?: string;
  loading: boolean;
  error: string | null;
  onRetry: () => void;
}) {
  if (loading) return <p {...stylex.props(styles.mutedText)}>Loading local models...</p>;
  if (error && installations.length === 0 && available.length === 0) return (
    <SettingsSection title="Local models" titleId="local-model-load-error">
      <SettingsSectionInset>
        <p role="alert" {...stylex.props(styles.errorText)}>{error}</p>
        <Button type="button" size="sm" variant="secondary" label="Retry" onClick={onRetry} />
      </SettingsSectionInset>
    </SettingsSection>
  );
  return <VStack gap={4}>
    {error ? <SettingsSection title="Local models" titleId="local-model-stale-error">
      <SettingsSectionInset>
        <HStack gap={2} wrap="wrap" vAlign="center">
          <p role="alert" {...stylex.props(styles.errorText)}>Local models could not refresh.</p>
          <Button type="button" size="sm" variant="secondary" label="Retry" onClick={onRetry} />
        </HStack>
      </SettingsSectionInset>
    </SettingsSection> : null}
    <VStack as="section" gap={1.5} aria-labelledby="device-models-title">
      <h2 id="device-models-title" {...stylex.props(styles.groupTitle)}>On this device</h2>
      {installations.length > 0 ? installations.map((installation) => (
        <InstallationCard
          key={installation.installationId}
          installation={installation}
          selected={selectedInstallationId === installation.installationId}
        />
      )) : <p {...stylex.props(styles.mutedText)}>No local models are on this device.</p>}
    </VStack>
    <VStack as="section" gap={1.5} aria-labelledby="available-models-title">
      <h2 id="available-models-title" {...stylex.props(styles.groupTitle)}>Available</h2>
      {available.length > 0 ? available.map((model) => (
        <CatalogModelCard key={model.modelId} model={model} selected={selectedModelId === model.modelId} />
      )) : <p {...stylex.props(styles.mutedText)}>No other curated models are available.</p>}
    </VStack>
  </VStack>;
}

function InstallationCard({ installation, selected }: { installation: LocalModelInstallationItem; selected: boolean }) {
  const statusVisible = installation.isActive || installation.status !== "INSTALLED";
  return (
    <ListCardLink
      to="/settings/models/installations/$installationId"
      params={{ installationId: installation.installationId }}
      selected={selected}
      aria-current={selected ? "page" : undefined}
      xstyle={styles.modelCard}
    >
      <Avatar name={installation.name} size="sm" tooltip={false} />
      <VStack gap={0.5} {...stylex.props(styles.cardCopy)}>
        <HStack gap={1} wrap="wrap" vAlign="center">
          <strong {...stylex.props(styles.cardTitle)}>{installation.name}</strong>
          {installation.isActive ? <Badge variant="info" label="Active model" /> : null}
        </HStack>
        {statusVisible && !installation.isActive ? (
          <HStack gap={1} vAlign="center">
            <StatusDot variant={installation.status === "FAILED" ? "error" : "neutral"} label={installationStatusLabel(installation.status)} />
            <span {...stylex.props(installation.status === "FAILED" ? styles.errorText : styles.cardMeta)}>
              {installationStatusLabel(installation.status)}
            </span>
          </HStack>
        ) : null}
      </VStack>
      <ChevronRight aria-hidden="true" {...stylex.props(styles.chevron)} />
    </ListCardLink>
  );
}

function CatalogModelCard({ model, selected }: { model: LocalModelCatalogItem; selected: boolean }) {
  return (
    <ListCardLink
      to="/settings/models/catalog/$modelId"
      params={{ modelId: model.modelId }}
      selected={selected}
      aria-current={selected ? "page" : undefined}
      xstyle={styles.modelCard}
    >
      <Avatar name={model.name} size="sm" tooltip={false} />
      <VStack gap={0.5} {...stylex.props(styles.cardCopy)}>
        <HStack gap={1} wrap="wrap" vAlign="center">
          <strong {...stylex.props(styles.cardTitle)}>{model.name}</strong>
          {model.isRecommended ? <Badge variant="info" label="Recommended" /> : null}
        </HStack>
        {!model.selectedBuild ? <span {...stylex.props(styles.errorText)}>Incompatible with this device</span> : null}
      </VStack>
      <ChevronRight aria-hidden="true" {...stylex.props(styles.chevron)} />
    </ListCardLink>
  );
}

function InstallationDetail({
  installation,
  model,
  setup,
  defaultPreference,
  totalDiskBytes,
  saving,
  error,
  onCancel,
  onActivate,
  onRetryRuntime,
  onRemove
}: {
  installation: LocalModelInstallationItem;
  model: LocalModelCatalogItem | null;
  setup: LocalModelSetupView;
  defaultPreference: DefaultModelPreference;
  totalDiskBytes: number;
  saving: boolean;
  error: string | null;
  onCancel: () => void;
  onActivate: () => void;
  onRetryRuntime: () => void;
  onRemove: () => void;
}) {
  const transferActive = isTransferActive(installation);
  const progress = installationProgress(installation);
  const affectsRuntime = installation.isActive || setup.installation?.installationId === installation.installationId;
  const installationAction = transferActive ? (
    <Button type="button" size="sm" variant="secondary" label="Cancel" icon={<CircleStop size={15} aria-hidden="true" />} isDisabled={saving} onClick={onCancel} />
  ) : installation.status === "INSTALLED" && !installation.isActive ? (
    <Button type="button" size="sm" label="Use this model" icon={<Play size={15} aria-hidden="true" />} isDisabled={saving} onClick={onActivate} />
  ) : undefined;
  return <>
    <VStack gap={0.5} {...stylex.props(styles.detailHeader)}>
      <span {...stylex.props(styles.eyebrow)}>Local model</span>
      <HStack gap={2} wrap="wrap" vAlign="center">
        <h1 {...stylex.props(styles.detailTitle)}>{installation.name}</h1>
        {installation.isActive ? <Badge variant="info" label="Active model" /> : null}
      </HStack>
      {installation.errorMessage ? <p role="alert" {...stylex.props(styles.errorText)}>{installation.errorMessage}</p> : null}
    </VStack>
    <SettingsSection title="Model" titleId="local-model-model">
      <SettingsList density="balanced" hasDividers>
        <SettingsListItem label="Hardware fit" description={model?.hardwareFit?.explanation ?? "No catalog hardware assessment is available."} />
        <SettingsListItem label="License" description={model?.license ?? "Not reported"} />
        <SettingsListItem label="Download" description={model?.selectedBuild ? formatGigabytes(model.selectedBuild.downloadGb) : "Imported model"} />
      </SettingsList>
    </SettingsSection>
    <SettingsSection title="Installation" titleId="local-model-installation" action={installationAction}>
      <SettingsList density="balanced" hasDividers>
        <SettingsListItem label="State" description={installationStatusLabel(installation.status)} />
        <SettingsListItem label="Disk usage" description={formatBytes(installation.diskBytes)} />
      </SettingsList>
      {progress !== null && transferActive ? <SettingsSectionInset divided>
        <progress {...stylex.props(styles.progress)} value={progress} max={1} />
        <span {...stylex.props(styles.mutedText)}>{formatBytes(installation.completedBytes)} of {formatBytes(installation.totalBytes ?? 0)}</span>
      </SettingsSectionInset> : null}
      {error ? <SettingsSectionInset divided><p role="alert" {...stylex.props(styles.errorText)}>{error}</p></SettingsSectionInset> : null}
      <SettingsTechnicalDetails>
        <SettingsList density="compact">
          <SettingsListItem label="Installation ID" description={installation.installationId} />
          <SettingsListItem label="Model ID" description={installation.modelId} />
          <SettingsListItem label="Source" description={installation.sourceKind.replaceAll("_", " ").toLowerCase()} />
          <SettingsListItem label="Filename" description={installation.file} />
          {installation.sha256 ? <SettingsListItem label="Digest" description={installation.sha256} /> : null}
          {installation.backend ? <SettingsListItem label="Backend" description={installation.backend} /> : null}
          <SettingsListItem label="Created" description={formatDate(installation.createdAt)} />
          <SettingsListItem label="Updated" description={formatDate(installation.updatedAt)} />
          {installation.errorCode ? <SettingsListItem label="Error code" description={installation.errorCode} /> : null}
        </SettingsList>
      </SettingsTechnicalDetails>
    </SettingsSection>
    {affectsRuntime ? <RuntimeSection
      setup={setup}
      defaultPreference={defaultPreference}
      totalDiskBytes={totalDiskBytes}
      saving={saving}
      onRetryRuntime={onRetryRuntime}
    /> : null}
    {!transferActive && !installation.isActive ? (
      <SettingsSection
        title="Lifecycle"
        titleId="local-model-lifecycle"
        action={<Button type="button" size="sm" variant="destructive" label="Remove model" icon={<Trash2 size={15} aria-hidden="true" />} isDisabled={saving} onClick={onRemove} />}
      >
        <SettingsSectionInset>
          <p {...stylex.props(styles.mutedText)}>Removing this installation deletes its local data from this device.</p>
        </SettingsSectionInset>
      </SettingsSection>
    ) : null}
  </>;
}

function CatalogModelDetail({
  model,
  saving,
  error,
  onInstall
}: {
  model: LocalModelCatalogItem;
  saving: boolean;
  error: string | null;
  onInstall: () => void;
}) {
  return <>
    <VStack gap={0.5} {...stylex.props(styles.detailHeader)}>
      <span {...stylex.props(styles.eyebrow)}>Available model</span>
      <h1 {...stylex.props(styles.detailTitle)}>{model.name}</h1>
      {!model.selectedBuild ? <p {...stylex.props(styles.errorText)}>This model has no compatible build for this device.</p> : null}
    </VStack>
    <SettingsSection
      title="Model"
      titleId="catalog-model"
      action={<Button type="button" size="sm" label="Install" icon={<Download size={15} aria-hidden="true" />} isDisabled={saving || !model.selectedBuild} onClick={onInstall} />}
    >
      <SettingsList density="balanced" hasDividers>
        <SettingsListItem label="Hardware fit" description={model.hardwareFit?.explanation ?? "No compatible build for this device."} />
        <SettingsListItem label="License" description={model.license} />
        <SettingsListItem label="Download" description={model.selectedBuild ? formatGigabytes(model.selectedBuild.downloadGb) : "Unavailable"} />
      </SettingsList>
      {error ? <SettingsSectionInset divided><p role="alert" {...stylex.props(styles.errorText)}>{error}</p></SettingsSectionInset> : null}
      <SettingsTechnicalDetails>
        <SettingsList density="compact">
          <SettingsListItem label="Model ID" description={model.modelId} />
          <SettingsListItem label="Repository" description={model.repo} />
          <SettingsListItem label="Revision" description={model.revision} />
          {model.selectedBuild ? <>
            <SettingsListItem label="Filename" description={model.selectedBuild.file} />
            <SettingsListItem label="Digest" description={model.selectedBuild.sha256} />
            <SettingsListItem label="Backends" description={model.selectedBuild.backends.join(", ")} />
          </> : null}
          {model.compatibleBackend ? <SettingsListItem label="Selected backend" description={model.compatibleBackend} /> : null}
        </SettingsList>
      </SettingsTechnicalDetails>
    </SettingsSection>
  </>;
}

function RuntimeSection({
  setup,
  totalDiskBytes,
  defaultPreference,
  saving,
  onRetryRuntime
}: {
  setup: LocalModelSetupView;
  totalDiskBytes: number;
  defaultPreference: DefaultModelPreference;
  saving: boolean;
  onRetryRuntime: () => void;
}) {
  return (
    <SettingsSection
      title="Runtime"
      titleId="local-model-runtime"
      action={setup.runtimeStatus === "FAILED" ? (
        <Button type="button" size="sm" variant="secondary" label="Retry runtime" icon={<RefreshCw size={15} aria-hidden="true" />} isDisabled={saving} onClick={onRetryRuntime} />
      ) : undefined}
    >
      <SettingsList density="balanced" hasDividers>
        <SettingsListItem label="State" description={runtimeStatusLabel(setup.runtimeStatus)} />
        <SettingsListItem label="System default" description={defaultPreference?.modelProfile ?? "No system default selected"} />
        <SettingsListItem label="Total disk usage" description={formatBytes(totalDiskBytes)} />
      </SettingsList>
    </SettingsSection>
  );
}

function AdvancedImportDialog({
  open,
  saving,
  error,
  onOpenChange,
  onImport
}: {
  open: boolean;
  saving: boolean;
  error: string | null;
  onOpenChange: (open: boolean) => void;
  onImport: (input: ImportLocalModelInput) => Promise<unknown>;
}) {
  const [kind, setKind] = useState<ImportKind>("LOCAL_FILE");
  const [name, setName] = useState("");
  const [localPath, setLocalPath] = useState("");
  const [repo, setRepo] = useState("");
  const [revision, setRevision] = useState("");
  const [file, setFile] = useState("");
  const [sha256, setSha256] = useState("");
  const canSubmit = name.trim().length > 0 && (
    kind === "LOCAL_FILE"
      ? localPath.trim().length > 0
      : repo.trim().length > 0 && revision.trim().length > 0 && file.trim().length > 0 && sha256.trim().length > 0
  );
  const sourceOptions: SelectorOptionType[] = [
    { value: "LOCAL_FILE", label: "Local file" },
    { value: "PUBLIC_GGUF", label: "Public Hugging Face GGUF" }
  ];

  const submit = async () => {
    if (!canSubmit) return;
    await onImport({
      name: name.trim(),
      sourceKind: kind,
      localPath: kind === "LOCAL_FILE" ? localPath.trim() : null,
      repo: kind === "PUBLIC_GGUF" ? repo.trim() : null,
      revision: kind === "PUBLIC_GGUF" ? revision.trim() : null,
      file: kind === "PUBLIC_GGUF" ? file.trim() : null,
      sha256: kind === "PUBLIC_GGUF" ? sha256.trim() : null,
      license: null
    });
    setName("");
    setLocalPath("");
    setRepo("");
    setRevision("");
    setFile("");
    setSha256("");
  };

  return (
    <SettingsEditDialog
      title="Advanced GGUF import"
      open={open}
      saving={saving}
      saveLabel="Import model"
      saveDisabled={!canSubmit}
      error={error}
      width={620}
      onOpenChange={onOpenChange}
      onSave={submit}
    >
      <VStack gap={3}>
        <p {...stylex.props(styles.mutedText)}>Imported models are selectable. Noema recommends only models from its bundled catalog.</p>
        <Selector label="Source" options={sourceOptions} value={kind} onChange={(value) => setKind(value as ImportKind)} />
        <TextInput hasAutoFocus label="Model name" value={name} onChange={setName} />
        {kind === "LOCAL_FILE" ? (
          <TextInput label="GGUF file path" value={localPath} placeholder="/path/to/model.gguf" onChange={setLocalPath} />
        ) : (
          <VStack gap={3}>
            <TextInput label="Repository" value={repo} placeholder="owner/repository" onChange={setRepo} />
            <TextInput label="Pinned revision" value={revision} placeholder="40-character commit" onChange={setRevision} />
            <TextInput label="GGUF filename" value={file} placeholder="model.gguf" onChange={setFile} />
            <TextInput label="SHA-256" value={sha256} placeholder="64-character digest" onChange={setSha256} />
          </VStack>
        )}
      </VStack>
    </SettingsEditDialog>
  );
}

function totalDiskBytes(installations: readonly LocalModelInstallationItem[]) {
  const bytesByDigest = new Map<string, number>();
  for (const installation of installations) {
    const key = installation.sha256 ?? installation.installationId;
    bytesByDigest.set(key, Math.max(bytesByDigest.get(key) ?? 0, installation.diskBytes));
  }
  return [...bytesByDigest.values()].reduce((sum, bytes) => sum + bytes, 0);
}

function formatDate(value: string) {
  return new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(new Date(value));
}

const styles = stylex.create({
  groupTitle: {
    margin: "var(--spacing-0)",
    color: "var(--muted-foreground)",
    fontFamily: "var(--font-heading)",
    fontSize: 13,
    fontWeight: 650,
    lineHeight: 1.3
  },
  modelCard: {
    gridTemplateColumns: "auto minmax(0, 1fr) auto",
    alignItems: "center",
    columnGap: "var(--spacing-2)"
  },
  cardCopy: { minWidth: 0 },
  cardTitle: { color: "var(--foreground)", fontSize: 14, overflowWrap: "anywhere" },
  cardMeta: { color: "var(--muted-foreground)", fontSize: 12 },
  chevron: { color: "var(--muted-foreground)", width: 16, height: 16 },
  detailHeader: { minWidth: 0, paddingBlockEnd: "var(--spacing-1)" },
  eyebrow: { color: "var(--muted-foreground)", fontSize: 12, fontWeight: 650 },
  detailTitle: {
    minWidth: 0,
    margin: "var(--spacing-0)",
    color: "var(--foreground)",
    fontFamily: "var(--font-heading)",
    fontSize: 20,
    fontWeight: 700,
    lineHeight: 1.2,
    overflowWrap: "anywhere"
  },
  mutedText: { margin: "var(--spacing-0)", color: "var(--muted-foreground)", fontSize: 13, lineHeight: 1.5 },
  errorText: { margin: "var(--spacing-0)", color: "var(--destructive)", fontSize: 12, lineHeight: 1.45, overflowWrap: "anywhere" },
  progress: { width: "100%", height: 6, accentColor: "var(--pine-500)" }
});

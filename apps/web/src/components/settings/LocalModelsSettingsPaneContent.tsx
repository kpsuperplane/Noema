import * as stylex from "@stylexjs/stylex";
import { Badge } from "@astryxdesign/core/Badge";
import { Button } from "@astryxdesign/core/Button";
import { HStack } from "@astryxdesign/core/HStack";
import { List, ListItem } from "@astryxdesign/core/List";
import { Section } from "@astryxdesign/core/Section";
import { Selector, type SelectorOptionType } from "@astryxdesign/core/Selector";
import { TextInput } from "@astryxdesign/core/TextInput";
import { VStack } from "@astryxdesign/core/VStack";
import { CircleStop, Download, Play, RefreshCw, Trash2, Upload } from "lucide-react";
import { useState } from "react";
import type { ImportLocalModelInput, LocalModelsSettingsQuery } from "@/generated/graphql";
import { ErrorMarker } from "../ErrorMarker";
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

type LocalModelSetupView = LocalModelsSettingsQuery["localModelSetup"];
type DefaultModelPreference = LocalModelsSettingsQuery["defaultModelPreference"];
type ImportKind = "LOCAL_FILE" | "PUBLIC_GGUF";

export function LocalModelsSettingsPaneContent({
  setup,
  catalog,
  installations,
  defaultPreference,
  loading,
  error,
  saving,
  saveError,
  onRetry,
  onInstall,
  onImport,
  onCancel,
  onRemove,
  onActivate,
  onRetryRuntime
}: {
  setup: LocalModelSetupView | null;
  catalog: readonly LocalModelCatalogItem[];
  installations: readonly LocalModelInstallationItem[];
  defaultPreference: DefaultModelPreference;
  loading: boolean;
  error: string | null;
  saving: boolean;
  saveError: string | null;
  onRetry: () => void;
  onInstall: (modelId: string, file?: string | null) => Promise<unknown>;
  onImport: (input: ImportLocalModelInput) => Promise<unknown>;
  onCancel: (installationId: string) => Promise<unknown>;
  onRemove: (installationId: string) => Promise<unknown>;
  onActivate: (installationId: string) => Promise<unknown>;
  onRetryRuntime: () => Promise<unknown>;
}) {
  const [importOpen, setImportOpen] = useState(false);

  if (loading) {
    return <p {...stylex.props(styles.mutedText)}>Loading local models...</p>;
  }

  if (error || !setup) {
    return (
      <Section variant="transparent" padding={0} aria-labelledby="local-runtime-title">
        <VStack gap={2}>
          <h2 id="local-runtime-title" {...stylex.props(styles.sectionTitle)}>Local runtime</h2>
          <p {...stylex.props(styles.mutedText)}>Local model settings could not be loaded.</p>
          {error ? <ErrorMarker message={error} /> : null}
          <Button
            type="button"
            variant="secondary"
            label="Retry"
            icon={<RefreshCw size={15} aria-hidden="true" />}
            {...stylex.props(styles.fitButton)}
            onClick={onRetry}
          />
        </VStack>
      </Section>
    );
  }

  const diskBytesByDigest = new Map<string, number>();
  for (const installation of installations) {
    const key = installation.sha256 ?? installation.installationId;
    diskBytesByDigest.set(key, Math.max(diskBytesByDigest.get(key) ?? 0, installation.diskBytes));
  }
  const totalDiskBytes = [...diskBytesByDigest.values()].reduce((sum, bytes) => sum + bytes, 0);
  const installedModelIds = new Set(
    installations
      .filter((installation) => installation.status !== "FAILED" && installation.status !== "CANCELLED")
      .map((installation) => installation.modelId)
  );
  const alternatives = catalog.filter((model) => !installedModelIds.has(model.modelId));

  return (
    <VStack gap={6} {...stylex.props(styles.stack)}>
      <RuntimeSection
        setup={setup}
        totalDiskBytes={totalDiskBytes}
        defaultPreference={defaultPreference}
        saving={saving}
        onRetryRuntime={onRetryRuntime}
      />
      <Section variant="transparent" padding={0} aria-labelledby="installed-models-title">
        <VStack gap={2}>
          <HStack wrap="wrap" gap={3} vAlign="center" hAlign="between">
            <h2 id="installed-models-title" {...stylex.props(styles.sectionTitle)}>Installed models</h2>
            <span {...stylex.props(styles.metric)}>{formatBytes(totalDiskBytes)} on disk</span>
          </HStack>
          {installations.length > 0 ? (
            <List density="balanced" hasDividers>
              {installations.map((installation) => (
                <InstallationRow
                  key={installation.installationId}
                  installation={installation}
                  saving={saving}
                  onCancel={onCancel}
                  onRemove={onRemove}
                  onActivate={onActivate}
                />
              ))}
            </List>
          ) : (
            <p {...stylex.props(styles.mutedText)}>No local models are installed yet.</p>
          )}
        </VStack>
      </Section>
      <Section variant="transparent" padding={0} aria-labelledby="curated-models-title">
        <VStack gap={2}>
          <h2 id="curated-models-title" {...stylex.props(styles.sectionTitle)}>Curated models</h2>
          {alternatives.length > 0 ? (
            <List density="balanced" hasDividers>
              {alternatives.map((model) => (
                <CatalogRow key={model.modelId} model={model} saving={saving} onInstall={onInstall} />
              ))}
            </List>
          ) : (
            <p {...stylex.props(styles.mutedText)}>Every compatible curated model is installed.</p>
          )}
        </VStack>
      </Section>
      <Section variant="transparent" padding={0} aria-labelledby="manual-imports-title">
        <VStack gap={2}>
          <h2 id="manual-imports-title" {...stylex.props(styles.sectionTitle)}>Manual imports</h2>
          <Button
            type="button"
            variant="secondary"
            label="Import GGUF"
            icon={<Upload size={15} aria-hidden="true" />}
            isDisabled={saving}
            {...stylex.props(styles.fitButton)}
            onClick={() => setImportOpen(true)}
          />
        </VStack>
      </Section>
      <AdvancedImportDialog
        open={importOpen}
        saving={saving}
        error={saveError}
        onOpenChange={setImportOpen}
        onImport={async (input) => {
          await onImport(input);
          setImportOpen(false);
        }}
      />
      {saveError ? <ErrorMarker message={saveError} /> : null}
    </VStack>
  );
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
  onRetryRuntime: () => Promise<unknown>;
}) {
  const statusVariant = setup.runtimeStatus === "RUNNING"
    ? "success"
    : setup.runtimeStatus === "FAILED"
      ? "error"
      : "neutral";
  return (
    <Section variant="transparent" padding={0} aria-labelledby="local-runtime-title">
      <VStack gap={2}>
        <HStack wrap="wrap" gap={3} vAlign="center" hAlign="between">
          <h2 id="local-runtime-title" {...stylex.props(styles.sectionTitle)}>Local runtime</h2>
          <Badge variant={statusVariant} label={runtimeStatusLabel(setup.runtimeStatus)} />
        </HStack>
        <List density="balanced" hasDividers>
          <ListItem
            label="Active model"
            description={setup.installation?.name ?? "No active local model"}
          />
          <ListItem label="Disk usage" description={formatBytes(totalDiskBytes)} />
          <ListItem
            label="System default"
            description={defaultPreference ? defaultPreference.modelProfile : "No system default selected"}
          />
        </List>
        {setup.runtimeStatus === "FAILED" ? (
          <Button
            type="button"
            variant="secondary"
            label="Retry runtime"
            icon={<RefreshCw size={15} aria-hidden="true" />}
            isDisabled={saving}
            {...stylex.props(styles.fitButton)}
            onClick={() => void onRetryRuntime()}
          />
        ) : null}
      </VStack>
    </Section>
  );
}

function InstallationRow({
  installation,
  saving,
  onCancel,
  onRemove,
  onActivate
}: {
  installation: LocalModelInstallationItem;
  saving: boolean;
  onCancel: (installationId: string) => Promise<unknown>;
  onRemove: (installationId: string) => Promise<unknown>;
  onActivate: (installationId: string) => Promise<unknown>;
}) {
  const progress = installationProgress(installation);
  const transferActive = isTransferActive(installation);
  return (
    <ListItem
      label={
        <HStack gap={2} vAlign="center" wrap="wrap">
          <span {...stylex.props(styles.rowLabel)}>{installation.name}</span>
          {installation.isActive ? <Badge variant="success" label="Active" /> : null}
          <Badge variant={installation.status === "FAILED" ? "error" : "neutral"} label={installationStatusLabel(installation.status)} />
        </HStack>
      }
      description={
        <VStack gap={1}>
          <span {...stylex.props(styles.monoText)}>{installation.file}</span>
          {progress !== null && installation.status !== "INSTALLED" ? (
            <VStack gap={1}>
              <progress {...stylex.props(styles.progress)} value={progress} max={1} />
              <span>{formatBytes(installation.completedBytes)} of {formatBytes(installation.totalBytes ?? 0)}</span>
            </VStack>
          ) : null}
          <span>{[installation.backend, formatBytes(installation.diskBytes), installation.sourceKind.replaceAll("_", " ").toLowerCase()].filter(Boolean).join(" · ")}</span>
          {installation.errorMessage ? <ErrorMarker message={installation.errorMessage} /> : null}
        </VStack>
      }
      endContent={
        <HStack wrap="wrap" gap={2} vAlign="center" {...stylex.props(styles.rowControl)}>
          {transferActive ? (
            <Button
              type="button"
              variant="secondary"
              size="sm"
              label="Cancel"
              icon={<CircleStop size={15} aria-hidden="true" />}
              isDisabled={saving}
              onClick={() => void onCancel(installation.installationId)}
            />
          ) : null}
          {installation.status === "INSTALLED" && !installation.isActive ? (
            <Button
              type="button"
              size="sm"
              label="Use this model"
              icon={<Play size={15} aria-hidden="true" />}
              isDisabled={saving}
              onClick={() => void onActivate(installation.installationId)}
            />
          ) : null}
          {!transferActive && !installation.isActive ? (
            <Button
              type="button"
              variant="destructive"
              size="sm"
              label="Remove"
              icon={<Trash2 size={15} aria-hidden="true" />}
              isDisabled={saving}
              onClick={() => void onRemove(installation.installationId)}
            />
          ) : null}
        </HStack>
      }
    />
  );
}

function CatalogRow({
  model,
  saving,
  onInstall
}: {
  model: LocalModelCatalogItem;
  saving: boolean;
  onInstall: (modelId: string, file?: string | null) => Promise<unknown>;
}) {
  return (
    <ListItem
      label={
        <HStack gap={2} vAlign="center" wrap="wrap">
          <span {...stylex.props(styles.rowLabel)}>{model.name}</span>
          {model.isRecommended ? <Badge variant="success" label="Recommended" /> : null}
          <Badge variant="neutral" label={model.license} />
        </HStack>
      }
      description={
        <VStack gap={1}>
          <span>{model.hardwareFit?.explanation ?? "No compatible build for this machine."}</span>
          <span {...stylex.props(styles.monoText)}>
            {[model.compatibleBackend, model.selectedBuild ? `${formatGigabytes(model.selectedBuild.downloadGb)} download` : null, model.selectedBuild?.file].filter(Boolean).join(" · ")}
          </span>
        </VStack>
      }
      endContent={
        <HStack wrap="wrap" gap={2} vAlign="center" {...stylex.props(styles.rowControl)}>
          <Button
            type="button"
            size="sm"
            label={`Install ${model.name}`}
            icon={<Download size={15} aria-hidden="true" />}
            isDisabled={saving || !model.selectedBuild}
            onClick={() => void onInstall(model.modelId, model.selectedBuild?.file)}
          />
        </HStack>
      }
    />
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
        <p {...stylex.props(styles.mutedText)}>
          Imported models are selectable, but Noema only recommends models from its bundled catalog.
        </p>
        <Selector
          label="Source"
          options={sourceOptions}
          value={kind}
          onChange={(value) => setKind(value as ImportKind)}
        />
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

const styles = stylex.create({
  stack: { minWidth: 0 },
  sectionTitle: {
    margin: "var(--spacing-0)",
    fontFamily: "var(--font-heading)",
    fontSize: 16,
    lineHeight: 1.3,
    color: "var(--foreground)"
  },
  mutedText: {
    margin: "var(--spacing-0)",
    color: "var(--muted-foreground)",
    fontSize: 13,
    lineHeight: 1.5
  },
  rowLabel: { color: "var(--foreground)", fontWeight: 650, overflowWrap: "anywhere" },
  metric: { color: "var(--muted-foreground)", fontFamily: "var(--font-mono)", fontSize: 12 },
  monoText: { overflowWrap: "anywhere", fontFamily: "var(--font-mono)", fontSize: 12, lineHeight: 1.45 },
  rowControl: {
    justifyContent: "flex-end",
    "@media (max-width: 620px)": {
      width: "100%",
      justifyContent: "flex-start",
      marginInlineStart: "0"
    }
  },
  progress: { width: "100%", height: 6, accentColor: "var(--pine-500)" },
  fitButton: { width: "fit-content" }
});

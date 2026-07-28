import { Badge } from "@astryxdesign/core/Badge";
import { Button } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import { CircleStop, Download, Play, RefreshCw, Trash2 } from "lucide-react";
import { useState } from "react";
import type {
  ImportLocalModelInput,
  LocalModelsSettingsQuery
} from "@/generated/graphql";
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

type LocalModelSetupView = LocalModelsSettingsQuery["localModelSetup"];
type DefaultModelPreference = LocalModelsSettingsQuery["defaultModelPreference"];

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
  if (loading) {
    return <p {...stylex.props(styles.mutedText)}>Loading local models...</p>;
  }

  if (error || !setup) {
    return (
      <section {...stylex.props(styles.card)}>
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
      </section>
    );
  }

  const diskBytesByDigest = new Map<string, number>();
  for (const installation of installations) {
    const key = installation.sha256 ?? installation.installationId;
    diskBytesByDigest.set(
      key,
      Math.max(diskBytesByDigest.get(key) ?? 0, installation.diskBytes)
    );
  }
  const totalDiskBytes = [...diskBytesByDigest.values()].reduce((sum, bytes) => sum + bytes, 0);
  const installedModelIds = new Set(
    installations
      .filter(
        (installation) =>
          installation.status !== "FAILED" && installation.status !== "CANCELLED"
      )
      .map((installation) => installation.modelId)
  );
  const alternatives = catalog.filter((model) => !installedModelIds.has(model.modelId));

  return (
    <div {...stylex.props(styles.stack)}>
      <RuntimeCard
        setup={setup}
        totalDiskBytes={totalDiskBytes}
        defaultPreference={defaultPreference}
        saving={saving}
        onRetryRuntime={onRetryRuntime}
      />

      <section {...stylex.props(styles.section)} aria-labelledby="installed-models-title">
        <div {...stylex.props(styles.sectionHeader)}>
          <div>
            <h2 id="installed-models-title" {...stylex.props(styles.sectionTitle)}>Installed</h2>
            <p {...stylex.props(styles.mutedText)}>
              Verified GGUF files stay in Noema's content-addressed model store.
            </p>
          </div>
          <span {...stylex.props(styles.metric)}>{formatBytes(totalDiskBytes)} on disk</span>
        </div>
        {installations.length > 0 ? (
          <div {...stylex.props(styles.cardList)}>
            {installations.map((installation) => (
              <InstallationCard
                key={installation.installationId}
                installation={installation}
                saving={saving}
                onCancel={onCancel}
                onRemove={onRemove}
                onActivate={onActivate}
              />
            ))}
          </div>
        ) : (
          <div {...stylex.props(styles.emptyCard)}>
            <p {...stylex.props(styles.mutedText)}>No local models are installed yet.</p>
          </div>
        )}
      </section>

      <section {...stylex.props(styles.section)} aria-labelledby="curated-models-title">
        <div>
          <h2 id="curated-models-title" {...stylex.props(styles.sectionTitle)}>Curated models</h2>
          <p {...stylex.props(styles.mutedText)}>
            Noema selects a compatible artifact using this machine's backend and memory.
          </p>
        </div>
        <div {...stylex.props(styles.cardList)}>
          {alternatives.map((model) => (
            <CatalogCard key={model.modelId} model={model} saving={saving} onInstall={onInstall} />
          ))}
          {alternatives.length === 0 ? (
            <div {...stylex.props(styles.emptyCard)}>
              <p {...stylex.props(styles.mutedText)}>Every compatible curated model is installed.</p>
            </div>
          ) : null}
        </div>
      </section>

      <AdvancedImport saving={saving} onImport={onImport} />
      {saveError ? <ErrorMarker message={saveError} /> : null}
    </div>
  );
}

function RuntimeCard({
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
  return (
    <section {...stylex.props(styles.runtimeCard)} aria-labelledby="local-runtime-title">
      <div {...stylex.props(styles.titleRow)}>
        <div>
          <p {...stylex.props(styles.eyebrow)}>llama.cpp</p>
          <h2 id="local-runtime-title" {...stylex.props(styles.cardTitle)}>Local runtime</h2>
        </div>
        <Badge
          variant={setup.runtimeStatus === "RUNNING" ? "success" : setup.runtimeStatus === "FAILED" ? "error" : "neutral"}
          label={runtimeStatusLabel(setup.runtimeStatus)}
        />
      </div>
      <div {...stylex.props(styles.runtimeMetrics)}>
        <span>{setup.installation?.name ?? "No active local model"}</span>
        <span>{formatBytes(totalDiskBytes)}</span>
        <span>
          {defaultPreference
            ? `System default: ${defaultPreference.modelProfile}`
            : "No system default selected"}
        </span>
      </div>
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
    </section>
  );
}

function InstallationCard({
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
    <article {...stylex.props(styles.card)}>
      <div {...stylex.props(styles.titleRow)}>
        <div {...stylex.props(styles.modelIdentity)}>
          <h3 {...stylex.props(styles.cardTitle)}>{installation.name}</h3>
          <p {...stylex.props(styles.monoText)}>{installation.file}</p>
        </div>
        <div {...stylex.props(styles.badges)}>
          {installation.isActive ? <Badge variant="success" label="Active" /> : null}
          <Badge
            variant={installation.status === "FAILED" ? "error" : "neutral"}
            label={installationStatusLabel(installation.status)}
          />
        </div>
      </div>
      {progress !== null && installation.status !== "INSTALLED" ? (
        <div {...stylex.props(styles.progressBlock)}>
          <progress {...stylex.props(styles.progress)} value={progress} max={1} />
          <span {...stylex.props(styles.mutedText)}>
            {formatBytes(installation.completedBytes)} of {formatBytes(installation.totalBytes ?? 0)}
          </span>
        </div>
      ) : null}
      <div {...stylex.props(styles.metadataRow)}>
        {installation.backend ? <span>{installation.backend}</span> : null}
        <span>{formatBytes(installation.diskBytes)}</span>
        <span>{installation.sourceKind.replaceAll("_", " ").toLowerCase()}</span>
      </div>
      {installation.errorMessage ? <ErrorMarker message={installation.errorMessage} /> : null}
      <div {...stylex.props(styles.actions)}>
        {transferActive ? (
          <Button
            type="button"
            variant="secondary"
            label="Cancel"
            icon={<CircleStop size={15} aria-hidden="true" />}
            isDisabled={saving}
            onClick={() => void onCancel(installation.installationId)}
          />
        ) : null}
        {installation.status === "INSTALLED" && !installation.isActive ? (
          <Button
            type="button"
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
            label="Remove"
            icon={<Trash2 size={15} aria-hidden="true" />}
            isDisabled={saving}
            onClick={() => void onRemove(installation.installationId)}
          />
        ) : null}
      </div>
    </article>
  );
}

function CatalogCard({
  model,
  saving,
  onInstall
}: {
  model: LocalModelCatalogItem;
  saving: boolean;
  onInstall: (modelId: string, file?: string | null) => Promise<unknown>;
}) {
  return (
    <article {...stylex.props(styles.card, model.isRecommended && styles.recommendedCard)}>
      <div {...stylex.props(styles.titleRow)}>
        <div {...stylex.props(styles.modelIdentity)}>
          <h3 {...stylex.props(styles.cardTitle)}>{model.name}</h3>
          <p {...stylex.props(styles.mutedText)}>{model.hardwareFit?.explanation ?? "No compatible build for this machine."}</p>
        </div>
        <div {...stylex.props(styles.badges)}>
          {model.isRecommended ? <Badge variant="success" label="Recommended" /> : null}
          <Badge variant="neutral" label={model.license} />
        </div>
      </div>
      <div {...stylex.props(styles.metadataRow)}>
        {model.compatibleBackend ? <span>{model.compatibleBackend}</span> : null}
        {model.selectedBuild ? <span>{formatGigabytes(model.selectedBuild.downloadGb)} download</span> : null}
        {model.selectedBuild ? <span>{model.selectedBuild.file}</span> : null}
      </div>
      <Button
        type="button"
        label={`Install ${model.name}`}
        icon={<Download size={15} aria-hidden="true" />}
        isDisabled={saving || !model.selectedBuild}
        {...stylex.props(styles.fitButton)}
        onClick={() => void onInstall(model.modelId, model.selectedBuild?.file)}
      />
    </article>
  );
}

function AdvancedImport({
  saving,
  onImport
}: {
  saving: boolean;
  onImport: (input: ImportLocalModelInput) => Promise<unknown>;
}) {
  const [kind, setKind] = useState<"LOCAL_FILE" | "PUBLIC_GGUF">("LOCAL_FILE");
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

  return (
    <details {...stylex.props(styles.advanced)}>
      <summary {...stylex.props(styles.advancedSummary)}>Advanced GGUF import</summary>
      <form
        {...stylex.props(styles.importForm)}
        onSubmit={(event) => {
          event.preventDefault();
          if (!canSubmit) {
            return;
          }
          void onImport({
            name: name.trim(),
            sourceKind: kind,
            localPath: kind === "LOCAL_FILE" ? localPath.trim() : null,
            repo: kind === "PUBLIC_GGUF" ? repo.trim() : null,
            revision: kind === "PUBLIC_GGUF" ? revision.trim() : null,
            file: kind === "PUBLIC_GGUF" ? file.trim() : null,
            sha256: kind === "PUBLIC_GGUF" ? sha256.trim() : null,
            license: null
          });
        }}
      >
        <p {...stylex.props(styles.mutedText)}>
          Imported models are selectable, but Noema only recommends models from its bundled catalog.
        </p>
        <div {...stylex.props(styles.importGrid)}>
          <label {...stylex.props(styles.field)}>
            <span {...stylex.props(styles.fieldLabel)}>Source</span>
            <select {...stylex.props(styles.input)} value={kind} onChange={(event) => setKind(event.currentTarget.value as typeof kind)}>
              <option value="LOCAL_FILE">Local file</option>
              <option value="PUBLIC_GGUF">Public Hugging Face GGUF</option>
            </select>
          </label>
          <label {...stylex.props(styles.field)}>
            <span {...stylex.props(styles.fieldLabel)}>Model name</span>
            <input {...stylex.props(styles.input)} value={name} onChange={(event) => setName(event.currentTarget.value)} />
          </label>
        </div>
        {kind === "LOCAL_FILE" ? (
          <label {...stylex.props(styles.field)}>
            <span {...stylex.props(styles.fieldLabel)}>GGUF file path</span>
            <input {...stylex.props(styles.input)} value={localPath} placeholder="/path/to/model.gguf" onChange={(event) => setLocalPath(event.currentTarget.value)} />
          </label>
        ) : (
          <div {...stylex.props(styles.importGrid)}>
            <TextField label="Repository" value={repo} placeholder="owner/repository" onChange={setRepo} />
            <TextField label="Pinned revision" value={revision} placeholder="40-character commit" onChange={setRevision} />
            <TextField label="GGUF filename" value={file} placeholder="model.gguf" onChange={setFile} />
            <TextField label="SHA-256" value={sha256} placeholder="64-character digest" onChange={setSha256} />
          </div>
        )}
        <Button type="submit" variant="secondary" label="Import model" isDisabled={saving || !canSubmit} {...stylex.props(styles.fitButton)} />
      </form>
    </details>
  );
}

function TextField({
  label,
  value,
  placeholder,
  onChange
}: {
  label: string;
  value: string;
  placeholder: string;
  onChange: (value: string) => void;
}) {
  return (
    <label {...stylex.props(styles.field)}>
      <span {...stylex.props(styles.fieldLabel)}>{label}</span>
      <input {...stylex.props(styles.input)} value={value} placeholder={placeholder} onChange={(event) => onChange(event.currentTarget.value)} />
    </label>
  );
}

const styles = stylex.create({
  stack: { display: "grid", gap: "var(--spacing-7)" },
  section: { display: "grid", gap: "var(--spacing-3)" },
  sectionHeader: { display: "flex", flexWrap: "wrap", justifyContent: "space-between", alignItems: "end", gap: "var(--spacing-3)" },
  sectionTitle: { margin: "var(--spacing-0)", fontFamily: "var(--font-heading)", fontSize: 18, lineHeight: 1.3, color: "var(--foreground)" },
  cardList: { display: "grid", gap: "var(--spacing-3)" },
  card: { display: "grid", gap: "calc(var(--spacing-3) + var(--spacing-0-5))", borderWidth: 1, borderStyle: "solid", borderColor: "var(--border-subtle)", borderRadius: 8, backgroundColor: "white", padding: "var(--spacing-4)" },
  recommendedCard: { borderColor: "var(--pine-100)", backgroundColor: "color-mix(in srgb, var(--pine-50) 42%, white)" },
  runtimeCard: { display: "grid", gap: "calc(var(--spacing-3) + var(--spacing-0-5))", borderRadius: 10, backgroundColor: "var(--surface-sunken)", padding: "calc(var(--spacing-4) + var(--spacing-0-5))" },
  emptyCard: { borderWidth: 1, borderStyle: "dashed", borderColor: "var(--border)", borderRadius: 8, padding: "var(--spacing-4)" },
  titleRow: { display: "flex", minWidth: 0, flexWrap: "wrap", justifyContent: "space-between", alignItems: "start", gap: "var(--spacing-3)" },
  modelIdentity: { display: "grid", minWidth: 0, gap: "var(--spacing-1)" },
  cardTitle: { margin: "var(--spacing-0)", fontFamily: "var(--font-heading)", fontSize: 18, lineHeight: 1.3, color: "var(--foreground)" },
  eyebrow: { margin: "var(--spacing-0)", fontFamily: "var(--font-mono)", fontSize: 10, letterSpacing: "0.12em", textTransform: "uppercase", color: "var(--text-accent)" },
  mutedText: { margin: "var(--spacing-0)", fontSize: 14, lineHeight: 1.5, color: "var(--muted-foreground)" },
  monoText: { margin: "var(--spacing-0)", overflowWrap: "anywhere", fontFamily: "var(--font-mono)", fontSize: 12, lineHeight: 1.45, color: "var(--muted-foreground)" },
  metric: { fontFamily: "var(--font-mono)", fontSize: 12, color: "var(--muted-foreground)" },
  badges: { display: "flex", flexWrap: "wrap", justifyContent: "flex-end", gap: "var(--spacing-2)" },
  metadataRow: { display: "flex", minWidth: 0, flexWrap: "wrap", gap: "var(--spacing-2)", fontFamily: "var(--font-mono)", fontSize: 12, color: "var(--muted-foreground)" },
  runtimeMetrics: { display: "flex", minWidth: 0, flexWrap: "wrap", gap: "var(--spacing-3)", fontSize: 13, color: "var(--muted-foreground)" },
  actions: { display: "flex", flexWrap: "wrap", gap: "var(--spacing-2)" },
  fitButton: { width: "fit-content" },
  progressBlock: { display: "grid", gap: "var(--spacing-1-5)" },
  progress: { width: "100%", height: 6, accentColor: "var(--pine-500)" },
  advanced: { borderTopWidth: 1, borderTopStyle: "solid", borderTopColor: "var(--border-subtle)", paddingTop: "calc(var(--spacing-3) + var(--spacing-0-5))" },
  advancedSummary: { width: "fit-content", fontWeight: 600, color: "var(--foreground)" },
  importForm: { display: "grid", gap: "calc(var(--spacing-3) + var(--spacing-0-5))", paddingTop: "var(--spacing-4)" },
  importGrid: { display: "grid", gridTemplateColumns: "repeat(2, minmax(0, 1fr))", gap: "var(--spacing-3)", "@media (max-width: 760px)": { gridTemplateColumns: "1fr" } },
  field: { display: "grid", gap: "var(--spacing-1-5)" },
  fieldLabel: { fontSize: 13, fontWeight: 500, color: "var(--muted-foreground)" },
  input: { width: "100%", minHeight: 38, borderWidth: 1, borderStyle: "solid", borderColor: "var(--border)", borderRadius: 6, backgroundColor: "white", paddingBlock: "calc(var(--spacing-1-5) + 1px)", paddingInline: "calc(var(--spacing-2) + var(--spacing-0-5))", font: "inherit", color: "var(--foreground)" }
});

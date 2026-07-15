import type {
  LocalModelCatalogEntryFieldsFragment,
  LocalModelInstallationFieldsFragment,
  LocalModelRuntimeStatus
} from "@/generated/graphql";

export type LocalModelCatalogItem = LocalModelCatalogEntryFieldsFragment;
export type LocalModelInstallationItem = LocalModelInstallationFieldsFragment;

export function formatGigabytes(value: number) {
  return `${value.toFixed(value >= 10 ? 0 : 1)} GB`;
}

export function formatBytes(value: number) {
  if (value <= 0) {
    return "0 bytes";
  }
  const units = ["bytes", "KB", "MB", "GB", "TB"];
  const unitIndex = Math.min(Math.floor(Math.log(value) / Math.log(1000)), units.length - 1);
  const scaled = value / 1000 ** unitIndex;
  return `${scaled.toFixed(unitIndex === 0 || scaled >= 10 ? 0 : 1)} ${units[unitIndex]}`;
}

export function installationProgress(installation: LocalModelInstallationItem) {
  if (!installation.totalBytes || installation.totalBytes <= 0) {
    return null;
  }
  return Math.min(1, installation.completedBytes / installation.totalBytes);
}

export function installationStatusLabel(status: LocalModelInstallationItem["status"]) {
  switch (status) {
    case "QUEUED":
      return "Queued";
    case "DOWNLOADING":
      return "Downloading";
    case "VERIFYING":
      return "Verifying";
    case "INSTALLED":
      return "Installed";
    case "CANCELLED":
      return "Cancelled";
    case "FAILED":
      return "Failed";
  }
}

export function runtimeStatusLabel(status: LocalModelRuntimeStatus) {
  switch (status) {
    case "INACTIVE":
      return "Inactive";
    case "STARTING":
      return "Starting";
    case "RUNNING":
      return "Running";
    case "STOPPING":
      return "Stopping";
    case "FAILED":
      return "Needs attention";
  }
}

export function isTransferActive(installation: LocalModelInstallationItem) {
  return (
    installation.status === "QUEUED" ||
    installation.status === "DOWNLOADING" ||
    installation.status === "VERIFYING"
  );
}

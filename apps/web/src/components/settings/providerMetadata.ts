import type { ProviderAccountsQuery } from "@/generated/graphql";
import { settingsStatusLabel } from "./settingsStatus";

export type ProviderSettingsAccount = ProviderAccountsQuery["providerAccounts"][number];
export type ProviderAccountCatalogEntry = ProviderAccountsQuery["providerAccountCatalog"][number];

export type ProviderMetadataRow = {
  label: string;
  value: string;
};

export function providerStatusLabel(status: ProviderSettingsAccount["status"]) {
  return settingsStatusLabel(status);
}

export function providerAuthMethodLabel(method: ProviderSettingsAccount["authMethod"]) {
  const labels: Record<string, string> = {
    external_manual: "External manual",
    none: "None",
    oauth_device_code: "OAuth device code",
    oauth_pkce: "OAuth with PKCE",
    secret_input: "Secret input"
  };
  return labels[method] ?? method;
}

function yesNo(value: boolean) {
  return value ? "Yes" : "No";
}

function optionalRow(label: string, value: string | null | undefined): ProviderMetadataRow[] {
  return value ? [{ label, value }] : [];
}

export function providerTechnicalRows(account: ProviderSettingsAccount): ProviderMetadataRow[] {
  return [
    { label: "Provider kind", value: account.providerKind },
    { label: "Account key", value: account.accountKey },
    { label: "Auth method", value: providerAuthMethodLabel(account.authMethod) },
    { label: "Status", value: providerStatusLabel(account.status) },
    { label: "Active", value: yesNo(account.isActive) },
    { label: "Default", value: yesNo(account.isDefault) },
    ...optionalRow("Last checked", account.lastCheckedAt),
    ...optionalRow("Last authenticated", account.lastAuthenticatedAt),
    ...optionalRow("Last error code", account.lastErrorCode),
    ...optionalRow("Last error message", account.lastErrorMessage)
  ];
}

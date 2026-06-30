import type { TrustedIdentitySettingsQuery } from "@/generated/graphql";

export type TrustedIdentitySelector =
  TrustedIdentitySettingsQuery["trustedIdentitySelectors"][number];

export type TrustedIdentityMetadataRow = {
  label: string;
  value: string;
};

export function trustedIdentityRows(
  selector: TrustedIdentitySelector
): TrustedIdentityMetadataRow[] {
  return [
    { label: "Kind", value: selector.selectorKind },
    { label: "Value", value: selector.normalizedValue },
    { label: "Effect", value: selector.effect },
    { label: "Owner scope", value: selector.ownerScopeId },
    { label: "Issuer", value: selector.issuerActorId }
  ];
}

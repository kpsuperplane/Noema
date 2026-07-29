import type {
  LocalModelSetupQuery,
  OnboardingStatusQuery,
  ChatBootQuery,
  ProviderAuthAttemptQuery,
  StartProviderAuthAttemptMutation
} from "../../generated/graphql";

export type OnboardingStatus = OnboardingStatusQuery["onboardingStatus"];
export type OnboardingProviderCatalog = ChatBootQuery["providerAccountCatalog"];
export type OnboardingConnectedAccount = ChatBootQuery["providerAccounts"][number];

export type ProviderAuthAttemptView =
  | StartProviderAuthAttemptMutation["startProviderAuthAttempt"]
  | NonNullable<ProviderAuthAttemptQuery["providerAuthAttempt"]>;

export type ProviderAccountStatus = NonNullable<OnboardingStatus["steps"][number]["providerAccountStatus"]>;

export type LocalModelSetupView = LocalModelSetupQuery["localModelSetup"];

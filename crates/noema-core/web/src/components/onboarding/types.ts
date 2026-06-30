import type { OnboardingStatusQuery, ProviderAuthAttemptQuery, StartProviderAuthAttemptMutation } from "../../generated/graphql";

export type OnboardingStatus = OnboardingStatusQuery["onboardingStatus"];

export type ProviderAuthAttemptView =
  | StartProviderAuthAttemptMutation["startProviderAuthAttempt"]
  | NonNullable<ProviderAuthAttemptQuery["providerAuthAttempt"]>;

export type ProviderAccountStatus = NonNullable<OnboardingStatus["steps"][number]["providerAccountStatus"]>;

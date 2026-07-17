import type { ReasoningEffort } from "@/generated/graphql";

export type ModelProfileOption = {
  id: string;
  label: string;
  disabledReason?: string | null;
  reasoningEfforts: readonly ReasoningEffort[];
  defaultReasoningEffort?: ReasoningEffort | null;
};

export type ModelProviderOption = {
  providerKind: string;
  providerAccountId: string;
  providerDisplayName: string;
  status: string;
  disabledReason?: string | null;
  defaultModelProfile?: string | null;
  profiles: readonly ModelProfileOption[];
};

export type ModelPreference = {
  providerKind: string;
  providerAccountId: string;
  modelProfile: string;
  reasoningEffort?: ReasoningEffort | null;
};

export type ModelPreferenceSaveInput = {
  providerAccountId: string;
  modelProfile: string;
  reasoningEffort?: ReasoningEffort | null;
};

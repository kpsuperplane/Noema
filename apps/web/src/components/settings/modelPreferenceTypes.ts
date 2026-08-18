import type {
  ModelPreferenceSelectionMode,
  NoemaModelUseCase,
  ReasoningEffort
} from "@/generated/graphql";

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
  profiles: readonly ModelProfileOption[];
  recommendations: readonly ModelRecommendation[];
};

export type ModelRecommendation = {
  useCase: NoemaModelUseCase;
  modelProfile: string;
  reasoningEffort?: ReasoningEffort | null;
  disabledReason?: string | null;
};

export type ModelPreference = {
  providerKind: string;
  providerAccountId: string;
  selectionMode: ModelPreferenceSelectionMode;
  modelProfile?: string | null;
  reasoningEffort?: ReasoningEffort | null;
  fastMode: boolean;
};

export type ModelPreferenceSaveInput = {
  providerAccountId: string;
  selectionMode: ModelPreferenceSelectionMode;
  modelProfile?: string | null;
  reasoningEffort?: ReasoningEffort | null;
  fastMode: boolean;
};

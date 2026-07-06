export type ModelProfileOption = {
  id: string;
  label: string;
  disabledReason?: string | null;
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
};

export type ModelPreferenceSaveInput = {
  providerAccountId: string;
  modelProfile: string;
};

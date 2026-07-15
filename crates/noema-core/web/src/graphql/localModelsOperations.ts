import { gql } from "@apollo/client";

export const LocalModelCatalogEntryFields = gql`
  fragment LocalModelCatalogEntryFields on LocalModelCatalogEntry {
    modelId
    name
    license
    priority
    repo
    revision
    isRecommended
    compatibleBackend
    selectedBuild {
      file
      sha256
      downloadGb
      backends
      minRamGb
      minVramGb
    }
    hardwareFit {
      backend
      ramGb
      vramGb
      unifiedMemory
      explanation
    }
  }
`;

export const LocalModelInstallationFields = gql`
  fragment LocalModelInstallationFields on LocalModelInstallation {
    installationId
    modelId
    name
    file
    sourceKind
    status
    sha256
    completedBytes
    totalBytes
    diskBytes
    backend
    isActive
    errorCode
    errorMessage
    createdAt
    updatedAt
  }
`;

export const LocalModelSetupDocument = gql`
  query LocalModelSetup {
    localModelSetup {
      isReady
      runtimeStatus
      recommendedModel {
        ...LocalModelCatalogEntryFields
      }
      installation {
        ...LocalModelInstallationFields
      }
    }
  }
  ${LocalModelCatalogEntryFields}
  ${LocalModelInstallationFields}
`;

export const LocalModelsSettingsDocument = gql`
  query LocalModelsSettings {
    localModelSetup {
      isReady
      runtimeStatus
      recommendedModel {
        ...LocalModelCatalogEntryFields
      }
      installation {
        ...LocalModelInstallationFields
      }
    }
    localModelCatalog {
      ...LocalModelCatalogEntryFields
    }
    localModelInstallations {
      ...LocalModelInstallationFields
    }
    defaultModelPreference {
      providerKind
      providerAccountId
      modelProfile
      reasoningEffort
    }
  }
  ${LocalModelCatalogEntryFields}
  ${LocalModelInstallationFields}
`;

export const InstallLocalModelDocument = gql`
  mutation InstallLocalModel($input: InstallLocalModelInput!) {
    installLocalModel(input: $input) {
      ...LocalModelInstallationFields
    }
  }
  ${LocalModelInstallationFields}
`;

export const ImportLocalModelDocument = gql`
  mutation ImportLocalModel($input: ImportLocalModelInput!) {
    importLocalModel(input: $input) {
      ...LocalModelInstallationFields
    }
  }
  ${LocalModelInstallationFields}
`;

export const CancelLocalModelInstallDocument = gql`
  mutation CancelLocalModelInstall($installationId: String!) {
    cancelLocalModelInstall(installationId: $installationId) {
      ...LocalModelInstallationFields
    }
  }
  ${LocalModelInstallationFields}
`;

export const RemoveLocalModelDocument = gql`
  mutation RemoveLocalModel($installationId: String!) {
    removeLocalModel(installationId: $installationId)
  }
`;

export const ActivateLocalModelDocument = gql`
  mutation ActivateLocalModel($installationId: String!) {
    activateLocalModel(installationId: $installationId) {
      ...LocalModelInstallationFields
    }
  }
  ${LocalModelInstallationFields}
`;

export const SaveDefaultModelPreferenceDocument = gql`
  mutation SaveDefaultModelPreference($input: SaveDefaultModelPreferenceInput!) {
    saveDefaultModelPreference(input: $input) {
      providerKind
      providerAccountId
      modelProfile
      reasoningEffort
    }
  }
`;

export const RetryLocalModelRuntimeDocument = gql`
  mutation RetryLocalModelRuntime {
    retryLocalModelRuntime
  }
`;

export const LocalModelEventsDocument = gql`
  subscription LocalModelEvents($after: String) {
    localModelEvents(after: $after) {
      cursor
      kind
      installationId
      modelId
      runtimeStatus
      createdAt
      installation {
        ...LocalModelInstallationFields
      }
    }
  }
  ${LocalModelInstallationFields}
`;

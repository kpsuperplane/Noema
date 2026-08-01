// @generated
// This file was automatically generated and should not be edited.

import ApolloAPI

nonisolated public protocol SelectionSet: ApolloAPI.SelectionSet & ApolloAPI.RootSelectionSet
where Schema == NoemaAPI.SchemaMetadata {}

nonisolated public protocol InlineFragment: ApolloAPI.SelectionSet & ApolloAPI.InlineFragment
where Schema == NoemaAPI.SchemaMetadata {}

nonisolated public protocol MutableSelectionSet: ApolloAPI.MutableRootSelectionSet
where Schema == NoemaAPI.SchemaMetadata {}

nonisolated public protocol MutableInlineFragment: ApolloAPI.MutableSelectionSet & ApolloAPI.InlineFragment
where Schema == NoemaAPI.SchemaMetadata {}

nonisolated public enum SchemaMetadata: ApolloAPI.SchemaMetadata {
  public static let configuration: any ApolloAPI.SchemaConfiguration.Type = SchemaConfiguration.self

  private static let objectTypeMap: [String: ApolloAPI.Object] = [
    "A2UISurface": NoemaAPI.Objects.A2UISurface,
    "ActionReviewerSettings": NoemaAPI.Objects.ActionReviewerSettings,
    "Activity": NoemaAPI.Objects.Activity,
    "AdapterAuthenticationIntervention": NoemaAPI.Objects.AdapterAuthenticationIntervention,
    "AdapterConnection": NoemaAPI.Objects.AdapterConnection,
    "AdapterDefinition": NoemaAPI.Objects.AdapterDefinition,
    "AdapterOauthSetupAttempt": NoemaAPI.Objects.AdapterOauthSetupAttempt,
    "AdapterOperation": NoemaAPI.Objects.AdapterOperation,
    "AdapterResponseTransform": NoemaAPI.Objects.AdapterResponseTransform,
    "Agent": NoemaAPI.Objects.Agent,
    "AgentModelPreference": NoemaAPI.Objects.AgentModelPreference,
    "AgentModelProfileOption": NoemaAPI.Objects.AgentModelProfileOption,
    "AgentModelProviderOption": NoemaAPI.Objects.AgentModelProviderOption,
    "AgentModelRecommendation": NoemaAPI.Objects.AgentModelRecommendation,
    "AgentStatusEvent": NoemaAPI.Objects.AgentStatusEvent,
    "Artifact": NoemaAPI.Objects.Artifact,
    "ArtifactReference": NoemaAPI.Objects.ArtifactReference,
    "ArtifactVersion": NoemaAPI.Objects.ArtifactVersion,
    "ArtifactVersionDetail": NoemaAPI.Objects.ArtifactVersionDetail,
    "AssistantText": NoemaAPI.Objects.AssistantText,
    "AssistantTextDeltaEvent": NoemaAPI.Objects.AssistantTextDeltaEvent,
    "CapabilityConnection": NoemaAPI.Objects.CapabilityConnection,
    "CapabilityFeatures": NoemaAPI.Objects.CapabilityFeatures,
    "CapabilityIntegration": NoemaAPI.Objects.CapabilityIntegration,
    "CapabilityManagedTool": NoemaAPI.Objects.CapabilityManagedTool,
    "CapabilityManagedToolHint": NoemaAPI.Objects.CapabilityManagedToolHint,
    "Client": NoemaAPI.Objects.Client,
    "ConversationItem": NoemaAPI.Objects.ConversationItem,
    "ConversationItemEvent": NoemaAPI.Objects.ConversationItemEvent,
    "ConversationTranscriptPage": NoemaAPI.Objects.ConversationTranscriptPage,
    "ConversationTranscriptPageInfo": NoemaAPI.Objects.ConversationTranscriptPageInfo,
    "CurrentRunSummary": NoemaAPI.Objects.CurrentRunSummary,
    "DefaultModelPreference": NoemaAPI.Objects.DefaultModelPreference,
    "ErrorNotice": NoemaAPI.Objects.ErrorNotice,
    "GovernedAction": NoemaAPI.Objects.GovernedAction,
    "GraphqlNativeMemoryPage": NoemaAPI.Objects.GraphqlNativeMemoryPage,
    "GraphqlNativeMemoryPageRef": NoemaAPI.Objects.GraphqlNativeMemoryPageRef,
    "GraphqlNativeMemorySettings": NoemaAPI.Objects.GraphqlNativeMemorySettings,
    "GraphqlNativeMemorySourceReference": NoemaAPI.Objects.GraphqlNativeMemorySourceReference,
    "GraphqlNativeMemoryTree": NoemaAPI.Objects.GraphqlNativeMemoryTree,
    "GraphqlNativeMemoryUpdateResult": NoemaAPI.Objects.GraphqlNativeMemoryUpdateResult,
    "GraphqlNativeMemoryUpdateStatus": NoemaAPI.Objects.GraphqlNativeMemoryUpdateStatus,
    "GraphqlTaskRuntimeEvent": NoemaAPI.Objects.GraphqlTaskRuntimeEvent,
    "HumanInterventionsChangedEvent": NoemaAPI.Objects.HumanInterventionsChangedEvent,
    "LocalModelBuild": NoemaAPI.Objects.LocalModelBuild,
    "LocalModelCatalogEntry": NoemaAPI.Objects.LocalModelCatalogEntry,
    "LocalModelEvent": NoemaAPI.Objects.LocalModelEvent,
    "LocalModelHardwareFit": NoemaAPI.Objects.LocalModelHardwareFit,
    "LocalModelInstallation": NoemaAPI.Objects.LocalModelInstallation,
    "LocalModelSetup": NoemaAPI.Objects.LocalModelSetup,
    "LocalStatus": NoemaAPI.Objects.LocalStatus,
    "McpAuthenticationIntervention": NoemaAPI.Objects.McpAuthenticationIntervention,
    "McpOAuthSetupAttempt": NoemaAPI.Objects.McpOAuthSetupAttempt,
    "McpServer": NoemaAPI.Objects.McpServer,
    "McpSetupIntervention": NoemaAPI.Objects.McpSetupIntervention,
    "MultipleChoiceOption": NoemaAPI.Objects.MultipleChoiceOption,
    "MultipleChoicePrompt": NoemaAPI.Objects.MultipleChoicePrompt,
    "MultipleChoiceSelection": NoemaAPI.Objects.MultipleChoiceSelection,
    "MutationRoot": NoemaAPI.Objects.MutationRoot,
    "OnboardingModelSelection": NoemaAPI.Objects.OnboardingModelSelection,
    "OnboardingModelSelections": NoemaAPI.Objects.OnboardingModelSelections,
    "OnboardingModelSetup": NoemaAPI.Objects.OnboardingModelSetup,
    "OnboardingStatus": NoemaAPI.Objects.OnboardingStatus,
    "OnboardingStep": NoemaAPI.Objects.OnboardingStep,
    "PageInfo": NoemaAPI.Objects.PageInfo,
    "PrimaryConversation": NoemaAPI.Objects.PrimaryConversation,
    "PrivacySettings": NoemaAPI.Objects.PrivacySettings,
    "Project": NoemaAPI.Objects.Project,
    "ProjectCommandPayload": NoemaAPI.Objects.ProjectCommandPayload,
    "ProjectConnection": NoemaAPI.Objects.ProjectConnection,
    "ProjectEdge": NoemaAPI.Objects.ProjectEdge,
    "ProviderAccount": NoemaAPI.Objects.ProviderAccount,
    "ProviderAccountCatalogEntry": NoemaAPI.Objects.ProviderAccountCatalogEntry,
    "ProviderAuthAttempt": NoemaAPI.Objects.ProviderAuthAttempt,
    "ProviderCapability": NoemaAPI.Objects.ProviderCapability,
    "QueryRoot": NoemaAPI.Objects.QueryRoot,
    "SubscriptionReadyEvent": NoemaAPI.Objects.SubscriptionReadyEvent,
    "SubscriptionRoot": NoemaAPI.Objects.SubscriptionRoot,
    "TaskAttention": NoemaAPI.Objects.TaskAttention,
    "TaskAttentionConnection": NoemaAPI.Objects.TaskAttentionConnection,
    "TaskAttentionEdge": NoemaAPI.Objects.TaskAttentionEdge,
    "TaskCard": NoemaAPI.Objects.TaskCard,
    "TaskCommandPayload": NoemaAPI.Objects.TaskCommandPayload,
    "TaskConnection": NoemaAPI.Objects.TaskConnection,
    "TaskDetail": NoemaAPI.Objects.TaskDetail,
    "TaskEdge": NoemaAPI.Objects.TaskEdge,
    "TaskExecutionContract": NoemaAPI.Objects.TaskExecutionContract,
    "TaskExecutionPolicy": NoemaAPI.Objects.TaskExecutionPolicy,
    "TaskGate": NoemaAPI.Objects.TaskGate,
    "TaskMessage": NoemaAPI.Objects.TaskMessage,
    "TaskModelSnapshot": NoemaAPI.Objects.TaskModelSnapshot,
    "TaskReference": NoemaAPI.Objects.TaskReference,
    "TaskReview": NoemaAPI.Objects.TaskReview,
    "TaskReviewCriterion": NoemaAPI.Objects.TaskReviewCriterion,
    "TaskReviewSummary": NoemaAPI.Objects.TaskReviewSummary,
    "TaskRun": NoemaAPI.Objects.TaskRun,
    "TaskRunItem": NoemaAPI.Objects.TaskRunItem,
    "TaskRunItemConnection": NoemaAPI.Objects.TaskRunItemConnection,
    "TaskRunItemEdge": NoemaAPI.Objects.TaskRunItemEdge,
    "TaskSource": NoemaAPI.Objects.TaskSource,
    "TaskSubmission": NoemaAPI.Objects.TaskSubmission,
    "TaskSubmissionArtifact": NoemaAPI.Objects.TaskSubmissionArtifact,
    "TaskSubmissionCriterion": NoemaAPI.Objects.TaskSubmissionCriterion,
    "TaskSummary": NoemaAPI.Objects.TaskSummary,
    "TaskValidationCriterion": NoemaAPI.Objects.TaskValidationCriterion,
    "ToolBehavior": NoemaAPI.Objects.ToolBehavior,
    "TurnAccepted": NoemaAPI.Objects.TurnAccepted,
    "TurnCompletedEvent": NoemaAPI.Objects.TurnCompletedEvent,
    "UserText": NoemaAPI.Objects.UserText,
    "WebFetchSettings": NoemaAPI.Objects.WebFetchSettings,
    "WebFetchSummarizerSettings": NoemaAPI.Objects.WebFetchSummarizerSettings,
    "WebToolBindingSettings": NoemaAPI.Objects.WebToolBindingSettings,
    "WebToolProviderOption": NoemaAPI.Objects.WebToolProviderOption,
    "WebToolSettings": NoemaAPI.Objects.WebToolSettings,
    "WorkEvent": NoemaAPI.Objects.WorkEvent,
    "WorkOverview": NoemaAPI.Objects.WorkOverview,
    "WorkStageColumn": NoemaAPI.Objects.WorkStageColumn,
    "Workflow": NoemaAPI.Objects.Workflow,
    "WorkflowStage": NoemaAPI.Objects.WorkflowStage,
    "Workspace": NoemaAPI.Objects.Workspace
  ]

  @_spi(Execution) public static func objectType(forTypename typename: String) -> ApolloAPI.Object? {
    objectTypeMap[typename]
  }
}

nonisolated public enum Objects {}
nonisolated public enum Interfaces {}
nonisolated public enum Unions {}

package graphql

import "testing"

// Rust: crates/noema-api/src/graphql/adapters.rs::connection_actions_keep_existing_accounts_and_applications_selectable.
func TestRustAPI_connection_actions_keep_existing_accounts_and_applications_selectable(t *testing.T) {
	rustAPIPortAdapterConnectionActions(t)
}

// Rust: crates/noema-api/src/graphql/adapters.rs::existing_grant_recovery_includes_exact_application_revision.
func TestRustAPI_existing_grant_recovery_includes_exact_application_revision(t *testing.T) {
	rustAPIPortExistingGrant(t)
}

// Rust: crates/noema-api/src/graphql/adapters.rs::composed_management_read_uses_one_current_snapshot.
func TestRustAPI_composed_management_read_uses_one_current_snapshot(t *testing.T) {
	rustAPIPortAdapterManagement(t)
}

// Rust: crates/noema-api/src/graphql/adapters.rs::approval_publishes_reviewed_definition_and_reconciles_projection.
func TestRustAPI_approval_publishes_reviewed_definition_and_reconciles_projection(t *testing.T) {
	rustAPIPortAdapterApproval(t)
}

// Rust: crates/noema-api/src/graphql/adapters.rs::approval_rejects_unknown_and_reconciles_reviewed_authority_idempotently.
func TestRustAPI_approval_rejects_unknown_and_reconciles_reviewed_authority_idempotently(t *testing.T) {
	rustAPIPortAdapterApprovalIdempotence(t)
}

// Rust: crates/noema-api/src/graphql/adapters.rs::cancellation_removes_pending_review_and_projection.
func TestRustAPI_cancellation_removes_pending_review_and_projection(t *testing.T) {
	rustAPIPortAdapterCancellation(t)
}

// Rust: crates/noema-api/src/graphql/adapters.rs::policyless_active_connection_remains_a_chat_intervention.
func TestRustAPI_policyless_active_connection_remains_a_chat_intervention(t *testing.T) {
	rustAPIPortPolicyIntervention(t)
}

// Rust: crates/noema-api/src/graphql/adapters.rs::reviewed_replacement_supersedes_prior_setup_intervention.
func TestRustAPI_reviewed_replacement_supersedes_prior_setup_intervention(t *testing.T) {
	rustAPIPortAdapterReplacement(t)
}

// Rust: crates/noema-api/src/graphql/adapters.rs::breaking_revision_migrates_one_connection_without_enabling_new_tools.
func TestRustAPI_breaking_revision_migrates_one_connection_without_enabling_new_tools(t *testing.T) {
	rustAPIPortAdapterBreakingRevision(t)
}

// Rust: crates/noema-api/src/graphql/adapters.rs::authentication_revision_keeps_connection_and_stops_access.
func TestRustAPI_authentication_revision_keeps_connection_and_stops_access(t *testing.T) {
	rustAPIPortAdapterAuthRevision(t)
}

// Rust: crates/noema-api/src/graphql/adapters.rs::oauth_application_import_is_reusable_and_secret_safe.
func TestRustAPI_oauth_application_import_is_reusable_and_secret_safe(t *testing.T) {
	rustAPIPortAdapterOAuthImport(t)
}

// Rust: crates/noema-api/src/graphql/adapters.rs::oauth_setup_is_grouped_by_shared_authority_after_definition_reviews.
func TestRustAPI_oauth_setup_is_grouped_by_shared_authority_after_definition_reviews(t *testing.T) {
	rustAPIPortAdapterOAuthSetup(t)
}

// Rust: crates/noema-api/src/graphql/adapters.rs::integration_projection_keeps_reviewed_authority_when_a_newer_draft_exists.
func TestRustAPI_integration_projection_keeps_reviewed_authority_when_a_newer_draft_exists(t *testing.T) {
	rustAPIPortAdapterIntegration(t)
}

// Rust: crates/noema-api/src/graphql/adapters.rs::adapter_deletion_guards_references_and_reconciles_projections.
func TestRustAPI_adapter_deletion_guards_references_and_reconciles_projections(t *testing.T) {
	rustAPIPortAdapterDeletion(t)
}

// Rust: crates/noema-api/src/graphql/adapters.rs::concurrent_imports_reuse_one_oauth_application.
func TestRustAPI_concurrent_imports_reuse_one_oauth_application(t *testing.T) {
	rustAPIPortAdapterConcurrentImport(t)
}

// Rust: crates/noema-api/src/graphql/apns.rs::provider_status_preserves_ids_without_exposing_private_key.
func TestRustAPI_provider_status_preserves_ids_without_exposing_private_key(t *testing.T) {
	rustAPIPortProviderStatus(t)
}

// Rust: crates/noema-api/src/graphql/apns.rs::protected_credential_file_round_trips_tombstone_and_permissions.
func TestRustAPI_protected_credential_file_round_trips_tombstone_and_permissions(t *testing.T) {
	rustAPIPortProtectedCredentialFile(t)
}

// Rust: crates/noema-api/src/graphql/artifacts.rs::milestone_three_formats_select_safe_preview_paths.
func TestRustAPI_milestone_three_formats_select_safe_preview_paths(t *testing.T) {
	rustAPIPortArtifactPreviewKinds(t)
}

// Rust: crates/noema-api/src/graphql/artifacts.rs::authorized_download_returns_validated_payload.
func TestRustAPI_authorized_download_returns_validated_payload(t *testing.T) {
	rustAPIPortAuthorizedDownload(t)
}

// Rust: crates/noema-api/src/graphql/artifacts.rs::authorized_download_hides_other_owner.
func TestRustAPI_authorized_download_hides_other_owner(t *testing.T) {
	rustAPIPortAuthorizedDownloadForeignOwner(t)
}

// Rust: crates/noema-api/src/graphql/artifacts.rs::authorized_download_hides_unreadable_local_payload.
func TestRustAPI_authorized_download_hides_unreadable_local_payload(t *testing.T) {
	rustAPIPortAuthorizedDownloadUnreadable(t)
}

// Rust: crates/noema-api/src/graphql/artifacts.rs::authorized_download_refuses_traversal.
func TestRustAPI_authorized_download_refuses_traversal(t *testing.T) {
	rustAPIPortAuthorizedDownloadTraversal(t)
}

// Rust: crates/noema-api/src/graphql/artifacts.rs::authorized_download_refuses_symlink.
func TestRustAPI_authorized_download_refuses_symlink(t *testing.T) {
	rustAPIPortAuthorizedDownloadSymlink(t)
}

// Rust: crates/noema-api/src/graphql/artifacts.rs::authorized_download_store_failure_writes_redacted_diagnostic.
func TestRustAPI_authorized_download_store_failure_writes_redacted_diagnostic(t *testing.T) {
	rustAPIPortAuthorizedDownloadStoreFailure(t)
}

// Rust: crates/noema-api/src/graphql/chat_tests.rs::runtime_turn_error_publishes_error_notice_and_completion.
func TestRustAPI_runtime_turn_error_publishes_error_notice_and_completion(t *testing.T) {
	rustAPIPortRuntimeTurnError(t)
}

// Rust: crates/noema-api/src/graphql/chat_tests.rs::runtime_turn_error_does_not_duplicate_published_error_notice.
func TestRustAPI_runtime_turn_error_does_not_duplicate_published_error_notice(t *testing.T) {
	rustAPIPortRuntimeTurnErrorNoDuplicate(t)
}

// Rust: crates/noema-api/src/graphql/clients.rs::client_graphql_lists_and_identifies_the_current_revocation.
func TestRustAPI_client_graphql_lists_and_identifies_the_current_revocation(t *testing.T) {
	rustAPIPortClientListAndRevoke(t)
}

// Rust: crates/noema-api/src/graphql/clients.rs::client_graphql_revokes_all_native_clients.
func TestRustAPI_client_graphql_revokes_all_native_clients(t *testing.T) {
	rustAPIPortClientRevokeAll(t)
}

// Rust: crates/noema-api/src/graphql/governed_actions.rs::graphql_arguments_show_the_exact_reviewed_values.
func TestRustAPI_graphql_arguments_show_the_exact_reviewed_values(t *testing.T) {
	rustAPIPortGovernedAction(t)
}

// Rust: crates/noema-api/src/graphql/local_models_tests.rs::generic_default_preference_rejects_ambiguous_local_model_selection.
func TestRustAPI_generic_default_preference_rejects_ambiguous_local_model_selection(t *testing.T) {
	rustAPIPortGenericLocalDefault(t)
}

// Rust: crates/noema-api/src/graphql/local_models_tests.rs::local_model_catalog_setup_and_agent_preferences_follow_exact_installation_state.
func TestRustAPI_local_model_catalog_setup_and_agent_preferences_follow_exact_installation_state(t *testing.T) {
	rustAPIPortLocalModelCatalog(t)
}

// Rust: crates/noema-api/src/graphql/mcp/input.rs::rejects_invalid_boundary_enums.
func TestRustAPI_rejects_invalid_boundary_enums(t *testing.T) {
	rustAPIPortInvalidMCPBoundary(t)
}

// Rust: crates/noema-api/src/graphql/onboarding.rs::onboarding_proposals_are_role_aware_for_each_first_run_provider.
func TestRustAPI_onboarding_proposals_are_role_aware_for_each_first_run_provider(t *testing.T) {
	rustAPIPortOnboardingProposals(t)
}

// Rust: crates/noema-api/src/graphql/onboarding.rs::provider_callback_keeps_attempt_identity_in_the_path.
func TestRustAPI_provider_callback_keeps_attempt_identity_in_the_path(t *testing.T) {
	rustAPIPortProviderCallback(t)
}

// Rust: crates/noema-api/src/graphql/runtime_debug.rs::wall_clock_intervals_do_not_double_count_clock_adjustment.
func TestRustAPI_wall_clock_intervals_do_not_double_count_clock_adjustment(t *testing.T) {
	rustAPIPortWallClockIntervals(t)
}

// Rust: crates/noema-api/src/graphql/schema/subscription.rs::projection_failure_does_not_advance_the_subscription_cursor.
func TestRustAPI_projection_failure_does_not_advance_the_subscription_cursor(t *testing.T) {
	rustAPIPortSubscriptionProjectionFailure(t)
}

// Rust: crates/noema-api/src/graphql/schema_tests.rs::schema_sdl_exposes_initial_noema_fields.
func TestRustAPI_schema_sdl_exposes_initial_noema_fields(t *testing.T) {
	rustAPIPortSchemaSDL(t)
}

// Rust: crates/noema-api/src/graphql/schema_tests.rs::notification_operations_enforce_session_and_paired_client_boundaries.
func TestRustAPI_notification_operations_enforce_session_and_paired_client_boundaries(t *testing.T) {
	rustAPIPortNotificationBoundaries(t)
}

// Rust: crates/noema-api/src/graphql/schema_tests.rs::owner_sensitive_operations_require_a_request_principal.
func TestRustAPI_owner_sensitive_operations_require_a_request_principal(t *testing.T) {
	rustAPIPortOwnerPrincipal(t)
}

// Rust: crates/noema-api/src/graphql/schema_tests/acp_agent_delete_tests.rs::delete_is_revision_fenced_and_removes_acp_setup_state.
func TestRustAPI_delete_is_revision_fenced_and_removes_acp_setup_state(t *testing.T) {
	rustAPIPortACPDeleteRevision(t)
}

// Rust: crates/noema-api/src/graphql/schema_tests/acp_agent_delete_tests.rs::delete_rejects_current_task_and_schedule_references.
func TestRustAPI_delete_rejects_current_task_and_schedule_references(t *testing.T) {
	rustAPIPortACPDeleteReferences(t)
}

// Rust: crates/noema-api/src/graphql/schema_tests/agent_errors.rs::agents_query_sanitizes_unavailable_provider_errors.
func TestRustAPI_agents_query_sanitizes_unavailable_provider_errors(t *testing.T) {
	rustAPIPortAgentErrorSanitization(t)
}

// Rust: crates/noema-api/src/graphql/schema_tests/artifacts.rs::local_artifact_queries_expose_versions_downloads_and_markdown.
func TestRustAPI_local_artifact_queries_expose_versions_downloads_and_markdown(t *testing.T) {
	rustAPIPortLocalArtifactQueries(t)
}

// Rust: crates/noema-api/src/graphql/schema_tests/artifacts.rs::artifact_operations_hide_foreign_human_resources.
func TestRustAPI_artifact_operations_hide_foreign_human_resources(t *testing.T) {
	rustAPIPortForeignArtifactOperations(t)
}

// Rust: crates/noema-api/src/graphql/schema_tests/artifacts.rs::inbox_task_upload_creates_one_task_owned_version.
func TestRustAPI_inbox_task_upload_creates_one_task_owned_version(t *testing.T) {
	rustAPIPortInboxArtifactUpload(t)
}

// Rust: crates/noema-api/src/graphql/schema_tests/conversations.rs::missing_primary_provider_returns_no_primary_conversation.
func TestRustAPI_missing_primary_provider_returns_no_primary_conversation(t *testing.T) {
	rustAPIPortMissingPrimaryConversation(t)
}

// Rust: crates/noema-api/src/graphql/schema_tests/conversations.rs::conversation_operations_reject_foreign_human_conversations.
func TestRustAPI_conversation_operations_reject_foreign_human_conversations(t *testing.T) {
	rustAPIPortForeignConversationOperations(t)
}

// Rust: crates/noema-api/src/graphql/schema_tests/mcp_boundaries.rs::mcp_graphql_route_and_setup_boundaries.
func TestRustAPI_mcp_graphql_route_and_setup_boundaries(t *testing.T) {
	rustAPIPortMCPRouteAndSetup(t)
}

// Rust: crates/noema-api/src/graphql/schema_tests/mcp_boundaries.rs::chat_mcp_setup_is_projected_as_a_pending_human_intervention.
func TestRustAPI_chat_mcp_setup_is_projected_as_a_pending_human_intervention(t *testing.T) {
	rustAPIPortMCPPendingIntervention(t)
}

// Rust: crates/noema-api/src/graphql/schema_tests/mcp_oauth_tests.rs::oauth_callback_completion_drains_the_bound_runtime_request.
func TestRustAPI_oauth_callback_completion_drains_the_bound_runtime_request(t *testing.T) {
	rustAPIPortMCPOAuthCompletion(t)
}

// Rust: crates/noema-api/src/graphql/schema_tests/runtime_debug.rs::runtime_debug_profile_projects_safe_timing_and_usage.
func TestRustAPI_runtime_debug_profile_projects_safe_timing_and_usage(t *testing.T) {
	rustAPIPortRuntimeDebugSchema(t)
}

// Rust: crates/noema-api/src/graphql/schema_tests/settings.rs::agent_preference_enforces_identity_and_reasoning_contracts.
func TestRustAPI_agent_preference_enforces_identity_and_reasoning_contracts(t *testing.T) {
	rustAPIPortAgentPreference(t)
}

// Rust: crates/noema-api/src/graphql/schema_tests/settings.rs::acp_agent_setup_is_revision_fenced_and_never_exposes_credentials.
func TestRustAPI_acp_agent_setup_is_revision_fenced_and_never_exposes_credentials(t *testing.T) {
	rustAPIPortACPSetup(t)
}

// Rust: crates/noema-api/src/graphql/schema_tests/subscriptions.rs::memory_events_emits_initial_and_invalidated_snapshots.
func TestRustAPI_memory_events_emits_initial_and_invalidated_snapshots(t *testing.T) {
	rustAPIPortMemoryEvents(t)
}

// Rust: crates/noema-api/src/graphql/schema_tests/subscriptions.rs::conversation_events_emits_ready_before_live_events.
func TestRustAPI_conversation_events_emits_ready_before_live_events(t *testing.T) {
	rustAPIPortConversationReady(t)
}

// Rust: crates/noema-api/src/graphql/schema_tests/subscriptions.rs::subscription_projects_live_delta_and_conversation_item_events.
func TestRustAPI_subscription_projects_live_delta_and_conversation_item_events(t *testing.T) {
	rustAPIPortConversationLiveEvents(t)
}

// Rust: crates/noema-api/src/graphql/schema_tests/subscriptions.rs::conversation_events_rejects_foreign_human_conversations.
func TestRustAPI_conversation_events_rejects_foreign_human_conversations(t *testing.T) {
	rustAPIPortForeignConversationSubscription(t)
}

// Rust: crates/noema-api/src/graphql/schema_tests/tasks_graphql.rs::tasks_schema_exposes_semantic_operations_without_task_status_aliases.
func TestRustAPI_tasks_schema_exposes_semantic_operations_without_task_status_aliases(t *testing.T) {
	rustAPIPortTaskSemanticSchema(t)
}

// Rust: crates/noema-api/src/graphql/schema_tests/tasks_graphql.rs::recurrence_list_does_not_depend_on_an_active_task_instance.
func TestRustAPI_recurrence_list_does_not_depend_on_an_active_task_instance(t *testing.T) {
	rustAPIPortRecurrenceList(t)
}

// Rust: crates/noema-api/src/graphql/schema_tests/tasks_graphql.rs::tasks_schema_exposes_exact_detail_attention_and_closed_vocabularies.
func TestRustAPI_tasks_schema_exposes_exact_detail_attention_and_closed_vocabularies(t *testing.T) {
	rustAPIPortTaskSchemaVocabulary(t)
}

// Rust: crates/noema-api/src/graphql/schema_tests/tasks_graphql.rs::task_reads_require_an_authenticated_owner.
func TestRustAPI_task_reads_require_an_authenticated_owner(t *testing.T) {
	rustAPIPortTaskReadsPrincipal(t)
}

// Rust: crates/noema-api/src/graphql/schema_tests/tasks_graphql.rs::task_connections_reject_malformed_cursors_with_stable_code.
func TestRustAPI_task_connections_reject_malformed_cursors_with_stable_code(t *testing.T) {
	rustAPIPortMalformedTaskCursor(t)
}

// Rust: crates/noema-api/src/graphql/schema_tests/tasks_graphql.rs::task_connections_enforce_page_bounds_before_store_reads.
func TestRustAPI_task_connections_enforce_page_bounds_before_store_reads(t *testing.T) {
	rustAPIPortTaskPageBounds(t)
}

// Rust: crates/noema-api/src/graphql/schema_tests/tasks_graphql.rs::task_authorization_is_indistinguishable_before_identifier_validation.
func TestRustAPI_task_authorization_is_indistinguishable_before_identifier_validation(t *testing.T) {
	rustAPIPortTaskAuthorization(t)
}

// Rust: crates/noema-api/src/graphql/schema_tests/tasks_graphql.rs::task_scope_filters_reject_semantic_stage_conflicts.
func TestRustAPI_task_scope_filters_reject_semantic_stage_conflicts(t *testing.T) {
	rustAPIPortTaskScopeConflicts(t)
}

// Rust: crates/noema-api/src/graphql/schema_tests/tasks_graphql.rs::task_mutations_require_client_idempotency_keys.
func TestRustAPI_task_mutations_require_client_idempotency_keys(t *testing.T) {
	rustAPIPortTaskIdempotencyRequired(t)
}

// Rust: crates/noema-api/src/graphql/schema_tests/tasks_graphql.rs::project_folder_and_task_executor_cwd_round_trip_through_graphql.
func TestRustAPI_project_folder_and_task_executor_cwd_round_trip_through_graphql(t *testing.T) {
	rustAPIPortProjectExecutorCWD(t)
}

// Rust: crates/noema-api/src/graphql/schema_tests/tasks_graphql.rs::project_document_reads_saves_and_conflicts_through_graphql.
func TestRustAPI_project_document_reads_saves_and_conflicts_through_graphql(t *testing.T) {
	rustAPIPortProjectDocument(t)
}

// Rust: crates/noema-api/src/graphql/schema_tests/tasks_graphql.rs::task_mutations_reject_whitespace_idempotency_aliases.
func TestRustAPI_task_mutations_reject_whitespace_idempotency_aliases(t *testing.T) {
	rustAPIPortWhitespaceIdempotency(t)
}

// Rust: crates/noema-api/src/graphql/schema_tests/tasks_graphql.rs::capture_task_returns_authoritative_task_projection.
func TestRustAPI_capture_task_returns_authoritative_task_projection(t *testing.T) {
	rustAPIPortCaptureProjection(t)
}

// Rust: crates/noema-api/src/graphql/schema_tests/tasks_graphql.rs::task_workspace_lists_nested_files_and_rejects_unsafe_reads.
func TestRustAPI_task_workspace_lists_nested_files_and_rejects_unsafe_reads(t *testing.T) {
	rustAPIPortTaskWorkspace(t)
}

// Rust: crates/noema-api/src/graphql/schema_tests/tasks_graphql.rs::task_gate_uses_the_unified_human_intervention_projection_until_resolved.
func TestRustAPI_task_gate_uses_the_unified_human_intervention_projection_until_resolved(t *testing.T) {
	rustAPIPortTaskGate(t)
}

// Rust: crates/noema-api/src/graphql/schema_tests/tasks_graphql.rs::task_mutation_replay_returns_the_original_committed_detail.
func TestRustAPI_task_mutation_replay_returns_the_original_committed_detail(t *testing.T) {
	rustAPIPortTaskMutationReplay(t)
}

// Rust: crates/noema-api/src/graphql/schema_tests/tasks_graphql.rs::task_subscription_recovers_from_wakeup_lag_and_keeps_its_task_filter.
func TestRustAPI_task_subscription_recovers_from_wakeup_lag_and_keeps_its_task_filter(t *testing.T) {
	rustAPIPortTaskSubscription(t)
}

// Rust: crates/noema-api/src/graphql/support_tests.rs::conversation_replay_projects_representative_transcript_items.
func TestRustAPI_conversation_replay_projects_representative_transcript_items(t *testing.T) {
	rustAPIPortConversationReplay(t)
}

// Rust: crates/noema-api/src/graphql/support_tests.rs::conversation_replay_rejects_malformed_activity_payload.
func TestRustAPI_conversation_replay_rejects_malformed_activity_payload(t *testing.T) {
	rustAPIPortMalformedReplay(t)
}

// Rust: crates/noema-api/src/graphql/support_tests.rs::conversation_replay_omits_hidden_tool_activity.
func TestRustAPI_conversation_replay_omits_hidden_tool_activity(t *testing.T) {
	rustAPIPortHiddenReplay(t)
}

// Rust: crates/noema-api/src/graphql/tasks/projections/mapping.rs::authoritative_u64_projection_fails_instead_of_saturating.
func TestRustAPI_authoritative_u64_projection_fails_instead_of_saturating(t *testing.T) {
	rustAPIPortAuthoritativeU64(t)
}

// Rust: crates/noema-api/src/graphql/tasks/resolvers.rs::policy_update_preserves_task_owned_bounds.
func TestRustAPI_policy_update_preserves_task_owned_bounds(t *testing.T) {
	rustAPIPortTaskPolicy(t)
}

// Rust: crates/noema-api/src/graphql/tasks/resolvers.rs::stale_pool_route_metadata_can_be_edited_and_disabled.
func TestRustAPI_stale_pool_route_metadata_can_be_edited_and_disabled(t *testing.T) {
	rustAPIPortStalePoolRoute(t)
}

// Rust: crates/noema-api/src/graphql/web_push.rs::generated_vapid_identity_uses_an_uncompressed_public_key.
func TestRustAPI_generated_vapid_identity_uses_an_uncompressed_public_key(t *testing.T) {
	rustAPIPortVAPIDIdentity(t)
}

// Rust: crates/noema-api/src/graphql/web_push.rs::idle_notification_reconciliation_does_not_wake_itself.
func TestRustAPI_idle_notification_reconciliation_does_not_wake_itself(t *testing.T) {
	rustAPIPortIdleNotifications(t)
}

// Rust: crates/noema-api/src/graphql/web_push.rs::task_events_debounce_live_reconciliation_and_work_clears_it.
func TestRustAPI_task_events_debounce_live_reconciliation_and_work_clears_it(t *testing.T) {
	rustAPIPortTaskEventDebounce(t)
}

// Rust: crates/noema-api/src/graphql/web_push.rs::only_final_primary_chat_text_becomes_a_notification.
func TestRustAPI_only_final_primary_chat_text_becomes_a_notification(t *testing.T) {
	rustAPIPortPrimaryChatNotification(t)
}

// Rust: crates/noema-api/src/graphql/web_push.rs::notification_preview_removes_markdown_markup.
func TestRustAPI_notification_preview_removes_markdown_markup(t *testing.T) {
	rustAPIPortNotificationPreview(t)
}

// Rust: crates/noema-api/src/graphql/web_push.rs::declarative_payload_keeps_required_fallback_fields.
func TestRustAPI_declarative_payload_keeps_required_fallback_fields(t *testing.T) {
	rustAPIPortDeclarativePayload(t)
}

// Rust: crates/noema-api/src/graphql/web_push.rs::live_activity_payload_keeps_activitykit_fields_and_task_alert_route.
func TestRustAPI_live_activity_payload_keeps_activitykit_fields_and_task_alert_route(t *testing.T) {
	rustAPIPortLiveActivityPayload(t)
}

// Rust: crates/noema-api/src/graphql/web_push.rs::live_activity_priority_reserves_high_delivery_for_immediate_events.
func TestRustAPI_live_activity_priority_reserves_high_delivery_for_immediate_events(t *testing.T) {
	rustAPIPortLiveActivityPriority(t)
}

// Rust: crates/noema-api/src/graphql/web_push.rs::live_activity_reconciliation_uses_one_mutation_lane.
func TestRustAPI_live_activity_reconciliation_uses_one_mutation_lane(t *testing.T) {
	rustAPIPortLiveActivityMutationLane(t)
}

// Rust: crates/noema-api/src/graphql/web_push.rs::live_activity_reconciliation_keeps_inflight_and_replaces_old_focus.
func TestRustAPI_live_activity_reconciliation_keeps_inflight_and_replaces_old_focus(t *testing.T) {
	rustAPIPortLiveActivityFocus(t)
}

// Rust: crates/noema-api/src/graphql/web_push.rs::live_activity_uses_shared_text_for_the_active_tool.
func TestRustAPI_live_activity_uses_shared_text_for_the_active_tool(t *testing.T) {
	rustAPIPortLiveActivitySharedText(t)
}

// Rust: crates/noema-api/src/graphql/web_push.rs::live_activity_retains_the_latest_tool_without_commentary.
func TestRustAPI_live_activity_retains_the_latest_tool_without_commentary(t *testing.T) {
	rustAPIPortLiveActivityLatestTool(t)
}

// Rust: crates/noema-api/src/graphql/web_push.rs::live_activity_ignores_partial_commentary_and_completed_tools.
func TestRustAPI_live_activity_ignores_partial_commentary_and_completed_tools(t *testing.T) {
	rustAPIPortLiveActivityIgnoresPartial(t)
}

// Rust: crates/noema-api/src/graphql/web_push.rs::live_activity_uses_the_first_running_tool_and_its_timestamp.
func TestRustAPI_live_activity_uses_the_first_running_tool_and_its_timestamp(t *testing.T) {
	rustAPIPortLiveActivityFirstRunning(t)
}

// Rust: crates/noema-api/src/graphql/web_push.rs::live_activity_partial_output_hides_an_older_completed_line.
func TestRustAPI_live_activity_partial_output_hides_an_older_completed_line(t *testing.T) {
	rustAPIPortLiveActivityPartialOutput(t)
}

// Rust: crates/noema-api/src/graphql/web_push.rs::live_activity_uses_the_latest_meaningful_task_line.
func TestRustAPI_live_activity_uses_the_latest_meaningful_task_line(t *testing.T) {
	rustAPIPortLiveActivityLatestMeaningful(t)
}

// Rust: crates/noema-api/src/graphql/web_push.rs::terminal_live_activity_keeps_completed_output_metadata.
func TestRustAPI_terminal_live_activity_keeps_completed_output_metadata(t *testing.T) {
	rustAPIPortTerminalLiveActivity(t)
}

// Rust: crates/noema-api/src/graphql/web_push.rs::delivery_statuses_have_bounded_retry_and_expiry_classes.
func TestRustAPI_delivery_statuses_have_bounded_retry_and_expiry_classes(t *testing.T) {
	rustAPIPortDeliveryStatuses(t)
}

// Rust: crates/noema-api/src/graphql/web_tool_settings.rs::web_tool_settings_treats_openai_as_the_default_selectable_provider.
func TestRustAPI_web_tool_settings_treats_openai_as_the_default_selectable_provider(t *testing.T) {
	rustAPIPortWebToolDefault(t)
}

// Rust: crates/noema-api/src/graphql/web_tool_settings.rs::web_tool_settings_filters_capabilities_and_ignores_stale_bindings.
func TestRustAPI_web_tool_settings_filters_capabilities_and_ignores_stale_bindings(t *testing.T) {
	rustAPIPortWebToolFiltering(t)
}

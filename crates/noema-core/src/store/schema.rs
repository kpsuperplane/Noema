/// Namespace used by the embedded Noema store.
pub const NOEMA_NAMESPACE: &str = "noema";

/// Database used by the embedded Noema store.
pub const NOEMA_DATABASE: &str = "main";

/// Current schema version for pre-stable local data.
pub const STORE_SCHEMA_VERSION: i64 = 1;

/// Minimal bootstrap used by the first store-runtime test.
pub const STORE_SCHEMA_SQL: &str = r#"
DEFINE TABLE IF NOT EXISTS schema_state SCHEMAFULL;
DEFINE FIELD OVERWRITE version ON TABLE schema_state TYPE int ASSERT $value >= 1;
DEFINE FIELD OVERWRITE name ON TABLE schema_state TYPE string;
DEFINE FIELD OVERWRITE applied_at ON TABLE schema_state TYPE datetime DEFAULT time::now();
UPSERT schema_state:current SET version = 1, name = 'surreal_graph_store_v1', applied_at = time::now();

DEFINE TABLE IF NOT EXISTS humans SCHEMAFULL;
DEFINE FIELD OVERWRITE human_id ON TABLE humans TYPE string;
DEFINE FIELD OVERWRITE display_name ON TABLE humans TYPE string;
DEFINE FIELD OVERWRITE primary_conversation_id ON TABLE humans TYPE option<string>;
DEFINE FIELD OVERWRITE created_at ON TABLE humans TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE updated_at ON TABLE humans TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS humans_human_id ON TABLE humans COLUMNS human_id UNIQUE;

DEFINE TABLE IF NOT EXISTS agents SCHEMAFULL;
DEFINE FIELD OVERWRITE agent_id ON TABLE agents TYPE string;
DEFINE FIELD OVERWRITE display_name ON TABLE agents TYPE option<string>;
DEFINE FIELD OVERWRITE created_at ON TABLE agents TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE updated_at ON TABLE agents TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS agents_agent_id ON TABLE agents COLUMNS agent_id UNIQUE;

DEFINE TABLE IF NOT EXISTS agent_runtime_preferences SCHEMAFULL;
DEFINE FIELD OVERWRITE agent_id ON TABLE agent_runtime_preferences TYPE string;
DEFINE FIELD OVERWRITE provider_kind ON TABLE agent_runtime_preferences TYPE string ASSERT $value INSIDE ['codex', 'openai', 'foundation_local'];
DEFINE FIELD OVERWRITE provider_account_id ON TABLE agent_runtime_preferences TYPE string;
DEFINE FIELD OVERWRITE model_profile ON TABLE agent_runtime_preferences TYPE string ASSERT $value != '';
DEFINE FIELD OVERWRITE created_at ON TABLE agent_runtime_preferences TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE updated_at ON TABLE agent_runtime_preferences TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS agent_runtime_preferences_agent_id ON TABLE agent_runtime_preferences COLUMNS agent_id UNIQUE;

DEFINE TABLE IF NOT EXISTS provider_accounts SCHEMAFULL;
DEFINE FIELD OVERWRITE provider_account_id ON TABLE provider_accounts TYPE string;
DEFINE FIELD OVERWRITE provider_kind ON TABLE provider_accounts TYPE string ASSERT $value INSIDE ['codex', 'openai', 'foundation_local'];
DEFINE FIELD OVERWRITE account_key ON TABLE provider_accounts TYPE string;
DEFINE FIELD OVERWRITE display_name ON TABLE provider_accounts TYPE string;
DEFINE FIELD OVERWRITE auth_method ON TABLE provider_accounts TYPE string ASSERT $value INSIDE ['oauth_device_code', 'secret_input', 'external_manual', 'none'];
DEFINE FIELD OVERWRITE is_active ON TABLE provider_accounts TYPE bool;
DEFINE FIELD OVERWRITE is_default ON TABLE provider_accounts TYPE bool;
DEFINE FIELD OVERWRITE status ON TABLE provider_accounts TYPE string ASSERT $value INSIDE ['unknown', 'checking', 'authenticated', 'unauthenticated', 'unavailable'];
DEFINE FIELD OVERWRITE last_checked_at ON TABLE provider_accounts TYPE option<string>;
DEFINE FIELD OVERWRITE last_authenticated_at ON TABLE provider_accounts TYPE option<string>;
DEFINE FIELD OVERWRITE last_error_code ON TABLE provider_accounts TYPE option<string>;
DEFINE FIELD OVERWRITE last_error_message ON TABLE provider_accounts TYPE option<string>;
DEFINE FIELD OVERWRITE metadata ON TABLE provider_accounts TYPE object FLEXIBLE;
DEFINE FIELD OVERWRITE created_at ON TABLE provider_accounts TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE updated_at ON TABLE provider_accounts TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS provider_accounts_account_id ON TABLE provider_accounts COLUMNS provider_account_id UNIQUE;
DEFINE INDEX IF NOT EXISTS provider_accounts_kind_key ON TABLE provider_accounts COLUMNS provider_kind, account_key UNIQUE;

DEFINE TABLE IF NOT EXISTS mcp_servers SCHEMAFULL;
DEFINE FIELD OVERWRITE mcp_server_id ON TABLE mcp_servers TYPE string;
DEFINE FIELD OVERWRITE display_name ON TABLE mcp_servers TYPE string;
DEFINE FIELD OVERWRITE transport_kind ON TABLE mcp_servers TYPE string ASSERT $value INSIDE ['stdio', 'sse', 'streamable_http'];
DEFINE FIELD OVERWRITE safe_config ON TABLE mcp_servers TYPE object FLEXIBLE DEFAULT {};
DEFINE FIELD OVERWRITE auth_status ON TABLE mcp_servers TYPE string ASSERT $value INSIDE ['none', 'needs_auth', 'authenticated', 'unavailable'];
DEFINE FIELD OVERWRITE health_status ON TABLE mcp_servers TYPE string ASSERT $value INSIDE ['unknown', 'healthy', 'unavailable'];
DEFINE FIELD OVERWRITE enabled ON TABLE mcp_servers TYPE bool DEFAULT false;
DEFINE FIELD OVERWRITE metadata_fingerprint ON TABLE mcp_servers TYPE option<string>;
DEFINE FIELD OVERWRITE last_discovered_at ON TABLE mcp_servers TYPE option<string>;
DEFINE FIELD OVERWRITE created_at ON TABLE mcp_servers TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE updated_at ON TABLE mcp_servers TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS mcp_servers_server_id ON TABLE mcp_servers COLUMNS mcp_server_id UNIQUE;

DEFINE TABLE IF NOT EXISTS mcp_tools SCHEMAFULL;
DEFINE FIELD OVERWRITE mcp_tool_id ON TABLE mcp_tools TYPE string;
DEFINE FIELD OVERWRITE mcp_server_id ON TABLE mcp_tools TYPE string;
DEFINE FIELD OVERWRITE name ON TABLE mcp_tools TYPE string;
DEFINE FIELD OVERWRITE description ON TABLE mcp_tools TYPE option<string>;
DEFINE FIELD OVERWRITE input_schema ON TABLE mcp_tools TYPE object FLEXIBLE DEFAULT {};
DEFINE FIELD OVERWRITE output_schema ON TABLE mcp_tools TYPE option<object> FLEXIBLE;
DEFINE FIELD OVERWRITE annotations ON TABLE mcp_tools TYPE object FLEXIBLE DEFAULT {};
DEFINE FIELD OVERWRITE metadata_fingerprint ON TABLE mcp_tools TYPE string;
DEFINE FIELD OVERWRITE discovered_at ON TABLE mcp_tools TYPE string;
DEFINE FIELD OVERWRITE created_at ON TABLE mcp_tools TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE updated_at ON TABLE mcp_tools TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS mcp_tools_tool_id ON TABLE mcp_tools COLUMNS mcp_tool_id UNIQUE;
DEFINE INDEX IF NOT EXISTS mcp_tools_server_name ON TABLE mcp_tools COLUMNS mcp_server_id, name UNIQUE;

DEFINE TABLE IF NOT EXISTS tool_calibrations SCHEMAFULL;
DEFINE FIELD OVERWRITE calibration_id ON TABLE tool_calibrations TYPE string;
DEFINE FIELD OVERWRITE mcp_tool_id ON TABLE tool_calibrations TYPE string;
DEFINE FIELD OVERWRITE read_classification ON TABLE tool_calibrations TYPE string ASSERT $value INSIDE ['none', 'trusted', 'untrusted', 'mixed'];
DEFINE FIELD OVERWRITE write_classification ON TABLE tool_calibrations TYPE string ASSERT $value INSIDE ['none', 'trusted', 'untrusted', 'mixed'];
DEFINE FIELD OVERWRITE export_classification ON TABLE tool_calibrations TYPE string ASSERT $value INSIDE ['none', 'trusted', 'untrusted', 'mixed'];
DEFINE FIELD OVERWRITE owner_extractors ON TABLE tool_calibrations TYPE array<object> DEFAULT [] ASSERT array::all($value, |$extractor| $extractor.source INSIDE ['arguments', 'structured_content', 'metadata', 'resource_uri', 'built_in_adapter'] AND $extractor.selector_kind INSIDE ['email', 'phone', 'domain'] AND $extractor.path != NONE AND $extractor.path != '');
DEFINE FIELD OVERWRITE owner_extractors[*].source ON TABLE tool_calibrations TYPE string;
DEFINE FIELD OVERWRITE owner_extractors[*].selector_kind ON TABLE tool_calibrations TYPE string;
DEFINE FIELD OVERWRITE owner_extractors[*].path ON TABLE tool_calibrations TYPE string;
DEFINE FIELD OVERWRITE status ON TABLE tool_calibrations TYPE string ASSERT $value INSIDE ['needs_review', 'blocked_unresolved_ownership', 'ready', 'disabled'];
DEFINE FIELD OVERWRITE reviewed_by ON TABLE tool_calibrations TYPE option<string>;
DEFINE FIELD OVERWRITE reviewed_metadata_fingerprint ON TABLE tool_calibrations TYPE option<string>;
DEFINE FIELD OVERWRITE created_at ON TABLE tool_calibrations TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE updated_at ON TABLE tool_calibrations TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS tool_calibrations_calibration_id ON TABLE tool_calibrations COLUMNS calibration_id UNIQUE;
DEFINE INDEX IF NOT EXISTS tool_calibrations_tool_id ON TABLE tool_calibrations COLUMNS mcp_tool_id UNIQUE;

DEFINE TABLE IF NOT EXISTS trusted_identity_selectors SCHEMAFULL;
DEFINE FIELD OVERWRITE selector_id ON TABLE trusted_identity_selectors TYPE string ASSERT $value != '';
DEFINE FIELD OVERWRITE owner_scope_id ON TABLE trusted_identity_selectors TYPE string ASSERT $value != '';
DEFINE FIELD OVERWRITE selector_kind ON TABLE trusted_identity_selectors TYPE string ASSERT $value INSIDE ['email', 'phone', 'domain'];
DEFINE FIELD OVERWRITE normalized_value ON TABLE trusted_identity_selectors TYPE string ASSERT $value != '' AND ((selector_kind = 'email' AND $value = string::lowercase($value) AND string::is_email($value) AND string::matches($value, /\s/) = false) OR (selector_kind = 'domain' AND string::matches($value, /^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]*[a-z0-9])?)+$/)) OR (selector_kind = 'phone' AND string::matches($value, /^\+[0-9]{8,15}$/)));
DEFINE FIELD OVERWRITE effect ON TABLE trusted_identity_selectors TYPE string ASSERT $value INSIDE ['trust', 'restrict'];
DEFINE FIELD OVERWRITE issuer_actor_id ON TABLE trusted_identity_selectors TYPE string ASSERT $value != '';
DEFINE FIELD OVERWRITE revoked_at ON TABLE trusted_identity_selectors TYPE option<string>;
DEFINE FIELD OVERWRITE created_at ON TABLE trusted_identity_selectors TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE updated_at ON TABLE trusted_identity_selectors TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS trusted_identity_selectors_selector_id ON TABLE trusted_identity_selectors COLUMNS selector_id UNIQUE;
DEFINE INDEX IF NOT EXISTS trusted_identity_selectors_owner_value ON TABLE trusted_identity_selectors COLUMNS owner_scope_id, selector_kind, normalized_value UNIQUE;

DEFINE TABLE IF NOT EXISTS approval_requests SCHEMAFULL;
DEFINE FIELD OVERWRITE approval_id ON TABLE approval_requests TYPE string ASSERT $value != '';
DEFINE FIELD OVERWRITE action_summary ON TABLE approval_requests TYPE string ASSERT $value != '';
DEFINE FIELD OVERWRITE tool_invocation_id ON TABLE approval_requests TYPE string ASSERT $value != '';
DEFINE FIELD OVERWRITE mcp_server_id ON TABLE approval_requests TYPE option<string>;
DEFINE FIELD OVERWRITE mcp_tool_id ON TABLE approval_requests TYPE option<string>;
DEFINE FIELD OVERWRITE requester_actor_id ON TABLE approval_requests TYPE string ASSERT $value != '';
DEFINE FIELD OVERWRITE owner_scope_id ON TABLE approval_requests TYPE string ASSERT $value != '';
DEFINE FIELD OVERWRITE active_scope_id ON TABLE approval_requests TYPE string ASSERT $value != '';
DEFINE FIELD OVERWRITE destination_summary ON TABLE approval_requests TYPE string ASSERT $value != '';
DEFINE FIELD OVERWRITE data_source_summary ON TABLE approval_requests TYPE string ASSERT $value != '';
DEFINE FIELD OVERWRITE source_owner_identity ON TABLE approval_requests TYPE string ASSERT $value != '';
DEFINE FIELD OVERWRITE source_owner_trust ON TABLE approval_requests TYPE string ASSERT $value INSIDE ['trusted', 'untrusted', 'mixed', 'unresolved'];
DEFINE FIELD OVERWRITE destination_owner_identity ON TABLE approval_requests TYPE string ASSERT $value != '';
DEFINE FIELD OVERWRITE destination_owner_trust ON TABLE approval_requests TYPE string ASSERT $value INSIDE ['trusted', 'untrusted', 'mixed', 'unresolved'];
DEFINE FIELD OVERWRITE export_summary ON TABLE approval_requests TYPE string ASSERT $value != '';
DEFINE FIELD OVERWRITE payload_preview ON TABLE approval_requests TYPE object FLEXIBLE DEFAULT {};
DEFINE FIELD OVERWRITE status ON TABLE approval_requests TYPE string ASSERT $value INSIDE ['pending', 'approved', 'denied', 'cancelled'] AND ($value = 'pending' OR (decision_actor_id != NONE AND decision_actor_id != '' AND decided_at != NONE AND decided_at != ''));
DEFINE FIELD OVERWRITE decision_actor_id ON TABLE approval_requests TYPE option<string> ASSERT status = 'pending' OR ($value != NONE AND $value != '');
DEFINE FIELD OVERWRITE decision_comment ON TABLE approval_requests TYPE option<string>;
DEFINE FIELD OVERWRITE decided_at ON TABLE approval_requests TYPE option<string> ASSERT status = 'pending' OR ($value != NONE AND $value != '');
DEFINE FIELD OVERWRITE created_at ON TABLE approval_requests TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE updated_at ON TABLE approval_requests TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS approval_requests_approval_id ON TABLE approval_requests COLUMNS approval_id UNIQUE;
DEFINE INDEX IF NOT EXISTS approval_requests_tool_invocation_id ON TABLE approval_requests COLUMNS tool_invocation_id;
DEFINE INDEX IF NOT EXISTS approval_requests_status ON TABLE approval_requests COLUMNS status;
DEFINE INDEX IF NOT EXISTS approval_requests_owner_scope_id ON TABLE approval_requests COLUMNS owner_scope_id;

DEFINE TABLE IF NOT EXISTS conversations SCHEMAFULL;
DEFINE FIELD OVERWRITE conversation_id ON TABLE conversations TYPE string;
DEFINE FIELD OVERWRITE title ON TABLE conversations TYPE option<string>;
DEFINE FIELD OVERWRITE owner_object_type ON TABLE conversations TYPE string ASSERT $value INSIDE ['human', 'agent', 'conversation', 'workspace', 'project', 'task', 'tool'];
DEFINE FIELD OVERWRITE owner_object_id ON TABLE conversations TYPE string;
DEFINE FIELD OVERWRITE primary_human_id ON TABLE conversations TYPE option<string>;
DEFINE FIELD OVERWRITE primary_agent_id ON TABLE conversations TYPE option<string>;
DEFINE FIELD OVERWRITE provider ON TABLE conversations TYPE string ASSERT $value INSIDE ['codex', 'openai', 'foundation_local'];
DEFINE FIELD OVERWRITE model ON TABLE conversations TYPE option<string>;
DEFINE FIELD OVERWRITE cwd ON TABLE conversations TYPE option<string>;
DEFINE FIELD OVERWRITE lifecycle_status ON TABLE conversations TYPE string DEFAULT 'active' ASSERT $value INSIDE ['active', 'archived'];
DEFINE FIELD OVERWRITE agent_status ON TABLE conversations TYPE string DEFAULT 'idle' ASSERT $value INSIDE ['idle', 'input_received', 'thinking', 'tool_running', 'waiting_for_previous_turn_completion', 'interrupting', 'error'];
DEFINE FIELD OVERWRITE metadata ON TABLE conversations TYPE object FLEXIBLE;
DEFINE FIELD OVERWRITE deleted_at ON TABLE conversations TYPE option<string>;
DEFINE FIELD OVERWRITE created_at ON TABLE conversations TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE updated_at ON TABLE conversations TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS conversations_conversation_id ON TABLE conversations COLUMNS conversation_id UNIQUE;

DEFINE TABLE IF NOT EXISTS conversation_turns SCHEMAFULL;
DEFINE FIELD OVERWRITE turn_id ON TABLE conversation_turns TYPE string;
DEFINE FIELD OVERWRITE conversation_id ON TABLE conversation_turns TYPE string;
DEFINE FIELD OVERWRITE trigger_item_id ON TABLE conversation_turns TYPE option<string>;
DEFINE FIELD OVERWRITE status ON TABLE conversation_turns TYPE string ASSERT $value INSIDE ['input_received', 'running', 'waiting_for_tool', 'interrupted', 'completed', 'failed', 'cancelled'];
DEFINE FIELD OVERWRITE metadata ON TABLE conversation_turns TYPE object FLEXIBLE;
DEFINE FIELD OVERWRITE started_at ON TABLE conversation_turns TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE completed_at ON TABLE conversation_turns TYPE option<string>;
DEFINE FIELD OVERWRITE created_at ON TABLE conversation_turns TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE updated_at ON TABLE conversation_turns TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS conversation_turns_turn_id ON TABLE conversation_turns COLUMNS turn_id UNIQUE;
DEFINE INDEX IF NOT EXISTS conversation_turns_conversation_id ON TABLE conversation_turns COLUMNS conversation_id;

DEFINE TABLE IF NOT EXISTS conversation_items SCHEMAFULL;
DEFINE FIELD OVERWRITE item_id ON TABLE conversation_items TYPE string;
DEFINE FIELD OVERWRITE conversation_id ON TABLE conversation_items TYPE string;
DEFINE FIELD OVERWRITE turn_id ON TABLE conversation_items TYPE option<string>;
DEFINE FIELD OVERWRITE parent_item_id ON TABLE conversation_items TYPE option<string>;
DEFINE FIELD OVERWRITE sequence_index ON TABLE conversation_items TYPE int;
DEFINE FIELD OVERWRITE kind ON TABLE conversation_items TYPE string ASSERT $value INSIDE ['user_text', 'assistant_text', 'activity', 'a2ui_card', 'tool_call', 'tool_result', 'approval_request', 'approval_result', 'error_notice'];
DEFINE FIELD OVERWRITE status ON TABLE conversation_items TYPE string ASSERT $value INSIDE ['pending', 'running', 'completed', 'failed', 'cancelled', 'interrupted'];
DEFINE FIELD OVERWRITE author_actor_id ON TABLE conversation_items TYPE string;
DEFINE FIELD OVERWRITE content_text ON TABLE conversation_items TYPE option<string>;
DEFINE FIELD OVERWRITE payload_json ON TABLE conversation_items TYPE object FLEXIBLE;
DEFINE FIELD OVERWRITE metadata ON TABLE conversation_items TYPE object FLEXIBLE;
DEFINE FIELD OVERWRITE deleted_at ON TABLE conversation_items TYPE option<string>;
DEFINE FIELD OVERWRITE created_at ON TABLE conversation_items TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE updated_at ON TABLE conversation_items TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS conversation_items_item_id ON TABLE conversation_items COLUMNS item_id UNIQUE;
DEFINE INDEX IF NOT EXISTS conversation_items_conversation_sequence ON TABLE conversation_items COLUMNS conversation_id, sequence_index UNIQUE;

DEFINE TABLE IF NOT EXISTS conversation_context_summaries SCHEMAFULL;
DEFINE FIELD OVERWRITE summary_id ON TABLE conversation_context_summaries TYPE string;
DEFINE FIELD OVERWRITE conversation_id ON TABLE conversation_context_summaries TYPE string;
DEFINE FIELD OVERWRITE provider_kind ON TABLE conversation_context_summaries TYPE string ASSERT $value INSIDE ['codex', 'openai', 'foundation_local'];
DEFINE FIELD OVERWRITE model_profile ON TABLE conversation_context_summaries TYPE option<string>;
DEFINE FIELD OVERWRITE summary_text ON TABLE conversation_context_summaries TYPE string;
DEFINE FIELD OVERWRITE covered_item_start_sequence ON TABLE conversation_context_summaries TYPE int;
DEFINE FIELD OVERWRITE covered_item_end_sequence ON TABLE conversation_context_summaries TYPE int;
DEFINE FIELD OVERWRITE source_item_ids ON TABLE conversation_context_summaries TYPE array<string> DEFAULT [];
DEFINE FIELD OVERWRITE input_token_estimate ON TABLE conversation_context_summaries TYPE int ASSERT $value >= 0;
DEFINE FIELD OVERWRITE summary_token_estimate ON TABLE conversation_context_summaries TYPE int ASSERT $value >= 0;
DEFINE FIELD OVERWRITE compaction_provider_kind ON TABLE conversation_context_summaries TYPE string ASSERT $value INSIDE ['codex', 'openai', 'foundation_local'];
DEFINE FIELD OVERWRITE compaction_model_profile ON TABLE conversation_context_summaries TYPE option<string>;
DEFINE FIELD OVERWRITE status ON TABLE conversation_context_summaries TYPE string ASSERT $value INSIDE ['pending', 'active', 'failed', 'superseded'];
DEFINE FIELD OVERWRITE error_code ON TABLE conversation_context_summaries TYPE option<string>;
DEFINE FIELD OVERWRITE error_message ON TABLE conversation_context_summaries TYPE option<string>;
DEFINE FIELD OVERWRITE created_at ON TABLE conversation_context_summaries TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE updated_at ON TABLE conversation_context_summaries TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS conversation_context_summaries_summary_id ON TABLE conversation_context_summaries COLUMNS summary_id UNIQUE;
DEFINE INDEX IF NOT EXISTS conversation_context_summaries_profile ON TABLE conversation_context_summaries COLUMNS conversation_id, provider_kind, model_profile, status, covered_item_end_sequence;

DEFINE TABLE IF NOT EXISTS entities SCHEMAFULL;
DEFINE FIELD OVERWRITE entity_id ON TABLE entities TYPE string;
DEFINE FIELD OVERWRITE entity_type ON TABLE entities TYPE string ASSERT $value INSIDE ['human', 'agent', 'person', 'organization', 'project', 'workspace', 'conversation', 'document', 'tool', 'place', 'task', 'goal', 'concept', 'other'];
DEFINE FIELD OVERWRITE canonical_name ON TABLE entities TYPE string;
DEFINE FIELD OVERWRITE aliases ON TABLE entities TYPE array<string> DEFAULT [];
DEFINE FIELD OVERWRITE linked_object_type ON TABLE entities TYPE option<string>;
DEFINE FIELD OVERWRITE linked_object_id ON TABLE entities TYPE option<string>;
DEFINE FIELD OVERWRITE metadata ON TABLE entities TYPE object FLEXIBLE DEFAULT {};
DEFINE FIELD OVERWRITE created_at ON TABLE entities TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE updated_at ON TABLE entities TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS entities_entity_id ON TABLE entities COLUMNS entity_id UNIQUE;

DEFINE TABLE IF NOT EXISTS predicates SCHEMAFULL;
DEFINE FIELD OVERWRITE predicate_id ON TABLE predicates TYPE string;
DEFINE FIELD OVERWRITE label ON TABLE predicates TYPE string;
DEFINE FIELD OVERWRITE description ON TABLE predicates TYPE string;
DEFINE FIELD OVERWRITE allowed_subject_types ON TABLE predicates TYPE array<string> ASSERT array::all($value, |$type| $type INSIDE ['human', 'agent', 'person', 'organization', 'project', 'workspace', 'conversation', 'document', 'tool', 'place', 'task', 'goal', 'concept', 'other']);
DEFINE FIELD OVERWRITE allowed_object_types ON TABLE predicates TYPE array<string> ASSERT array::all($value, |$type| $type INSIDE ['human', 'agent', 'person', 'organization', 'project', 'workspace', 'conversation', 'document', 'tool', 'place', 'task', 'goal', 'concept', 'other']);
DEFINE FIELD OVERWRITE allowed_use_modes ON TABLE predicates TYPE array<string> ASSERT array::all($value, |$mode| $mode INSIDE ['answer', 'personalize', 'plan', 'act', 'notify', 'inspect', 'export']);
DEFINE FIELD OVERWRITE default_sensitivity ON TABLE predicates TYPE string ASSERT $value INSIDE ['public', 'normal', 'private', 'sensitive', 'secret'];
DEFINE FIELD OVERWRITE conflict_policy ON TABLE predicates TYPE string ASSERT $value INSIDE ['allow_many', 'single_current', 'mutually_exclusive'];
DEFINE FIELD OVERWRITE review_policy ON TABLE predicates TYPE string ASSERT $value INSIDE ['auto_candidate', 'auto_active', 'requires_review'];
DEFINE FIELD OVERWRITE inverse_behavior ON TABLE predicates TYPE string ASSERT $value INSIDE ['none', 'symmetric', 'inverse_predicate'];
DEFINE FIELD OVERWRITE inverse_predicate_id ON TABLE predicates TYPE option<string>;
DEFINE FIELD OVERWRITE proactivity_default ON TABLE predicates TYPE int ASSERT $value >= 0 AND $value <= 6;
DEFINE FIELD OVERWRITE merge_hints ON TABLE predicates TYPE object FLEXIBLE DEFAULT {};
DEFINE FIELD OVERWRITE synonym_hints ON TABLE predicates TYPE array<string> DEFAULT [];
DEFINE FIELD OVERWRITE extraction_hints ON TABLE predicates TYPE object FLEXIBLE DEFAULT {};
DEFINE FIELD OVERWRITE created_at ON TABLE predicates TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE updated_at ON TABLE predicates TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS predicates_predicate_id ON TABLE predicates COLUMNS predicate_id UNIQUE;

DEFINE TABLE IF NOT EXISTS predicate_proposals SCHEMAFULL;
DEFINE FIELD OVERWRITE proposal_id ON TABLE predicate_proposals TYPE string;
DEFINE FIELD OVERWRITE label ON TABLE predicate_proposals TYPE string;
DEFINE FIELD OVERWRITE description ON TABLE predicate_proposals TYPE string;
DEFINE FIELD OVERWRITE proposed_predicate ON TABLE predicate_proposals TYPE object FLEXIBLE;
DEFINE FIELD OVERWRITE status ON TABLE predicate_proposals TYPE string ASSERT $value INSIDE ['candidate', 'approved', 'rejected', 'merged'];
DEFINE FIELD OVERWRITE source_item_id ON TABLE predicate_proposals TYPE option<string>;
DEFINE FIELD OVERWRITE created_at ON TABLE predicate_proposals TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE updated_at ON TABLE predicate_proposals TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS predicate_proposals_proposal_id ON TABLE predicate_proposals COLUMNS proposal_id UNIQUE;

DEFINE TABLE IF NOT EXISTS claims SCHEMAFULL;
DEFINE FIELD OVERWRITE claim_id ON TABLE claims TYPE string;
DEFINE FIELD OVERWRITE subject_entity_id ON TABLE claims TYPE string;
DEFINE FIELD OVERWRITE object_entity_id ON TABLE claims TYPE string;
DEFINE FIELD OVERWRITE predicate_id ON TABLE claims TYPE string;
DEFINE FIELD OVERWRITE fact ON TABLE claims TYPE string;
DEFINE FIELD OVERWRITE status ON TABLE claims TYPE string ASSERT $value INSIDE ['candidate', 'active', 'confirmed', 'disputed', 'superseded', 'archived', 'deleted'];
DEFINE FIELD OVERWRITE sensitivity ON TABLE claims TYPE string ASSERT $value INSIDE ['public', 'normal', 'private', 'sensitive', 'secret'];
DEFINE FIELD OVERWRITE valid_from ON TABLE claims TYPE option<datetime>;
DEFINE FIELD OVERWRITE valid_to ON TABLE claims TYPE option<datetime>;
DEFINE FIELD OVERWRITE observed_at ON TABLE claims TYPE option<datetime>;
DEFINE FIELD OVERWRITE confidence ON TABLE claims TYPE option<float> ASSERT $value = NONE OR ($value >= 0 AND $value <= 1);
DEFINE FIELD OVERWRITE dedupe_fingerprint ON TABLE claims TYPE option<string>;
DEFINE FIELD OVERWRITE retrieval_hints ON TABLE claims TYPE object FLEXIBLE DEFAULT {};
DEFINE FIELD OVERWRITE policy_overrides ON TABLE claims TYPE object FLEXIBLE DEFAULT {};
DEFINE FIELD OVERWRITE metadata ON TABLE claims TYPE object FLEXIBLE DEFAULT {};
DEFINE FIELD OVERWRITE created_at ON TABLE claims TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE updated_at ON TABLE claims TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS claims_claim_id ON TABLE claims COLUMNS claim_id UNIQUE;
DEFINE INDEX IF NOT EXISTS claims_predicate_subject ON TABLE claims COLUMNS predicate_id, subject_entity_id;
DEFINE INDEX IF NOT EXISTS claims_subject ON TABLE claims COLUMNS subject_entity_id;
DEFINE INDEX IF NOT EXISTS claims_object ON TABLE claims COLUMNS object_entity_id;
DEFINE INDEX IF NOT EXISTS claims_predicate_object ON TABLE claims COLUMNS predicate_id, object_entity_id;
DEFINE INDEX IF NOT EXISTS claims_dedupe_fingerprint ON TABLE claims COLUMNS dedupe_fingerprint UNIQUE;

DEFINE TABLE IF NOT EXISTS supported_by SCHEMAFULL;
DEFINE FIELD OVERWRITE relation_id ON TABLE supported_by TYPE string;
DEFINE FIELD OVERWRITE claim_id ON TABLE supported_by TYPE string;
DEFINE FIELD OVERWRITE source_kind ON TABLE supported_by TYPE string ASSERT (($value = 'item' AND source_item_id != NONE AND source_object_type = NONE AND source_object_id = NONE) OR ($value = 'object' AND source_item_id = NONE AND source_object_type != NONE AND source_object_id != NONE));
DEFINE FIELD OVERWRITE source_item_id ON TABLE supported_by TYPE option<string>;
DEFINE FIELD OVERWRITE source_object_type ON TABLE supported_by TYPE option<string> ASSERT $value = NONE OR $value INSIDE ['human', 'agent', 'tool', 'conversation', 'conversation_turn', 'conversation_item', 'entity'];
DEFINE FIELD OVERWRITE source_object_id ON TABLE supported_by TYPE option<string>;
DEFINE FIELD OVERWRITE authority ON TABLE supported_by TYPE string ASSERT $value INSIDE ['human_correction', 'explicit_human_statement', 'document_source', 'repeated_observation', 'agent_inference', 'weak_inference', 'system_rule'];
DEFINE FIELD OVERWRITE excerpt ON TABLE supported_by TYPE option<string>;
DEFINE FIELD OVERWRITE observed_at ON TABLE supported_by TYPE option<datetime>;
DEFINE FIELD OVERWRITE created_by ON TABLE supported_by TYPE string;
DEFINE FIELD OVERWRITE metadata ON TABLE supported_by TYPE object FLEXIBLE DEFAULT {};
DEFINE FIELD OVERWRITE created_at ON TABLE supported_by TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS supported_by_relation_id ON TABLE supported_by COLUMNS relation_id UNIQUE;
DEFINE INDEX IF NOT EXISTS supported_by_claim_source_item ON TABLE supported_by COLUMNS claim_id, source_item_id;
DEFINE INDEX IF NOT EXISTS supported_by_claim_source_object ON TABLE supported_by COLUMNS claim_id, source_object_type, source_object_id;
DEFINE INDEX IF NOT EXISTS supported_by_source_item_claim ON TABLE supported_by COLUMNS source_item_id, claim_id;
DEFINE INDEX IF NOT EXISTS supported_by_source_object_claim ON TABLE supported_by COLUMNS source_object_type, source_object_id, claim_id;

DEFINE TABLE IF NOT EXISTS corrected_by SCHEMAFULL;
DEFINE FIELD OVERWRITE relation_id ON TABLE corrected_by TYPE string;
DEFINE FIELD OVERWRITE claim_id ON TABLE corrected_by TYPE string;
DEFINE FIELD OVERWRITE source_kind ON TABLE corrected_by TYPE string ASSERT (($value = 'item' AND source_item_id != NONE AND source_object_type = NONE AND source_object_id = NONE) OR ($value = 'object' AND source_item_id = NONE AND source_object_type != NONE AND source_object_id != NONE));
DEFINE FIELD OVERWRITE source_item_id ON TABLE corrected_by TYPE option<string>;
DEFINE FIELD OVERWRITE source_object_type ON TABLE corrected_by TYPE option<string> ASSERT $value = NONE OR $value INSIDE ['human', 'agent', 'tool', 'conversation', 'conversation_turn', 'conversation_item', 'entity'];
DEFINE FIELD OVERWRITE source_object_id ON TABLE corrected_by TYPE option<string>;
DEFINE FIELD OVERWRITE authority ON TABLE corrected_by TYPE string ASSERT $value INSIDE ['human_correction'];
DEFINE FIELD OVERWRITE excerpt ON TABLE corrected_by TYPE option<string>;
DEFINE FIELD OVERWRITE observed_at ON TABLE corrected_by TYPE option<datetime>;
DEFINE FIELD OVERWRITE created_by ON TABLE corrected_by TYPE string;
DEFINE FIELD OVERWRITE metadata ON TABLE corrected_by TYPE object FLEXIBLE DEFAULT {};
DEFINE FIELD OVERWRITE created_at ON TABLE corrected_by TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS corrected_by_relation_id ON TABLE corrected_by COLUMNS relation_id UNIQUE;
DEFINE INDEX IF NOT EXISTS corrected_by_claim_source_item ON TABLE corrected_by COLUMNS claim_id, source_item_id;
DEFINE INDEX IF NOT EXISTS corrected_by_claim_source_object ON TABLE corrected_by COLUMNS claim_id, source_object_type, source_object_id;
DEFINE INDEX IF NOT EXISTS corrected_by_source_item_claim ON TABLE corrected_by COLUMNS source_item_id, claim_id;
DEFINE INDEX IF NOT EXISTS corrected_by_source_object_claim ON TABLE corrected_by COLUMNS source_object_type, source_object_id, claim_id;

DEFINE TABLE IF NOT EXISTS contradicted_by SCHEMAFULL;
DEFINE FIELD OVERWRITE relation_id ON TABLE contradicted_by TYPE string;
DEFINE FIELD OVERWRITE claim_id ON TABLE contradicted_by TYPE string;
DEFINE FIELD OVERWRITE source_kind ON TABLE contradicted_by TYPE string ASSERT (($value = 'item' AND source_item_id != NONE AND source_object_type = NONE AND source_object_id = NONE) OR ($value = 'object' AND source_item_id = NONE AND source_object_type != NONE AND source_object_id != NONE));
DEFINE FIELD OVERWRITE source_item_id ON TABLE contradicted_by TYPE option<string>;
DEFINE FIELD OVERWRITE source_object_type ON TABLE contradicted_by TYPE option<string> ASSERT $value = NONE OR $value INSIDE ['human', 'agent', 'tool', 'conversation', 'conversation_turn', 'conversation_item', 'entity'];
DEFINE FIELD OVERWRITE source_object_id ON TABLE contradicted_by TYPE option<string>;
DEFINE FIELD OVERWRITE authority ON TABLE contradicted_by TYPE string ASSERT $value INSIDE ['explicit_human_statement', 'document_source', 'agent_inference', 'weak_inference'];
DEFINE FIELD OVERWRITE excerpt ON TABLE contradicted_by TYPE option<string>;
DEFINE FIELD OVERWRITE observed_at ON TABLE contradicted_by TYPE option<datetime>;
DEFINE FIELD OVERWRITE created_by ON TABLE contradicted_by TYPE string;
DEFINE FIELD OVERWRITE metadata ON TABLE contradicted_by TYPE object FLEXIBLE DEFAULT {};
DEFINE FIELD OVERWRITE created_at ON TABLE contradicted_by TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS contradicted_by_relation_id ON TABLE contradicted_by COLUMNS relation_id UNIQUE;
DEFINE INDEX IF NOT EXISTS contradicted_by_claim_source_item ON TABLE contradicted_by COLUMNS claim_id, source_item_id;
DEFINE INDEX IF NOT EXISTS contradicted_by_claim_source_object ON TABLE contradicted_by COLUMNS claim_id, source_object_type, source_object_id;
DEFINE INDEX IF NOT EXISTS contradicted_by_source_item_claim ON TABLE contradicted_by COLUMNS source_item_id, claim_id;
DEFINE INDEX IF NOT EXISTS contradicted_by_source_object_claim ON TABLE contradicted_by COLUMNS source_object_type, source_object_id, claim_id;

DEFINE TABLE IF NOT EXISTS supersedes SCHEMAFULL;
DEFINE FIELD OVERWRITE relation_id ON TABLE supersedes TYPE string;
DEFINE FIELD OVERWRITE claim_id ON TABLE supersedes TYPE string;
DEFINE FIELD OVERWRITE superseded_claim_id ON TABLE supersedes TYPE string;
DEFINE FIELD OVERWRITE metadata ON TABLE supersedes TYPE object FLEXIBLE DEFAULT {};
DEFINE FIELD OVERWRITE created_at ON TABLE supersedes TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS supersedes_relation_id ON TABLE supersedes COLUMNS relation_id UNIQUE;
DEFINE INDEX IF NOT EXISTS supersedes_claim_pair ON TABLE supersedes COLUMNS claim_id, superseded_claim_id UNIQUE;

DEFINE TABLE IF NOT EXISTS derived_from SCHEMAFULL;
DEFINE FIELD OVERWRITE relation_id ON TABLE derived_from TYPE string;
DEFINE FIELD OVERWRITE claim_id ON TABLE derived_from TYPE string;
DEFINE FIELD OVERWRITE source_claim_id ON TABLE derived_from TYPE string;
DEFINE FIELD OVERWRITE metadata ON TABLE derived_from TYPE object FLEXIBLE DEFAULT {};
DEFINE FIELD OVERWRITE created_at ON TABLE derived_from TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS derived_from_relation_id ON TABLE derived_from COLUMNS relation_id UNIQUE;
DEFINE INDEX IF NOT EXISTS derived_from_claim_pair ON TABLE derived_from COLUMNS claim_id, source_claim_id UNIQUE;

DEFINE TABLE IF NOT EXISTS related_to SCHEMAFULL;
DEFINE FIELD OVERWRITE relation_id ON TABLE related_to TYPE string;
DEFINE FIELD OVERWRITE claim_id ON TABLE related_to TYPE string;
DEFINE FIELD OVERWRITE related_claim_id ON TABLE related_to TYPE string;
DEFINE FIELD OVERWRITE relation_kind ON TABLE related_to TYPE string;
DEFINE FIELD OVERWRITE rationale ON TABLE related_to TYPE string;
DEFINE FIELD OVERWRITE metadata ON TABLE related_to TYPE object FLEXIBLE DEFAULT {};
DEFINE FIELD OVERWRITE created_at ON TABLE related_to TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS related_to_relation_id ON TABLE related_to COLUMNS relation_id UNIQUE;
DEFINE INDEX IF NOT EXISTS related_to_claim_pair ON TABLE related_to COLUMNS claim_id, related_claim_id, relation_kind UNIQUE;

DEFINE TABLE IF NOT EXISTS retrieval_packets SCHEMAFULL;
DEFINE FIELD OVERWRITE run_id ON TABLE retrieval_packets TYPE string;
DEFINE FIELD OVERWRITE requesting_agent_id ON TABLE retrieval_packets TYPE string;
DEFINE FIELD OVERWRITE active_human_ids ON TABLE retrieval_packets TYPE array<string>;
DEFINE FIELD OVERWRITE active_objects ON TABLE retrieval_packets TYPE array DEFAULT [];
DEFINE FIELD OVERWRITE use_mode ON TABLE retrieval_packets TYPE string ASSERT $value INSIDE ['answer', 'personalize', 'plan', 'act', 'notify', 'inspect', 'export'];
DEFINE FIELD OVERWRITE included_claim_ids ON TABLE retrieval_packets TYPE array<string> DEFAULT [];
DEFINE FIELD OVERWRITE redacted_omissions ON TABLE retrieval_packets TYPE array DEFAULT [];
DEFINE FIELD OVERWRITE policy_version ON TABLE retrieval_packets TYPE int ASSERT $value >= 1;
DEFINE FIELD OVERWRITE created_at ON TABLE retrieval_packets TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS retrieval_packets_run_id ON TABLE retrieval_packets COLUMNS run_id;

UPSERT type::record('predicates', 'likes') SET
  predicate_id = 'likes',
  label = 'likes',
  description = 'The subject likes the object.',
  allowed_subject_types = ['human', 'agent', 'person'],
  allowed_object_types = ['human', 'agent', 'person', 'organization', 'project', 'workspace', 'conversation', 'document', 'tool', 'place', 'task', 'goal', 'concept', 'other'],
  allowed_use_modes = ['answer', 'personalize', 'plan'],
  default_sensitivity = 'normal',
  conflict_policy = 'allow_many',
  review_policy = 'auto_candidate',
  inverse_behavior = 'none',
  inverse_predicate_id = NONE,
  proactivity_default = 2,
  merge_hints = { strategy: 'object_identity' },
  synonym_hints = ['enjoys', 'is into'],
  extraction_hints = {},
  updated_at = time::now();
UPSERT type::record('predicates', 'dislikes') SET
  predicate_id = 'dislikes',
  label = 'dislikes',
  description = 'The subject dislikes the object.',
  allowed_subject_types = ['human', 'agent', 'person'],
  allowed_object_types = ['human', 'agent', 'person', 'organization', 'project', 'workspace', 'conversation', 'document', 'tool', 'place', 'task', 'goal', 'concept', 'other'],
  allowed_use_modes = ['answer', 'personalize', 'plan'],
  default_sensitivity = 'normal',
  conflict_policy = 'allow_many',
  review_policy = 'auto_candidate',
  inverse_behavior = 'none',
  inverse_predicate_id = NONE,
  proactivity_default = 2,
  merge_hints = { strategy: 'object_identity' },
  synonym_hints = ['does not like', 'avoids'],
  extraction_hints = {},
  updated_at = time::now();
UPSERT type::record('predicates', 'prefers') SET
  predicate_id = 'prefers',
  label = 'prefers',
  description = 'The subject prefers the object or option.',
  allowed_subject_types = ['human', 'agent', 'person', 'organization', 'project', 'workspace'],
  allowed_object_types = ['human', 'agent', 'person', 'organization', 'project', 'workspace', 'conversation', 'document', 'tool', 'place', 'task', 'goal', 'concept', 'other'],
  allowed_use_modes = ['answer', 'personalize', 'plan', 'act'],
  default_sensitivity = 'normal',
  conflict_policy = 'allow_many',
  review_policy = 'auto_candidate',
  inverse_behavior = 'none',
  inverse_predicate_id = NONE,
  proactivity_default = 2,
  merge_hints = { strategy: 'preference_scope' },
  synonym_hints = ['would rather', 'favors'],
  extraction_hints = {},
  updated_at = time::now();
UPSERT type::record('predicates', 'uses') SET
  predicate_id = 'uses',
  label = 'uses',
  description = 'The subject uses the object.',
  allowed_subject_types = ['human', 'agent', 'person', 'organization', 'project', 'workspace'],
  allowed_object_types = ['tool', 'document', 'project', 'workspace', 'concept', 'other'],
  allowed_use_modes = ['answer', 'personalize', 'plan', 'act', 'inspect'],
  default_sensitivity = 'normal',
  conflict_policy = 'allow_many',
  review_policy = 'auto_candidate',
  inverse_behavior = 'none',
  inverse_predicate_id = NONE,
  proactivity_default = 2,
  merge_hints = { strategy: 'tool_or_object_identity' },
  synonym_hints = ['works with', 'relies on'],
  extraction_hints = {},
  updated_at = time::now();
UPSERT type::record('predicates', 'works_on') SET
  predicate_id = 'works_on',
  label = 'works on',
  description = 'The subject works on the object.',
  allowed_subject_types = ['human', 'agent', 'person', 'organization'],
  allowed_object_types = ['project', 'workspace', 'task', 'goal', 'concept', 'other'],
  allowed_use_modes = ['answer', 'personalize', 'plan', 'act', 'notify', 'inspect'],
  default_sensitivity = 'normal',
  conflict_policy = 'allow_many',
  review_policy = 'auto_candidate',
  inverse_behavior = 'none',
  inverse_predicate_id = NONE,
  proactivity_default = 3,
  merge_hints = { strategy: 'active_work_scope' },
  synonym_hints = ['is working on', 'focuses on'],
  extraction_hints = {},
  updated_at = time::now();
UPSERT type::record('predicates', 'has_note') SET
  predicate_id = 'has_note',
  label = 'has note',
  description = 'The subject has a durable remembered note about the object.',
  allowed_subject_types = ['human', 'agent', 'person'],
  allowed_object_types = ['concept', 'other'],
  allowed_use_modes = ['answer', 'personalize', 'plan', 'inspect'],
  default_sensitivity = 'normal',
  conflict_policy = 'allow_many',
  review_policy = 'auto_candidate',
  inverse_behavior = 'none',
  inverse_predicate_id = NONE,
  proactivity_default = 1,
  merge_hints = { strategy: 'note_identity' },
  synonym_hints = ['remembered', 'noted', 'said'],
  extraction_hints = {},
  updated_at = time::now();
UPSERT type::record('predicates', 'prefers_interaction_style') SET
  predicate_id = 'prefers_interaction_style',
  label = 'prefers interaction style',
  description = 'The subject prefers a specific interaction style.',
  allowed_subject_types = ['human', 'agent', 'person'],
  allowed_object_types = ['concept', 'other'],
  allowed_use_modes = ['answer', 'personalize', 'plan', 'act', 'notify'],
  default_sensitivity = 'normal',
  conflict_policy = 'single_current',
  review_policy = 'auto_candidate',
  inverse_behavior = 'none',
  inverse_predicate_id = NONE,
  proactivity_default = 2,
  merge_hints = { strategy: 'latest_current_style' },
  synonym_hints = ['likes responses to be', 'wants interaction to be'],
  extraction_hints = {},
  updated_at = time::now();
"#;

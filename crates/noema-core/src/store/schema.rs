/// Namespace used by the embedded Noema store.
pub const NOEMA_NAMESPACE: &str = "noema";

/// Database used by the embedded Noema store.
pub const NOEMA_DATABASE: &str = "main";

/// Current schema version for pre-stable local data.
pub const STORE_SCHEMA_VERSION: i64 = 1;

/// Minimal bootstrap used by the first store-runtime test.
pub const STORE_SCHEMA_SQL: &str = r#"
DEFINE TABLE IF NOT EXISTS schema_state SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS version ON TABLE schema_state TYPE int ASSERT $value >= 1;
DEFINE FIELD IF NOT EXISTS name ON TABLE schema_state TYPE string;
DEFINE FIELD IF NOT EXISTS applied_at ON TABLE schema_state TYPE datetime DEFAULT time::now();
UPSERT schema_state:current SET version = 1, name = 'surreal_graph_store_v1', applied_at = time::now();

DEFINE TABLE IF NOT EXISTS humans SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS human_id ON TABLE humans TYPE string;
DEFINE FIELD IF NOT EXISTS display_name ON TABLE humans TYPE string;
DEFINE FIELD IF NOT EXISTS primary_conversation_id ON TABLE humans TYPE option<string>;
DEFINE FIELD IF NOT EXISTS created_at ON TABLE humans TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS updated_at ON TABLE humans TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS humans_human_id ON TABLE humans COLUMNS human_id UNIQUE;

DEFINE TABLE IF NOT EXISTS agents SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS agent_id ON TABLE agents TYPE string;
DEFINE FIELD IF NOT EXISTS display_name ON TABLE agents TYPE string;
DEFINE FIELD IF NOT EXISTS created_at ON TABLE agents TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS updated_at ON TABLE agents TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS agents_agent_id ON TABLE agents COLUMNS agent_id UNIQUE;

DEFINE TABLE IF NOT EXISTS provider_accounts SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS provider_account_id ON TABLE provider_accounts TYPE string;
DEFINE FIELD IF NOT EXISTS provider_kind ON TABLE provider_accounts TYPE string ASSERT $value INSIDE ['codex'];
DEFINE FIELD IF NOT EXISTS account_key ON TABLE provider_accounts TYPE string;
DEFINE FIELD IF NOT EXISTS display_name ON TABLE provider_accounts TYPE string;
DEFINE FIELD IF NOT EXISTS auth_method ON TABLE provider_accounts TYPE string ASSERT $value INSIDE ['oauth_device_code', 'secret_input', 'external_manual', 'none'];
DEFINE FIELD IF NOT EXISTS is_active ON TABLE provider_accounts TYPE bool;
DEFINE FIELD IF NOT EXISTS is_default ON TABLE provider_accounts TYPE bool;
DEFINE FIELD IF NOT EXISTS status ON TABLE provider_accounts TYPE string ASSERT $value INSIDE ['unknown', 'checking', 'authenticated', 'unauthenticated', 'unavailable'];
DEFINE FIELD IF NOT EXISTS last_checked_at ON TABLE provider_accounts TYPE option<string>;
DEFINE FIELD IF NOT EXISTS last_authenticated_at ON TABLE provider_accounts TYPE option<string>;
DEFINE FIELD IF NOT EXISTS last_error_code ON TABLE provider_accounts TYPE option<string>;
DEFINE FIELD IF NOT EXISTS last_error_message ON TABLE provider_accounts TYPE option<string>;
DEFINE FIELD IF NOT EXISTS metadata ON TABLE provider_accounts FLEXIBLE TYPE object;
DEFINE FIELD IF NOT EXISTS created_at ON TABLE provider_accounts TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS updated_at ON TABLE provider_accounts TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS provider_accounts_account_id ON TABLE provider_accounts COLUMNS provider_account_id UNIQUE;
DEFINE INDEX IF NOT EXISTS provider_accounts_kind_key ON TABLE provider_accounts COLUMNS provider_kind, account_key UNIQUE;

DEFINE TABLE IF NOT EXISTS conversations SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS conversation_id ON TABLE conversations TYPE string;
DEFINE FIELD IF NOT EXISTS title ON TABLE conversations TYPE option<string>;
DEFINE FIELD IF NOT EXISTS owner_object_type ON TABLE conversations TYPE string ASSERT $value INSIDE ['human', 'agent', 'conversation', 'workspace', 'project', 'task', 'tool'];
DEFINE FIELD IF NOT EXISTS owner_object_id ON TABLE conversations TYPE string;
DEFINE FIELD IF NOT EXISTS primary_human_id ON TABLE conversations TYPE option<string>;
DEFINE FIELD IF NOT EXISTS primary_agent_id ON TABLE conversations TYPE option<string>;
DEFINE FIELD IF NOT EXISTS provider ON TABLE conversations TYPE string ASSERT $value INSIDE ['codex'];
DEFINE FIELD IF NOT EXISTS model ON TABLE conversations TYPE option<string>;
DEFINE FIELD IF NOT EXISTS cwd ON TABLE conversations TYPE option<string>;
DEFINE FIELD IF NOT EXISTS lifecycle_status ON TABLE conversations TYPE string DEFAULT 'active' ASSERT $value INSIDE ['active', 'archived'];
DEFINE FIELD IF NOT EXISTS agent_status ON TABLE conversations TYPE string DEFAULT 'idle' ASSERT $value INSIDE ['idle', 'input_received', 'thinking', 'tool_running', 'waiting_for_previous_turn_completion', 'interrupting', 'error'];
DEFINE FIELD IF NOT EXISTS metadata ON TABLE conversations FLEXIBLE TYPE object;
DEFINE FIELD IF NOT EXISTS deleted_at ON TABLE conversations TYPE option<string>;
DEFINE FIELD IF NOT EXISTS created_at ON TABLE conversations TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS updated_at ON TABLE conversations TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS conversations_conversation_id ON TABLE conversations COLUMNS conversation_id UNIQUE;

DEFINE TABLE IF NOT EXISTS conversation_turns SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS turn_id ON TABLE conversation_turns TYPE string;
DEFINE FIELD IF NOT EXISTS conversation_id ON TABLE conversation_turns TYPE string;
DEFINE FIELD IF NOT EXISTS trigger_item_id ON TABLE conversation_turns TYPE option<string>;
DEFINE FIELD IF NOT EXISTS status ON TABLE conversation_turns TYPE string ASSERT $value INSIDE ['input_received', 'running', 'waiting_for_tool', 'interrupted', 'completed', 'failed', 'cancelled'];
DEFINE FIELD IF NOT EXISTS metadata ON TABLE conversation_turns FLEXIBLE TYPE object;
DEFINE FIELD IF NOT EXISTS started_at ON TABLE conversation_turns TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS completed_at ON TABLE conversation_turns TYPE option<string>;
DEFINE FIELD IF NOT EXISTS created_at ON TABLE conversation_turns TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS updated_at ON TABLE conversation_turns TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS conversation_turns_turn_id ON TABLE conversation_turns COLUMNS turn_id UNIQUE;
DEFINE INDEX IF NOT EXISTS conversation_turns_conversation_id ON TABLE conversation_turns COLUMNS conversation_id;

DEFINE TABLE IF NOT EXISTS conversation_items SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS item_id ON TABLE conversation_items TYPE string;
DEFINE FIELD IF NOT EXISTS conversation_id ON TABLE conversation_items TYPE string;
DEFINE FIELD IF NOT EXISTS turn_id ON TABLE conversation_items TYPE option<string>;
DEFINE FIELD IF NOT EXISTS parent_item_id ON TABLE conversation_items TYPE option<string>;
DEFINE FIELD IF NOT EXISTS sequence_index ON TABLE conversation_items TYPE int;
DEFINE FIELD IF NOT EXISTS kind ON TABLE conversation_items TYPE string ASSERT $value INSIDE ['user_text', 'assistant_text', 'activity', 'a2ui_card', 'tool_call', 'tool_result', 'approval_request', 'approval_result', 'error_notice'];
DEFINE FIELD IF NOT EXISTS status ON TABLE conversation_items TYPE string ASSERT $value INSIDE ['pending', 'running', 'completed', 'failed', 'cancelled', 'interrupted'];
DEFINE FIELD IF NOT EXISTS author_actor_id ON TABLE conversation_items TYPE string;
DEFINE FIELD IF NOT EXISTS content_text ON TABLE conversation_items TYPE option<string>;
DEFINE FIELD IF NOT EXISTS payload_json ON TABLE conversation_items FLEXIBLE TYPE object;
DEFINE FIELD IF NOT EXISTS metadata ON TABLE conversation_items FLEXIBLE TYPE object;
DEFINE FIELD IF NOT EXISTS deleted_at ON TABLE conversation_items TYPE option<string>;
DEFINE FIELD IF NOT EXISTS created_at ON TABLE conversation_items TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS updated_at ON TABLE conversation_items TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS conversation_items_item_id ON TABLE conversation_items COLUMNS item_id UNIQUE;
DEFINE INDEX IF NOT EXISTS conversation_items_conversation_sequence ON TABLE conversation_items COLUMNS conversation_id, sequence_index UNIQUE;

DEFINE TABLE IF NOT EXISTS entities SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS entity_id ON TABLE entities TYPE string;
DEFINE FIELD IF NOT EXISTS entity_type ON TABLE entities TYPE string ASSERT $value INSIDE ['human', 'agent', 'person', 'organization', 'project', 'workspace', 'conversation', 'document', 'tool', 'place', 'task', 'goal', 'concept', 'other'];
DEFINE FIELD IF NOT EXISTS canonical_name ON TABLE entities TYPE string;
DEFINE FIELD IF NOT EXISTS aliases ON TABLE entities TYPE array<string> DEFAULT [];
DEFINE FIELD IF NOT EXISTS linked_object_type ON TABLE entities TYPE option<string>;
DEFINE FIELD IF NOT EXISTS linked_object_id ON TABLE entities TYPE option<string>;
DEFINE FIELD IF NOT EXISTS metadata ON TABLE entities FLEXIBLE TYPE object DEFAULT {};
DEFINE FIELD IF NOT EXISTS created_at ON TABLE entities TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS updated_at ON TABLE entities TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS entities_entity_id ON TABLE entities COLUMNS entity_id UNIQUE;

DEFINE TABLE IF NOT EXISTS predicates SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS predicate_id ON TABLE predicates TYPE string;
DEFINE FIELD IF NOT EXISTS label ON TABLE predicates TYPE string;
DEFINE FIELD IF NOT EXISTS description ON TABLE predicates TYPE string;
DEFINE FIELD IF NOT EXISTS allowed_subject_types ON TABLE predicates TYPE array<string> ASSERT array::all($value, |$type| $type INSIDE ['human', 'agent', 'person', 'organization', 'project', 'workspace', 'conversation', 'document', 'tool', 'place', 'task', 'goal', 'concept', 'other']);
DEFINE FIELD IF NOT EXISTS allowed_object_types ON TABLE predicates TYPE array<string> ASSERT array::all($value, |$type| $type INSIDE ['human', 'agent', 'person', 'organization', 'project', 'workspace', 'conversation', 'document', 'tool', 'place', 'task', 'goal', 'concept', 'other']);
DEFINE FIELD IF NOT EXISTS allowed_use_modes ON TABLE predicates TYPE array<string> ASSERT array::all($value, |$mode| $mode INSIDE ['answer', 'personalize', 'plan', 'act', 'notify', 'inspect', 'export']);
DEFINE FIELD IF NOT EXISTS default_sensitivity ON TABLE predicates TYPE string ASSERT $value INSIDE ['public', 'normal', 'private', 'sensitive', 'secret'];
DEFINE FIELD IF NOT EXISTS conflict_policy ON TABLE predicates TYPE string ASSERT $value INSIDE ['allow_many', 'single_current', 'mutually_exclusive'];
DEFINE FIELD IF NOT EXISTS review_policy ON TABLE predicates TYPE string ASSERT $value INSIDE ['auto_candidate', 'auto_active', 'requires_review'];
DEFINE FIELD IF NOT EXISTS extraction_hints ON TABLE predicates FLEXIBLE TYPE object DEFAULT {};
DEFINE FIELD IF NOT EXISTS created_at ON TABLE predicates TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS updated_at ON TABLE predicates TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS predicates_predicate_id ON TABLE predicates COLUMNS predicate_id UNIQUE;

DEFINE TABLE IF NOT EXISTS predicate_proposals SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS proposal_id ON TABLE predicate_proposals TYPE string;
DEFINE FIELD IF NOT EXISTS label ON TABLE predicate_proposals TYPE string;
DEFINE FIELD IF NOT EXISTS description ON TABLE predicate_proposals TYPE string;
DEFINE FIELD IF NOT EXISTS proposed_predicate ON TABLE predicate_proposals FLEXIBLE TYPE object;
DEFINE FIELD IF NOT EXISTS status ON TABLE predicate_proposals TYPE string ASSERT $value INSIDE ['candidate', 'approved', 'rejected', 'merged'];
DEFINE FIELD IF NOT EXISTS source_item_id ON TABLE predicate_proposals TYPE option<string>;
DEFINE FIELD IF NOT EXISTS created_at ON TABLE predicate_proposals TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS updated_at ON TABLE predicate_proposals TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS predicate_proposals_proposal_id ON TABLE predicate_proposals COLUMNS proposal_id UNIQUE;

DEFINE TABLE IF NOT EXISTS claims SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS claim_id ON TABLE claims TYPE string;
DEFINE FIELD IF NOT EXISTS subject_entity_id ON TABLE claims TYPE string;
DEFINE FIELD IF NOT EXISTS object_entity_id ON TABLE claims TYPE option<string>;
DEFINE FIELD IF NOT EXISTS predicate_id ON TABLE claims TYPE string;
DEFINE FIELD IF NOT EXISTS fact ON TABLE claims TYPE string;
DEFINE FIELD IF NOT EXISTS status ON TABLE claims TYPE string ASSERT $value INSIDE ['candidate', 'active', 'confirmed', 'disputed', 'superseded', 'archived', 'deleted'];
DEFINE FIELD IF NOT EXISTS sensitivity ON TABLE claims TYPE string ASSERT $value INSIDE ['public', 'normal', 'private', 'sensitive', 'secret'];
DEFINE FIELD IF NOT EXISTS valid_from ON TABLE claims TYPE option<string>;
DEFINE FIELD IF NOT EXISTS valid_to ON TABLE claims TYPE option<string>;
DEFINE FIELD IF NOT EXISTS observed_at ON TABLE claims TYPE option<string>;
DEFINE FIELD IF NOT EXISTS confidence ON TABLE claims TYPE option<float> ASSERT $value = NONE OR ($value >= 0 AND $value <= 1);
DEFINE FIELD IF NOT EXISTS dedupe_fingerprint ON TABLE claims TYPE option<string>;
DEFINE FIELD IF NOT EXISTS retrieval_hints ON TABLE claims FLEXIBLE TYPE object DEFAULT {};
DEFINE FIELD IF NOT EXISTS policy_overrides ON TABLE claims FLEXIBLE TYPE object DEFAULT {};
DEFINE FIELD IF NOT EXISTS metadata ON TABLE claims FLEXIBLE TYPE object DEFAULT {};
DEFINE FIELD IF NOT EXISTS created_at ON TABLE claims TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS updated_at ON TABLE claims TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS claims_claim_id ON TABLE claims COLUMNS claim_id UNIQUE;
DEFINE INDEX IF NOT EXISTS claims_predicate_subject ON TABLE claims COLUMNS predicate_id, subject_entity_id;
DEFINE INDEX IF NOT EXISTS claims_dedupe_fingerprint ON TABLE claims COLUMNS dedupe_fingerprint;

DEFINE TABLE IF NOT EXISTS supported_by SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS relation_id ON TABLE supported_by TYPE string;
DEFINE FIELD IF NOT EXISTS claim_id ON TABLE supported_by TYPE string;
DEFINE FIELD IF NOT EXISTS source_item_id ON TABLE supported_by TYPE string;
DEFINE FIELD IF NOT EXISTS metadata ON TABLE supported_by FLEXIBLE TYPE object DEFAULT {};
DEFINE FIELD IF NOT EXISTS created_at ON TABLE supported_by TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS supported_by_relation_id ON TABLE supported_by COLUMNS relation_id UNIQUE;
DEFINE INDEX IF NOT EXISTS supported_by_claim_source ON TABLE supported_by COLUMNS claim_id, source_item_id UNIQUE;

DEFINE TABLE IF NOT EXISTS corrected_by SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS relation_id ON TABLE corrected_by TYPE string;
DEFINE FIELD IF NOT EXISTS claim_id ON TABLE corrected_by TYPE string;
DEFINE FIELD IF NOT EXISTS correcting_claim_id ON TABLE corrected_by TYPE string;
DEFINE FIELD IF NOT EXISTS metadata ON TABLE corrected_by FLEXIBLE TYPE object DEFAULT {};
DEFINE FIELD IF NOT EXISTS created_at ON TABLE corrected_by TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS corrected_by_relation_id ON TABLE corrected_by COLUMNS relation_id UNIQUE;
DEFINE INDEX IF NOT EXISTS corrected_by_claim_pair ON TABLE corrected_by COLUMNS claim_id, correcting_claim_id UNIQUE;

DEFINE TABLE IF NOT EXISTS contradicted_by SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS relation_id ON TABLE contradicted_by TYPE string;
DEFINE FIELD IF NOT EXISTS claim_id ON TABLE contradicted_by TYPE string;
DEFINE FIELD IF NOT EXISTS contradicting_claim_id ON TABLE contradicted_by TYPE string;
DEFINE FIELD IF NOT EXISTS metadata ON TABLE contradicted_by FLEXIBLE TYPE object DEFAULT {};
DEFINE FIELD IF NOT EXISTS created_at ON TABLE contradicted_by TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS contradicted_by_relation_id ON TABLE contradicted_by COLUMNS relation_id UNIQUE;
DEFINE INDEX IF NOT EXISTS contradicted_by_claim_pair ON TABLE contradicted_by COLUMNS claim_id, contradicting_claim_id UNIQUE;

DEFINE TABLE IF NOT EXISTS supersedes SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS relation_id ON TABLE supersedes TYPE string;
DEFINE FIELD IF NOT EXISTS claim_id ON TABLE supersedes TYPE string;
DEFINE FIELD IF NOT EXISTS superseded_claim_id ON TABLE supersedes TYPE string;
DEFINE FIELD IF NOT EXISTS metadata ON TABLE supersedes FLEXIBLE TYPE object DEFAULT {};
DEFINE FIELD IF NOT EXISTS created_at ON TABLE supersedes TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS supersedes_relation_id ON TABLE supersedes COLUMNS relation_id UNIQUE;
DEFINE INDEX IF NOT EXISTS supersedes_claim_pair ON TABLE supersedes COLUMNS claim_id, superseded_claim_id UNIQUE;

DEFINE TABLE IF NOT EXISTS derived_from SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS relation_id ON TABLE derived_from TYPE string;
DEFINE FIELD IF NOT EXISTS claim_id ON TABLE derived_from TYPE string;
DEFINE FIELD IF NOT EXISTS source_claim_id ON TABLE derived_from TYPE string;
DEFINE FIELD IF NOT EXISTS metadata ON TABLE derived_from FLEXIBLE TYPE object DEFAULT {};
DEFINE FIELD IF NOT EXISTS created_at ON TABLE derived_from TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS derived_from_relation_id ON TABLE derived_from COLUMNS relation_id UNIQUE;
DEFINE INDEX IF NOT EXISTS derived_from_claim_pair ON TABLE derived_from COLUMNS claim_id, source_claim_id UNIQUE;

DEFINE TABLE IF NOT EXISTS retrieval_packets SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS run_id ON TABLE retrieval_packets TYPE string;
DEFINE FIELD IF NOT EXISTS requesting_agent_id ON TABLE retrieval_packets TYPE string;
DEFINE FIELD IF NOT EXISTS active_human_ids ON TABLE retrieval_packets TYPE array<string>;
DEFINE FIELD IF NOT EXISTS active_objects ON TABLE retrieval_packets FLEXIBLE TYPE array DEFAULT [];
DEFINE FIELD IF NOT EXISTS use_mode ON TABLE retrieval_packets TYPE string ASSERT $value INSIDE ['answer', 'personalize', 'plan', 'act', 'notify', 'inspect', 'export'];
DEFINE FIELD IF NOT EXISTS included_claim_ids ON TABLE retrieval_packets TYPE array<string> DEFAULT [];
DEFINE FIELD IF NOT EXISTS redacted_omissions ON TABLE retrieval_packets FLEXIBLE TYPE array DEFAULT [];
DEFINE FIELD IF NOT EXISTS policy_version ON TABLE retrieval_packets TYPE int ASSERT $value >= 1;
DEFINE FIELD IF NOT EXISTS created_at ON TABLE retrieval_packets TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS retrieval_packets_run_id ON TABLE retrieval_packets COLUMNS run_id;

UPSERT type::thing('predicates', 'likes') SET
  predicate_id = 'likes',
  label = 'likes',
  description = 'The subject likes the object.',
  allowed_subject_types = ['human', 'agent', 'person'],
  allowed_object_types = ['human', 'agent', 'person', 'organization', 'project', 'workspace', 'conversation', 'document', 'tool', 'place', 'task', 'goal', 'concept', 'other'],
  allowed_use_modes = ['answer', 'personalize', 'plan'],
  default_sensitivity = 'normal',
  conflict_policy = 'allow_many',
  review_policy = 'auto_candidate',
  extraction_hints = {},
  updated_at = time::now();
UPSERT type::thing('predicates', 'dislikes') SET
  predicate_id = 'dislikes',
  label = 'dislikes',
  description = 'The subject dislikes the object.',
  allowed_subject_types = ['human', 'agent', 'person'],
  allowed_object_types = ['human', 'agent', 'person', 'organization', 'project', 'workspace', 'conversation', 'document', 'tool', 'place', 'task', 'goal', 'concept', 'other'],
  allowed_use_modes = ['answer', 'personalize', 'plan'],
  default_sensitivity = 'normal',
  conflict_policy = 'allow_many',
  review_policy = 'auto_candidate',
  extraction_hints = {},
  updated_at = time::now();
UPSERT type::thing('predicates', 'prefers') SET
  predicate_id = 'prefers',
  label = 'prefers',
  description = 'The subject prefers the object or option.',
  allowed_subject_types = ['human', 'agent', 'person', 'organization', 'project', 'workspace'],
  allowed_object_types = ['human', 'agent', 'person', 'organization', 'project', 'workspace', 'conversation', 'document', 'tool', 'place', 'task', 'goal', 'concept', 'other'],
  allowed_use_modes = ['answer', 'personalize', 'plan', 'act'],
  default_sensitivity = 'normal',
  conflict_policy = 'allow_many',
  review_policy = 'auto_candidate',
  extraction_hints = {},
  updated_at = time::now();
UPSERT type::thing('predicates', 'uses') SET
  predicate_id = 'uses',
  label = 'uses',
  description = 'The subject uses the object.',
  allowed_subject_types = ['human', 'agent', 'person', 'organization', 'project', 'workspace'],
  allowed_object_types = ['tool', 'document', 'project', 'workspace', 'concept', 'other'],
  allowed_use_modes = ['answer', 'personalize', 'plan', 'act', 'inspect'],
  default_sensitivity = 'normal',
  conflict_policy = 'allow_many',
  review_policy = 'auto_candidate',
  extraction_hints = {},
  updated_at = time::now();
UPSERT type::thing('predicates', 'works_on') SET
  predicate_id = 'works_on',
  label = 'works on',
  description = 'The subject works on the object.',
  allowed_subject_types = ['human', 'agent', 'person', 'organization'],
  allowed_object_types = ['project', 'workspace', 'task', 'goal', 'concept', 'other'],
  allowed_use_modes = ['answer', 'personalize', 'plan', 'act', 'notify', 'inspect'],
  default_sensitivity = 'normal',
  conflict_policy = 'allow_many',
  review_policy = 'auto_candidate',
  extraction_hints = {},
  updated_at = time::now();
UPSERT type::thing('predicates', 'prefers_interaction_style') SET
  predicate_id = 'prefers_interaction_style',
  label = 'prefers interaction style',
  description = 'The subject prefers a specific interaction style.',
  allowed_subject_types = ['human', 'agent', 'person'],
  allowed_object_types = ['concept', 'other'],
  allowed_use_modes = ['answer', 'personalize', 'plan', 'act', 'notify'],
  default_sensitivity = 'normal',
  conflict_policy = 'single_current',
  review_policy = 'auto_candidate',
  extraction_hints = {},
  updated_at = time::now();
"#;

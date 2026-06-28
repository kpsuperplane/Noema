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
DEFINE FIELD IF NOT EXISTS metadata ON TABLE provider_accounts TYPE object;
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
DEFINE FIELD IF NOT EXISTS metadata ON TABLE conversations TYPE object;
DEFINE FIELD IF NOT EXISTS deleted_at ON TABLE conversations TYPE option<string>;
DEFINE FIELD IF NOT EXISTS created_at ON TABLE conversations TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS updated_at ON TABLE conversations TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS conversations_conversation_id ON TABLE conversations COLUMNS conversation_id UNIQUE;

DEFINE TABLE IF NOT EXISTS conversation_turns SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS turn_id ON TABLE conversation_turns TYPE string;
DEFINE FIELD IF NOT EXISTS conversation_id ON TABLE conversation_turns TYPE string;
DEFINE FIELD IF NOT EXISTS trigger_item_id ON TABLE conversation_turns TYPE option<string>;
DEFINE FIELD IF NOT EXISTS status ON TABLE conversation_turns TYPE string ASSERT $value INSIDE ['input_received', 'running', 'waiting_for_tool', 'interrupted', 'completed', 'failed', 'cancelled'];
DEFINE FIELD IF NOT EXISTS metadata ON TABLE conversation_turns TYPE object;
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
DEFINE FIELD IF NOT EXISTS payload_json ON TABLE conversation_items TYPE object;
DEFINE FIELD IF NOT EXISTS metadata ON TABLE conversation_items TYPE object;
DEFINE FIELD IF NOT EXISTS deleted_at ON TABLE conversation_items TYPE option<string>;
DEFINE FIELD IF NOT EXISTS created_at ON TABLE conversation_items TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS updated_at ON TABLE conversation_items TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS conversation_items_item_id ON TABLE conversation_items COLUMNS item_id UNIQUE;
DEFINE INDEX IF NOT EXISTS conversation_items_conversation_sequence ON TABLE conversation_items COLUMNS conversation_id, sequence_index UNIQUE;
"#;

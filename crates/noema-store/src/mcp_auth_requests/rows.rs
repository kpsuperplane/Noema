use super::*;
use rusqlite::{OptionalExtension, Row, params};

pub(super) const REQUEST_SELECT: &str = r#"
SELECT requests.request_id, requests.revision, requests.owner_human_id,
       requests.conversation_id, requests.turn_id, requests.task_id, requests.run_id,
       requests.task_generation,
       requests.requesting_agent_id, requests.mcp_server_id, requests.adapter_connection_id,
       requests.challenge_kind, requests.authority_revision,
       COALESCE(servers.display_name, requests.mcp_server_id, requests.adapter_connection_id),
       requests.capability_name, requests.operation_token, requests.input_schema_json,
       requests.protected_arguments_ref, requests.arguments_sha256,
       requests.provider_selection_digest, requests.output_index,
       requests.call_id, requests.provider_call_id, requests.provider_name,
       requests.governed_action_id, requests.governed_action_revision,
       requests.authentication_attempt_id, requests.state, requests.output_json,
       requests.failure_code, requests.supersession_reason,
       requests.created_at, requests.updated_at,
       requests.result_context_json
FROM capability_auth_requests requests
LEFT JOIN mcp_servers servers ON servers.mcp_server_id = requests.mcp_server_id
"#;

pub(super) fn request_by_id(
    connection: &rusqlite::Connection,
    request_id: &str,
) -> Result<Option<CapabilityAuthenticationRequestRecord>, StoreError> {
    connection
        .query_row(
            &format!("{REQUEST_SELECT} WHERE requests.request_id = ?1"),
            [request_id],
            request_from_row,
        )
        .optional()
        .map_err(Into::into)
}

pub(super) fn request_by_origin(
    connection: &rusqlite::Connection,
    conversation_id: Option<&str>,
    turn_id: Option<&str>,
    run_id: Option<&str>,
    output_index: i64,
) -> Result<Option<CapabilityAuthenticationRequestRecord>, StoreError> {
    connection
        .query_row(
            &format!(
                "{REQUEST_SELECT} WHERE (requests.conversation_id = ?1 AND requests.turn_id = ?2 AND requests.output_index = ?4) OR (requests.run_id = ?3 AND requests.output_index = ?4)"
            ),
            params![conversation_id, turn_id, run_id, output_index],
            request_from_row,
        )
        .optional()
        .map_err(Into::into)
}

pub(super) fn request_by_governed_action(
    connection: &rusqlite::Connection,
    action_id: &str,
    revision: u64,
) -> Result<Option<CapabilityAuthenticationRequestRecord>, StoreError> {
    let revision = i64::try_from(revision)
        .map_err(|_| conflict("capability authentication action revision is invalid"))?;
    connection
        .query_row(
            &format!(
                "{REQUEST_SELECT} WHERE requests.governed_action_id = ?1 AND requests.governed_action_revision = ?2"
            ),
            params![action_id, revision],
            request_from_row,
        )
        .optional()
        .map_err(Into::into)
}

pub(super) fn request_from_row(
    row: &Row<'_>,
) -> rusqlite::Result<CapabilityAuthenticationRequestRecord> {
    let revision = row.get::<_, i64>(1)?;
    let task_generation = row.get::<_, Option<i64>>(7)?;
    let mcp_server_id = row.get::<_, Option<String>>(9)?;
    let adapter_connection_id = row.get::<_, Option<String>>(10)?;
    let authority_kind = match (mcp_server_id, adapter_connection_id) {
        (Some(authority_id), None) => (
            CapabilityAuthenticationAuthorityKind::McpServer,
            authority_id,
        ),
        (None, Some(authority_id)) => (
            CapabilityAuthenticationAuthorityKind::AdapterConnection,
            authority_id,
        ),
        _ => {
            return Err(invalid_row(
                10,
                "invalid capability authentication authority",
            ));
        }
    };
    let challenge_kind = match row.get::<_, String>(11)?.as_str() {
        "reauthenticate" => CapabilityAuthenticationChallengeKind::Reauthenticate,
        "replace_credential" => CapabilityAuthenticationChallengeKind::ReplaceCredential,
        _ => return Err(invalid_row(11, "invalid authentication challenge kind")),
    };
    let challenge = CapabilityAuthenticationChallenge::new(
        challenge_kind,
        authority_kind.0,
        authority_kind.1,
        row.get::<_, String>(12)?,
    )
    .map_err(|_| invalid_row(12, "invalid capability authentication challenge"))?;
    let output_index = row.get::<_, i64>(20)?;
    let governed_action_id = row.get::<_, Option<String>>(24)?;
    let governed_action_revision = row.get::<_, Option<i64>>(25)?;
    Ok(CapabilityAuthenticationRequestRecord {
        request_id: row.get(0)?,
        revision: u64::try_from(revision)
            .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(1, revision))?,
        owner_human_id: row.get(2)?,
        conversation_id: row.get(3)?,
        turn_id: row.get(4)?,
        task_id: row.get(5)?,
        run_id: row.get(6)?,
        task_generation: task_generation
            .map(|generation| {
                u64::try_from(generation)
                    .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(7, generation))
            })
            .transpose()?,
        requesting_agent_id: row.get(8)?,
        challenge,
        authority_display_name: row.get(13)?,
        capability_name: row.get(14)?,
        operation_token: row.get(15)?,
        input_schema: serde_json::from_str(&row.get::<_, String>(16)?).map_err(json_error)?,
        protected_arguments_ref: row.get(17)?,
        arguments_sha256: row.get(18)?,
        provider_selection_digest: row.get(19)?,
        output_index: usize::try_from(output_index)
            .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(20, output_index))?,
        call_id: row.get(21)?,
        provider_call_id: row.get(22)?,
        provider_name: row.get(23)?,
        governed_action: match (governed_action_id, governed_action_revision) {
            (Some(id), Some(revision)) => Some((
                id,
                u64::try_from(revision)
                    .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(25, revision))?,
            )),
            _ => None,
        },
        authentication_attempt_id: row.get(26)?,
        state: CapabilityAuthenticationRequestState::parse(&row.get::<_, String>(27)?).map_err(
            |error| {
                rusqlite::Error::FromSqlConversionFailure(
                    27,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            },
        )?,
        output: row
            .get::<_, Option<String>>(28)?
            .map(|value| serde_json::from_str(&value).map_err(json_error))
            .transpose()?,
        failure_code: row.get(29)?,
        supersession_reason: row.get(30)?,
        created_at: row.get(31)?,
        updated_at: row.get(32)?,
        result_context: serde_json::from_str(&row.get::<_, String>(33)?).map_err(json_error)?,
    })
}

fn invalid_row(index: usize, message: &'static str) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        index,
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::other(message)),
    )
}

fn json_error(error: serde_json::Error) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
}

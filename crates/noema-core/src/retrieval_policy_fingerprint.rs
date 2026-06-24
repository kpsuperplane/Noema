//! Deterministic retrieval-policy fingerprints for SQLite memory rows.

use crate::memory_persistence::MemoryPersistenceError;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use std::fmt::Write as _;

const FINGERPRINT_VERSION: &str = "retrieval-policy-v1";
const SHA256_INITIAL_STATE: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];
const SHA256_ROUND_CONSTANTS: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

pub(crate) fn current_fingerprint(
    conn: &Connection,
    memory_id: &str,
) -> Result<String, MemoryPersistenceError> {
    let basis = fingerprint_basis(conn, memory_id)?;
    let canonical = serde_json::to_string(&basis)?;
    Ok(format!("sha256:{}", sha256_hex(canonical.as_bytes())))
}

fn fingerprint_basis(conn: &Connection, memory_id: &str) -> Result<Value, MemoryPersistenceError> {
    let memory = memory_basis(conn, memory_id)?;
    Ok(json!({
        "version": FINGERPRINT_VERSION,
        "memory": memory,
        "subjects": subjects_basis(conn, memory_id)?,
        "participants": participants_basis(conn, memory_id)?,
        "purpose_rules": purpose_rules_basis(conn, memory_id)?,
        "trusted_object_links": object_links_basis(conn, memory_id)?,
        "access_grants": access_grants_basis(conn, memory_id)?,
        "relationships": relationships_basis(conn, memory_id)?,
        "provenance": provenance_basis(conn, memory_id)?,
    }))
}

fn memory_basis(conn: &Connection, memory_id: &str) -> Result<Value, MemoryPersistenceError> {
    conn.query_row(
        r"
        SELECT
          memory_id,
          home_scope_id,
          memory_type,
          title,
          content,
          structured_value,
          status,
          sensitivity,
          proactivity_level,
          retrieval_policy_version,
          participant_visibility_policy,
          external_egress_policy,
          owner_principal_id,
          authority_level,
          extraction_method,
          valid_from,
          valid_to,
          expires_at
        FROM memory_items
        WHERE memory_id = ?1
        ",
        params![memory_id],
        |row| {
            Ok(json!({
                "memory_id": row.get::<_, String>(0)?,
                "home_scope_id": row.get::<_, String>(1)?,
                "memory_type": row.get::<_, String>(2)?,
                "title": row.get::<_, String>(3)?,
                "content": row.get::<_, String>(4)?,
                "structured_value": json_value(row.get::<_, String>(5)?),
                "status": row.get::<_, String>(6)?,
                "sensitivity": row.get::<_, String>(7)?,
                "proactivity_level": row.get::<_, i64>(8)?,
                "retrieval_policy_version": row.get::<_, i64>(9)?,
                "participant_visibility_policy": row.get::<_, String>(10)?,
                "external_egress_policy": row.get::<_, String>(11)?,
                "owner_principal_id": row.get::<_, Option<String>>(12)?,
                "authority_level": row.get::<_, String>(13)?,
                "extraction_method": row.get::<_, String>(14)?,
                "valid_from": row.get::<_, Option<String>>(15)?,
                "valid_to": row.get::<_, Option<String>>(16)?,
                "expires_at": row.get::<_, Option<String>>(17)?,
            }))
        },
    )
    .optional()
    .map_err(MemoryPersistenceError::Sqlite)?
    .ok_or_else(|| MemoryPersistenceError::MemoryNotFound {
        memory_id: memory_id.to_string(),
    })
}

fn subjects_basis(
    conn: &Connection,
    memory_id: &str,
) -> Result<Vec<Value>, MemoryPersistenceError> {
    rows_to_json(
        conn,
        r"
        SELECT entity_id, role
        FROM memory_subjects
        WHERE memory_id = ?1
        ORDER BY entity_id ASC, role ASC
        ",
        memory_id,
        |row| {
            Ok(json!({
                "entity_id": row.get::<_, String>(0)?,
                "role": row.get::<_, String>(1)?,
            }))
        },
    )
}

fn participants_basis(
    conn: &Connection,
    memory_id: &str,
) -> Result<Vec<Value>, MemoryPersistenceError> {
    rows_to_json(
        conn,
        r"
        SELECT principal_id, role
        FROM memory_participants
        WHERE memory_id = ?1
        ORDER BY principal_id ASC, role ASC
        ",
        memory_id,
        |row| {
            Ok(json!({
                "principal_id": row.get::<_, String>(0)?,
                "role": row.get::<_, String>(1)?,
            }))
        },
    )
}

fn purpose_rules_basis(
    conn: &Connection,
    memory_id: &str,
) -> Result<Vec<Value>, MemoryPersistenceError> {
    rows_to_json(
        conn,
        r"
        SELECT purpose, effect
        FROM memory_retrieval_purpose_rules
        WHERE memory_id = ?1
        ORDER BY purpose ASC, effect ASC
        ",
        memory_id,
        |row| {
            Ok(json!({
                "purpose": row.get::<_, String>(0)?,
                "effect": row.get::<_, String>(1)?,
            }))
        },
    )
}

fn object_links_basis(
    conn: &Connection,
    memory_id: &str,
) -> Result<Vec<Value>, MemoryPersistenceError> {
    rows_to_json(
        conn,
        r"
        SELECT
          object_type,
          object_id,
          relation,
          resolver_principal_id,
          resolver_version,
          source_run_id,
          authorized_scope_id
        FROM memory_retrieval_object_links
        WHERE memory_id = ?1
        ORDER BY object_type ASC, object_id ASC, relation ASC
        ",
        memory_id,
        |row| {
            Ok(json!({
                "object_type": row.get::<_, String>(0)?,
                "object_id": row.get::<_, String>(1)?,
                "relation": row.get::<_, String>(2)?,
                "resolver_principal_id": row.get::<_, Option<String>>(3)?,
                "resolver_version": row.get::<_, Option<String>>(4)?,
                "source_run_id": row.get::<_, Option<String>>(5)?,
                "authorized_scope_id": row.get::<_, Option<String>>(6)?,
            }))
        },
    )
}

fn access_grants_basis(
    conn: &Connection,
    memory_id: &str,
) -> Result<Vec<Value>, MemoryPersistenceError> {
    rows_to_json(
        conn,
        r"
        SELECT
          grant_id,
          memory_id,
          scope_id,
          principal_id,
          permission,
          effect,
          expires_at
        FROM memory_access_grants
        WHERE memory_id = ?1
           OR scope_id = (
             SELECT home_scope_id
             FROM memory_items
             WHERE memory_id = ?1
           )
        ORDER BY principal_id ASC, permission ASC, effect ASC, grant_id ASC
        ",
        memory_id,
        |row| {
            Ok(json!({
                "grant_id": row.get::<_, String>(0)?,
                "memory_id": row.get::<_, Option<String>>(1)?,
                "scope_id": row.get::<_, Option<String>>(2)?,
                "principal_id": row.get::<_, String>(3)?,
                "permission": row.get::<_, String>(4)?,
                "effect": row.get::<_, String>(5)?,
                "expires_at": row.get::<_, Option<String>>(6)?,
            }))
        },
    )
}

fn relationships_basis(
    conn: &Connection,
    memory_id: &str,
) -> Result<Vec<Value>, MemoryPersistenceError> {
    rows_to_json(
        conn,
        r"
        SELECT
          relationship_id,
          home_scope_id,
          subject_entity_id,
          predicate,
          object_entity_id,
          status,
          confidence,
          valid_from,
          valid_to,
          metadata
        FROM relationships
        WHERE memory_id = ?1
        ORDER BY relationship_id ASC
        ",
        memory_id,
        |row| {
            Ok(json!({
                "relationship_id": row.get::<_, String>(0)?,
                "home_scope_id": row.get::<_, String>(1)?,
                "subject_entity_id": row.get::<_, String>(2)?,
                "predicate": row.get::<_, String>(3)?,
                "object_entity_id": row.get::<_, String>(4)?,
                "status": row.get::<_, String>(5)?,
                "confidence": row.get::<_, Option<f64>>(6)?,
                "valid_from": row.get::<_, Option<String>>(7)?,
                "valid_to": row.get::<_, Option<String>>(8)?,
                "metadata": json_value(row.get::<_, String>(9)?),
            }))
        },
    )
}

fn provenance_basis(
    conn: &Connection,
    memory_id: &str,
) -> Result<Vec<Value>, MemoryPersistenceError> {
    rows_to_json(
        conn,
        r"
        SELECT source_type, source_id, relation, evidence_excerpt
        FROM memory_provenance_edges
        WHERE memory_id = ?1
        ORDER BY source_type ASC, source_id ASC, relation ASC, edge_id ASC
        ",
        memory_id,
        |row| {
            Ok(json!({
                "source_type": row.get::<_, String>(0)?,
                "source_id": row.get::<_, String>(1)?,
                "relation": row.get::<_, String>(2)?,
                "evidence_excerpt": row.get::<_, Option<String>>(3)?,
            }))
        },
    )
}

fn rows_to_json(
    conn: &Connection,
    sql: &str,
    memory_id: &str,
    mapper: impl Fn(&rusqlite::Row<'_>) -> rusqlite::Result<Value>,
) -> Result<Vec<Value>, MemoryPersistenceError> {
    let mut stmt = conn.prepare(sql).map_err(MemoryPersistenceError::Sqlite)?;
    let rows = stmt
        .query_map(params![memory_id], mapper)
        .map_err(MemoryPersistenceError::Sqlite)?;
    let mut values = Vec::new();
    for row in rows {
        values.push(row.map_err(MemoryPersistenceError::Sqlite)?);
    }
    Ok(values)
}

fn json_value(value: String) -> Value {
    serde_json::from_str(&value).unwrap_or(Value::String(value))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut state = SHA256_INITIAL_STATE;
    let mut padded = bytes.to_vec();
    let bit_len = (bytes.len() as u64).wrapping_mul(8);
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in padded.chunks_exact(64) {
        let mut words = [0_u32; 64];
        for (index, word) in words.iter_mut().take(16).enumerate() {
            let start = index * 4;
            *word = u32::from_be_bytes([
                chunk[start],
                chunk[start + 1],
                chunk[start + 2],
                chunk[start + 3],
            ]);
        }
        for index in 16..64 {
            let s0 = words[index - 15].rotate_right(7)
                ^ words[index - 15].rotate_right(18)
                ^ (words[index - 15] >> 3);
            let s1 = words[index - 2].rotate_right(17)
                ^ words[index - 2].rotate_right(19)
                ^ (words[index - 2] >> 10);
            words[index] = words[index - 16]
                .wrapping_add(s0)
                .wrapping_add(words[index - 7])
                .wrapping_add(s1);
        }

        let mut a = state[0];
        let mut b = state[1];
        let mut c = state[2];
        let mut d = state[3];
        let mut e = state[4];
        let mut f = state[5];
        let mut g = state[6];
        let mut h = state[7];

        for index in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choice = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(choice)
                .wrapping_add(SHA256_ROUND_CONSTANTS[index])
                .wrapping_add(words[index]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(majority);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        state[0] = state[0].wrapping_add(a);
        state[1] = state[1].wrapping_add(b);
        state[2] = state[2].wrapping_add(c);
        state[3] = state[3].wrapping_add(d);
        state[4] = state[4].wrapping_add(e);
        state[5] = state[5].wrapping_add(f);
        state[6] = state[6].wrapping_add(g);
        state[7] = state[7].wrapping_add(h);
    }

    let mut output = String::with_capacity(64);
    for word in state {
        let _ = write!(output, "{word:08x}");
    }
    output
}

use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::StoreError;
use ring::rand::{SecureRandom, SystemRandom};
use rusqlite::Transaction;

static ID_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Build a [`StoreError::InvalidEnum`] for an unrecognized stored enum label.
pub(super) fn invalid_enum<T>(kind: &'static str, value: &str) -> Result<T, StoreError> {
    Err(StoreError::InvalidEnum {
        kind,
        value: value.to_string(),
    })
}

pub(super) fn allocate_id(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let counter = ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}:{nanos:x}{counter:x}")
}

pub(super) fn now_string() -> String {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or_else(
        |_| "0".to_string(),
        |duration| duration.as_secs().to_string(),
    )
}

const INSTANCE_ADJECTIVES: [&str; 32] = [
    "Amber",
    "Bright",
    "Calm",
    "Clever",
    "Copper",
    "Daring",
    "Gentle",
    "Golden",
    "Hushed",
    "Jolly",
    "Kind",
    "Lively",
    "Mellow",
    "Misty",
    "Nimble",
    "Quiet",
    "Rapid",
    "Ready",
    "Ruby",
    "Silver",
    "Soft",
    "Steady",
    "Sunny",
    "Swift",
    "Tender",
    "Vivid",
    "Warm",
    "Wandering",
    "Wild",
    "Wise",
    "Young",
    "Zephyr",
];

const INSTANCE_NOUNS: [&str; 32] = [
    "Badger", "Beacon", "Brook", "Cedar", "Comet", "Dove", "Falcon", "Finch", "Fox", "Grove",
    "Harbor", "Hawk", "Juniper", "Lark", "Meadow", "Otter", "Pine", "Robin", "Sparrow", "Stone",
    "Summit", "Thistle", "Trail", "Valley", "Willow", "Wren", "Yarrow", "Canyon", "Cloud", "Ember",
    "Maple", "Orchid",
];

pub(super) fn allocate_instance_name(transaction: &Transaction<'_>) -> Result<String, StoreError> {
    let random = SystemRandom::new();
    let mut bytes = [0_u8; 2];
    random
        .fill(&mut bytes)
        .map_err(|_| StoreError::InvariantViolation {
            message: "secure randomness unavailable while naming agent run".to_string(),
        })?;
    let base = format!(
        "{} {}",
        INSTANCE_ADJECTIVES[usize::from(bytes[0]) % INSTANCE_ADJECTIVES.len()],
        INSTANCE_NOUNS[usize::from(bytes[1]) % INSTANCE_NOUNS.len()]
    );
    for suffix in 0..10_000_u32 {
        let candidate = if suffix == 0 {
            base.clone()
        } else {
            format!("{base} {suffix}")
        };
        let exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM agent_runs WHERE instance_name = ?1)",
            [&candidate],
            |row| row.get(0),
        )?;
        if !exists {
            return Ok(candidate);
        }
    }
    Err(StoreError::InvariantViolation {
        message: "could not allocate a unique agent run instance name".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn instance_names_are_friendly_and_unique() {
        let mut connection = Connection::open_in_memory().expect("in-memory sqlite");
        connection
            .execute(
                "CREATE TABLE agent_runs (instance_name TEXT UNIQUE NOT NULL)",
                [],
            )
            .expect("create name table");
        let transaction = connection.transaction().expect("start transaction");
        let first = allocate_instance_name(&transaction).expect("first name");
        transaction
            .execute(
                "INSERT INTO agent_runs (instance_name) VALUES (?1)",
                [&first],
            )
            .expect("insert first name");
        let second = allocate_instance_name(&transaction).expect("second name");
        assert_ne!(first, second);
        assert!(
            first
                .split_once(' ')
                .is_some_and(|(_, noun)| !noun.is_empty())
        );
        assert!(
            second
                .split_once(' ')
                .is_some_and(|(_, noun)| !noun.is_empty())
        );
    }
}

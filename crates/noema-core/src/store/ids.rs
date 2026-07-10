use ring::rand::{SecureRandom, SystemRandom};
use time::{
    OffsetDateTime, UtcOffset,
    format_description::{FormatItem, well_known::Rfc3339},
    macros::format_description,
};

use crate::store::StoreError;

static RFC3339_UTC_MILLIS: &[FormatItem<'static>] =
    format_description!("[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z");

/// Validated persisted RFC 3339 UTC timestamp.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Timestamp(String);

impl Timestamp {
    /// Parse a persisted timestamp, rejecting offsets, missing subseconds, and integer strings.
    pub(super) fn parse(value: &str) -> Result<Self, StoreError> {
        let parsed =
            OffsetDateTime::parse(value, &Rfc3339).map_err(|_| StoreError::InvalidTimestamp {
                value: value.to_string(),
            })?;
        let canonical =
            parsed
                .format(RFC3339_UTC_MILLIS)
                .map_err(|_| StoreError::InvalidTimestamp {
                    value: value.to_string(),
                })?;
        if parsed.offset() != UtcOffset::UTC || canonical != value {
            return Err(StoreError::InvalidTimestamp {
                value: value.to_string(),
            });
        }
        Ok(Self(value.to_string()))
    }

    pub(super) fn into_string(self) -> String {
        self.0
    }
}

pub(super) fn validate_timestamp(value: String) -> Result<String, StoreError> {
    Timestamp::parse(&value).map(Timestamp::into_string)
}

pub(super) fn validate_optional_timestamp(
    value: Option<String>,
) -> Result<Option<String>, StoreError> {
    value.map(validate_timestamp).transpose()
}

/// Build a [`StoreError::InvalidEnum`] for an unrecognized stored enum label.
pub(super) fn invalid_enum<T>(kind: &'static str, value: &str) -> Result<T, StoreError> {
    Err(StoreError::InvalidEnum {
        kind,
        value: value.to_string(),
    })
}

pub(super) fn allocate_id(prefix: &str) -> Result<String, StoreError> {
    allocate_id_with(prefix, |bytes| {
        SystemRandom::new().fill(bytes).map_err(|_| ())
    })
}

fn allocate_id_with(
    prefix: &str,
    fill: impl FnOnce(&mut [u8]) -> Result<(), ()>,
) -> Result<String, StoreError> {
    let mut random = [0_u8; 16];
    fill(&mut random).map_err(|()| StoreError::RandomnessUnavailable)?;
    let suffix = random
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok(format!("{prefix}:{suffix}"))
}

pub(super) fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(RFC3339_UTC_MILLIS)
        .expect("static RFC 3339 UTC format must remain valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persisted_timestamp_is_rfc3339_utc_with_subseconds() {
        let timestamp = now_rfc3339();
        assert!(timestamp.ends_with('Z'));
        assert!(timestamp.contains('.'));
        assert!(Timestamp::parse(&timestamp).is_ok());
    }

    #[test]
    fn persisted_timestamp_rejects_integer_strings() {
        for invalid in [
            "1720000000",
            "2026-07-10T12:00:00Z",
            "2026-07-10T12:00:00.12Z",
            "2026-07-10T12:00:00.1234Z",
            "2026-07-10T08:00:00.123-04:00",
        ] {
            assert!(Timestamp::parse(invalid).is_err(), "accepted {invalid}");
        }
    }

    #[test]
    fn allocated_ids_are_opaque_random_values() {
        let first = allocate_id("approval").expect("random id");
        let second = allocate_id("approval").expect("random id");
        assert_ne!(first, second);
        assert_eq!(first.len(), "approval:".len() + 32);
        assert!(
            first["approval:".len()..]
                .chars()
                .all(|c| c.is_ascii_hexdigit())
        );
    }

    #[test]
    fn allocated_ids_fail_closed_when_randomness_is_unavailable() {
        assert!(matches!(
            allocate_id_with("approval", |_| Err(())),
            Err(StoreError::RandomnessUnavailable)
        ));
    }
}

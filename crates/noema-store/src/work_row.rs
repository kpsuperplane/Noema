use rusqlite::{Row, types::Type};

use crate::sqlite::conversion_failure;

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let hash = ring::digest::digest(&ring::digest::SHA256, bytes);
    let mut encoded = String::with_capacity(64);
    for byte in hash.as_ref() {
        write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
    }
    encoded
}

pub(crate) fn positive_u64(row: &Row<'_>, index: usize) -> rusqlite::Result<u64> {
    positive(row, index)
}

pub(crate) fn positive_u32(row: &Row<'_>, index: usize) -> rusqlite::Result<u32> {
    positive(row, index)
}

fn positive<T>(row: &Row<'_>, index: usize) -> rusqlite::Result<T>
where
    T: TryFrom<i64> + PartialEq + Default,
    T::Error: std::error::Error + Send + Sync + 'static,
{
    let value = T::try_from(row.get::<_, i64>(index)?)
        .map_err(|error| conversion_failure(index, Type::Integer, error))?;
    if value == T::default() {
        Err(invalid(index, "persisted integer must be positive"))
    } else {
        Ok(value)
    }
}

pub(crate) fn nonnegative_u32(row: &Row<'_>, index: usize) -> rusqlite::Result<u32> {
    u32::try_from(row.get::<_, i64>(index)?)
        .map_err(|error| conversion_failure(index, Type::Integer, error))
}

pub(crate) fn nonnegative_u64(row: &Row<'_>, index: usize) -> rusqlite::Result<u64> {
    u64::try_from(row.get::<_, i64>(index)?)
        .map_err(|error| conversion_failure(index, Type::Integer, error))
}

pub(crate) fn strict_bool(row: &Row<'_>, index: usize) -> rusqlite::Result<bool> {
    match row.get::<_, i64>(index)? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(invalid(index, "persisted boolean must be 0 or 1")),
    }
}

pub(crate) fn optional_id<T, E>(
    row: &Row<'_>,
    index: usize,
    parse: impl FnOnce(String) -> Result<T, E>,
) -> rusqlite::Result<Option<T>>
where
    E: std::error::Error + Send + Sync + 'static,
{
    row.get::<_, Option<String>>(index)?
        .map(parse)
        .transpose()
        .map_err(|error| conversion_failure(index, Type::Text, error))
}

pub(crate) fn invalid(index: usize, message: &'static str) -> rusqlite::Error {
    conversion_failure(
        index,
        Type::Text,
        std::io::Error::new(std::io::ErrorKind::InvalidData, message),
    )
}

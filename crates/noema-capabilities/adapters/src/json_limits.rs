//! Shared bounded JSON shape validation for adapter inputs and responses.

use serde::{Deserialize, de};
use serde_json::{Map, Value};

pub(crate) const MAX_JSON_DEPTH: usize = 64;
pub(crate) const MAX_JSON_NODES: usize = 16_384;
pub(crate) const MAX_JSON_COLLECTION: usize = 1_024;
pub(crate) const MAX_JSON_STRING_BYTES: usize = 256 * 1024;

pub(crate) fn validate_json_shape(value: &Value) -> bool {
    fn visit(value: &Value, depth: usize, nodes: &mut usize) -> bool {
        *nodes += 1;
        if depth > MAX_JSON_DEPTH || *nodes > MAX_JSON_NODES {
            return false;
        }
        match value {
            Value::String(value) => value.len() <= MAX_JSON_STRING_BYTES,
            Value::Array(values) => {
                values.len() <= MAX_JSON_COLLECTION
                    && values.iter().all(|value| visit(value, depth + 1, nodes))
            }
            Value::Object(values) => {
                values.len() <= MAX_JSON_COLLECTION
                    && values.iter().all(|(key, value)| {
                        key.len() <= MAX_JSON_STRING_BYTES && visit(value, depth + 1, nodes)
                    })
            }
            _ => true,
        }
    }
    visit(value, 0, &mut 0)
}

/// Parse JSON while rejecting duplicate object keys before `serde_json::Value`
/// can collapse them into one value.
pub(crate) fn parse_without_duplicate_keys(bytes: &[u8]) -> Result<Value, serde_json::Error> {
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let value = UniqueValue::deserialize(&mut deserializer)?.0;
    deserializer.end()?;
    Ok(value)
}

struct UniqueValue(Value);

impl<'de> Deserialize<'de> for UniqueValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = UniqueValue;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a JSON value without duplicate object keys")
            }

            fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(UniqueValue(Value::Bool(value)))
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(UniqueValue(Value::from(value)))
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(UniqueValue(Value::from(value)))
            }

            fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(UniqueValue(Value::from(value)))
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(UniqueValue(Value::String(value.to_string())))
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(UniqueValue(Value::String(value)))
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(UniqueValue(Value::Null))
            }

            fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                let mut values = Vec::new();
                while let Some(value) = sequence.next_element::<UniqueValue>()? {
                    values.push(value.0);
                }
                Ok(UniqueValue(Value::Array(values)))
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: de::MapAccess<'de>,
            {
                let mut values = Map::new();
                while let Some(key) = map.next_key::<String>()? {
                    if values.contains_key(&key) {
                        return Err(de::Error::custom("duplicate object key"));
                    }
                    values.insert(key, map.next_value::<UniqueValue>()?.0);
                }
                Ok(UniqueValue(Value::Object(values)))
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}

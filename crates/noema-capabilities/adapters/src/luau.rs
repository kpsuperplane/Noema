//! Fresh, deterministic Luau sandbox for reviewed response transforms.

use crate::{ResponseTransform, network::AdapterHttpResponse};
use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use mlua::{Function, Lua, MultiValue, Table, Value as LuaValue, VmState};
use serde_json::{Map, Number, Value};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

const MEMORY_LIMIT: usize = 16 * 1024 * 1024;
const OUTPUT_LIMIT: usize = 1024 * 1024;
const INTERRUPT_LIMIT: u64 = 1_000_000;
const DEADLINE: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("adapter response transform is invalid")]
pub(crate) struct LuauError;

#[derive(Clone, Copy)]
enum TableKind {
    Array,
    Object,
}

type TableKinds = Rc<RefCell<BTreeMap<usize, (TableKind, Table)>>>;

pub(crate) fn validate_source(source: &str) -> Result<(), LuauError> {
    if !source.trim_start().starts_with("return function(") {
        return Err(LuauError);
    }
    let (lua, _) = sandbox(SandboxProfile::Response).map_err(|_| LuauError)?;
    load_function(&lua, source)
        .map(|_| ())
        .map_err(|_| LuauError)
}

pub(crate) fn transform(
    transform: &ResponseTransform,
    response: &AdapterHttpResponse,
) -> Result<Value, String> {
    let ResponseTransform::Luau { source } = transform;
    let (lua, table_kinds) =
        sandbox(SandboxProfile::Response).map_err(|error| error.to_string())?;
    let function = load_function(&lua, source).map_err(|error| error.to_string())?;
    let input = lua.create_table().map_err(|error| error.to_string())?;
    input
        .set("status", response.status)
        .map_err(|error| error.to_string())?;
    input
        .set(
            "body",
            lua.create_string(&response.body)
                .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
    if let Some(content_type) = &response.content_type {
        input
            .set("content_type", content_type.as_str())
            .map_err(|error| error.to_string())?;
    }
    input.set_readonly(true);
    let output = function
        .call::<LuaValue>(input)
        .map_err(|error| error.to_string())?;
    let output = lua_to_json(output, &table_kinds, 0, &mut 0, &mut BTreeSet::new())
        .map_err(|error| error.to_string())?;
    if !crate::json_limits::validate_json_shape(&output) {
        return Err(LuauError.to_string());
    }
    Ok(output)
}

#[derive(Clone, Copy)]
enum SandboxProfile {
    Response,
    Credential,
    RequestAuth,
}

pub(crate) fn normalize_credential(source: &str, input: &Value) -> Result<Value, LuauError> {
    execute(source, input, SandboxProfile::Credential)
}

pub(crate) fn decorate_request(source: &str, input: &Value) -> Result<Value, LuauError> {
    execute(source, input, SandboxProfile::RequestAuth)
}

fn execute(source: &str, input: &Value, profile: SandboxProfile) -> Result<Value, LuauError> {
    let (lua, table_kinds) = sandbox(profile).map_err(|_| LuauError)?;
    let function = load_function(&lua, source).map_err(|_| LuauError)?;
    let input = json_to_readonly_lua(&lua, input).map_err(|_| LuauError)?;
    let output = function.call::<LuaValue>(input).map_err(|_| LuauError)?;
    let output = lua_to_json(output, &table_kinds, 0, &mut 0, &mut BTreeSet::new())?;
    if serde_json::to_vec(&output).map_err(|_| LuauError)?.len() > OUTPUT_LIMIT {
        return Err(LuauError);
    }
    Ok(output)
}

fn sandbox(profile: SandboxProfile) -> mlua::Result<(Lua, TableKinds)> {
    let lua = Lua::new();
    lua.set_memory_limit(MEMORY_LIMIT)?;
    let started = Instant::now();
    let interrupts = Arc::new(AtomicU64::new(0));
    lua.set_interrupt(move |_| {
        if started.elapsed() > DEADLINE
            || interrupts.fetch_add(1, Ordering::Relaxed) >= INTERRUPT_LIMIT
        {
            return Err(mlua::Error::runtime("transform limit"));
        }
        Ok(VmState::Continue)
    });

    let globals = lua.globals();
    for name in [
        "collectgarbage",
        "coroutine",
        "debug",
        "dofile",
        "getfenv",
        "load",
        "loadfile",
        "loadstring",
        "newproxy",
        "os",
        "package",
        "require",
        "setfenv",
    ] {
        globals.set(name, LuaValue::Nil)?;
    }
    if let Ok(math) = globals.get::<Table>("math") {
        math.set("random", LuaValue::Nil)?;
        math.set("randomseed", LuaValue::Nil)?;
    }

    let table_kinds = Rc::new(RefCell::new(BTreeMap::new()));
    let json = lua.create_table()?;
    json.set("null", LuaValue::NULL)?;
    json.set(
        "decode",
        lua.create_function(|lua, bytes: mlua::LuaString| {
            let value = crate::json_limits::parse_without_duplicate_keys(bytes.as_bytes().as_ref())
                .map_err(|_| mlua::Error::runtime("invalid JSON"))?;
            if !crate::json_limits::validate_json_shape(&value) {
                return Err(mlua::Error::runtime("invalid JSON"));
            }
            json_to_lua(lua, &value)
        })?,
    )?;
    for (name, kind) in [("array", TableKind::Array), ("object", TableKind::Object)] {
        let tags = Rc::clone(&table_kinds);
        json.set(
            name,
            lua.create_function(move |lua, ()| {
                let table = lua.create_table()?;
                tags.borrow_mut()
                    .insert(table.to_pointer() as usize, (kind, table.clone()));
                Ok(table)
            })?,
        )?;
    }
    json.set_readonly(true);
    globals.set("json", json)?;
    if matches!(profile, SandboxProfile::Response) {
        let text = lua.create_table()?;
        text.set(
            "truncate_utf8",
            lua.create_function(|lua, (value, max_bytes): (mlua::LuaString, usize)| {
                if max_bytes > crate::output_schema::MAX_MODEL_RESULT_BYTES {
                    return Err(mlua::Error::runtime("text limit exceeds response bound"));
                }
                let value = value.to_str()?;
                let mut end = value.len().min(max_bytes);
                while !value.is_char_boundary(end) {
                    end -= 1;
                }
                lua.create_string(&value[..end])
            })?,
        )?;
        text.set(
            "decode_base64url_utf8",
            lua.create_function(|lua, (value, max_bytes): (mlua::LuaString, usize)| {
                if max_bytes > crate::output_schema::MAX_MODEL_RESULT_BYTES {
                    return Err(mlua::Error::runtime("text limit exceeds response bound"));
                }
                let decoded = URL_SAFE_NO_PAD
                    .decode(value.as_bytes())
                    .map_err(|_| mlua::Error::runtime("invalid base64url text"))?;
                let decoded = std::str::from_utf8(&decoded)
                    .map_err(|_| mlua::Error::runtime("invalid UTF-8 text"))?;
                let mut end = decoded.len().min(max_bytes);
                while !decoded.is_char_boundary(end) {
                    end -= 1;
                }
                lua.create_string(&decoded[..end])
            })?,
        )?;
        text.set_readonly(true);
        globals.set("text", text)?;
    }
    if matches!(profile, SandboxProfile::RequestAuth) {
        let encoding = lua.create_table()?;
        encoding.set(
            "base64",
            lua.create_function(|lua, value: mlua::LuaString| {
                lua.create_string(STANDARD.encode(value.as_bytes()))
            })?,
        )?;
        encoding.set_readonly(true);
        globals.set("encoding", encoding)?;
    }
    lua.sandbox(true)?;
    drop(globals);
    Ok((lua, table_kinds))
}

fn json_to_readonly_lua(lua: &Lua, value: &Value) -> mlua::Result<LuaValue> {
    let value = json_to_lua(lua, value)?;
    set_readonly_recursive(&value)?;
    Ok(value)
}

fn set_readonly_recursive(value: &LuaValue) -> mlua::Result<()> {
    let LuaValue::Table(table) = value else {
        return Ok(());
    };
    for pair in table.clone().pairs::<LuaValue, LuaValue>() {
        let (_, value) = pair?;
        set_readonly_recursive(&value)?;
    }
    table.set_readonly(true);
    Ok(())
}

fn load_function(lua: &Lua, source: &str) -> mlua::Result<Function> {
    let values = lua
        .load(source)
        .set_name("adapter_transform")
        .eval::<MultiValue>()?;
    if values.len() != 1 {
        return Err(mlua::Error::runtime("transform must return one function"));
    }
    match values.into_iter().next() {
        Some(LuaValue::Function(function)) => Ok(function),
        _ => Err(mlua::Error::runtime("transform must return one function")),
    }
}

fn json_to_lua(lua: &Lua, value: &Value) -> mlua::Result<LuaValue> {
    Ok(match value {
        Value::Null => LuaValue::NULL,
        Value::Bool(value) => LuaValue::Boolean(*value),
        Value::Number(value) if value.is_i64() => LuaValue::Integer(value.as_i64().unwrap_or(0)),
        Value::Number(value) if value.is_u64() => {
            let integer = i64::try_from(value.as_u64().unwrap_or(0))
                .map_err(|_| mlua::Error::runtime("integer out of range"))?;
            LuaValue::Integer(integer)
        }
        Value::Number(value) => LuaValue::Number(
            value
                .as_f64()
                .filter(|value| value.is_finite())
                .ok_or_else(|| mlua::Error::runtime("invalid number"))?,
        ),
        Value::String(value) => LuaValue::String(lua.create_string(value)?),
        Value::Array(values) => {
            let table = lua.create_table_with_capacity(values.len(), 0)?;
            for (index, value) in values.iter().enumerate() {
                table.raw_set(index + 1, json_to_lua(lua, value)?)?;
            }
            LuaValue::Table(table)
        }
        Value::Object(values) => {
            let table = lua.create_table_with_capacity(0, values.len())?;
            for (key, value) in values {
                table.raw_set(key.as_str(), json_to_lua(lua, value)?)?;
            }
            LuaValue::Table(table)
        }
    })
}

#[allow(dead_code)]
fn lua_to_json(
    value: LuaValue,
    table_kinds: &RefCell<BTreeMap<usize, (TableKind, Table)>>,
    depth: usize,
    nodes: &mut usize,
    active: &mut BTreeSet<usize>,
) -> Result<Value, LuauError> {
    *nodes += 1;
    if depth > crate::json_limits::MAX_JSON_DEPTH || *nodes > crate::json_limits::MAX_JSON_NODES {
        return Err(LuauError);
    }
    match value {
        LuaValue::Nil => Err(LuauError),
        LuaValue::Boolean(value) => Ok(Value::Bool(value)),
        LuaValue::Integer(value) => Ok(Value::Number(Number::from(value))),
        LuaValue::Number(value) => Number::from_f64(value).map(Value::Number).ok_or(LuauError),
        LuaValue::String(value) => Ok(Value::String(
            value.to_str().map_err(|_| LuauError)?.to_string(),
        )),
        LuaValue::LightUserData(value) if value.0.is_null() => Ok(Value::Null),
        LuaValue::Table(table) => {
            let pointer = table.to_pointer() as usize;
            if !active.insert(pointer) {
                return Err(LuauError);
            }
            let result = table_to_json(table, table_kinds, depth, nodes, active);
            active.remove(&pointer);
            result
        }
        _ => Err(LuauError),
    }
}

#[allow(dead_code)]
fn table_to_json(
    table: Table,
    table_kinds: &RefCell<BTreeMap<usize, (TableKind, Table)>>,
    depth: usize,
    nodes: &mut usize,
    active: &mut BTreeSet<usize>,
) -> Result<Value, LuauError> {
    let kind = table_kinds
        .borrow()
        .get(&(table.to_pointer() as usize))
        .map(|(kind, _)| *kind);
    let entries = table
        .pairs::<LuaValue, LuaValue>()
        .collect::<mlua::Result<Vec<_>>>()
        .map_err(|_| LuauError)?;
    if entries.len() > crate::json_limits::MAX_JSON_COLLECTION {
        return Err(LuauError);
    }
    let array_like = entries
        .iter()
        .all(|(key, _)| matches!(key, LuaValue::Integer(index) if *index > 0));
    let object_like = entries
        .iter()
        .all(|(key, _)| matches!(key, LuaValue::String(_)));
    match kind {
        Some(TableKind::Array) if !array_like => return Err(LuauError),
        Some(TableKind::Object) if !object_like => return Err(LuauError),
        None if entries.is_empty() || (!array_like && !object_like) => return Err(LuauError),
        _ => {}
    }
    if matches!(kind, Some(TableKind::Array)) || kind.is_none() && array_like {
        let mut values = BTreeMap::new();
        for (key, value) in entries {
            let LuaValue::Integer(index) = key else {
                return Err(LuauError);
            };
            values.insert(index, value);
        }
        if values.keys().copied().ne(1..=values.len() as i64) {
            return Err(LuauError);
        }
        return values
            .into_values()
            .map(|value| lua_to_json(value, table_kinds, depth + 1, nodes, active))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array);
    }
    let mut values = Map::new();
    for (key, value) in entries {
        let LuaValue::String(key) = key else {
            return Err(LuauError);
        };
        let key = key.to_str().map_err(|_| LuauError)?.to_string();
        values.insert(
            key,
            lua_to_json(value, table_kinds, depth + 1, nodes, active)?,
        );
    }
    Ok(Value::Object(values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AdapterCompiler, OutputSchema, OutputType, ResponseTransform};

    fn response(body: &[u8]) -> AdapterHttpResponse {
        AdapterHttpResponse {
            status: 200,
            content_type: Some("application/json".to_string()),
            body: body.to_vec(),
        }
    }

    fn scalar(value_type: OutputType) -> OutputSchema {
        OutputSchema {
            value_type,
            properties: BTreeMap::new(),
            required: Vec::new(),
            additional_properties: None,
            items: None,
            max_bytes: (value_type == OutputType::String).then_some(1024),
            max_items: None,
        }
    }

    fn object(properties: &[(&str, OutputSchema)], required: &[&str]) -> OutputSchema {
        OutputSchema {
            value_type: OutputType::Object,
            properties: properties
                .iter()
                .map(|(name, schema)| ((*name).to_string(), schema.clone()))
                .collect(),
            required: required.iter().map(|name| (*name).to_string()).collect(),
            additional_properties: Some(false),
            items: None,
            max_bytes: None,
            max_items: None,
        }
    }

    fn run(source: &str, schema: &OutputSchema, body: &[u8]) -> Result<Value, String> {
        let value = transform(
            &ResponseTransform::Luau {
                source: source.to_string(),
            },
            &response(body),
        )?;
        crate::output_schema::matches(schema, &value)
            .then_some(value)
            .ok_or_else(|| LuauError.to_string())
    }

    #[test]
    fn transform_decodes_json_and_preserves_explicit_empty_shapes() {
        let schema = object(
            &[
                ("email", scalar(OutputType::String)),
                ("empty", object(&[], &[])),
                ("missing", scalar(OutputType::Null)),
            ],
            &["email", "empty", "missing"],
        );
        let value = run(
            r#"return function(response)
                local profile = json.decode(response.body)
                return { email = profile.emailAddress, empty = json.object(), missing = json.null }
            end"#,
            &schema,
            br#"{"emailAddress":"person@example.test"}"#,
        )
        .expect("transformed output");
        assert_eq!(
            value,
            serde_json::json!({
                "email": "person@example.test",
                "empty": {},
                "missing": null
            })
        );
        assert!(run("return function() return {} end", &object(&[], &[]), b"{}").is_err());
    }

    #[test]
    fn sandbox_has_no_ambient_authority_or_cross_call_state() {
        let visibility = object(
            &[
                ("os", scalar(OutputType::Boolean)),
                ("require", scalar(OutputType::Boolean)),
                ("random", scalar(OutputType::Boolean)),
            ],
            &["os", "require", "random"],
        );
        assert_eq!(
            run(
                "return function() return { os = os ~= nil, require = require ~= nil, random = math.random ~= nil } end",
                &visibility,
                b"{}",
            )
            .expect("visibility"),
            serde_json::json!({"os": false, "require": false, "random": false})
        );
        let source = "return function() counter = (counter or 0) + 1 return counter end";
        assert_eq!(run(source, &scalar(OutputType::Integer), b"{}").unwrap(), 1);
        assert_eq!(run(source, &scalar(OutputType::Integer), b"{}").unwrap(), 1);
    }

    #[test]
    fn response_text_truncation_is_utf8_safe_and_profile_scoped() {
        let schema = object(
            &[
                ("ascii", scalar(OutputType::String)),
                ("unicode", scalar(OutputType::String)),
                ("boundary", scalar(OutputType::String)),
                ("unchanged", scalar(OutputType::String)),
            ],
            &["ascii", "unicode", "boundary", "unchanged"],
        );
        assert_eq!(
            run(
                "return function() return { ascii = text.truncate_utf8('abcdef', 3), unicode = text.truncate_utf8('éclair', 3), boundary = text.truncate_utf8('éclair', 1), unchanged = text.truncate_utf8('ok', 8) } end",
                &schema,
                b"{}",
            )
            .expect("bounded text"),
            serde_json::json!({
                "ascii": "abc",
                "unicode": "éc",
                "boundary": "",
                "unchanged": "ok"
            })
        );
        assert!(
            run(
                "return function() return text.truncate_utf8('x', 32769) end",
                &scalar(OutputType::String),
                b"{}",
            )
            .is_err()
        );
        for profile in [SandboxProfile::Credential, SandboxProfile::RequestAuth] {
            assert_eq!(
                execute(
                    "return function() return { available = text ~= nil } end",
                    &serde_json::json!({}),
                    profile,
                )
                .expect("profile visibility"),
                serde_json::json!({"available": false})
            );
        }
    }

    #[test]
    fn response_base64url_text_decoding_is_bounded_and_fails_closed() {
        let schema = object(
            &[
                ("decoded", scalar(OutputType::String)),
                ("bounded", scalar(OutputType::String)),
            ],
            &["decoded", "bounded"],
        );
        assert_eq!(
            run(
                "return function() return { decoded = text.decode_base64url_utf8('aGVsbG8', 8), bounded = text.decode_base64url_utf8('w6ljbGFpcg', 3) } end",
                &schema,
                b"{}",
            )
            .expect("decoded text"),
            serde_json::json!({"decoded": "hello", "bounded": "éc"})
        );
        assert!(
            run(
                "return function() return text.decode_base64url_utf8('not*base64url', 8) end",
                &scalar(OutputType::String),
                b"{}",
            )
            .is_err()
        );
    }

    #[test]
    fn malformed_json_cycles_and_runaway_code_fail_closed() {
        assert!(
            run(
                "return function(response) return json.decode(response.body) end",
                &object(&[("a", scalar(OutputType::Integer))], &["a"]),
                br#"{"a":1,"a":2}"#,
            )
            .is_err()
        );
        assert!(
            run(
                "return function() local x = json.object() x.self = x return x end",
                &object(&[], &[]),
                b"{}",
            )
            .is_err()
        );
        let started = Instant::now();
        assert!(
            run(
                "return function() while true do end end",
                &scalar(OutputType::Null),
                b"{}",
            )
            .is_err()
        );
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn reviewed_provider_and_non_json_fixtures_share_one_transform_contract() {
        let cases = [
            (
                include_bytes!("../tests/fixtures/gmail-profile-transform.json").as_slice(),
                include_bytes!("../tests/fixtures/gmail-profile-response.json").as_slice(),
                "get_profile",
                "application/json",
                serde_json::json!({"account": "person@example.test", "message_count": 42}),
            ),
            (
                include_bytes!("../tests/fixtures/github-user-transform.json").as_slice(),
                include_bytes!("../tests/fixtures/github-user-response.json").as_slice(),
                "get_user",
                "application/json",
                serde_json::json!({"account": "fixture-user", "provider_id": 7}),
            ),
            (
                include_bytes!("../tests/fixtures/csv-response-transform.json").as_slice(),
                include_bytes!("../tests/fixtures/csv-response.csv").as_slice(),
                "get_record",
                "text/csv",
                serde_json::json!({"id": "item-7", "total": 42}),
            ),
        ];

        for (manifest, body, operation_id, content_type, expected) in cases {
            let compiled = AdapterCompiler::compile_json(manifest).expect("reviewed fixture");
            let operation = compiled
                .operations
                .iter()
                .find(|operation| operation.operation_id == operation_id)
                .expect("fixture operation");
            let contract = &operation.response;
            assert!(
                contract
                    .accepted_content_types
                    .iter()
                    .any(|accepted| accepted == content_type)
            );
            assert_eq!(
                transform(
                    contract.transform.as_ref().expect("response transform"),
                    &AdapterHttpResponse {
                        status: 200,
                        content_type: Some(content_type.to_string()),
                        body: body.to_vec(),
                    },
                )
                .expect("normalized fixture response"),
                expected
            );
        }
    }
}

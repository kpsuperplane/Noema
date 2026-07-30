use serde_json::{Map, Value};

use super::{
    authority::supports_component,
    validation::{
        A2UIValidationLimits, Code, Result, as_object, at, identifier, only, pointer, text,
    },
};

pub(super) fn validate_component(
    value: &Value,
    line: usize,
    limits: &A2UIValidationLimits,
) -> Result<String> {
    let value = as_object(value, line, Code::InvalidComponent)?;
    let id = identifier(value.get("id"), line, limits)?;
    let kind = text(value.get("component"), line)?;
    if !supports_component(kind) {
        return Err(at(
            Code::UnsupportedCatalog,
            line,
            Some("component"),
            "unsupported component",
        ));
    }
    match kind {
        "Text" => {
            only(value, &["id", "component", "text", "variant"], line)?;
            dynamic(value, "text", DynamicKind::String, line, limits)?;
            optional_enum(
                value,
                "variant",
                &["h1", "h2", "h3", "h4", "h5", "caption", "body"],
                line,
            )?;
        }
        "Row" | "Column" => {
            only(
                value,
                &["id", "component", "children", "justify", "align"],
                line,
            )?;
            let children = required(value, "children", line)?
                .as_array()
                .ok_or_else(|| {
                    at(
                        Code::InvalidComponent,
                        line,
                        Some("children"),
                        "expected array",
                    )
                })?;
            for child in children {
                identifier(Some(child), line, limits)?;
            }
            optional_enum(
                value,
                "justify",
                &[
                    "start",
                    "center",
                    "end",
                    "spaceBetween",
                    "spaceAround",
                    "spaceEvenly",
                    "stretch",
                ],
                line,
            )?;
            optional_enum(value, "align", &["start", "center", "end", "stretch"], line)?;
        }
        "Card" => {
            only(value, &["id", "component", "child"], line)?;
            identifier(Some(required(value, "child", line)?), line, limits)?;
        }
        "Divider" => {
            only(value, &["id", "component", "axis"], line)?;
            optional_enum(value, "axis", &["horizontal", "vertical"], line)?;
        }
        "Button" => {
            only(
                value,
                &["id", "component", "child", "action", "variant"],
                line,
            )?;
            identifier(Some(required(value, "child", line)?), line, limits)?;
            event(required(value, "action", line)?, line, limits)?;
            optional_enum(
                value,
                "variant",
                &["default", "primary", "borderless"],
                line,
            )?;
        }
        "TextField" => {
            only(
                value,
                &["id", "component", "label", "value", "variant"],
                line,
            )?;
            dynamic(value, "label", DynamicKind::String, line, limits)?;
            if value.contains_key("value") {
                dynamic(value, "value", DynamicKind::String, line, limits)?;
            }
            optional_enum(
                value,
                "variant",
                &["longText", "number", "shortText", "obscured"],
                line,
            )?;
        }
        "CheckBox" => {
            only(value, &["id", "component", "label", "value"], line)?;
            dynamic(value, "label", DynamicKind::String, line, limits)?;
            dynamic(value, "value", DynamicKind::Boolean, line, limits)?;
        }
        "ChoicePicker" => {
            only(
                value,
                &[
                    "id",
                    "component",
                    "label",
                    "options",
                    "value",
                    "variant",
                    "displayStyle",
                    "filterable",
                ],
                line,
            )?;
            if value.contains_key("label") {
                dynamic(value, "label", DynamicKind::String, line, limits)?;
            }
            choice_options(required(value, "options", line)?, line, limits)?;
            dynamic(value, "value", DynamicKind::StringList, line, limits)?;
            optional_enum(
                value,
                "variant",
                &["multipleSelection", "mutuallyExclusive"],
                line,
            )?;
            optional_enum(value, "displayStyle", &["checkbox", "chips"], line)?;
            if value
                .get("filterable")
                .is_some_and(|value| !value.is_boolean())
            {
                return Err(at(
                    Code::InvalidComponent,
                    line,
                    Some("filterable"),
                    "expected boolean",
                ));
            }
        }
        _ => unreachable!(),
    }
    Ok(id)
}

fn required<'a>(value: &'a Map<String, Value>, key: &str, line: usize) -> Result<&'a Value> {
    value
        .get(key)
        .ok_or_else(|| at(Code::InvalidComponent, line, Some(key), "missing field"))
}

#[derive(Clone, Copy)]
enum DynamicKind {
    String,
    Boolean,
    StringList,
}

fn dynamic(
    value: &Map<String, Value>,
    key: &str,
    kind: DynamicKind,
    line: usize,
    limits: &A2UIValidationLimits,
) -> Result<()> {
    let value = required(value, key, line)?;
    let literal = match kind {
        DynamicKind::String => value.is_string(),
        DynamicKind::Boolean => value.is_boolean(),
        DynamicKind::StringList => value.as_array().is_some_and(|values| {
            values.len() <= limits.max_collection_items && values.iter().all(Value::is_string)
        }),
    };
    if literal {
        return Ok(());
    }
    let binding = as_object(value, line, Code::InvalidComponent)?;
    only(binding, &["path"], line)?;
    pointer(text(binding.get("path"), line)?, line).map(|_| ())
}

fn optional_enum(
    value: &Map<String, Value>,
    key: &str,
    allowed: &[&str],
    line: usize,
) -> Result<()> {
    if let Some(value) = value.get(key)
        && !allowed.contains(&text(Some(value), line)?)
    {
        return Err(at(
            Code::InvalidComponent,
            line,
            Some(key),
            "unsupported value",
        ));
    }
    Ok(())
}

fn choice_options(value: &Value, line: usize, limits: &A2UIValidationLimits) -> Result<()> {
    let options = value.as_array().ok_or_else(|| {
        at(
            Code::InvalidComponent,
            line,
            Some("options"),
            "expected array",
        )
    })?;
    if options.is_empty() || options.len() > limits.max_collection_items {
        return Err(at(
            Code::InvalidComponent,
            line,
            Some("options"),
            "invalid option count",
        ));
    }
    for option in options {
        let option = as_object(option, line, Code::InvalidComponent)?;
        only(option, &["label", "value"], line)?;
        dynamic(option, "label", DynamicKind::String, line, limits)?;
        identifier(option.get("value"), line, limits)?;
    }
    Ok(())
}

fn event(value: &Value, line: usize, limits: &A2UIValidationLimits) -> Result<()> {
    let value = as_object(value, line, Code::InvalidAction)?;
    only(value, &["event"], line)?;
    let event = as_object(required(value, "event", line)?, line, Code::InvalidAction)?;
    only(event, &["name", "context"], line)?;
    identifier(event.get("name"), line, limits)?;
    if event.get("context").is_some_and(|value| !value.is_object()) {
        return Err(at(
            Code::InvalidAction,
            line,
            Some("context"),
            "expected object",
        ));
    }
    Ok(())
}

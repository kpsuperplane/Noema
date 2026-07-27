//! Small, fail-closed OpenAPI schema lowering helpers.

use openapiv3::{
    AdditionalProperties, Components, ReferenceOr, Schema, SchemaKind, Type,
    VariantOrUnknownOrEmpty,
};
use std::collections::BTreeSet;

use crate::ArgumentType;

/// A schema feature that cannot be represented by the v1 adapter vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SchemaFailure(pub(crate) &'static str);

/// Resolves only local component schema references while lowering supported
/// primitive arguments. The stack makes recursive documents fail closed.
#[derive(Debug)]
pub(crate) struct SchemaResolver<'a> {
    components: Option<&'a Components>,
    stack: BTreeSet<String>,
}

pub(crate) type ObjectProperty = (String, ArgumentType, Vec<String>, bool);

impl<'a> SchemaResolver<'a> {
    pub(crate) fn new(components: Option<&'a Components>) -> Self {
        Self {
            components,
            stack: BTreeSet::new(),
        }
    }

    /// Lower one parameter schema to the closed argument type and string enum.
    pub(crate) fn primitive(
        &mut self,
        schema: &ReferenceOr<Schema>,
    ) -> Result<(ArgumentType, Vec<String>), SchemaFailure> {
        let schema = self.resolve(schema)?;
        if schema.schema_data.nullable
            || schema.schema_data.read_only
            || schema.schema_data.default.is_some()
        {
            return Err(SchemaFailure("schema_constraint_unsupported"));
        }
        match schema.schema_kind {
            SchemaKind::Type(Type::String(value)) => {
                if !matches!(value.format, VariantOrUnknownOrEmpty::Empty)
                    || value.pattern.is_some()
                    || value.min_length.is_some()
                    || value.max_length.is_some()
                {
                    return Err(SchemaFailure("schema_constraint_unsupported"));
                }
                let values = value
                    .enumeration
                    .into_iter()
                    .map(|value| value.ok_or(SchemaFailure("null_enum_value")))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok((ArgumentType::String, values))
            }
            SchemaKind::Type(Type::Integer(value)) => {
                if !value.enumeration.is_empty()
                    || value.multiple_of.is_some()
                    || value.minimum.is_some()
                    || value.maximum.is_some()
                    || value.exclusive_minimum
                    || value.exclusive_maximum
                {
                    return Err(SchemaFailure("schema_constraint_unsupported"));
                }
                Ok((ArgumentType::Integer, Vec::new()))
            }
            SchemaKind::Type(Type::Number(value)) => {
                if !value.enumeration.is_empty()
                    || value.multiple_of.is_some()
                    || value.minimum.is_some()
                    || value.maximum.is_some()
                    || value.exclusive_minimum
                    || value.exclusive_maximum
                {
                    return Err(SchemaFailure("schema_constraint_unsupported"));
                }
                Ok((ArgumentType::Number, Vec::new()))
            }
            SchemaKind::Type(Type::Boolean(value)) => {
                if !value.enumeration.is_empty() {
                    return Err(SchemaFailure("non_string_enum"));
                }
                Ok((ArgumentType::Boolean, Vec::new()))
            }
            SchemaKind::Type(Type::Array(value)) => {
                if value.min_items.is_some() || value.max_items.is_some() || value.unique_items {
                    return Err(SchemaFailure("schema_constraint_unsupported"));
                }
                let Some(items) = value.items else {
                    return Err(SchemaFailure("array_items_required"));
                };
                let (item_type, item_enum) = self.primitive_boxed(&items)?;
                if item_type != ArgumentType::String || !item_enum.is_empty() {
                    return Err(SchemaFailure("array_item_unsupported"));
                }
                Ok((ArgumentType::StringArray, Vec::new()))
            }
            SchemaKind::Type(Type::Object(_))
            | SchemaKind::OneOf { .. }
            | SchemaKind::AllOf { .. }
            | SchemaKind::AnyOf { .. }
            | SchemaKind::Not { .. }
            | SchemaKind::Any(_) => Err(SchemaFailure("schema_shape_unsupported")),
        }
    }

    /// Lower an application/json object body to top-level JSON members.
    pub(crate) fn object_properties(
        &mut self,
        schema: &ReferenceOr<Schema>,
    ) -> Result<Vec<ObjectProperty>, SchemaFailure> {
        let schema = self.resolve(schema)?;
        if schema.schema_data.nullable
            || schema.schema_data.read_only
            || schema.schema_data.write_only
            || schema.schema_data.default.is_some()
        {
            return Err(SchemaFailure("schema_constraint_unsupported"));
        }
        let SchemaKind::Type(Type::Object(value)) = schema.schema_kind else {
            return Err(SchemaFailure("request_body_object_required"));
        };
        if !matches!(
            value.additional_properties,
            Some(AdditionalProperties::Any(false))
        ) {
            return Err(SchemaFailure("additional_properties_unsupported"));
        }
        let required = value.required.into_iter().collect::<BTreeSet<_>>();
        let mut properties = Vec::with_capacity(value.properties.len());
        for (name, property) in value.properties {
            let (argument_type, enum_values) = self.primitive_boxed(&property)?;
            properties.push((
                name.clone(),
                argument_type,
                enum_values,
                required.contains(&name),
            ));
        }
        if required.iter().any(|name| {
            !properties
                .iter()
                .any(|(property, _, _, _)| property == name)
        }) {
            return Err(SchemaFailure("required_property_missing"));
        }
        Ok(properties)
    }

    fn primitive_boxed(
        &mut self,
        schema: &ReferenceOr<Box<Schema>>,
    ) -> Result<(ArgumentType, Vec<String>), SchemaFailure> {
        match schema {
            ReferenceOr::Item(schema) => self.primitive(&ReferenceOr::Item((**schema).clone())),
            ReferenceOr::Reference { reference } => self.primitive(&ReferenceOr::Reference {
                reference: reference.clone(),
            }),
        }
    }

    fn resolve(&mut self, schema: &ReferenceOr<Schema>) -> Result<Schema, SchemaFailure> {
        match schema {
            ReferenceOr::Item(schema) => Ok(schema.clone()),
            ReferenceOr::Reference { reference } => {
                let name = component_name(reference, "schemas")?;
                if !self.stack.insert(name.to_string()) {
                    return Err(SchemaFailure("recursive_schema"));
                }
                let resolved = self
                    .components
                    .and_then(|components| components.schemas.get(name))
                    .ok_or(SchemaFailure("unresolved_local_ref"))?;
                let result = self.resolve(resolved);
                self.stack.remove(name);
                result
            }
        }
    }
}

/// Resolve one local component name and reject every other reference shape.
pub(crate) fn component_name<'a>(
    reference: &'a str,
    component_kind: &str,
) -> Result<&'a str, SchemaFailure> {
    let prefix = format!("#/components/{component_kind}/");
    let encoded = reference
        .strip_prefix(&prefix)
        .ok_or(SchemaFailure("non_local_ref"))?;
    if encoded.is_empty() || encoded.contains('/') {
        return Err(SchemaFailure("invalid_local_ref"));
    }
    Ok(encoded)
}

//! GraphQL projections for the Work read and command surfaces.
//!
//! The concrete types are grouped by the stable API domains so that the
//! schema façade stays navigable while all projections remain in one module
//! namespace for async-graphql registration.

/// Declare a GraphQL object without repeating derive/name scaffolding while
/// keeping every schema description next to its explicit Rust field.
macro_rules! graphql_object {
    (
        $object_doc:literal => $vis:vis struct $name:ident($graphql_name:literal) {
            $($field_doc:literal => $field:ident: $field_type:ty),* $(,)?
        }
    ) => {
        #[doc = $object_doc]
        #[derive(Clone, Debug, async_graphql::SimpleObject)]
        #[graphql(name = $graphql_name)]
        $vis struct $name {
            $(#[doc = $field_doc] $vis $field: $field_type,)*
        }
    };
}

/// Declare a projection and its domain conversion from one field map.  Most
/// Work projections are mechanical adapters; keeping the schema field and the
/// conversion expression together prevents the two representations drifting.
macro_rules! graphql_object_from {
    (
        $object_doc:literal => $vis:vis struct $name:ident($graphql_name:literal)
        from $source:ty as $value:ident {
            $($field_doc:literal => $field:ident: $field_type:ty = $mapping:expr),* $(,)?
        }
    ) => {
        graphql_object! { $object_doc => $vis struct $name($graphql_name) {
            $($field_doc => $field: $field_type),*
        } }

        impl From<$source> for $name {
            fn from($value: $source) -> Self {
                Self { $($field: $mapping),* }
            }
        }
    };
    (
        $object_doc:literal => $vis:vis struct $name:ident($graphql_name:literal)
        try_from $source:ty as $value:ident {
            $($field_doc:literal => $field:ident: $field_type:ty = $mapping:expr),* $(,)?
        }
    ) => {
        graphql_object! { $object_doc => $vis struct $name($graphql_name) {
            $($field_doc => $field: $field_type),*
        } }

        impl TryFrom<$source> for $name {
            type Error = async_graphql::Error;

            fn try_from($value: $source) -> Result<Self, Self::Error> {
                Ok(Self { $($field: $mapping),* })
            }
        }
    };
}

mod connections;
mod evidence;
mod foundation;
mod mapping;
mod settings;
mod task;
mod vocabulary;

pub(in crate::graphql) use connections::*;
pub(in crate::graphql) use evidence::*;
pub(in crate::graphql) use foundation::*;
pub(in crate::graphql) use mapping::*;
pub(in crate::graphql) use settings::*;
pub(in crate::graphql) use task::*;
pub(in crate::graphql) use vocabulary::*;

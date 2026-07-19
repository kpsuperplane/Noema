//! GraphQL client API facade.
//!
//! The GraphQL layer is the first-party client API. Resolvers must stay thin:
//! they call Noema read models, command/runtime paths, provider auth, and
//! repository methods instead of owning product behavior.

macro_rules! graphql_enum_from {
    ($source:ty => $target:ty { $($source_variant:ident => $target_variant:ident),+ $(,)? }) => {
        impl From<$source> for $target {
            fn from(value: $source) -> Self {
                match value {
                    $(<$source>::$source_variant => Self::$target_variant),+
                }
            }
        }
    };
}

macro_rules! graphql_enum_bidi {
    ($domain:ty => $graphql:ty { $($domain_variant:ident => $graphql_variant:ident),+ $(,)? }) => {
        graphql_enum_from!($domain => $graphql {
            $($domain_variant => $graphql_variant),+
        });
        graphql_enum_from!($graphql => $domain {
            $($graphql_variant => $domain_variant),+
        });
    };
}

mod agents;
mod artifacts;
mod chat;
mod errors;
mod local_models;
#[cfg(test)]
mod local_models_tests;
mod local_status;
mod mcp;
mod memory;
mod onboarding;
mod privacy_settings;
mod provider_accounts;
mod provider_selection;
mod replay;
mod runtime_state;
mod schema;
#[cfg(test)]
mod support_tests;
mod tasks;
mod usage_settings;
mod web_fetch_settings;
mod web_tool_settings;

pub use artifacts::{
    AuthorizedArtifactDownload, AuthorizedArtifactDownloadError, authorized_artifact_download,
};
pub use mcp::complete_mcp_server_oauth_setup;
pub(crate) use replay::{ConversationReplayItem, web_conversation_item_from_record};
pub use runtime_state::GraphqlState;
pub use schema::{GraphqlSchema, build_schema, schema_sdl};

/// Server-derived identity attached to every authenticated GraphQL operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequestPrincipal {
    subject_id: &'static str,
}

impl RequestPrincipal {
    /// Return the single local authenticated subject.
    #[must_use]
    pub fn local() -> Self {
        Self {
            subject_id: "human:local",
        }
    }

    /// Return the stable subject identifier.
    #[must_use]
    pub fn subject_id(&self) -> &'static str {
        self.subject_id
    }
}

pub(crate) fn request_principal_subject(
    ctx: &async_graphql::Context<'_>,
) -> async_graphql::Result<&'static str> {
    ctx.data_opt::<RequestPrincipal>()
        .map(RequestPrincipal::subject_id)
        .ok_or_else(|| async_graphql::Error::new("request is unauthenticated"))
}

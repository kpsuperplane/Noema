use std::{fmt, ops::Deref};

use super::MemoryPersistenceError;

macro_rules! persistence_id {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, sqlx::Type)]
        #[sqlx(transparent)]
        pub struct $name(String);

        impl $name {
            /// Create a typed persistence id.
            ///
            /// # Errors
            ///
            /// Returns [`MemoryPersistenceError::EmptyObjectId`] when the id is empty.
            pub fn new(value: impl Into<String>) -> Result<Self, MemoryPersistenceError> {
                let value = value.into();
                if value.trim().is_empty() {
                    return Err(MemoryPersistenceError::EmptyObjectId {
                        object_type: stringify!($name).to_string(),
                    });
                }
                Ok(Self(value))
            }

            /// Borrow the id as a string slice.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// Consume the typed id and return its inner string.
            #[must_use]
            pub fn into_string(self) -> String {
                self.0
            }
        }

        impl TryFrom<String> for $name {
            type Error = MemoryPersistenceError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl TryFrom<&str> for $name {
            type Error = MemoryPersistenceError;

            fn try_from(value: &str) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }

        impl Deref for $name {
            type Target = str;

            fn deref(&self) -> &Self::Target {
                self.as_str()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }
    };
}

persistence_id!(ActorId, "A typed id for rows in `actors`.");
persistence_id!(ObjectId, "A typed id for concrete object rows.");
persistence_id!(MemoryItemId, "A typed id for rows in `memory_items`.");
persistence_id!(ConversationId, "A typed id for rows in `conversations`.");
persistence_id!(
    ConversationItemId,
    "A typed id for rows in `conversation_items`."
);
persistence_id!(ContextPacketId, "A typed id for rows in `context_packets`.");

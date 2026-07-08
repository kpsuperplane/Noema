//! Mem0 local service client and lifecycle support.

mod client;

#[cfg(test)]
mod tests;

pub use client::{
    Mem0AddMemoryRequest, Mem0Client, Mem0ClientError, Mem0ListMemoriesRequest,
    Mem0ListMemoriesResponse, Mem0Memory, Mem0Message, Mem0SearchRequest, Mem0SearchResponse,
};

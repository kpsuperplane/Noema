//! Capability Gateway runtime entrypoint.

pub mod gateway;

pub use gateway::{
    CapabilityGateway, GatewayToolProposal, GatewayToolResult, is_mcp_shaped_tool_name,
};

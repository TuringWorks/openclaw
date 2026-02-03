//! WebSocket gateway server for OpenClaw.
//!
//! This crate provides:
//! - JSON-RPC 2.0 over WebSocket
//! - Agent and session management endpoints
//! - Channel status and control
//! - Real-time message streaming

pub mod error;
pub mod server;
pub mod rpc;
pub mod methods;
pub mod session;

pub use error::GatewayError;
pub use server::{Gateway, GatewayConfig};
pub use rpc::{JsonRpcRequest, JsonRpcResponse, JsonRpcError};

/// Result type for gateway operations.
pub type Result<T> = std::result::Result<T, GatewayError>;

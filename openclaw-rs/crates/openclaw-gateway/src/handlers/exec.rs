//! Execution approval RPC method handlers.
//!
//! Handles command execution approval configuration and requests.

use super::HandlerContext;
use crate::error::GatewayError;
use crate::methods::MethodHandler;
use crate::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::debug;

/// Approval configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApprovalConfig {
    /// Whether approval is required by default.
    pub require_approval: bool,
    /// Allowlist patterns that don't require approval.
    pub allowlist: Vec<String>,
    /// Denylist patterns that always require approval.
    pub denylist: Vec<String>,
    /// Timeout for approval requests in seconds.
    pub timeout_seconds: u64,
}

/// Exec approvals get handler.
pub struct ExecApprovalsGetHandler {
    _context: Arc<HandlerContext>,
}

impl ExecApprovalsGetHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { _context: context }
    }
}

#[async_trait]
impl MethodHandler for ExecApprovalsGetHandler {
    async fn call(&self, _params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        debug!("Exec approvals get request");

        // TODO: Get from config
        // Default allowlist is intentionally minimal — `cat` was removed because it can
        // exfiltrate arbitrary files (e.g. `cat /etc/shadow`). `git status` is kept
        // because it is read-only, but `git push/reset/checkout` are on the denylist.
        let config = ApprovalConfig {
            require_approval: true,
            allowlist: vec![
                "ls".to_string(),
                "pwd".to_string(),
                "echo".to_string(),
                "git status".to_string(),
                "git log".to_string(),
                "git diff".to_string(),
            ],
            denylist: vec![
                "rm -rf".to_string(),
                "rm -fr".to_string(),
                "sudo".to_string(),
                "su -".to_string(),
                "doas".to_string(),
                "chmod 777".to_string(),
                "git push".to_string(),
                "git reset".to_string(),
                "git checkout".to_string(),
                "curl".to_string(),
                "wget".to_string(),
                "ssh".to_string(),
                "nc ".to_string(),
                "ncat".to_string(),
            ],
            timeout_seconds: 30,
        };

        Ok(serde_json::to_value(config).unwrap())
    }
}

/// Parameters for exec.approvals.set method.
#[derive(Debug, Deserialize)]
pub struct ExecApprovalsSetParams {
    /// Whether approval is required by default.
    pub require_approval: Option<bool>,
    /// Allowlist patterns.
    pub allowlist: Option<Vec<String>>,
    /// Denylist patterns.
    pub denylist: Option<Vec<String>>,
    /// Timeout in seconds.
    pub timeout_seconds: Option<u64>,
}

/// Exec approvals set handler.
pub struct ExecApprovalsSetHandler {
    _context: Arc<HandlerContext>,
}

impl ExecApprovalsSetHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { _context: context }
    }
}

#[async_trait]
impl MethodHandler for ExecApprovalsSetHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: ExecApprovalsSetParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!("Exec approvals set: {:?}", params.require_approval);

        // TODO: Actually update config

        Ok(serde_json::json!({
            "updated": true,
        }))
    }
}

/// Parameters for exec.approvals.node.get method.
#[derive(Debug, Deserialize)]
pub struct ExecApprovalsNodeGetParams {
    /// Node ID.
    pub node_id: String,
}

/// Exec approvals node get handler.
pub struct ExecApprovalsNodeGetHandler {
    _context: Arc<HandlerContext>,
}

impl ExecApprovalsNodeGetHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { _context: context }
    }
}

#[async_trait]
impl MethodHandler for ExecApprovalsNodeGetHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: ExecApprovalsNodeGetParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!("Exec approvals node get: {}", params.node_id);

        // TODO: Get node-specific config
        let config = ApprovalConfig::default();

        Ok(serde_json::json!({
            "node_id": params.node_id,
            "config": config,
        }))
    }
}

/// Parameters for exec.approvals.node.set method.
#[derive(Debug, Deserialize)]
pub struct ExecApprovalsNodeSetParams {
    /// Node ID.
    pub node_id: String,
    /// Whether approval is required.
    pub require_approval: Option<bool>,
    /// Allowlist patterns.
    pub allowlist: Option<Vec<String>>,
    /// Denylist patterns.
    pub denylist: Option<Vec<String>>,
}

/// Exec approvals node set handler.
pub struct ExecApprovalsNodeSetHandler {
    _context: Arc<HandlerContext>,
}

impl ExecApprovalsNodeSetHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { _context: context }
    }
}

#[async_trait]
impl MethodHandler for ExecApprovalsNodeSetHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: ExecApprovalsNodeSetParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!("Exec approvals node set: {}", params.node_id);

        Ok(serde_json::json!({
            "node_id": params.node_id,
            "updated": true,
        }))
    }
}

/// Pending approval request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingApproval {
    /// Request ID.
    pub id: String,
    /// Command to approve.
    pub command: String,
    /// Working directory.
    pub cwd: Option<String>,
    /// Agent ID.
    pub agent_id: String,
    /// Session key.
    pub session_key: String,
    /// Node ID (if from remote node).
    pub node_id: Option<String>,
    /// Request timestamp.
    pub requested_at: String,
    /// Expiry timestamp.
    pub expires_at: String,
}

/// Parameters for exec.approval.request method.
#[derive(Debug, Deserialize)]
pub struct ExecApprovalRequestParams {
    /// Command to execute.
    pub command: String,
    /// Working directory.
    pub cwd: Option<String>,
    /// Agent ID.
    pub agent_id: String,
    /// Session key.
    pub session_key: String,
    /// Timeout in milliseconds.
    pub timeout_ms: Option<u64>,
}

/// Exec approval request handler.
pub struct ExecApprovalRequestHandler {
    _context: Arc<HandlerContext>,
}

impl ExecApprovalRequestHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { _context: context }
    }
}

#[async_trait]
impl MethodHandler for ExecApprovalRequestHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: ExecApprovalRequestParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!("Exec approval request: {}", params.command);

        let request_id = uuid::Uuid::new_v4().to_string();
        let timeout_ms = params.timeout_ms.unwrap_or(60_000);
        let now = chrono::Utc::now();
        let expires_at = now + chrono::Duration::milliseconds(timeout_ms as i64);

        // TODO: Actually queue the approval request and wait for response

        Ok(serde_json::json!({
            "request_id": request_id,
            "command": params.command,
            "status": "pending",
            "expires_at": expires_at.to_rfc3339(),
        }))
    }
}

/// Parameters for exec.approval.resolve method.
#[derive(Debug, Deserialize)]
pub struct ExecApprovalResolveParams {
    /// Request ID.
    pub request_id: String,
    /// Whether to approve.
    pub approved: bool,
    /// Optional reason for rejection.
    pub reason: Option<String>,
}

/// Exec approval resolve handler.
pub struct ExecApprovalResolveHandler {
    _context: Arc<HandlerContext>,
}

impl ExecApprovalResolveHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { _context: context }
    }
}

#[async_trait]
impl MethodHandler for ExecApprovalResolveHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: ExecApprovalResolveParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!(
            "Exec approval resolve: {} = {}",
            params.request_id, params.approved
        );

        // TODO: Actually resolve the pending approval

        Ok(serde_json::json!({
            "request_id": params.request_id,
            "approved": params.approved,
            "resolved": true,
        }))
    }
}

// TryFrom implementations

impl TryFrom<serde_json::Value> for ExecApprovalsSetParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

impl TryFrom<serde_json::Value> for ExecApprovalsNodeGetParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

impl TryFrom<serde_json::Value> for ExecApprovalsNodeSetParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

impl TryFrom<serde_json::Value> for ExecApprovalRequestParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

impl TryFrom<serde_json::Value> for ExecApprovalResolveParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_approval_config_default() {
        let config = ApprovalConfig::default();
        assert!(!config.require_approval);
        assert!(config.allowlist.is_empty());
    }

    #[test]
    fn test_pending_approval_serialization() {
        let approval = PendingApproval {
            id: "req-1".to_string(),
            command: "rm -rf /tmp/test".to_string(),
            cwd: Some("/home/user".to_string()),
            agent_id: "agent-1".to_string(),
            session_key: "session-1".to_string(),
            node_id: None,
            requested_at: chrono::Utc::now().to_rfc3339(),
            expires_at: chrono::Utc::now().to_rfc3339(),
        };

        let json = serde_json::to_value(&approval).unwrap();
        assert_eq!(json["command"], "rm -rf /tmp/test");
    }
}

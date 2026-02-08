//! Cron job RPC method handlers.
//!
//! Handles scheduling and management of cron jobs.

use super::HandlerContext;
use crate::error::GatewayError;
use crate::methods::MethodHandler;
use crate::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::debug;

/// Cron job info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJobInfo {
    /// Job ID.
    pub id: String,
    /// Cron schedule expression.
    pub schedule: String,
    /// Job description.
    pub description: Option<String>,
    /// Agent ID to run.
    pub agent_id: String,
    /// Prompt to send.
    pub prompt: String,
    /// Enabled status.
    pub enabled: bool,
    /// Next run time.
    pub next_run: Option<String>,
    /// Last run time.
    pub last_run: Option<String>,
}

/// Cron list handler.
pub struct CronListHandler {
    _context: Arc<HandlerContext>,
}

impl CronListHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { _context: context }
    }
}

#[async_trait]
impl MethodHandler for CronListHandler {
    async fn call(&self, _params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        debug!("Cron list request");

        // TODO: Get jobs from cron scheduler
        let jobs: Vec<CronJobInfo> = vec![];

        Ok(serde_json::json!({
            "jobs": jobs,
            "count": jobs.len(),
        }))
    }
}

/// Cron status handler.
pub struct CronStatusHandler {
    _context: Arc<HandlerContext>,
}

impl CronStatusHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { _context: context }
    }
}

#[async_trait]
impl MethodHandler for CronStatusHandler {
    async fn call(&self, _params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        debug!("Cron status request");

        Ok(serde_json::json!({
            "enabled": true,
            "job_count": 0,
            "next_job": null,
        }))
    }
}

/// Parameters for cron.add method.
#[derive(Debug, Deserialize)]
pub struct CronAddParams {
    /// Cron schedule expression.
    pub schedule: String,
    /// Job description.
    pub description: Option<String>,
    /// Agent ID to run.
    pub agent_id: String,
    /// Prompt to send.
    pub prompt: String,
    /// Whether to enable immediately.
    pub enabled: Option<bool>,
}

/// Cron add handler.
pub struct CronAddHandler {
    _context: Arc<HandlerContext>,
}

impl CronAddHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { _context: context }
    }
}

#[async_trait]
impl MethodHandler for CronAddHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: CronAddParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!("Cron add: schedule={}", params.schedule);

        // Validate cron expression
        // TODO: Use cron crate to parse and validate

        let job_id = uuid::Uuid::new_v4().to_string();

        Ok(serde_json::json!({
            "id": job_id,
            "schedule": params.schedule,
            "agent_id": params.agent_id,
            "enabled": params.enabled.unwrap_or(true),
            "created": true,
        }))
    }
}

/// Parameters for cron.update method.
#[derive(Debug, Deserialize)]
pub struct CronUpdateParams {
    /// Job ID.
    pub id: String,
    /// New cron schedule expression.
    pub schedule: Option<String>,
    /// New description.
    pub description: Option<String>,
    /// New prompt.
    pub prompt: Option<String>,
    /// Enable/disable.
    pub enabled: Option<bool>,
}

/// Cron update handler.
pub struct CronUpdateHandler {
    _context: Arc<HandlerContext>,
}

impl CronUpdateHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { _context: context }
    }
}

#[async_trait]
impl MethodHandler for CronUpdateHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: CronUpdateParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!("Cron update: id={}", params.id);

        // TODO: Actually update in scheduler

        Ok(serde_json::json!({
            "id": params.id,
            "updated": true,
        }))
    }
}

/// Parameters for cron.remove method.
#[derive(Debug, Deserialize)]
pub struct CronRemoveParams {
    /// Job ID.
    pub id: String,
}

/// Cron remove handler.
pub struct CronRemoveHandler {
    _context: Arc<HandlerContext>,
}

impl CronRemoveHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { _context: context }
    }
}

#[async_trait]
impl MethodHandler for CronRemoveHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: CronRemoveParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!("Cron remove: id={}", params.id);

        Ok(serde_json::json!({
            "id": params.id,
            "removed": true,
        }))
    }
}

/// Parameters for cron.run method.
#[derive(Debug, Deserialize)]
pub struct CronRunParams {
    /// Job ID.
    pub id: String,
}

/// Cron run handler (manual trigger).
pub struct CronRunHandler {
    _context: Arc<HandlerContext>,
}

impl CronRunHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { _context: context }
    }
}

#[async_trait]
impl MethodHandler for CronRunHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: CronRunParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!("Cron run: id={}", params.id);

        let run_id = uuid::Uuid::new_v4().to_string();

        Ok(serde_json::json!({
            "job_id": params.id,
            "run_id": run_id,
            "triggered": true,
        }))
    }
}

/// Parameters for cron.runs method.
#[derive(Debug, Deserialize)]
pub struct CronRunsParams {
    /// Job ID (optional, all jobs if not specified).
    pub id: Option<String>,
    /// Maximum runs to return.
    pub limit: Option<usize>,
}

/// Cron runs handler (run history).
pub struct CronRunsHandler {
    _context: Arc<HandlerContext>,
}

impl CronRunsHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { _context: context }
    }
}

#[async_trait]
impl MethodHandler for CronRunsHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: CronRunsParams = params
            .map(|v| serde_json::from_value(v).unwrap_or_default())
            .unwrap_or_default();

        debug!("Cron runs: id={:?}", params.id);

        Ok(serde_json::json!({
            "runs": [],
            "count": 0,
        }))
    }
}

impl Default for CronRunsParams {
    fn default() -> Self {
        Self {
            id: None,
            limit: Some(20),
        }
    }
}

/// Wake handler - send wake event.
pub struct WakeHandler {
    _context: Arc<HandlerContext>,
}

impl WakeHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { _context: context }
    }
}

#[async_trait]
impl MethodHandler for WakeHandler {
    async fn call(&self, _params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        debug!("Wake event");

        Ok(serde_json::json!({
            "woke": true,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        }))
    }
}

// TryFrom implementations

impl TryFrom<serde_json::Value> for CronAddParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

impl TryFrom<serde_json::Value> for CronUpdateParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

impl TryFrom<serde_json::Value> for CronRemoveParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

impl TryFrom<serde_json::Value> for CronRunParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cron_job_info() {
        let job = CronJobInfo {
            id: "job-1".to_string(),
            schedule: "0 * * * *".to_string(),
            description: Some("Hourly job".to_string()),
            agent_id: "agent-1".to_string(),
            prompt: "Check status".to_string(),
            enabled: true,
            next_run: None,
            last_run: None,
        };

        let json = serde_json::to_value(&job).unwrap();
        assert_eq!(json["schedule"], "0 * * * *");
    }
}

//! WebSocket gateway server.

use crate::error::GatewayError;
use crate::methods::MethodRegistry;
use crate::rpc::{JsonRpcError, JsonRpcRequest, JsonRpcResponse};
use crate::Result;
use axum::{
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use futures::{SinkExt, StreamExt};
use openclaw_core::config::BindMode;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tower_http::cors::CorsLayer;
use tracing::{debug, error, info, warn};

/// Gateway configuration.
#[derive(Debug, Clone)]
pub struct GatewayConfig {
    /// Bind mode.
    pub bind: BindMode,

    /// Port number.
    pub port: u16,

    /// Enable CORS.
    pub cors: bool,

    /// Maximum connections.
    pub max_connections: usize,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            bind: BindMode::Loopback,
            port: 18789,
            cors: true,
            max_connections: 100,
        }
    }
}

/// Gateway server state.
pub struct GatewayState {
    /// Method registry.
    pub methods: Arc<MethodRegistry>,

    /// Connected clients.
    pub clients: RwLock<HashMap<String, ClientInfo>>,

    /// Broadcast channel for notifications.
    pub broadcast_tx: broadcast::Sender<String>,

    /// Configuration.
    pub config: GatewayConfig,
}

/// Information about a connected client.
#[derive(Debug, Clone)]
pub struct ClientInfo {
    /// Client ID.
    pub id: String,

    /// Connection time.
    pub connected_at: chrono::DateTime<chrono::Utc>,

    /// Remote address.
    pub remote_addr: Option<SocketAddr>,
}

/// The WebSocket gateway server.
pub struct Gateway {
    /// Server state.
    state: Arc<GatewayState>,
}

impl Gateway {
    /// Create a new gateway.
    pub fn new(config: GatewayConfig) -> Self {
        let (broadcast_tx, _) = broadcast::channel(1000);

        let state = Arc::new(GatewayState {
            methods: Arc::new(MethodRegistry::new()),
            clients: RwLock::new(HashMap::new()),
            broadcast_tx,
            config,
        });

        Self { state }
    }

    /// Get the method registry for registering handlers.
    pub fn methods(&self) -> &Arc<MethodRegistry> {
        &self.state.methods
    }

    /// Run the gateway server.
    pub async fn run(&self) -> Result<()> {
        let addr = self.bind_address();

        let app = self.create_router();

        info!("Starting gateway server on {}", addr);

        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|e| GatewayError::Io(e))?;

        axum::serve(listener, app)
            .await
            .map_err(|e| GatewayError::Internal(e.to_string()))?;

        Ok(())
    }

    /// Create the Axum router.
    fn create_router(&self) -> Router {
        let state = self.state.clone();

        let mut router = Router::new()
            .route("/ws", get(ws_handler))
            .route("/health", get(health_handler))
            .with_state(state);

        if self.state.config.cors {
            router = router.layer(CorsLayer::permissive());
        }

        router
    }

    /// Get the bind address.
    fn bind_address(&self) -> SocketAddr {
        let ip = match self.state.config.bind {
            BindMode::Loopback => [127, 0, 0, 1],
            BindMode::Lan | BindMode::Tailnet | BindMode::Auto => [0, 0, 0, 0],
        };

        SocketAddr::from((ip, self.state.config.port))
    }

    /// Broadcast a notification to all clients.
    pub fn broadcast(&self, message: &str) {
        let _ = self.state.broadcast_tx.send(message.to_string());
    }

    /// Get connected client count.
    pub async fn client_count(&self) -> usize {
        self.state.clients.read().await.len()
    }
}

/// WebSocket upgrade handler.
async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<GatewayState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

/// Handle a WebSocket connection.
async fn handle_socket(socket: WebSocket, state: Arc<GatewayState>) {
    let client_id = uuid::Uuid::new_v4().to_string();

    // Register client
    {
        let mut clients = state.clients.write().await;
        clients.insert(
            client_id.clone(),
            ClientInfo {
                id: client_id.clone(),
                connected_at: chrono::Utc::now(),
                remote_addr: None,
            },
        );
    }

    info!("Client connected: {}", client_id);

    let (mut sender, mut receiver) = socket.split();
    let mut broadcast_rx = state.broadcast_tx.subscribe();

    // Handle incoming messages
    let state_clone = state.clone();
    let client_id_clone = client_id.clone();

    let recv_task = tokio::spawn(async move {
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    let response = handle_message(&text, &state_clone).await;
                    if let Err(e) = sender.send(Message::Text(response)).await {
                        error!("Failed to send response: {}", e);
                        break;
                    }
                }
                Ok(Message::Close(_)) => {
                    debug!("Client {} closed connection", client_id_clone);
                    break;
                }
                Err(e) => {
                    warn!("WebSocket error: {}", e);
                    break;
                }
                _ => {}
            }
        }
    });

    // Wait for task to complete
    let _ = recv_task.await;

    // Unregister client
    {
        let mut clients = state.clients.write().await;
        clients.remove(&client_id);
    }

    info!("Client disconnected: {}", client_id);
}

/// Handle a JSON-RPC message.
async fn handle_message(text: &str, state: &GatewayState) -> String {
    // Parse request
    let request: JsonRpcRequest = match serde_json::from_str(text) {
        Ok(r) => r,
        Err(e) => {
            let response = JsonRpcResponse::error(
                None,
                JsonRpcError::parse_error(e.to_string()),
            );
            return serde_json::to_string(&response).unwrap_or_default();
        }
    };

    debug!("Received RPC request: {}", request.method);

    // Dispatch to method handler
    let result = state.methods.call(&request.method, request.params.clone()).await;

    let response = match result {
        Ok(value) => JsonRpcResponse::success(request.id, value),
        Err(e) => JsonRpcResponse::error(
            request.id,
            JsonRpcError::new(e.code(), e.to_string()),
        ),
    };

    serde_json::to_string(&response).unwrap_or_default()
}

/// Health check handler.
async fn health_handler(State(state): State<Arc<GatewayState>>) -> impl IntoResponse {
    let clients = state.clients.read().await.len();
    serde_json::json!({
        "status": "ok",
        "clients": clients,
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gateway_config_default() {
        let config = GatewayConfig::default();
        assert_eq!(config.port, 18789);
        assert_eq!(config.bind, BindMode::Loopback);
    }
}

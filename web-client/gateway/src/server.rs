use std::{net::SocketAddr, path::PathBuf, sync::Arc};

use axum::{
    extract::{ws::WebSocketUpgrade, State},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde_json::{json, Value};
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};

use crate::{session, ResultType};

#[derive(Clone)]
pub struct GatewayConfig {
    pub static_dir: Option<PathBuf>,
    pub token: Option<String>,
}

pub fn health_body(auth_required: bool) -> Value {
    json!({
        "ok": true,
        "service": "rustdesk-web-gateway",
        "authRequired": auth_required,
    })
}

pub fn router(config: GatewayConfig) -> Router {
    let static_dir = config.static_dir.clone();
    let mut router = Router::new()
        .route("/api/health", get(health))
        .route("/ws", get(ws))
        .layer(CorsLayer::permissive())
        .with_state(Arc::new(config));
    if let Some(dir) = static_dir {
        let index = dir.join("index.html");
        let serve = ServeDir::new(&dir).not_found_service(ServeFile::new(index));
        router = router.fallback_service(serve);
    }
    router
}

async fn health(State(config): State<Arc<GatewayConfig>>) -> Json<Value> {
    let auth_required = config
        .token
        .as_deref()
        .map(str::trim)
        .is_some_and(|token| !token.is_empty());
    Json(health_body(auth_required))
}

async fn ws(State(config): State<Arc<GatewayConfig>>, upgrade: WebSocketUpgrade) -> impl IntoResponse {
    let token = config.token.clone();
    upgrade.on_upgrade(move |socket| session::serve_socket(socket, token))
}

pub async fn serve(listen: SocketAddr, config: GatewayConfig) -> ResultType<()> {
    if let Some(dir) = &config.static_dir {
        hbb_common::log::info!("serving the web client from {}", dir.display());
    }
    if config
        .token
        .as_deref()
        .map(str::trim)
        .is_some_and(|token| !token.is_empty())
    {
        hbb_common::log::info!("gateway access token is required");
    }
    let app = router(config);
    let listener = tokio::net::TcpListener::bind(listen).await?;
    hbb_common::log::info!("web gateway listening on http://{listen}");
    axum::serve(listener, app).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_reports_whether_a_token_is_required() {
        let open = health_body(false);
        assert_eq!(open["ok"], true);
        assert_eq!(open["service"], "rustdesk-web-gateway");
        assert_eq!(open["authRequired"], false);
        assert_eq!(health_body(true)["authRequired"], true);
    }
}

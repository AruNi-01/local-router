use std::{env, fs};

use anyhow::Result;
use axum::{
    Router,
    body::Body,
    extract::Path,
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use include_dir::{Dir, include_dir};
use localrouter_core::{
    AppState,
    api::{api_router, proxy_router},
    app::{api_addr, proxy_addr},
    models::DaemonConfig,
    storage::pid_file_path,
};
use tokio::{net::TcpListener, signal};
use tracing::info;

static DASHBOARD_DIST: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../apps/dashboard/dist");

#[tokio::main]
async fn main() -> Result<()> {
    let state = AppState::load().await?;
    init_tracing(&state.config().await);
    let api_addr = api_addr(&state).await;
    let proxy_addr = proxy_addr(&state).await;
    let api_listener = TcpListener::bind(api_addr).await?;
    let proxy_listener = TcpListener::bind(proxy_addr).await?;

    if let Some(parent) = pid_file_path().parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(pid_file_path(), std::process::id().to_string())?;

    let shutdown = async {
        let _ = signal::ctrl_c().await;
    };

    info!("localrouterd api listening on {}", api_addr);
    info!("localrouterd proxy listening on {}", proxy_addr);

    let api = Router::new()
        .nest("/v1", api_router(state.clone()))
        .route("/", get(serve_dashboard_index))
        .route("/{*path}", get(serve_dashboard_asset));
    let proxy = proxy_router(state.clone());

    let api_server = axum::serve(api_listener, api).with_graceful_shutdown(shutdown_signal());
    let proxy_server = axum::serve(proxy_listener, proxy).with_graceful_shutdown(shutdown_signal());

    tokio::select! {
        result = api_server => {
            result?;
        }
        result = proxy_server => {
            result?;
        }
        _ = shutdown => {}
    }

    let _ = fs::remove_file(pid_file_path());
    Ok(())
}

async fn shutdown_signal() {
    let _ = signal::ctrl_c().await;
}

async fn serve_dashboard_index() -> Response {
    serve_dashboard_path(None)
}

async fn serve_dashboard_asset(Path(path): Path<String>) -> Response {
    serve_dashboard_path(Some(path))
}

fn serve_dashboard_path(path: Option<String>) -> Response {
    let requested = path.unwrap_or_else(|| "index.html".to_string());
    let asset_path = requested.trim_start_matches('/');

    if let Some(file) = DASHBOARD_DIST.get_file(asset_path) {
        return file_response(asset_path, file.contents());
    }

    let is_spa_route = asset_path.is_empty() || !asset_path.contains('.');
    if is_spa_route {
        if let Some(file) = DASHBOARD_DIST.get_file("index.html") {
            return file_response("index.html", file.contents());
        }
    }

    (StatusCode::NOT_FOUND, "not found").into_response()
}

fn file_response(path: &str, contents: &[u8]) -> Response {
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    let mut response = Response::new(Body::from(contents.to_vec()));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(mime.as_ref())
            .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
    );
    response
}

fn init_tracing(config: &DaemonConfig) {
    let directive = tracing_directive(env::var("RUST_LOG").ok(), &config.log_level);
    let filter = tracing_subscriber::EnvFilter::try_new(directive)
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    tracing_subscriber::fmt().with_env_filter(filter).init();
}

fn tracing_directive(env_override: Option<String>, configured_level: &str) -> String {
    let env_override = env_override.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    });
    env_override.unwrap_or_else(|| {
        let trimmed = configured_level.trim();
        if trimmed.is_empty() {
            "info".to_string()
        } else {
            trimmed.to_string()
        }
    })
}

#[cfg(test)]
mod tests {
    use super::tracing_directive;

    #[test]
    fn prefers_env_override_when_present() {
        assert_eq!(
            tracing_directive(Some("debug".to_string()), "warn"),
            "debug"
        );
    }

    #[test]
    fn falls_back_to_configured_level() {
        assert_eq!(tracing_directive(None, "trace"), "trace");
    }

    #[test]
    fn defaults_to_info_when_configured_level_is_empty() {
        assert_eq!(tracing_directive(None, "   "), "info");
    }
}

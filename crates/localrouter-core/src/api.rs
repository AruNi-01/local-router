use anyhow::Result;
use axum::{
    Router,
    body::{Body, to_bytes},
    extract::{
        FromRequestParts, Path, Query, Request, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::{HeaderMap, HeaderName, Method, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{get, post, put},
};
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio_tungstenite::{connect_async, tungstenite};
use tower_http::cors::{Any, CorsLayer};

use crate::{
    AppState,
    models::{AddProjectRequest, DaemonConfig, LogsQuery, ManifestUpdateRequest},
};

pub fn api_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(get_health))
        .route("/config", get(get_config).put(put_config))
        .route("/projects", get(list_projects).post(add_project))
        .route(
            "/projects/{project_id}",
            get(get_project).delete(delete_project),
        )
        .route("/projects/{project_id}/rescan", post(rescan_project))
        .route("/projects/{project_id}/manifest", put(update_manifest))
        .route("/workspaces", get(list_workspaces))
        .route("/services", get(list_services))
        .route("/services/{service_id}/start", post(start_service))
        .route("/services/{service_id}/stop", post(stop_service))
        .route("/instances", get(list_instances))
        .route("/instances/{instance_id}/start", post(start_instance))
        .route("/instances/{instance_id}/stop", post(stop_instance))
        .route("/instances/{instance_id}/restart", post(restart_instance))
        .route("/routes", get(list_routes))
        .route("/logs", get(get_logs))
        .route("/graph", get(get_graph))
        .route("/events", get(events_ws))
        .with_state(state)
        .layer(
            CorsLayer::new()
                .allow_headers(Any)
                .allow_methods(Any)
                .allow_origin(Any),
        )
}

pub fn proxy_router(state: AppState) -> Router {
    Router::new().fallback(proxy_request).with_state(state)
}

async fn get_health(State(state): State<AppState>) -> impl IntoResponse {
    axum::Json(state.health().await)
}

async fn get_config(State(state): State<AppState>) -> impl IntoResponse {
    axum::Json(state.config().await)
}

async fn put_config(
    State(state): State<AppState>,
    axum::Json(config): axum::Json<DaemonConfig>,
) -> Result<impl IntoResponse, StatusCode> {
    state
        .update_config(config)
        .await
        .map(axum::Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn list_projects(State(state): State<AppState>) -> impl IntoResponse {
    axum::Json(state.projects().await)
}

async fn add_project(
    State(state): State<AppState>,
    axum::Json(request): axum::Json<AddProjectRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    state
        .add_project_request(request)
        .await
        .map(axum::Json)
        .map_err(|_| StatusCode::BAD_REQUEST)
}

async fn get_project(
    Path(project_id): Path<String>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    state
        .project_detail(&project_id)
        .await
        .map(axum::Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}

async fn delete_project(
    Path(project_id): Path<String>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    state
        .remove_project(&project_id)
        .await
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|_| StatusCode::NOT_FOUND)
}

async fn rescan_project(
    Path(project_id): Path<String>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    state
        .rescan_project(&project_id)
        .await
        .map(axum::Json)
        .map_err(|_| StatusCode::BAD_REQUEST)
}

async fn update_manifest(
    Path(project_id): Path<String>,
    State(state): State<AppState>,
    axum::Json(request): axum::Json<ManifestUpdateRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    state
        .update_manifest(&project_id, request.manifest)
        .await
        .map(axum::Json)
        .map_err(|_| StatusCode::BAD_REQUEST)
}

async fn list_workspaces(State(state): State<AppState>) -> impl IntoResponse {
    axum::Json(state.workspaces().await)
}

async fn list_services(State(state): State<AppState>) -> impl IntoResponse {
    axum::Json(state.services().await)
}

async fn start_service(
    Path(service_id): Path<String>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    state
        .start_service(&service_id)
        .await
        .map(axum::Json)
        .map_err(|_| StatusCode::BAD_REQUEST)
}

async fn stop_service(
    Path(service_id): Path<String>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    state
        .stop_service(&service_id)
        .await
        .map(axum::Json)
        .map_err(|_| StatusCode::BAD_REQUEST)
}

async fn list_instances(State(state): State<AppState>) -> impl IntoResponse {
    axum::Json(state.instances().await)
}

async fn start_instance(
    Path(instance_id): Path<String>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    state
        .start_instance(&instance_id)
        .await
        .map(axum::Json)
        .map_err(|_| StatusCode::BAD_REQUEST)
}

async fn stop_instance(
    Path(instance_id): Path<String>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    state
        .stop_instance(&instance_id)
        .await
        .map(axum::Json)
        .map_err(|_| StatusCode::BAD_REQUEST)
}

async fn restart_instance(
    Path(instance_id): Path<String>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    state
        .restart_instance(&instance_id)
        .await
        .map(axum::Json)
        .map_err(|_| StatusCode::BAD_REQUEST)
}

async fn list_routes(State(state): State<AppState>) -> impl IntoResponse {
    axum::Json(state.routes().await)
}

async fn get_logs(
    Query(query): Query<LogsQuery>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    axum::Json(state.logs(query).await)
}

async fn get_graph(State(state): State<AppState>) -> impl IntoResponse {
    axum::Json(state.graph().await)
}

async fn events_ws(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_events_socket(socket, state))
}

async fn handle_events_socket(mut socket: WebSocket, state: AppState) {
    let mut rx = state.subscribe();
    while let Ok(event) = rx.recv().await {
        let Ok(text) = serde_json::to_string(&event) else {
            continue;
        };
        if socket.send(Message::Text(text.into())).await.is_err() {
            break;
        }
    }
}

async fn proxy_request(
    State(state): State<AppState>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    request: Request<Body>,
) -> Response {
    if is_websocket_upgrade(&headers) {
        let (mut parts, _body) = request.into_parts();
        match WebSocketUpgrade::from_request_parts(&mut parts, &state).await {
            Ok(ws) => return proxy_websocket(ws, state, uri, headers).await,
            Err(_) => {
                return (
                    StatusCode::BAD_REQUEST,
                    axum::Json(json!({ "error": "invalid websocket upgrade request" })),
                )
                    .into_response();
            }
        }
    }
    match proxy_request_impl(state, method, headers, request).await {
        Ok(response) => response,
        Err(error) => (
            StatusCode::BAD_GATEWAY,
            axum::Json(json!({ "error": error.to_string() })),
        )
            .into_response(),
    }
}

fn is_websocket_upgrade(headers: &HeaderMap) -> bool {
    headers
        .get("upgrade")
        .and_then(|value| value.to_str().ok())
        .map(|value| value.eq_ignore_ascii_case("websocket"))
        .unwrap_or(false)
}

async fn proxy_websocket(
    ws: WebSocketUpgrade,
    state: AppState,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    let host = headers
        .get("host")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .split(':')
        .next()
        .unwrap_or_default()
        .to_string();
    let Some(route) = state.active_route_for_host(&host).await else {
        return (
            StatusCode::BAD_GATEWAY,
            axum::Json(json!({ "error": format!("route not found for host {host}") })),
        )
            .into_response();
    };
    let path_and_query = uri
        .path_and_query()
        .map(|item| item.as_str())
        .unwrap_or("/");
    let target = format!("ws://{}{}", route.target, path_and_query);
    ws.on_upgrade(move |socket| tunnel_websocket(socket, target))
        .into_response()
}

async fn proxy_request_impl(
    state: AppState,
    method: Method,
    headers: HeaderMap,
    request: Request<Body>,
) -> Result<Response> {
    let host = headers
        .get("host")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .split(':')
        .next()
        .unwrap_or_default()
        .to_string();
    let route = state
        .routes()
        .await
        .into_iter()
        .find(|route| route.pattern == host && route.status == crate::models::RouteStatus::Active)
        .ok_or_else(|| anyhow::anyhow!("route not found for host {host}"))?;
    let path_and_query = request
        .uri()
        .path_and_query()
        .map(|item| item.as_str())
        .unwrap_or("/");
    let target = format!("http://{}{}", route.target, path_and_query);
    let body = to_bytes(request.into_body(), usize::MAX).await?;
    let mut proxied = reqwest::Client::new().request(method, target);
    for (name, value) in &headers {
        if name.as_str().eq_ignore_ascii_case("host") {
            continue;
        }
        proxied = proxied.header(name, value);
    }
    let upstream: reqwest::Response = proxied.body(body).send().await?;
    let status = upstream.status();
    let response_headers = filter_proxy_response_headers(upstream.headers());
    let bytes = upstream.bytes().await?;
    Ok((status, response_headers, bytes).into_response())
}

fn filter_proxy_response_headers(headers: &HeaderMap) -> HeaderMap {
    let mut filtered = HeaderMap::new();
    for (name, value) in headers {
        if is_hop_by_hop_response_header(name) {
            continue;
        }
        filtered.append(name, value.clone());
    }
    filtered
}

fn is_hop_by_hop_response_header(name: &HeaderName) -> bool {
    matches!(
        name.as_str().to_ascii_lowercase().as_str(),
        "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
    )
}

async fn tunnel_websocket(socket: WebSocket, target: String) {
    let Ok((upstream, _)) = connect_async(target).await else {
        return;
    };
    let (mut downstream_tx, mut downstream_rx) = socket.split();
    let (mut upstream_tx, mut upstream_rx) = upstream.split();

    let downstream_to_upstream = async {
        while let Some(Ok(message)) = downstream_rx.next().await {
            if let Some(mapped) = map_downstream_message(message) {
                if upstream_tx.send(mapped).await.is_err() {
                    break;
                }
            }
        }
    };
    let upstream_to_downstream = async {
        while let Some(Ok(message)) = upstream_rx.next().await {
            if let Some(mapped) = map_upstream_message(message) {
                if downstream_tx.send(mapped).await.is_err() {
                    break;
                }
            }
        }
    };

    tokio::select! {
        _ = downstream_to_upstream => {}
        _ = upstream_to_downstream => {}
    }
}

fn map_downstream_message(message: Message) -> Option<tungstenite::Message> {
    match message {
        Message::Text(text) => Some(tungstenite::Message::Text(text.to_string().into())),
        Message::Binary(data) => Some(tungstenite::Message::Binary(data)),
        Message::Ping(data) => Some(tungstenite::Message::Ping(data)),
        Message::Pong(data) => Some(tungstenite::Message::Pong(data)),
        Message::Close(frame) => Some(tungstenite::Message::Close(frame.map(|frame| {
            tungstenite::protocol::CloseFrame {
                code: frame.code.into(),
                reason: frame.reason.to_string().into(),
            }
        }))),
    }
}

fn map_upstream_message(message: tungstenite::Message) -> Option<Message> {
    match message {
        tungstenite::Message::Text(text) => Some(Message::Text(text.to_string().into())),
        tungstenite::Message::Binary(data) => Some(Message::Binary(data)),
        tungstenite::Message::Ping(data) => Some(Message::Ping(data)),
        tungstenite::Message::Pong(data) => Some(Message::Pong(data)),
        tungstenite::Message::Close(frame) => Some(Message::Close(frame.map(|frame| {
            axum::extract::ws::CloseFrame {
                code: frame.code.into(),
                reason: frame.reason.to_string().into(),
            }
        }))),
        tungstenite::Message::Frame(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AppState,
        models::{
            DaemonConfig, HealthStatus, Instance, Project, Route, RouteStatus, ServiceDef,
            Workspace, now_rfc3339,
        },
        storage::PersistedState,
    };
    use axum::{http::header, response::Html, routing::get};
    use tokio::net::TcpListener;
    use tokio_tungstenite::tungstenite::{client::IntoClientRequest, http::HeaderValue};

    async fn html_response() -> impl IntoResponse {
        (
            [(header::CONTENT_DISPOSITION, "inline")],
            Html("<html><body>ok</body></html>"),
        )
    }

    async fn ws_echo(ws: WebSocketUpgrade) -> impl IntoResponse {
        ws.on_upgrade(|mut socket| async move {
            while let Some(Ok(message)) = socket.recv().await {
                if socket.send(message).await.is_err() {
                    break;
                }
            }
        })
    }

    #[tokio::test]
    async fn websocket_proxy_roundtrip_works() {
        let upstream = Router::new().route("/ws", get(ws_echo));
        let upstream_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let upstream_addr = upstream_listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(upstream_listener, upstream).await;
        });

        let project = Project {
            id: "proj-1".to_string(),
            name: "demo".to_string(),
            path: "/tmp/demo".to_string(),
            created_at: now_rfc3339(),
            config_source: "manifest".to_string(),
            proxy_disabled: false,
        };
        let workspace = Workspace {
            id: "ws-1".to_string(),
            project_id: project.id.clone(),
            name: "main".to_string(),
            branch: "main".to_string(),
            path: "/tmp/demo".to_string(),
            is_active: true,
            slug: "main".to_string(),
        };
        let service = ServiceDef {
            id: "svc-1".to_string(),
            project_id: project.id.clone(),
            name: "ws".to_string(),
            command: "echo".to_string(),
            protocol: "http".to_string(),
            adapter: "generic".to_string(),
            route: "ws".to_string(),
            healthcheck: String::new(),
            language: "generic".to_string(),
            cwd: None,
            env: Default::default(),
            depends_on: Vec::new(),
            enabled: true,
        };
        let instance = Instance {
            id: "inst-1".to_string(),
            service_id: service.id.clone(),
            service_name: service.name.clone(),
            workspace_id: workspace.id.clone(),
            workspace_name: workspace.name.clone(),
            project_id: project.id.clone(),
            project_name: project.name.clone(),
            port: upstream_addr.port(),
            pid: 1,
            status: HealthStatus::Healthy,
            url: String::new(),
            uptime: "1m".to_string(),
            cpu: 0.0,
            memory: 0,
            started_at: Some(now_rfc3339()),
            last_exit: None,
            status_reason: None,
        };
        let route = Route {
            id: "rt-1".to_string(),
            pattern: "ws.demo.localhost".to_string(),
            url: format!(
                "http://ws.demo.localhost:{}",
                DaemonConfig::default().proxy_port
            ),
            target: format!("127.0.0.1:{}", upstream_addr.port()),
            service_id: service.id.clone(),
            service_name: service.name.clone(),
            workspace_id: workspace.id.clone(),
            workspace_name: workspace.name.clone(),
            project_id: project.id.clone(),
            project_name: project.name.clone(),
            status: RouteStatus::Active,
            conflict_reason: None,
        };
        let state = AppState::for_tests(PersistedState {
            config: DaemonConfig::default(),
            manifests: Default::default(),
            projects: vec![project],
            workspaces: vec![workspace],
            services: vec![service],
            instances: vec![instance],
            routes: vec![route],
            logs: Default::default(),
        })
        .await
        .unwrap();

        let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_addr = proxy_listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(proxy_listener, proxy_router(state)).await;
        });

        let mut request = format!("ws://127.0.0.1:{}/ws", proxy_addr.port())
            .into_client_request()
            .unwrap();
        request
            .headers_mut()
            .insert("Host", HeaderValue::from_static("ws.demo.localhost"));
        let (mut client, _) = connect_async(request).await.unwrap();
        client
            .send(tungstenite::Message::Text("hello".into()))
            .await
            .unwrap();
        let echoed = client.next().await.unwrap().unwrap();
        assert!(matches!(echoed, tungstenite::Message::Text(text) if text == "hello"));
    }

    #[tokio::test]
    async fn http_proxy_preserves_content_headers() {
        let upstream = Router::new().route("/", get(html_response));
        let upstream_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let upstream_addr = upstream_listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(upstream_listener, upstream).await;
        });

        let project = Project {
            id: "proj-1".to_string(),
            name: "demo".to_string(),
            path: "/tmp/demo".to_string(),
            created_at: now_rfc3339(),
            config_source: "manifest".to_string(),
            proxy_disabled: false,
        };
        let workspace = Workspace {
            id: "ws-1".to_string(),
            project_id: project.id.clone(),
            name: "main".to_string(),
            branch: "main".to_string(),
            path: "/tmp/demo".to_string(),
            is_active: true,
            slug: "main".to_string(),
        };
        let service = ServiceDef {
            id: "svc-1".to_string(),
            project_id: project.id.clone(),
            name: "web".to_string(),
            command: "echo".to_string(),
            protocol: "http".to_string(),
            adapter: "generic".to_string(),
            route: "web".to_string(),
            healthcheck: String::new(),
            language: "generic".to_string(),
            cwd: None,
            env: Default::default(),
            depends_on: Vec::new(),
            enabled: true,
        };
        let instance = Instance {
            id: "inst-1".to_string(),
            service_id: service.id.clone(),
            service_name: service.name.clone(),
            workspace_id: workspace.id.clone(),
            workspace_name: workspace.name.clone(),
            project_id: project.id.clone(),
            project_name: project.name.clone(),
            port: upstream_addr.port(),
            pid: 1,
            status: HealthStatus::Healthy,
            url: String::new(),
            uptime: "1m".to_string(),
            cpu: 0.0,
            memory: 0,
            started_at: Some(now_rfc3339()),
            last_exit: None,
            status_reason: None,
        };
        let route = Route {
            id: "rt-1".to_string(),
            pattern: "web.demo.localhost".to_string(),
            url: format!(
                "http://web.demo.localhost:{}",
                DaemonConfig::default().proxy_port
            ),
            target: format!("127.0.0.1:{}", upstream_addr.port()),
            service_id: service.id.clone(),
            service_name: service.name.clone(),
            workspace_id: workspace.id.clone(),
            workspace_name: workspace.name.clone(),
            project_id: project.id.clone(),
            project_name: project.name.clone(),
            status: RouteStatus::Active,
            conflict_reason: None,
        };
        let state = AppState::for_tests(PersistedState {
            config: DaemonConfig::default(),
            manifests: Default::default(),
            projects: vec![project],
            workspaces: vec![workspace],
            services: vec![service],
            instances: vec![instance],
            routes: vec![route],
            logs: Default::default(),
        })
        .await
        .unwrap();

        let proxy_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_addr = proxy_listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(proxy_listener, proxy_router(state)).await;
        });

        let response = reqwest::Client::new()
            .get(format!("http://127.0.0.1:{}/", proxy_addr.port()))
            .header("host", "web.demo.localhost")
            .send()
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("text/html; charset=utf-8")
        );
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_DISPOSITION)
                .and_then(|value| value.to_str().ok()),
            Some("inline")
        );
    }
}

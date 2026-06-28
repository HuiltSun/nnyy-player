#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use std::{collections::HashMap, sync::Arc};

// 内嵌版本作为离线兜底
const PLAYER_HTML: &str = include_str!("../../nnyy_player.html");
const HISTORY_HTML: &str = include_str!("../../history.html");

const REMOTE_PLAYER_URL: &str =
    "https://raw.githubusercontent.com/HuiltSun/nnyy-player/master/nnyy_player.html";
const REMOTE_HISTORY_URL: &str =
    "https://raw.githubusercontent.com/HuiltSun/nnyy-player/master/history.html";

#[derive(Clone)]
struct AppState {
    player: Arc<String>,
    history: Arc<String>,
}

// 启动时拉取远程 HTML，5 秒超时；失败则用内嵌版本
async fn fetch_or_fallback(url: &str, fallback: &'static str) -> String {
    let result: Result<String, _> = async {
        reqwest::Client::new()
            .get(url)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await?
            .error_for_status()?
            .text()
            .await
    }
    .await;
    result.unwrap_or_else(|_| fallback.to_owned())
}

async fn serve_player(State(s): State<AppState>) -> impl IntoResponse {
    Response::builder()
        .header("Content-Type", "text/html; charset=utf-8")
        .body(axum::body::Body::from(s.player.as_str().to_owned()))
        .unwrap()
}

async fn serve_history(State(s): State<AppState>) -> impl IntoResponse {
    Response::builder()
        .header("Content-Type", "text/html; charset=utf-8")
        .body(axum::body::Body::from(s.history.as_str().to_owned()))
        .unwrap()
}

async fn proxy_handler(Query(params): Query<HashMap<String, String>>) -> Response {
    let target = match params.get("url").filter(|u| !u.is_empty()) {
        Some(u) => u.clone(),
        None => return (StatusCode::BAD_REQUEST, "Missing url").into_response(),
    };

    let parsed = match reqwest::Url::parse(&target) {
        Ok(u) => u,
        Err(_) => return (StatusCode::BAD_REQUEST, "Invalid url").into_response(),
    };
    let referer = format!(
        "{}://{}/",
        parsed.scheme(),
        parsed.host_str().unwrap_or("")
    );

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
    {
        Ok(c) => c,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Client error").into_response(),
    };

    match client
        .get(&target)
        .header(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
        )
        .header("Referer", &referer)
        .header("Accept", "text/html,application/json,*/*")
        .header("Accept-Language", "zh-CN,zh;q=0.9")
        .send()
        .await
    {
        Ok(resp) => {
            let status =
                StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::OK);
            let ct = resp
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("application/octet-stream")
                .to_owned();
            let body = resp.bytes().await.unwrap_or_default();
            Response::builder()
                .status(status)
                .header("Content-Type", ct)
                .header("Access-Control-Allow-Origin", "*")
                .body(axum::body::Body::from(body))
                .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
        }
        Err(_) => (StatusCode::BAD_GATEWAY, "Bad Gateway").into_response(),
    }
}

fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(serve_player))
        .route("/nnyy_player.html", get(serve_player))
        .route("/history", get(serve_history))
        .route("/history.html", get(serve_history))
        .route("/proxy", get(proxy_handler))
        .with_state(state)
}

// 启动时拉取最新 HTML，再开 HTTP 服务；返回监听端口
fn start_server() -> u16 {
    let (tx, rx) = std::sync::mpsc::channel::<u16>();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        rt.block_on(async move {
            // 并行拉取两个页面，加速启动
            let (player, history) = tokio::join!(
                fetch_or_fallback(REMOTE_PLAYER_URL, PLAYER_HTML),
                fetch_or_fallback(REMOTE_HISTORY_URL, HISTORY_HTML),
            );
            let state = AppState {
                player: Arc::new(player),
                history: Arc::new(history),
            };
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind to random port");
            let port = listener.local_addr().expect("local addr").port();
            let _ = tx.send(port);
            axum::serve(listener, build_router(state))
                .await
                .expect("axum serve");
        });
    });
    rx.recv().expect("port from server thread")
}

fn main() {
    let port = start_server();
    let url = format!("http://localhost:{}", port);

    tauri::Builder::default()
        .setup(move |app| {
            tauri::WebviewWindowBuilder::new(
                app,
                "main",
                tauri::WebviewUrl::External(url.parse().unwrap()),
            )
            .title("努努影院播放器")
            .inner_size(1280.0, 800.0)
            .build()?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("tauri error");
}

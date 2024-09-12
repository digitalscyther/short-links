use axum::{
    extract::{Path, Query},
    http::{StatusCode, HeaderMap},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::{env};
use redis::{AsyncCommands, RedisResult};
use std::sync::Arc;
use axum::body::{Body, to_bytes};
use axum::extract::{Host, State};
use axum::http::Request;
use axum::response::Redirect;
use rand::distr::Alphanumeric;
use rand::Rng;
use tracing::{error, info};

#[derive(Deserialize)]
struct CreateLinkRequest {
    url: String,
}

#[derive(Serialize)]
struct CreateLinkResponse {
    short_url: String,
    stats_url: String,
}

#[derive(Deserialize)]
struct StatsQuery {
    token: Option<String>,
}


async fn redis_connection(redis_client: &redis::Client) -> RedisResult<redis::aio::MultiplexedConnection> {
    redis_client
        .get_multiplexed_async_connection().await
}

async fn generate_link(
    State(state): State<Arc<AppState>>,
    Host(hostname): Host,
    headers: HeaderMap,
    request: Request<Body>,
) -> Result<Json<CreateLinkResponse>, StatusCode> {
    let scheme = request.uri().scheme_str().unwrap_or("http").to_string();

    let req_body = request.into_body();
    let data = to_bytes(req_body, 10000).await.expect("Unable to read data");
    let payload: CreateLinkRequest = serde_json::from_slice(&data).unwrap();

    let auth_token = env::var("AUTH_TOKEN").map_err(|_| StatusCode::UNAUTHORIZED)?;
    let req_auth_token = headers.get("Authorization").and_then(|v| v.to_str().ok()).ok_or(StatusCode::UNAUTHORIZED)?;
    if auth_token != req_auth_token {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let mut redis_conn = redis_connection(&state.redis_client).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let short_key = generate_and_save_key(&mut redis_conn).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let token = rand_string(24);

    let short_url = format!("{scheme}://{hostname}/{short_key}");
    let stats_url = format!("{short_url}/stats?token={token}");

    redis_conn.hset(&short_key, "url", payload.url).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    redis_conn.hset(&short_key, "token", token).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    redis_conn.hset(&short_key, "clicks", 0).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(CreateLinkResponse {
        short_url,
        stats_url,
    }))
}

async fn generate_and_save_key(
    redis_conn: &mut redis::aio::MultiplexedConnection
) -> Result<String, String> {
    for attempt in 0..3 {
        let short_key: String = rand_string(6);

        let key_exists: bool = redis_conn.exists(&short_key).await.map_err(|e| {
            error!("Redis error on check: {:?}", e);
            "Redis check error".to_string()
        })?;

        if !key_exists {
            redis_conn.hset(&short_key, "url", "").await.map_err(|e| format!("{:?}", e))?;
            redis_conn.expire(&short_key, 60 * 60 * 24 * 30).await.map_err(|e| format!("{:?}", e))?;

            return Ok(short_key);
        }

        error!(
            "Generated key already exists in Redis. Attempt {}/3. Retrying...",
            attempt + 1
        );
    }

    Err("Failed to generate a unique key after 3 attempts".to_string())
}

fn rand_string(n: usize) -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(n)
        .map(char::from)
        .collect()
}

async fn proxy_link(
    Path(short_key): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Redirect, StatusCode> {
    let mut redis_conn = redis_connection(&state.redis_client).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let original_url: String = redis_conn.hget(&short_key, "url").await.map_err(|_| StatusCode::NOT_FOUND)?;

    redis_conn.hincr(&short_key, "clicks", 1).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Redirect::temporary(&original_url))
}

async fn get_stats(
    Path(short_key): Path<String>,
    Query(params): Query<StatsQuery>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<HashMap<String, usize>>, StatusCode> {
    let mut redis_conn = redis_connection(&state.redis_client).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let stored_token: String = redis_conn.hget(&short_key, "token").await.map_err(|_| StatusCode::NOT_FOUND)?;

    match params.token {
        None => return Err(StatusCode::NOT_FOUND),
        Some(token) if token != stored_token => return Err(StatusCode::UNAUTHORIZED),
        _ => {},
    }

    let clicks: usize = redis_conn.hget(&short_key, "clicks").await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut stats = HashMap::new();
    stats.insert("clicks".to_string(), clicks);

    Ok(Json(stats))
}

pub struct AppState {
    pub redis_client: redis::Client,
}

#[tokio::main]
async fn main() {
    let redis_url = env::var("REDIS_URL").unwrap_or("redis://127.0.0.1/".to_string());
    let redis_client = redis::Client::open(redis_url).unwrap();
    let app_state = AppState { redis_client };

    let router = Router::new()
        .route("/:short_key", get(proxy_link))
        .route("/:short_key/stats", get(get_stats))
        .route("/generate", post(generate_link))
        .with_state(Arc::new(app_state));

    let host = env::var("HOST").unwrap_or("127.0.0.1".to_string());
    let port = env::var("PORT").unwrap_or("3000".to_string());
    let bind_address = format!("{}:{}", host, port);
    info!("Listening on {}", bind_address);
    let listener = tokio::net::TcpListener::bind(bind_address)
        .await
        .unwrap();

    axum::serve(listener, router.into_make_service()).await.unwrap();
}

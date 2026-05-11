//! Opt-in bearer-token auth middleware.
//!
//! When `ServerConfig.auth_bearer` is `Some(token)`, every route except
//! `/health` and `/version` requires `Authorization: Bearer <token>`.
//! Health and version stay public so external uptime checkers don't need
//! to manage credentials.

use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderMap, StatusCode};
use axum::middleware::Next;
use axum::response::Response;

/// Compare the request's `Authorization` header against `expected_token`.
/// Returns `Ok(())` if the header matches (case-sensitive on the token
/// itself, "Bearer" prefix is mandatory). Errors are mapped to 401.
pub fn check_bearer(headers: &HeaderMap, expected_token: &str) -> Result<(), StatusCode> {
    let header = headers
        .get("authorization")
        .or_else(|| headers.get("Authorization"))
        .and_then(|h| h.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let provided = header
        .strip_prefix("Bearer ")
        .map(str::trim)
        .ok_or(StatusCode::UNAUTHORIZED)?;
    if provided == expected_token {
        Ok(())
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

/// Axum middleware factory. The returned closure can be plugged into
/// `axum::middleware::from_fn` via the call site (kept here as a free
/// function so the router can decide whether to wire it at all).
pub async fn require_bearer(
    expected_token: String,
    req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    check_bearer(req.headers(), &expected_token)?;
    Ok(next.run(req).await)
}

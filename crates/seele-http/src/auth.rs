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
use subtle::ConstantTimeEq;

/// Compare the request's `Authorization` header against `expected_token`.
/// Returns `Ok(())` if the header matches (case-sensitive on the token
/// itself, "Bearer" prefix is mandatory). Errors are mapped to 401.
/// The token comparison is constant-time (`subtle`) so a wrong guess
/// doesn't leak how many leading bytes were correct.
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
    if bool::from(provided.as_bytes().ct_eq(expected_token.as_bytes())) {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn headers_with_bearer(token: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", format!("Bearer {token}").parse().unwrap());
        headers
    }

    #[test]
    fn correct_token_passes() {
        let headers = headers_with_bearer("s3cret");
        assert!(check_bearer(&headers, "s3cret").is_ok());
    }

    #[test]
    fn same_length_different_token_fails() {
        let headers = headers_with_bearer("s3creX");
        assert_eq!(
            check_bearer(&headers, "s3cret"),
            Err(StatusCode::UNAUTHORIZED)
        );
    }

    #[test]
    fn token_differing_only_in_last_byte_fails() {
        // Pins the semantics of the constant-time comparison: a mismatch
        // in the final byte must reject just like any other.
        let headers = headers_with_bearer("token-abcy");
        assert_eq!(
            check_bearer(&headers, "token-abcz"),
            Err(StatusCode::UNAUTHORIZED)
        );
    }

    #[test]
    fn different_length_token_fails() {
        let headers = headers_with_bearer("s3cret-extended");
        assert_eq!(
            check_bearer(&headers, "s3cret"),
            Err(StatusCode::UNAUTHORIZED)
        );
    }
}

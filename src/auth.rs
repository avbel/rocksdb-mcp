use std::sync::Arc;

use axum::{
    extract::{Request, State},
    http::{HeaderValue, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};

#[derive(Clone)]
pub struct BearerToken(pub Arc<String>);

pub async fn require_bearer(
    State(token): State<BearerToken>,
    req: Request,
    next: Next,
) -> Response {
    let expected = format!("Bearer {}", token.0);
    let expected = HeaderValue::from_str(&expected).expect("ASCII bearer token");

    match req.headers().get(header::AUTHORIZATION) {
        Some(got) if got == expected => next.run(req).await,
        Some(_) => (StatusCode::UNAUTHORIZED, "invalid bearer token").into_response(),
        None => (
            StatusCode::UNAUTHORIZED,
            [(header::WWW_AUTHENTICATE, "Bearer")],
            "missing bearer token",
        )
            .into_response(),
    }
}

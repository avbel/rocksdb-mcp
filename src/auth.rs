use std::sync::Arc;

use axum::{
    extract::{Request, State},
    http::{HeaderValue, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};

#[derive(Clone)]
pub struct BearerToken(Arc<HeaderValue>);

impl BearerToken {
    pub fn new(token: &str) -> anyhow::Result<Self> {
        let header = HeaderValue::from_str(&format!("Bearer {token}"))
            .map_err(|_| anyhow::anyhow!("API token contains non-ASCII characters"))?;
        Ok(Self(Arc::new(header)))
    }
}

pub async fn require_bearer(
    State(token): State<BearerToken>,
    req: Request,
    next: Next,
) -> Response {
    match req.headers().get(header::AUTHORIZATION) {
        Some(got) if got == token.0.as_ref() => next.run(req).await,
        Some(_) => (StatusCode::UNAUTHORIZED, "invalid bearer token").into_response(),
        None => (
            StatusCode::UNAUTHORIZED,
            [(header::WWW_AUTHENTICATE, "Bearer")],
            "missing bearer token",
        )
            .into_response(),
    }
}

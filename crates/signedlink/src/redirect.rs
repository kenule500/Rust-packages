//! Optional `axum` redirect handler.
//!
//! The handler resolves the code, verifies the token, enforces store-backed
//! revocation and single use, records a click hook, and redirects. It sets
//! `Referrer-Policy: no-referrer` and `Cache-Control: no-store`, refuses a
//! non-HTTPS target, and never returns the token in a response body.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::{Path, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};

use crate::{Code, LinkError, LinkStore, Signer};

/// State for the redirect handler.
pub struct RedirectState<S: LinkStore> {
    /// The verifier.
    pub signer: Arc<Signer>,
    /// The authoritative store.
    pub store: Arc<S>,
    /// Optional click hook, called only after a successful verification.
    pub on_click: Option<fn(&Code)>,
}

impl<S: LinkStore> RedirectState<S> {
    /// Creates redirect state without a click hook.
    #[must_use]
    pub fn new(signer: Arc<Signer>, store: Arc<S>) -> Self {
        Self {
            signer,
            store,
            on_click: None,
        }
    }
}

/// Resolves `/{code}/{token}` and redirects to the signed target.
///
/// Every failure returns a static body; the token, the key, and the target are
/// never echoed.
pub async fn redirect_handler<S>(
    State(state): State<Arc<RedirectState<S>>>,
    Path((code, token)): Path<(String, String)>,
) -> Response
where
    S: LinkStore + Send + Sync + 'static,
{
    let code = match Code::new(code) {
        Ok(code) => code,
        Err(_) => return error_response(StatusCode::BAD_REQUEST),
    };
    match state
        .signer
        .verify_with_store(&token, &code, unix_now(), state.store.as_ref())
        .await
    {
        Ok(claims) => {
            if claims.target.scheme() != "https" {
                return error_response(StatusCode::BAD_REQUEST);
            }
            if let Some(hook) = state.on_click {
                hook(&code);
            }
            let mut response = Redirect::temporary(claims.target.as_str()).into_response();
            let headers = response.headers_mut();
            headers.insert(
                header::REFERRER_POLICY,
                HeaderValue::from_static("no-referrer"),
            );
            headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
            response
        }
        Err(error) => error_response(status_for(&error)),
    }
}

/// Returns the current Unix time in seconds.
///
/// A system clock before the epoch is reported as zero, which makes links fail
/// closed rather than open.
#[must_use]
pub fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn status_for(error: &LinkError) -> StatusCode {
    match error {
        LinkError::NotFound => StatusCode::NOT_FOUND,
        LinkError::Revoked | LinkError::Consumed | LinkError::Expired { .. } => StatusCode::GONE,
        LinkError::Tampered
        | LinkError::InvalidToken { .. }
        | LinkError::InvalidCode { .. }
        | LinkError::InvalidTarget { .. }
        | LinkError::InvalidTenant
        | LinkError::InvalidText { .. }
        | LinkError::InvalidUtmKey { .. }
        | LinkError::InvalidUtmValue { .. }
        | LinkError::NotYetValid { .. } => StatusCode::BAD_REQUEST,
        LinkError::InvalidKey { .. }
        | LinkError::InvalidPolicy { .. }
        | LinkError::Entropy
        | LinkError::CodeExhausted { .. }
        | LinkError::Store { .. } => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn error_response(status: StatusCode) -> Response {
    (status, "invalid or unavailable link").into_response()
}

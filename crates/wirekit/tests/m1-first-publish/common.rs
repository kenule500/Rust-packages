//! Shared helpers for the `m1-first-publish` acceptance tests.
//!
//! Everything here is test infrastructure: a mock server wrapper that records
//! the requests it receives, a capture subscriber for the tracing events the
//! pipeline emits, repository-file readers, and precomputed crypto fixtures.
//! No production behaviour is re-implemented except the independent HMAC and
//! ECDSA fixtures, which are embedded as constants so a provider signature is
//! checked against bytes produced outside this crate.

#![allow(dead_code)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;
use wirekit::{RetryPolicy, WirekitClient, WirekitClientBuilder};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

// ---------------------------------------------------------------------------
// Mock HTTP server
// ---------------------------------------------------------------------------

/// One request the mock server received.
#[derive(Debug, Clone)]
pub struct Recorded {
    /// The HTTP method, as text.
    pub method: String,
    /// The full request URL, including the query string.
    pub url: String,
    /// Every header, lowercased by the HTTP stack.
    pub headers: Vec<(String, String)>,
    /// The raw body bytes.
    pub body: Vec<u8>,
}

impl Recorded {
    fn from_request(request: &Request) -> Self {
        let headers = request
            .headers
            .iter()
            .map(|(name, value)| {
                (
                    name.as_str().to_string(),
                    value.to_str().unwrap_or_default().to_string(),
                )
            })
            .collect();
        Self {
            method: request.method.to_string(),
            url: request.url.to_string(),
            headers,
            body: request.body.clone(),
        }
    }

    /// The first value of `name`, matched case-insensitively.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    /// The body as lossy UTF-8, for form and JSON assertions.
    #[must_use]
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    /// The body decoded as JSON.
    #[must_use]
    pub fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap_or_else(|error| {
            panic!("the request body was not JSON ({error}): {}", self.text())
        })
    }

    /// The query value for `name`, if the URL carries one.
    #[must_use]
    pub fn query_param(&self, name: &str) -> Option<String> {
        let query = self.url.split_once('?').map(|(_, query)| query)?;
        query.split('&').find_map(|pair| {
            let (key, value) = pair.split_once('=')?;
            (key == name).then(|| value.to_string())
        })
    }
}

/// A responder that records every request and can answer the first one
/// differently, which is how a retry is provoked.
#[derive(Clone, Debug)]
pub struct Recorder {
    status: u16,
    body: String,
    first_status: Option<u16>,
    headers: Vec<(String, String)>,
    delay_ms: u64,
    hits: Arc<AtomicUsize>,
    seen: Arc<Mutex<Vec<Recorded>>>,
}

impl Recorder {
    /// Answer every request with `status` and `body`.
    pub fn new(status: u16, body: impl Into<String>) -> Self {
        Self {
            status,
            body: body.into(),
            first_status: None,
            headers: Vec::new(),
            delay_ms: 0,
            hits: Arc::new(AtomicUsize::new(0)),
            seen: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Answer every request with a JSON body.
    pub fn json(status: u16, value: Value) -> Self {
        Self::new(
            status,
            serde_json::to_string(&value).expect("the fixture is serializable"),
        )
    }

    /// Answer the first request with `status`, then continue with the
    /// configured status.
    #[must_use]
    pub fn first_status(mut self, status: u16) -> Self {
        self.first_status = Some(status);
        self
    }

    /// Add a response header to every answer.
    #[must_use]
    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_string(), value.to_string()));
        self
    }

    /// Delay every answer by `ms` milliseconds.
    #[must_use]
    pub fn delay(mut self, ms: u64) -> Self {
        self.delay_ms = ms;
        self
    }

    /// How many requests reached this responder.
    #[must_use]
    pub fn hits(&self) -> usize {
        self.hits.load(Ordering::SeqCst)
    }

    /// Every request this responder saw, in order.
    #[must_use]
    pub fn calls(&self) -> Vec<Recorded> {
        self.seen
            .lock()
            .expect("the request log is not poisoned")
            .clone()
    }

    /// The most recent request.
    #[must_use]
    pub fn last(&self) -> Recorded {
        self.calls()
            .pop()
            .expect("at least one request must have reached the mock server")
    }
}

impl Respond for Recorder {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let index = self.hits.fetch_add(1, Ordering::SeqCst);
        self.seen
            .lock()
            .expect("the request log is not poisoned")
            .push(Recorded::from_request(request));

        let status = if index == 0 {
            self.first_status.unwrap_or(self.status)
        } else {
            self.status
        };
        let mut template = ResponseTemplate::new(status).set_body_string(self.body.clone());
        if self.delay_ms > 0 {
            template = template.set_delay(Duration::from_millis(self.delay_ms));
        }
        for (name, value) in &self.headers {
            template = template.append_header(name.as_str(), value.as_str());
        }
        template
    }
}

/// Start a mock server.
pub async fn server() -> MockServer {
    MockServer::start().await
}

/// Register a responder for one method and path, returning it for assertions.
pub async fn mount(
    server: &MockServer,
    http_method: &str,
    path_pattern: &str,
    recorder: Recorder,
) -> Recorder {
    server
        .register(
            Mock::given(method(http_method))
                .and(path(path_pattern))
                .respond_with(recorder.clone()),
        )
        .await;
    recorder
}

/// A builder for a client pointed at the mock server, with retries disabled so
/// request counts are exact. Tests that exercise retries replace the policy.
pub fn builder(server: &MockServer) -> WirekitClientBuilder {
    WirekitClient::builder(server.uri())
        .expect("the mock server URI is a valid base URL")
        .allow_insecure_localhost(true)
        .retry(RetryPolicy::none())
}

/// A client pointed at the mock server with retries disabled.
pub fn client(server: &MockServer) -> WirekitClient {
    builder(server).build().expect("the test client builds")
}

// ---------------------------------------------------------------------------
// Tracing capture
// ---------------------------------------------------------------------------

/// One captured tracing event.
#[derive(Debug, Clone)]
pub struct Captured {
    /// The event target, such as `wirekit::request`.
    pub target: String,
    /// The event level, such as `DEBUG`.
    pub level: String,
    /// The message field.
    pub message: String,
    /// Every other field, rendered for assertions.
    pub fields: Vec<(String, String)>,
}

impl Captured {
    /// The rendered value of `name`, if the event carried it.
    #[must_use]
    pub fn field(&self, name: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
}

/// A subscriber that records every event emitted by `wirekit` targets.
#[derive(Debug, Default)]
pub struct Capture {
    events: Mutex<Vec<Captured>>,
}

impl Capture {
    /// A shared, empty capture.
    #[must_use]
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Every event seen so far.
    #[must_use]
    pub fn events(&self) -> Vec<Captured> {
        self.events
            .lock()
            .expect("the event log is not poisoned")
            .clone()
    }

    /// Every message seen so far.
    #[must_use]
    pub fn messages(&self) -> Vec<String> {
        self.events()
            .into_iter()
            .map(|event| event.message)
            .collect()
    }

    /// The first event whose message equals `message`.
    #[must_use]
    pub fn find(&self, message: &str) -> Option<Captured> {
        self.events()
            .into_iter()
            .find(|event| event.message == message)
    }

    /// Every event whose message contains `fragment`.
    #[must_use]
    pub fn matching(&self, fragment: &str) -> Vec<Captured> {
        self.events()
            .into_iter()
            .filter(|event| event.message.contains(fragment))
            .collect()
    }

    /// How many events carry exactly `message`.
    #[must_use]
    pub fn count(&self, message: &str) -> usize {
        self.matching(message).len()
    }

    /// Everything captured, flattened into one string, so a secret-leak
    /// assertion covers every field of every event.
    #[must_use]
    pub fn rendered(&self) -> String {
        let mut output = String::new();
        for event in self.events() {
            output.push_str(&event.target);
            output.push(' ');
            output.push_str(&event.level);
            output.push(' ');
            output.push_str(&event.message);
            for (key, value) in event.fields {
                output.push(' ');
                output.push_str(&key);
                output.push('=');
                output.push_str(&value);
            }
            output.push('\n');
        }
        output
    }
}

#[derive(Default)]
struct FieldVisitor {
    message: String,
    fields: Vec<(String, String)>,
}

impl tracing::field::Visit for FieldVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        let rendered = format!("{value:?}");
        if field.name() == "message" {
            self.message = rendered;
        } else {
            self.fields.push((field.name().to_string(), rendered));
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_string();
        } else {
            self.fields
                .push((field.name().to_string(), value.to_string()));
        }
    }
}

impl tracing::Subscriber for Capture {
    fn enabled(&self, metadata: &tracing::Metadata<'_>) -> bool {
        metadata.target().starts_with("wirekit")
    }

    fn new_span(&self, _span: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(1)
    }

    fn record(&self, _span: &tracing::span::Id, _values: &tracing::span::Record<'_>) {}

    fn record_follows_from(&self, _span: &tracing::span::Id, _follows: &tracing::span::Id) {}

    fn event(&self, event: &tracing::Event<'_>) {
        let mut visitor = FieldVisitor::default();
        event.record(&mut visitor);
        let metadata = event.metadata();
        self.events
            .lock()
            .expect("the event log is not poisoned")
            .push(Captured {
                target: metadata.target().to_string(),
                level: metadata.level().to_string(),
                message: visitor.message,
                fields: visitor.fields,
            });
    }

    fn enter(&self, _span: &tracing::span::Id) {}

    fn exit(&self, _span: &tracing::span::Id) {}
}

/// Install `capture` as the thread-local subscriber for the test's lifetime.
pub fn capture() -> (Arc<Capture>, tracing::subscriber::DefaultGuard) {
    let capture = Capture::new();
    let guard = tracing::subscriber::set_default(Arc::clone(&capture));
    (capture, guard)
}

// ---------------------------------------------------------------------------
// Repository files
// ---------------------------------------------------------------------------

/// Read a file relative to the `wirekit` crate manifest directory.
pub fn repo_file(relative: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{} could not be read: {error}", path.display()))
}

/// Read one source file of the crate.
pub fn crate_source(name: &str) -> String {
    repo_file(&format!("src/{name}.rs"))
}

/// Every `src/*.rs` file of the crate, as `(file name, contents)` pairs.
pub fn crate_sources() -> Vec<(String, String)> {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut sources: Vec<(String, String)> = std::fs::read_dir(&directory)
        .unwrap_or_else(|error| panic!("{} could not be listed: {error}", directory.display()))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
        .map(|path| {
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            let contents = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("{} could not be read: {error}", path.display()));
            (name, contents)
        })
        .collect();
    sources.sort_by(|left, right| left.0.cmp(&right.0));
    sources
}

/// Every `tests/m1-first-publish/*.rs` file, as `(file name, contents)` pairs.
pub fn test_sources() -> Vec<(String, String)> {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/m1-first-publish");
    let mut sources: Vec<(String, String)> = std::fs::read_dir(&directory)
        .unwrap_or_else(|error| panic!("{} could not be listed: {error}", directory.display()))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
        .map(|path| {
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            let contents = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("{} could not be read: {error}", path.display()));
            (name, contents)
        })
        .collect();
    sources.sort_by(|left, right| left.0.cmp(&right.0));
    sources
}

/// The acceptance-criterion ids declared by the specification, without the
/// `AC-` prefix, in ascending order.
pub fn spec_ac_ids() -> Vec<u32> {
    let spec = repo_file("../../.sdrive/specs/ongoing/m1-first-publish/v1/spec.md");
    let mut ids: Vec<u32> = spec
        .lines()
        .filter_map(|line| line.strip_prefix("- AC-"))
        .filter_map(|rest| rest.get(..3))
        .filter_map(|digits| digits.parse::<u32>().ok())
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

// ---------------------------------------------------------------------------
// Precomputed crypto fixtures
// ---------------------------------------------------------------------------

/// Twilio's documented webhook auth token for this fixture.
pub const TWILIO_TOKEN: &str = "12345";
/// The webhook URL the fixture signature covers.
pub const TWILIO_WEBHOOK_URL: &str = "https://example.com/webhooks/twilio";
/// The raw form body the fixture signature covers.
pub const TWILIO_WEBHOOK_BODY: &str = "CallSid=CA123&From=%2B15550001111&To=%2B15550002222";
/// HMAC-SHA1 (base64) over `URL + sorted(name + value)`, produced outside this
/// crate.
pub const TWILIO_WEBHOOK_SIGNATURE: &str = "2dhn9hiBFDEvGgvknVOEL8c4JO8=";
/// The base64 of `AC123:secret`, for the Basic-auth assertion.
pub const TWILIO_BASIC_AUTH: &str = "QUMxMjM6c2VjcmV0";
/// The base64 of `SK123:secret2`, for the API-key Basic-auth assertion.
pub const TWILIO_API_KEY_AUTH: &str = "U0sxMjM6c2VjcmV0Mg==";

/// Slack's signing secret for the signature fixture.
pub const SLACK_SIGNING_SECRET: &str = "8f742231b10e8888abcd99yyyzzz85a5";
/// The request timestamp the fixture signature covers.
pub const SLACK_FIXTURE_TIMESTAMP: &str = "1700000000";
/// The raw body the fixture signature covers.
pub const SLACK_FIXTURE_BODY: &str = r#"{"type":"event_callback","event":{"type":"app_mention"}}"#;
/// HMAC-SHA256 (hex) over `v0:{timestamp}:{body}`, produced outside this crate.
pub const SLACK_FIXTURE_SIGNATURE: &str =
    "v0=a26c279d1cb3398a35a4c509f20523b2c5e109184463f759ec2abd4017d47bf7";

/// A P-256 public key (SPKI DER, base64) for the SendGrid event fixture.
pub const SENDGRID_EVENT_KEY: &str = "MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEY9duIf139r6+Ms0YoFh8Zli3o42I5hTSo5DxAeBYSGMNdGD1lvTrBCA1+yWX0p8V/RKpTrYE84XIcK4Josquqg==";
/// The timestamp the SendGrid fixture signature covers.
pub const SENDGRID_EVENT_TIMESTAMP: &str = "1700000000";
/// The raw Event Webhook payload the fixture signature covers.
pub const SENDGRID_EVENT_PAYLOAD: &str = r#"[{"email":"example@test.com","timestamp":1700000000,"sg_event_id":"abc123","event":"delivered"}]"#;
/// ECDSA P-256 / SHA-256 DER signature (base64) over
/// `timestamp || payload`, produced outside this crate.
pub const SENDGRID_EVENT_SIGNATURE: &str = "MEUCIQDx8cpVj6wM5PQPKdPElwOBfi+Nd3kiM3Cf7QpqJnqVEgIgeQsP3nLKMBpLisNKCJgP2S2cxzuCGLZSzv+m5wk2q88=";

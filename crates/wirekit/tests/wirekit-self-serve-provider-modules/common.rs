//! Shared helpers for the `wirekit-self-serve-provider-modules` acceptance
//! tests.
//!
//! Everything here is test infrastructure: a mock-server helper that can answer
//! the first request differently (which is how a retry is provoked), request
//! inspection helpers, a tracing capture subscriber, and client builders
//! pointed at the loopback mock.

#![allow(dead_code)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::ThreadId;
use std::time::Duration;

use wirekit::{JitterBounds, RetryPolicy, WirekitClient, WirekitClientBuilder};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

/// The credential every fixture uses. It is a literal test value, never a real
/// key, and no test may let it reach a log line.
pub const API_KEY: &str = "test_credential_do_not_log_1234";

/// The webhook secret the PagerDuty signature tests sign with.
pub const WEBHOOK_SECRET: &str = "webhook_secret_do_not_log_5678";

// ---------------------------------------------------------------------------
// Mock HTTP server
// ---------------------------------------------------------------------------

/// A responder that records how many requests it saw and can answer the first
/// one with a different status.
#[derive(Clone, Debug)]
pub struct Fixture {
    status: u16,
    body: String,
    first_status: Option<u16>,
    headers: Vec<(String, String)>,
    delay_ms: u64,
    hits: Arc<AtomicUsize>,
}

impl Fixture {
    /// Answer every request with a JSON body.
    pub fn json(status: u16, value: serde_json::Value) -> Self {
        Self {
            status,
            body: serde_json::to_string(&value).expect("the fixture is serializable"),
            first_status: None,
            headers: Vec::new(),
            delay_ms: 0,
            hits: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Answer every request with a raw body.
    pub fn raw(status: u16, body: impl Into<String>) -> Self {
        Self {
            status,
            body: body.into(),
            first_status: None,
            headers: Vec::new(),
            delay_ms: 0,
            hits: Arc::new(AtomicUsize::new(0)),
        }
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
}

impl Respond for Fixture {
    fn respond(&self, _request: &Request) -> ResponseTemplate {
        let index = self.hits.fetch_add(1, Ordering::SeqCst);
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
    fixture: Fixture,
) -> Fixture {
    server
        .register(
            Mock::given(method(http_method))
                .and(path(path_pattern))
                .respond_with(fixture.clone()),
        )
        .await;
    fixture
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
pub fn transport(server: &MockServer) -> WirekitClient {
    builder(server).build().expect("the test client builds")
}

/// A one-retry policy with a short base delay, used by the `Retry-After` tests.
pub fn one_retry() -> RetryPolicy {
    RetryPolicy {
        attempts: 2,
        base_delay: Duration::from_millis(1),
        max_delay: Duration::from_secs(2),
        jitter: JitterBounds::new(1.0, 1.0).expect("valid jitter bounds"),
    }
}

/// Every request the server received, in order.
pub async fn requests(server: &MockServer) -> Vec<Request> {
    server
        .received_requests()
        .await
        .expect("the mock server records requests")
}

/// The request body decoded as JSON.
pub fn json(request: &Request) -> serde_json::Value {
    serde_json::from_slice(&request.body).unwrap_or_else(|error| {
        panic!(
            "the request body is JSON ({error}): {}",
            String::from_utf8_lossy(&request.body)
        )
    })
}

/// The first value of `name`, matched case-insensitively.
pub fn header<'a>(request: &'a Request, name: &str) -> Option<&'a str> {
    request
        .headers
        .get(name)
        .and_then(|value| value.to_str().ok())
}

/// The query string of the request URL, if it has one.
pub fn query(request: &Request) -> &str {
    request.url.query().unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Tracing capture
// ---------------------------------------------------------------------------

/// One captured tracing event.
#[derive(Debug, Clone)]
pub struct Captured {
    /// The event target, such as `wirekit::request`.
    pub target: String,
    /// The message field.
    pub message: String,
    /// The thread that emitted the event, so parallel tests stay isolated.
    pub thread: ThreadId,
}

/// A subscriber that records the message of every `wirekit` event.
#[derive(Debug, Default)]
pub struct Capture {
    events: Mutex<Vec<Captured>>,
}

/// A no-op guard kept for call-site symmetry with `tracing::subscriber::set_default`.
pub struct CaptureGuard;

impl Capture {
    /// A shared, empty capture.
    #[must_use]
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Every message this test's thread emitted.
    #[must_use]
    pub fn messages(&self) -> Vec<String> {
        let thread = std::thread::current().id();
        self.events
            .lock()
            .expect("the event log is not poisoned")
            .iter()
            .filter(|event| event.thread == thread)
            .map(|event| event.message.clone())
            .collect()
    }
}

/// The process-wide capture subscriber.
///
/// A thread-local default is not enough here: `tracing` caches a callsite's
/// interest globally, so a callsite first reached by a test without a
/// subscriber can be cached as "never" and then skipped even when a later test
/// installs one. A single global subscriber registers every callsite, and
/// [`Capture::messages`] filters by thread so parallel tests stay isolated.
static GLOBAL_CAPTURE: OnceLock<Arc<Capture>> = OnceLock::new();

struct MessageVisitor(String);

impl tracing::field::Visit for MessageVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.0 = format!("{value:?}");
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.0 = value.to_string();
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
        let mut visitor = MessageVisitor(String::new());
        event.record(&mut visitor);
        self.events
            .lock()
            .expect("the event log is not poisoned")
            .push(Captured {
                target: event.metadata().target().to_string(),
                message: visitor.0,
                thread: std::thread::current().id(),
            });
    }

    fn enter(&self, _span: &tracing::span::Id) {}

    fn exit(&self, _span: &tracing::span::Id) {}
}

/// Return the shared capture, clearing this thread's earlier events.
pub fn capture() -> (Arc<Capture>, CaptureGuard) {
    let capture = GLOBAL_CAPTURE.get_or_init(|| {
        let capture = Capture::new();
        let _ = tracing::subscriber::set_global_default(Arc::clone(&capture));
        capture
    });
    let thread = std::thread::current().id();
    capture
        .events
        .lock()
        .expect("the event log is not poisoned")
        .retain(|event| event.thread != thread);
    (Arc::clone(capture), CaptureGuard)
}

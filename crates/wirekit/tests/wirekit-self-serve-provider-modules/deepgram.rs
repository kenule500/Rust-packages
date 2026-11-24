//! Acceptance tests for the Deepgram module (AC-001 to AC-005).

use std::time::{Duration, Instant};

use serde_json::json;
use wirekit::deepgram::{DeepgramClient, DeepgramConfig, TranscriptionRequest};
use wirekit::{Secret, WirekitError};
use wiremock::MockServer;

use crate::common::{self, Fixture};

fn config() -> DeepgramConfig {
    DeepgramConfig::new(Secret::new(String::from(common::API_KEY)))
}

fn client(server: &MockServer) -> DeepgramClient {
    DeepgramClient::new(common::transport(server), config()).expect("the client builds")
}

fn transcription_json() -> serde_json::Value {
    json!({
        "metadata": { "request_id": "req-1", "duration": 12.5 },
        "results": {
            "channels": [{
                "alternatives": [{
                    "transcript": "Hello world",
                    "confidence": 0.99,
                    "words": [
                        {
                            "word": "hello",
                            "punctuated_word": "Hello",
                            "start": 0.1,
                            "end": 0.4,
                            "confidence": 0.98,
                            "speaker": 0
                        },
                        {
                            "word": "world",
                            "start": 0.5,
                            "end": 0.9,
                            "confidence": 0.99,
                            "speaker": 1
                        }
                    ]
                }]
            }],
            "utterances": [
                {
                    "speaker": 0,
                    "transcript": "Hello",
                    "start": 0.1,
                    "end": 0.4,
                    "confidence": 0.98,
                    "words": []
                },
                {
                    "speaker": 1,
                    "transcript": "world",
                    "start": 0.5,
                    "end": 0.9,
                    "confidence": 0.99,
                    "words": []
                }
            ]
        }
    })
}

#[tokio::test]
async fn ac_001_deepgram_surface_matches_the_catalogue() {
    let server = common::server().await;
    common::mount(
        &server,
        "POST",
        "/listen",
        Fixture::json(200, transcription_json()),
    )
    .await;
    common::mount(
        &server,
        "GET",
        "/projects",
        Fixture::json(
            200,
            json!({ "projects": [{ "project_id": "p1", "name": "Storm", "company": "Acme" }] }),
        ),
    )
    .await;

    let client = client(&server);

    let request = TranscriptionRequest::from_url("https://example.com/call.mp3")
        .with_model("nova-3")
        .with_language("en")
        .with_diarization()
        .with_punctuation()
        .with_smart_format()
        .with_keywords(["shingle", "hail:2"]);
    let transcript = client
        .transcribe(&request)
        .await
        .expect("URL transcription succeeds");
    assert_eq!(transcript.request_id.as_deref(), Some("req-1"));
    assert_eq!(transcript.transcript, "Hello world");
    assert_eq!(transcript.confidence, 0.99);
    assert_eq!(transcript.duration_seconds, Some(12.5));
    assert_eq!(transcript.words.len(), 2);
    assert_eq!(
        transcript.words[0].punctuated_word.as_deref(),
        Some("Hello")
    );
    assert_eq!(transcript.words[1].speaker, Some(1));
    assert_eq!(transcript.utterances.len(), 2);
    assert_eq!(transcript.utterances[0].speaker, Some(0));
    assert_eq!(transcript.utterances[1].transcript, "world");

    let bytes = TranscriptionRequest::from_bytes("audio/wav", b"RIFFdata".to_vec());
    let transcript = client
        .transcribe(&bytes)
        .await
        .expect("byte transcription succeeds");
    assert_eq!(transcript.transcript, "Hello world");

    let projects = client
        .list_projects()
        .await
        .expect("project listing succeeds");
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].project_id, "p1");
    assert_eq!(projects[0].name.as_deref(), Some("Storm"));

    let requests = common::requests(&server).await;
    assert_eq!(requests.len(), 3);

    let url_request = &requests[0];
    let query = common::query(url_request);
    for expected in [
        "model=nova-3",
        "language=en",
        "diarize=true",
        "utterances=true",
        "punctuate=true",
        "smart_format=true",
        "keywords=shingle",
        "keywords=hail%3A2",
    ] {
        assert!(query.contains(expected), "{expected} missing from {query}");
    }
    assert_eq!(
        common::header(url_request, "content-type"),
        Some("application/json")
    );
    assert_eq!(
        common::json(url_request)["url"],
        "https://example.com/call.mp3"
    );

    let bytes_request = &requests[1];
    assert_eq!(
        common::header(bytes_request, "content-type"),
        Some("audio/wav")
    );
    assert_eq!(bytes_request.body, b"RIFFdata");
}

#[tokio::test]
async fn ac_002_deepgram_failures_are_wirekit_errors() {
    let server = common::server().await;
    common::mount(
        &server,
        "GET",
        "/projects",
        Fixture::json(
            401,
            json!({ "err_code": "INVALID_AUTH", "err_msg": "Invalid credentials" }),
        ),
    )
    .await;
    let error = client(&server)
        .list_projects()
        .await
        .expect_err("a 401 is an error");
    assert!(matches!(error, WirekitError::Auth { .. }), "got {error:?}");

    let server = common::server().await;
    common::mount(
        &server,
        "POST",
        "/listen",
        Fixture::json(
            400,
            json!({ "err_code": "FAILED_TO_PARSE", "err_msg": "Unsupported audio format" }),
        ),
    )
    .await;
    let error = client(&server)
        .transcribe(&TranscriptionRequest::from_url(
            "https://example.com/call.xyz",
        ))
        .await
        .expect_err("a 400 is an error");
    assert!(
        matches!(error, WirekitError::Api { status: 400, .. }),
        "got {error:?}"
    );
    assert_eq!(error.api_code(), Some("FAILED_TO_PARSE"));
}

#[tokio::test]
async fn ac_003_deepgram_rejects_an_error_body_at_http_200() {
    let server = common::server().await;
    common::mount(
        &server,
        "GET",
        "/projects",
        Fixture::json(
            200,
            json!({ "err_code": "PROJECT_ACCESS", "err_msg": "Project is suspended" }),
        ),
    )
    .await;
    let error = client(&server)
        .list_projects()
        .await
        .expect_err("a 200 with an error envelope is not a success");
    assert!(
        matches!(error, WirekitError::Api { status: 200, .. }),
        "got {error:?}"
    );
    assert_eq!(error.api_code(), Some("PROJECT_ACCESS"));
}

#[tokio::test]
async fn ac_004_deepgram_honours_retry_after_and_never_retries_a_400() {
    let server = common::server().await;
    let fixture = common::mount(
        &server,
        "POST",
        "/listen",
        Fixture::json(200, transcription_json())
            .first_status(429)
            .header("Retry-After", "1"),
    )
    .await;

    let transport = common::builder(&server)
        .retry(common::one_retry())
        .build()
        .expect("the transport builds");
    let client = DeepgramClient::new(transport, config()).expect("the client builds");

    let started = Instant::now();
    client
        .transcribe(&TranscriptionRequest::from_url(
            "https://example.com/call.mp3",
        ))
        .await
        .expect("the retry succeeds");
    assert_eq!(fixture.hits(), 2);
    assert!(
        started.elapsed() >= Duration::from_millis(900),
        "Retry-After: 1 must delay the retry, elapsed {:?}",
        started.elapsed()
    );

    let server = common::server().await;
    let fixture = common::mount(
        &server,
        "POST",
        "/listen",
        Fixture::json(400, json!({ "err_code": "BAD", "err_msg": "Bad audio" })),
    )
    .await;
    let transport = common::builder(&server)
        .retry(common::one_retry())
        .build()
        .expect("the transport builds");
    let client = DeepgramClient::new(transport, config()).expect("the client builds");
    let _ = client
        .transcribe(&TranscriptionRequest::from_url(
            "https://example.com/call.xyz",
        ))
        .await;
    assert_eq!(fixture.hits(), 1, "a 4xx other than 429 is not retried");
}

#[tokio::test]
async fn ac_004_deepgram_emits_the_shared_tracing_events() {
    let (capture, _guard) = common::capture();
    let server = common::server().await;
    common::mount(
        &server,
        "GET",
        "/projects",
        Fixture::json(200, json!({ "projects": [] })),
    )
    .await;

    client(&server)
        .list_projects()
        .await
        .expect("the call succeeds");

    let messages = capture.messages();
    assert!(
        messages.iter().any(|message| message == "request started"),
        "the start is traced: {messages:?}"
    );
    assert!(
        messages.iter().any(|message| message == "request finished"),
        "the finish is traced: {messages:?}"
    );
}

#[tokio::test]
async fn ac_005_deepgram_secret_is_redacted() {
    let config = DeepgramConfig::new(Secret::new(String::from(common::API_KEY)));
    let rendered = format!("{config:?}");
    assert!(rendered.contains("***"));
    assert!(!rendered.contains(common::API_KEY));
}

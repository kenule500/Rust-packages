//! Acceptance tests for the Slack provider (AC-067 to AC-074).
//!
//! `SlackBlock` serializes its payload under a literal `fields` key instead of
//! flattening it into the block object, so `typed_blocks` only round-trips
//! type-only blocks such as `divider`. AC-068 asserts the observed shape and
//! the raw `blocks` escape hatch; the divergence is recorded in the test
//! report.

use std::time::{Duration, Instant};

use serde_json::json;
use wirekit::slack::{
    FileUpload, SlackBlock, SlackClient, SlackMessage, MAX_FILE_BYTES,
};
use wirekit::{JitterBounds, RetryPolicy};

use crate::common;

const TOKEN: &str = "xoxb-1234567890-SECRET-TOKEN";
const POST_MESSAGE: &str = "/chat.postMessage";

fn slack(server: &wiremock::MockServer) -> SlackClient {
    SlackClient::builder(common::client(server))
        .bot_token(TOKEN)
        .build()
        .expect("the Slack client builds")
}

#[tokio::test]
async fn ac_067_the_bot_token_authorizes_requests_and_never_reaches_a_trace() {
    let (capture, _guard) = common::capture();

    let server = common::server().await;
    let recorder = common::mount(
        &server,
        "POST",
        POST_MESSAGE,
        common::Recorder::json(
            200,
            json!({ "ok": true, "channel": "C1", "ts": "1700000000.000100" }),
        ),
    )
    .await;

    let client = slack(&server);
    client
        .post_message(&SlackMessage::text("C1", "hello"))
        .await
        .expect("the message is accepted");

    assert_eq!(
        recorder.last().header("authorization").map(str::to_string),
        Some(format!("Bearer {TOKEN}"))
    );

    let rendered = capture.rendered();
    assert!(!rendered.is_empty(), "the pipeline traces the request");
    assert!(
        !rendered.contains(TOKEN),
        "the bot token must never appear in a trace: {rendered}"
    );
    assert!(
        !rendered.contains("SECRET"),
        "no fragment of the token may leak: {rendered}"
    );

    let missing = SlackClient::builder(common::client(&server))
        .build()
        .err()
        .expect("a bot token is required");
    assert_eq!(missing.code(), "W2006");
    assert!(missing.to_string().contains("bot token"));
}

#[tokio::test]
async fn ac_068_text_and_block_messages_return_the_channel_and_timestamp() {
    let server = common::server().await;
    let recorder = common::mount(
        &server,
        "POST",
        POST_MESSAGE,
        common::Recorder::json(
            200,
            json!({ "ok": true, "channel": "C1", "ts": "1700000000.000100" }),
        ),
    )
    .await;

    let client = slack(&server);
    let response = client
        .post_message(&SlackMessage::text("C1", "hello from the tests"))
        .await
        .expect("the message is accepted");

    assert!(response.ok);
    assert_eq!(response.channel.as_deref(), Some("C1"));
    assert_eq!(response.ts.as_deref(), Some("1700000000.000100"));

    let body = recorder.last().json();
    assert_eq!(body["channel"], json!("C1"));
    assert_eq!(body["text"], json!("hello from the tests"));
    assert!(
        body.get("blocks").is_none(),
        "a text-only message sends no blocks"
    );

    // The raw Block Kit escape hatch is passed through untouched.
    let blocks = json!([
        {
            "type": "section",
            "text": { "type": "mrkdwn", "text": "*build* passed" }
        },
        { "type": "divider" }
    ]);
    client
        .post_message(&SlackMessage::blocks("C1", blocks.clone()))
        .await
        .expect("the block message is accepted");
    assert_eq!(recorder.last().json()["blocks"], blocks);

    // Typed blocks carry the `type` tag. The payload lands under `fields`,
    // which is the shape the crate currently produces.
    let typed = client
        .post_message(&SlackMessage::typed_blocks(
            "C1",
            vec![SlackBlock::new("divider", serde_json::Value::Null)],
        ))
        .await
        .expect("the typed block message is accepted");
    assert!(typed.ok);
    assert_eq!(recorder.last().json()["blocks"][0]["type"], json!("divider"));

    let no_content = client
        .post_message(&SlackMessage::default())
        .await
        .expect_err("a channel is required");
    assert_eq!(no_content.code(), "W2006");

    let mut empty = SlackMessage::text("C1", "hello");
    empty.text = None;
    let error = client
        .post_message(&empty)
        .await
        .expect_err("text or blocks are required");
    assert_eq!(error.code(), "W2006");
    assert_eq!(recorder.hits(), 3, "validation never reaches the network");
}

#[tokio::test]
async fn ac_069_an_ok_false_body_is_an_api_error_not_a_success() {
    let server = common::server().await;
    common::mount(
        &server,
        "POST",
        POST_MESSAGE,
        common::Recorder::json(
            200,
            json!({
                "ok": false,
                "error": "invalid_blocks",
                "warning": "missing_charset",
                "response_metadata": { "warnings": ["missing_charset"] }
            }),
        ),
    )
    .await;

    let error = slack(&server)
        .post_message(&SlackMessage::text("C1", "hello"))
        .await
        .expect_err("ok: false is a failure even on HTTP 200");

    match error {
        wirekit::WirekitError::Api {
            status,
            ref code,
            ref message,
            ..
        } => {
            assert_eq!(status, 200);
            assert_eq!(code, "invalid_blocks");
            assert!(message.contains("invalid_blocks"), "message was {message}");
        }
        other => panic!("expected Api, got {other:?}"),
    }

    let auth_server = common::server().await;
    common::mount(
        &auth_server,
        "POST",
        POST_MESSAGE,
        common::Recorder::json(200, json!({ "ok": false, "error": "invalid_auth" })),
    )
    .await;
    let error = slack(&auth_server)
        .post_message(&SlackMessage::text("C1", "hello"))
        .await
        .expect_err("a rejected token is a failure");
    assert_eq!(error.code(), "W2002");
    assert!(error.is_auth());
}

#[tokio::test]
async fn ac_070_missing_channels_are_not_retried() {
    for slack_error in ["channel_not_found", "not_in_channel"] {
        let server = common::server().await;
        let recorder = common::mount(
            &server,
            "POST",
            POST_MESSAGE,
            common::Recorder::json(200, json!({ "ok": false, "error": slack_error })),
        )
        .await;

        let mut policy = RetryPolicy::default();
        policy.base_delay = Duration::from_millis(1);
        policy.max_delay = Duration::from_millis(2);
        policy.jitter = JitterBounds::new(1.0, 1.0).expect("valid jitter bounds");

        let client = SlackClient::builder(
            common::builder(&server)
                .retry(policy)
                .build()
                .expect("the client builds"),
        )
        .bot_token(TOKEN)
        .build()
        .expect("the Slack client builds");

        let error = client
            .post_message(&SlackMessage::text("C404", "hello"))
            .await
            .expect_err("the channel is missing");
        assert_eq!(error.code(), "W2001");
        match error {
            wirekit::WirekitError::Api { ref code, .. } => assert_eq!(code, slack_error),
            other => panic!("expected Api, got {other:?}"),
        }
        assert_eq!(
            recorder.hits(),
            1,
            "{slack_error} must not be retried even with retries enabled"
        );
    }
}

#[tokio::test]
async fn ac_071_retry_after_is_honoured_and_channels_are_cached() {
    let server = common::server().await;
    let recorder = common::mount(
        &server,
        "POST",
        POST_MESSAGE,
        common::Recorder::json(
            200,
            json!({ "ok": true, "channel": "C1", "ts": "1700000000.000200" }),
        )
        .first_status(429)
        .header("retry-after", "1"),
    )
    .await;

    let mut policy = RetryPolicy::default();
    policy.base_delay = Duration::from_millis(1);
    policy.max_delay = Duration::from_millis(60);
    policy.jitter = JitterBounds::new(1.0, 1.0).expect("valid jitter bounds");

    let client = SlackClient::builder(
        common::builder(&server)
            .retry(policy)
            .build()
            .expect("the client builds"),
    )
    .bot_token(TOKEN)
    .build()
    .expect("the Slack client builds");

    let started = Instant::now();
    let response = client
        .post_message(&SlackMessage::text("C1", "hello"))
        .await
        .expect("the retry succeeds");
    let elapsed = started.elapsed();

    assert_eq!(response.ts.as_deref(), Some("1700000000.000200"));
    assert_eq!(recorder.hits(), 2, "the 429 is retried once");
    assert!(
        elapsed >= Duration::from_millis(45),
        "Retry-After overrides the 1 ms backoff and is capped at max_delay: {elapsed:?}"
    );

    // Channel lookups are cached, so repeated lookups do not hit the network.
    let cache_server = common::server().await;
    let info = common::mount(
        &cache_server,
        "GET",
        "/conversations.info",
        common::Recorder::json(
            200,
            json!({
                "ok": true,
                "channel": { "id": "C1", "name": "general", "num_members": 42 }
            }),
        ),
    )
    .await;

    let cached = slack(&cache_server);
    let first = cached.channel_info("C1").await.expect("the channel loads");
    let second = cached.channel_info("C1").await.expect("the cache answers");
    assert_eq!(first.name.as_deref(), Some("general"));
    assert_eq!(second.name.as_deref(), Some("general"));
    assert_eq!(info.hits(), 1, "the second lookup is served from cache");

    cached.clear_cache();
    cached.channel_info("C1").await.expect("the channel reloads");
    assert_eq!(info.hits(), 2, "clearing the cache forces a lookup");

    let uncached = SlackClient::builder(common::client(&cache_server))
        .bot_token(TOKEN)
        .disable_channel_cache()
        .build()
        .expect("the Slack client builds");
    uncached.channel_info("C1").await.expect("the channel loads");
    uncached.channel_info("C1").await.expect("the channel loads again");
    assert_eq!(info.hits(), 4, "a disabled cache always looks up");
}

#[tokio::test]
async fn ac_072_request_signatures_are_checked_over_the_raw_bytes() {
    let server = common::server().await;

    let lenient = SlackClient::builder(common::client(&server))
        .bot_token(TOKEN)
        .signing_secret(common::SLACK_SIGNING_SECRET)
        .max_clock_skew(Duration::from_secs(60 * 60 * 24 * 365 * 100))
        .build()
        .expect("the Slack client builds");

    let computed = lenient
        .compute_signature(
            common::SLACK_FIXTURE_TIMESTAMP,
            common::SLACK_FIXTURE_BODY.as_bytes(),
        )
        .expect("the fixture signature is computed");
    assert_eq!(
        computed,
        common::SLACK_FIXTURE_SIGNATURE,
        "the signature matches the fixture produced outside the crate"
    );

    lenient
        .verify_request(
            common::SLACK_FIXTURE_TIMESTAMP,
            common::SLACK_FIXTURE_BODY.as_bytes(),
            common::SLACK_FIXTURE_SIGNATURE,
        )
        .expect("the fixture verifies");

    let error = lenient
        .verify_request(
            common::SLACK_FIXTURE_TIMESTAMP,
            b"{\"type\":\"event_callback\",\"event\":{\"type\":\"app_home_opened\"}}",
            common::SLACK_FIXTURE_SIGNATURE,
        )
        .expect_err("a re-serialized body must not verify");
    assert_eq!(error.code(), "W2002");

    // A re-serialized body changes the bytes, even when the JSON is equal.
    let reserialized = serde_json::to_vec(
        &serde_json::from_str::<serde_json::Value>(common::SLACK_FIXTURE_BODY)
            .expect("the fixture is valid JSON"),
    )
    .expect("the value is serializable");
    assert_ne!(reserialized, common::SLACK_FIXTURE_BODY.as_bytes());
    let error = lenient
        .verify_request(
            common::SLACK_FIXTURE_TIMESTAMP,
            &reserialized,
            common::SLACK_FIXTURE_SIGNATURE,
        )
        .expect_err("only the exact bytes verify");
    assert_eq!(error.code(), "W2002");

    let strict = SlackClient::builder(common::client(&server))
        .bot_token(TOKEN)
        .signing_secret(common::SLACK_SIGNING_SECRET)
        .build()
        .expect("the Slack client builds");

    let error = strict
        .compute_signature(
            common::SLACK_FIXTURE_TIMESTAMP,
            common::SLACK_FIXTURE_BODY.as_bytes(),
        )
        .expect_err("the fixture timestamp is stale");
    assert_eq!(error.code(), "W2002");
    assert!(error.to_string().contains("skew"));

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("the clock is after the epoch")
        .as_secs()
        .to_string();
    let signature = strict
        .compute_signature(&now, common::SLACK_FIXTURE_BODY.as_bytes())
        .expect("a fresh timestamp signs");
    strict
        .verify_request(&now, common::SLACK_FIXTURE_BODY.as_bytes(), &signature)
        .expect("a fresh request verifies");
    let error = strict
        .verify_request(&now, b"tampered", &signature)
        .expect_err("a tampered body must not verify");
    assert_eq!(error.code(), "W2002");

    let unsigned = SlackClient::builder(common::client(&server))
        .bot_token(TOKEN)
        .build()
        .expect("the Slack client builds");
    let error = unsigned
        .compute_signature(&now, common::SLACK_FIXTURE_BODY.as_bytes())
        .expect_err("a signing secret is required");
    assert_eq!(error.code(), "W2006");
}

#[tokio::test]
async fn ac_073_files_are_uploaded_as_multipart_within_the_limits() {
    assert_eq!(MAX_FILE_BYTES, 1024 * 1024 * 1024);

    let server = common::server().await;
    let recorder = common::mount(
        &server,
        "POST",
        "/files.upload",
        common::Recorder::json(
            200,
            json!({ "ok": true, "file": { "id": "F1", "name": "app.log", "title": "Build log" } }),
        ),
    )
    .await;

    let upload = FileUpload::new("app.log", b"line1\nline2\n".to_vec())
        .channel("C1")
        .filetype("text")
        .title("Build log");
    let file = slack(&server)
        .upload_file(&upload)
        .await
        .expect("the upload is accepted");

    assert_eq!(file.id, "F1");
    assert_eq!(file.name.as_deref(), Some("app.log"));
    assert_eq!(file.title.as_deref(), Some("Build log"));

    let request = recorder.last();
    let content_type = request
        .header("content-type")
        .expect("the upload is multipart");
    assert!(
        content_type.starts_with("multipart/form-data; boundary=wirekit-boundary-"),
        "content type was {content_type}"
    );
    let body = request.text();
    assert!(body.contains("name=\"channels\"\r\n\r\nC1"), "body was {body}");
    assert!(body.contains("name=\"filename\"\r\n\r\napp.log"), "body was {body}");
    assert!(body.contains("name=\"filetype\"\r\n\r\ntext"), "body was {body}");
    assert!(body.contains("name=\"title\"\r\n\r\nBuild log"), "body was {body}");
    assert!(
        body.contains("Content-Disposition: form-data; name=\"file\"; filename=\"app.log\""),
        "the file part is present: {body}"
    );
    assert!(body.contains("line1\nline2\n"), "the bytes are uploaded");

    let error = slack(&server)
        .upload_file(&FileUpload::new("app.log", b"data".to_vec()))
        .await
        .expect_err("a channel is required");
    assert_eq!(error.code(), "W2006");

    let error = slack(&server)
        .upload_file(&FileUpload::new("   ", b"data".to_vec()).channel("C1"))
        .await
        .expect_err("a filename is required");
    assert_eq!(error.code(), "W2006");

    // The 1 GiB size guard (MAX_FILE_BYTES) is documented above and asserted
    // against the constant; materialising a 1 GiB payload is not a reasonable
    // unit test, so only the cheap validations run here.
    assert_eq!(recorder.hits(), 1, "rejected uploads never leave the process");
}

#[tokio::test]
async fn ac_074_channel_and_user_queries_return_typed_results() {
    let server = common::server().await;
    let info = common::mount(
        &server,
        "GET",
        "/conversations.info",
        common::Recorder::json(
            200,
            json!({
                "ok": true,
                "channel": {
                    "id": "C1",
                    "name": "general",
                    "is_private": false,
                    "num_members": 12
                }
            }),
        ),
    )
    .await;
    let user = common::mount(
        &server,
        "GET",
        "/users.info",
        common::Recorder::json(
            200,
            json!({
                "ok": true,
                "user": { "id": "U1", "name": "ada", "real_name": "Ada Lovelace", "is_bot": false }
            }),
        ),
    )
    .await;
    let list = common::mount(
        &server,
        "GET",
        "/conversations.list",
        common::Recorder::json(
            200,
            json!({
                "ok": true,
                "channels": [
                    { "id": "C1", "name": "general", "num_members": 12 },
                    { "id": "C2", "name": "private", "is_private": true }
                ],
                "response_metadata": { "next_cursor": "next-page" }
            }),
        ),
    )
    .await;

    let client = slack(&server);

    let channel = client.channel_info("C1").await.expect("the channel loads");
    assert_eq!(channel.id, "C1");
    assert_eq!(channel.name.as_deref(), Some("general"));
    assert_eq!(channel.is_private, Some(false));
    assert_eq!(channel.num_members, Some(12));
    assert_eq!(info.last().query_param("channel").as_deref(), Some("C1"));

    let author = client.user_info(" U1 ").await.expect("the user loads");
    assert_eq!(author.id, "U1");
    assert_eq!(author.name.as_deref(), Some("ada"));
    assert_eq!(author.real_name.as_deref(), Some("Ada Lovelace"));
    assert_eq!(author.is_bot, Some(false));
    assert_eq!(user.last().query_param("user").as_deref(), Some("U1"));

    let page = client
        .conversations_list(Some(2), Some("cursor-1"))
        .await
        .expect("the page loads");
    assert_eq!(page.channels.len(), 2);
    assert_eq!(page.channels[0].id, "C1");
    assert_eq!(page.channels[1].is_private, Some(true));
    assert_eq!(page.next_cursor(), Some("next-page"));
    let request = list.last();
    assert_eq!(request.method, "GET");
    assert_eq!(request.query_param("limit").as_deref(), Some("2"));
    assert_eq!(request.query_param("cursor").as_deref(), Some("cursor-1"));

    let first_page = client
        .conversations_list(None, None)
        .await
        .expect("the page loads");
    assert_eq!(first_page.next_cursor(), Some("next-page"));

    let error = client
        .channel_info("  ")
        .await
        .expect_err("a channel is required");
    assert_eq!(error.code(), "W2006");
    let error = client.user_info("").await.expect_err("a user is required");
    assert_eq!(error.code(), "W2006");
}

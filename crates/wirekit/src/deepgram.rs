//! Deepgram — batch speech-to-text: transcribe an audio URL or audio bytes,
//! with optional diarization, keyword boosting, and project listing.
//!
//! # Scope
//!
//! Built from `catalogue/24_deepgram.md`: submit a pre-recorded file by URL or
//! bytes, choose a model and language, turn on diarization to receive
//! speaker-separated utterances, boost keywords, and list the projects the
//! token can see. Auth, the token bucket, the retry policy with `Retry-After`,
//! tracing, and the response size limit are the shared ones.
//!
//! # Non-goals
//!
//! - **No streaming session.** The catalogue lists live streaming; an HTTP
//!   request client cannot hold a WebSocket open, and a reconnect strategy is a
//!   different transport with its own lifecycle. The caller opens that session
//!   directly. Batch transcription, which the task defines, is here.
//! - **No callback receiver.** A `callback` URL is a Deepgram-side delivery to
//!   the caller's HTTP surface; this crate makes outbound requests only and
//!   stores no job ledger.
//! - **No media storage or redaction.** The caller owns retention and decides
//!   what may be uploaded; the module sends the bytes it is given.
//! - **No project administration.** Listing is read-only; creating projects,
//!   rotating keys, and usage billing belong to the console.
//!
//! # Credentials
//!
//! The API key is a bearer token. It is held in a [`Secret`], renders as `***`,
//! and is never placed in a URL, so it cannot leak through a traced query
//! string.
//!
//! ```no_run
//! use wirekit::deepgram::{DeepgramClient, DeepgramConfig, TranscriptionRequest};
//! use wirekit::{Secret, WirekitClient};
//!
//! # async fn run() -> Result<(), wirekit::WirekitError> {
//! let transport = WirekitClient::new(wirekit::deepgram::BASE_URL)?;
//! let client = DeepgramClient::new(
//!     transport,
//!     DeepgramConfig::new(Secret::new(String::from("<api-key>"))),
//! )?;
//!
//! let request = TranscriptionRequest::from_url("https://example.com/call.mp3")
//!     .with_model("nova-3")
//!     .with_diarization();
//! let transcript = client.transcribe(&request).await?;
//! println!("{}", transcript.transcript);
//! # Ok(())
//! # }
//! ```

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::auth::{AuthScheme, BearerAuth};
use crate::client::WirekitClient;
use crate::error::WirekitError;
use crate::pipeline::{RequestSpec, Response};
use crate::secret::Secret;

/// The Deepgram API base URL.
pub const BASE_URL: &str = "https://api.deepgram.com/v1";

/// How the client is configured.
#[derive(Debug, Clone)]
pub struct DeepgramConfig {
    /// The API key, sent as `Authorization: Bearer <key>`.
    pub api_key: Secret<String>,
}

impl DeepgramConfig {
    /// Build a configuration with the API key.
    #[must_use]
    pub fn new(api_key: Secret<String>) -> Self {
        Self { api_key }
    }
}

/// Where the audio to transcribe lives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioSource {
    /// A URL Deepgram fetches itself.
    Url(String),
    /// Audio bytes the caller already holds.
    Bytes {
        /// The audio media type, for example `audio/wav`.
        content_type: String,
        /// The audio itself.
        data: Vec<u8>,
    },
}

/// A batch transcription request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptionRequest {
    /// The audio to transcribe.
    pub source: AudioSource,
    /// The model to use. `None` uses the account default.
    pub model: Option<String>,
    /// The expected language. `None` lets the provider detect it.
    pub language: Option<String>,
    /// Whether to label speakers.
    pub diarize: bool,
    /// Whether to add punctuation and casing.
    pub punctuate: bool,
    /// Whether to format numbers, dates, and currency.
    pub smart_format: bool,
    /// Terms to boost, one per entry. A value may carry `:boost`, for example
    /// `shingle:2`.
    pub keywords: Vec<String>,
}

impl TranscriptionRequest {
    /// Transcribe audio at a URL the provider fetches.
    #[must_use]
    pub fn from_url(url: impl Into<String>) -> Self {
        Self {
            source: AudioSource::Url(url.into()),
            model: None,
            language: None,
            diarize: false,
            punctuate: false,
            smart_format: false,
            keywords: Vec::new(),
        }
    }

    /// Transcribe audio bytes the caller already holds.
    ///
    /// The content type is required: Declaring the wrong type is the usual
    /// cause of a provider 400. The header is sent as `Content-Type`.
    #[must_use]
    pub fn from_bytes(content_type: impl Into<String>, data: Vec<u8>) -> Self {
        Self {
            source: AudioSource::Bytes {
                content_type: content_type.into(),
                data,
            },
            model: None,
            language: None,
            diarize: false,
            punctuate: false,
            smart_format: false,
            keywords: Vec::new(),
        }
    }

    /// Set the model.
    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Set the expected language.
    #[must_use]
    pub fn with_language(mut self, language: impl Into<String>) -> Self {
        self.language = Some(language.into());
        self
    }

    /// Label speakers and receive speaker-separated utterances.
    #[must_use]
    pub fn with_diarization(mut self) -> Self {
        self.diarize = true;
        self
    }

    /// Add punctuation and casing.
    #[must_use]
    pub fn with_punctuation(mut self) -> Self {
        self.punctuate = true;
        self
    }

    /// Format numbers, dates, and currency.
    #[must_use]
    pub fn with_smart_format(mut self) -> Self {
        self.smart_format = true;
        self
    }

    /// Boost one or more terms.
    #[must_use]
    pub fn with_keywords(mut self, keywords: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.keywords.extend(keywords.into_iter().map(Into::into));
        self
    }
}

/// One recognised word and its timing.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Word {
    /// The word as the model heard it.
    #[serde(default)]
    pub word: String,
    /// The punctuated form, when punctuation was requested.
    #[serde(default)]
    pub punctuated_word: Option<String>,
    /// Start offset in seconds.
    #[serde(default)]
    pub start: f64,
    /// End offset in seconds.
    #[serde(default)]
    pub end: f64,
    /// The model's confidence, between 0 and 1.
    #[serde(default)]
    pub confidence: f64,
    /// The speaker index, present when diarization was requested.
    #[serde(default)]
    pub speaker: Option<u32>,
}

/// A speaker-separated segment.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Utterance {
    /// The speaker index.
    pub speaker: Option<u32>,
    /// The segment's transcript.
    pub transcript: String,
    /// Start offset in seconds.
    pub start: f64,
    /// End offset in seconds.
    pub end: f64,
    /// The provider's confidence for the segment.
    pub confidence: Option<f64>,
    /// The words in the segment.
    pub words: Vec<Word>,
}

/// A completed batch transcription.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Transcription {
    /// The provider's request id, useful when reporting a problem.
    pub request_id: Option<String>,
    /// The best alternative's transcript.
    pub transcript: String,
    /// The best alternative's confidence, between 0 and 1.
    pub confidence: f64,
    /// The recognised words.
    pub words: Vec<Word>,
    /// Speaker-separated segments, present when diarization was requested.
    pub utterances: Vec<Utterance>,
    /// The audio duration in seconds, as the provider reports it.
    pub duration_seconds: Option<f64>,
}

/// A Deepgram project.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct Project {
    /// The provider's identifier.
    #[serde(default)]
    pub project_id: String,
    /// The project's name.
    #[serde(default)]
    pub name: Option<String>,
    /// The company the project belongs to.
    #[serde(default)]
    pub company: Option<String>,
}

struct DeepgramInner {
    client: WirekitClient,
    config: DeepgramConfig,
    auth: Arc<dyn AuthScheme>,
}

/// The Deepgram client. Clone it freely; the transport is shared.
#[derive(Clone)]
pub struct DeepgramClient {
    inner: Arc<DeepgramInner>,
}

impl DeepgramClient {
    /// Build a client over a shared [`WirekitClient`].
    ///
    /// Build the underlying client against [`BASE_URL`]. An empty API key is
    /// [`WirekitError::InvalidConfig`].
    pub fn new(client: WirekitClient, config: DeepgramConfig) -> Result<Self, WirekitError> {
        if config.api_key.expose().trim().is_empty() {
            return Err(WirekitError::invalid_config(
                "a Deepgram API key is required",
            ));
        }

        let auth: Arc<dyn AuthScheme> = Arc::new(BearerAuth::new(config.api_key.expose().clone()));
        Ok(Self {
            inner: Arc::new(DeepgramInner {
                client,
                config,
                auth,
            }),
        })
    }

    /// The client configuration.
    #[must_use]
    pub fn config(&self) -> &DeepgramConfig {
        &self.inner.config
    }

    /// The shared transport.
    #[must_use]
    pub fn client(&self) -> &WirekitClient {
        &self.inner.client
    }

    /// Transcribe a pre-recorded file.
    ///
    /// A 400 is almost always an unsupported or mislabelled audio format, so it
    /// is reported rather than retried. The request is idempotent in the
    /// provider's billing model only when the caller deduplicates its own media;
    /// this module sends no idempotency key because the endpoint has none.
    pub async fn transcribe(
        &self,
        request: &TranscriptionRequest,
    ) -> Result<Transcription, WirekitError> {
        let mut url = self.inner.client.url("listen")?;
        {
            let mut pairs = url.query_pairs_mut();
            if let Some(model) = request.model.as_deref() {
                if model.trim().is_empty() {
                    return Err(WirekitError::invalid_config(
                        "a Deepgram model must not be blank",
                    ));
                }
                pairs.append_pair("model", model.trim());
            }
            if let Some(language) = request.language.as_deref() {
                if language.trim().is_empty() {
                    return Err(WirekitError::invalid_config(
                        "a Deepgram language must not be blank",
                    ));
                }
                pairs.append_pair("language", language.trim());
            }
            if request.diarize {
                pairs.append_pair("diarize", "true");
                pairs.append_pair("utterances", "true");
            }
            if request.punctuate {
                pairs.append_pair("punctuate", "true");
            }
            if request.smart_format {
                pairs.append_pair("smart_format", "true");
            }
            for keyword in &request.keywords {
                if keyword.trim().is_empty() {
                    return Err(WirekitError::invalid_config(
                        "a Deepgram keyword must not be blank",
                    ));
                }
                pairs.append_pair("keywords", keyword.trim());
            }
        }

        let mut spec = RequestSpec::post(url)
            .auth(self.inner.auth.clone())
            .rate_limit_key("deepgram")
            .body_error_hook(reject_error_envelope);

        spec = match &request.source {
            AudioSource::Url(source) => {
                if source.trim().is_empty() {
                    return Err(WirekitError::invalid_config(
                        "a Deepgram transcription needs an audio URL",
                    ));
                }
                spec.try_header("content-type", "application/json")?
                    .json_body(&WireAudioUrl { url: source.trim() })?
            }
            AudioSource::Bytes { content_type, data } => {
                if content_type.trim().is_empty() {
                    return Err(WirekitError::invalid_config(
                        "transcribing audio bytes needs a content type",
                    ));
                }
                if data.is_empty() {
                    return Err(WirekitError::invalid_config(
                        "the audio to transcribe is empty",
                    ));
                }
                spec.try_header("content-type", content_type.trim())?
                    .bytes_body(data.clone())
            }
        };

        let response: Response<WireResponse> = self
            .inner
            .client
            .execute(spec)
            .await
            .map_err(map_deepgram_error)?;

        Ok(response.value.into_transcription())
    }

    /// List the projects the token can see.
    pub async fn list_projects(&self) -> Result<Vec<Project>, WirekitError> {
        let url = self.inner.client.url("projects")?;
        let spec = RequestSpec::get(url)
            .auth(self.inner.auth.clone())
            .rate_limit_key("deepgram")
            .body_error_hook(reject_error_envelope);
        let response: Response<WireProjectEnvelope> = self
            .inner
            .client
            .execute(spec)
            .await
            .map_err(map_deepgram_error)?;

        Ok(response.value.projects)
    }
}

/// Reject a 2xx response whose body reports a failure (CON-213).
fn reject_error_envelope(status: u16, body: &[u8]) -> Option<WirekitError> {
    let value: serde_json::Value = serde_json::from_slice(body).ok()?;
    let code = value
        .get("err_code")
        .and_then(serde_json::Value::as_str)
        .or_else(|| value.get("category").and_then(serde_json::Value::as_str))?;
    let message = value
        .get("err_msg")
        .and_then(serde_json::Value::as_str)
        .or_else(|| value.get("message").and_then(serde_json::Value::as_str))
        .unwrap_or("Deepgram reported a failure");

    Some(WirekitError::api_with_details(
        status,
        code,
        message,
        value.clone(),
    ))
}

/// Map a Deepgram rejection onto the shared error type.
///
/// Deepgram carries the reason in `err_msg` (with `err_code`) or in `message`
/// (with `category`). Both are lifted out so the caller sees the provider's
/// text. A 401 or 403 becomes an authentication failure.
fn map_deepgram_error(error: WirekitError) -> WirekitError {
    let WirekitError::Api {
        status,
        details: Some(details),
        ..
    } = &error
    else {
        return error;
    };

    if *status == 401 || *status == 403 {
        return WirekitError::auth("Deepgram rejected the API key");
    }

    let code = details
        .get("err_code")
        .and_then(serde_json::Value::as_str)
        .or_else(|| details.get("category").and_then(serde_json::Value::as_str));
    let message = details
        .get("err_msg")
        .and_then(serde_json::Value::as_str)
        .or_else(|| details.get("message").and_then(serde_json::Value::as_str));

    match (code, message) {
        (Some(code), Some(message)) => {
            WirekitError::api_with_details(*status, code, message, details.clone())
        }
        (None, Some(message)) => {
            WirekitError::api_with_details(*status, "deepgram_error", message, details.clone())
        }
        (Some(code), None) => WirekitError::api_with_details(
            *status,
            code,
            "Deepgram reported a failure",
            details.clone(),
        ),
        _ => error,
    }
}

#[derive(Debug, Serialize)]
struct WireAudioUrl<'a> {
    url: &'a str,
}

#[derive(Debug, Default, Deserialize)]
struct WireResponse {
    #[serde(default)]
    metadata: WireMetadata,
    #[serde(default)]
    results: WireResults,
}

impl WireResponse {
    fn into_transcription(self) -> Transcription {
        let metadata = self.metadata;
        let results = self.results;
        let channel = results.channels.into_iter().next();
        let alternative = channel
            .as_ref()
            .and_then(|channel| channel.alternatives.first())
            .cloned()
            .unwrap_or_default();

        Transcription {
            request_id: metadata.request_id,
            transcript: alternative.transcript,
            confidence: alternative.confidence,
            words: alternative.words,
            utterances: results
                .utterances
                .into_iter()
                .map(WireUtterance::into_utterance)
                .collect(),
            duration_seconds: metadata.duration,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct WireMetadata {
    #[serde(default)]
    request_id: Option<String>,
    #[serde(default)]
    duration: Option<f64>,
}

#[derive(Debug, Default, Deserialize)]
struct WireResults {
    #[serde(default)]
    channels: Vec<WireChannel>,
    #[serde(default)]
    utterances: Vec<WireUtterance>,
}

#[derive(Debug, Default, Deserialize)]
struct WireChannel {
    #[serde(default)]
    alternatives: Vec<WireAlternative>,
}

#[derive(Debug, Default, Clone, Deserialize)]
struct WireAlternative {
    #[serde(default)]
    transcript: String,
    #[serde(default)]
    confidence: f64,
    #[serde(default)]
    words: Vec<Word>,
}

#[derive(Debug, Default, Deserialize)]
struct WireUtterance {
    #[serde(default)]
    speaker: Option<u32>,
    #[serde(default)]
    transcript: String,
    #[serde(default)]
    start: f64,
    #[serde(default)]
    end: f64,
    #[serde(default)]
    confidence: Option<f64>,
    #[serde(default)]
    words: Vec<Word>,
}

impl WireUtterance {
    fn into_utterance(self) -> Utterance {
        Utterance {
            speaker: self.speaker,
            transcript: self.transcript,
            start: self.start,
            end: self.end,
            confidence: self.confidence,
            words: self.words,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct WireProjectEnvelope {
    #[serde(default)]
    projects: Vec<Project>,
}

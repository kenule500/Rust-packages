//! Sage Intacct XML gateway client.
//!
//! Scope: build the documented request envelope, POST it to the gateway, derive
//! success from the gateway's own `status` and `errorno` rather than the HTTP
//! status, bound the response before parsing, and keep the sender password out of
//! every rendering.
//!
//! Non-goals: the full Intacct object schema, asynchronous responses, Platform
//! Services, and webhooks - the classic gateway has none, so reconciliation is the
//! caller's job (FR-018).
//!
//! The endpoint, envelope, and idempotence control follow the published Sage
//! Intacct Web Services contract: `POST` XML to
//! `https://api.intacct.com/ia/xml/xmlgw.phtml`. A retried request carries the
//! caller's key in `<uniqueid>`, which is what makes
//! [`create`](SageIntacctClient::create) safe to repeat instead of guessing
//! whether an earlier attempt landed.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

use quick_xml::events::Event;
use quick_xml::Reader;

use crate::pipeline::RequestSpec;
use crate::{Secret, WirekitClient, WirekitError};

/// The documented Sage Intacct XML gateway endpoint.
pub const GATEWAY_URL: &str = "https://api.intacct.com/ia/xml/xmlgw.phtml";

/// Default ceiling for a gateway response body, in bytes.
pub const DEFAULT_MAX_RESPONSE_BYTES: usize = 1024 * 1024;

/// Ceiling for element nesting in a gateway response, so a hostile document
/// cannot drive unbounded recursion or memory.
pub const DEFAULT_MAX_DEPTH: usize = 32;

/// Web Services credentials for one Intacct company.
///
/// `Debug` is hand-written so the sender password is redacted (CON-214). The type
/// deliberately does not implement `Serialize`.
#[derive(Clone)]
pub struct SageIntacctConfig {
    /// Web Services sender ID.
    pub sender_id: String,
    /// Web Services sender password. Never rendered.
    pub sender_password: Secret<String>,
    /// Company ID every request is scoped to.
    pub company_id: String,
    /// Web Services user ID authorised for that company.
    pub user_id: String,
}

impl fmt::Debug for SageIntacctConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SageIntacctConfig")
            .field("sender_id", &self.sender_id)
            .field("sender_password", &"***")
            .field("company_id", &self.company_id)
            .field("user_id", &self.user_id)
            .finish()
    }
}

/// What the gateway reported for one function call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntacctOutcome {
    /// The gateway's own `status`, lowercased: `success` or `failure`.
    pub status: String,
    /// The gateway error number, for example `BL01001986`.
    pub errorno: Option<String>,
    /// The gateway description, when it supplied one.
    pub description: Option<String>,
    /// The record key returned for a create.
    pub key: Option<String>,
    /// Leaf values from `<data>`, in document order.
    pub fields: Vec<(String, String)>,
}

impl IntacctOutcome {
    /// Whether the gateway reported success.
    ///
    /// This is the only correct success test: a gateway failure arrives with HTTP
    /// 200, so the HTTP status cannot answer it (CON-213).
    #[must_use]
    pub fn is_success(&self) -> bool {
        self.status == "success"
    }

    /// The first `<data>` value with this element name.
    #[must_use]
    pub fn field(&self, name: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
}

/// A client for one tenant's Intacct company.
///
/// Holds no cache and no shared state, so two clients for two companies cannot
/// observe each other (NFR-002).
pub struct SageIntacctClient {
    config: SageIntacctConfig,
    client: WirekitClient,
    max_response_bytes: usize,
    control: AtomicU64,
}

impl fmt::Debug for SageIntacctClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SageIntacctClient")
            .field("config", &self.config)
            .field("max_response_bytes", &self.max_response_bytes)
            .finish_non_exhaustive()
    }
}

impl SageIntacctClient {
    /// Build a client for one company, reusing the caller's pooled HTTP client.
    #[must_use]
    pub fn new(config: SageIntacctConfig, client: WirekitClient) -> Self {
        Self {
            config,
            client,
            max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
            control: AtomicU64::new(1),
        }
    }

    /// Override the response byte ceiling (FR-016, NFR-007).
    #[must_use]
    pub fn max_response_bytes(mut self, limit: usize) -> Self {
        self.max_response_bytes = limit;
        self
    }

    /// Create a record and return what the gateway reported.
    ///
    /// `fields` are emitted in order as child elements of `object`, so
    /// `[("CUSTOMERID", "C-1"), ("NAME", "Acme")]` becomes a `<create>` for that
    /// object. When `unique_id` is supplied it is also sent as `<uniqueid>`, which
    /// makes a replayed request idempotent instead of a duplicate record (FR-017).
    pub async fn create(
        &self,
        object: &str,
        fields: &[(&str, &str)],
        unique_id: Option<&str>,
    ) -> Result<IntacctOutcome, WirekitError> {
        check_identifier(object, "object")?;

        let mut body = String::new();
        body.push_str("<create>");
        body.push('<');
        body.push_str(object);
        body.push('>');
        for (name, value) in fields {
            check_identifier(name, "field")?;
            body.push('<');
            body.push_str(name);
            body.push('>');
            body.push_str(&escape_xml(value));
            body.push_str("</");
            body.push_str(name);
            body.push('>');
        }
        body.push_str("</");
        body.push_str(object);
        body.push_str("></create>");

        self.call(&body, unique_id).await
    }

    /// Read one record by its key.
    pub async fn read(&self, object: &str, key: &str) -> Result<IntacctOutcome, WirekitError> {
        check_identifier(object, "object")?;

        let body = format!(
            "<read><object>{object}</object><keys>{}</keys></read>",
            escape_xml(key)
        );

        self.call(&body, None).await
    }
}

impl SageIntacctClient {
    /// Send an arbitrary documented function body, for objects this crate does not
    /// model (invoices, journal entries, reports).
    ///
    /// The body is inserted inside `<function>` unchanged, so a caller can use any
    /// documented function without waiting for a new wirekit release (FR-013).
    pub async fn call(
        &self,
        function_body: &str,
        unique_id: Option<&str>,
    ) -> Result<IntacctOutcome, WirekitError> {
        let control_id = self.control.fetch_add(1, Ordering::Relaxed);
        let body = self.build_request(function_body, unique_id, control_id);

        let url = self.client.base_url().clone();
        let spec = RequestSpec::post(url)
            .try_header("content-type", "application/xml")?
            .try_header("accept", "application/xml")?
            .bytes_body(body.into_bytes())
            .rate_limit_key(format!("sage-intacct:{}", self.config.company_id))
            .max_response_bytes(self.max_response_bytes);

        let raw = self.client.send(spec).await?;
        let outcome = parse_response(&raw.body, self.max_response_bytes)?;

        if raw.status >= 400 {
            return Err(WirekitError::Api {
                status: raw.status,
                code: outcome
                    .errorno
                    .clone()
                    .unwrap_or_else(|| String::from("INTACCT_HTTP_ERROR")),
                message: outcome
                    .description
                    .clone()
                    .unwrap_or_else(|| String::from("the gateway rejected the request")),
                details: None,
            });
        }

        if outcome.is_success() {
            Ok(outcome)
        } else {
            Err(WirekitError::Api {
                status: raw.status,
                code: outcome
                    .errorno
                    .clone()
                    .unwrap_or_else(|| String::from("INTACCT_FAILURE")),
                message: outcome.description.clone().unwrap_or_else(|| {
                    String::from("the gateway reported failure with HTTP success")
                }),
                details: None,
            })
        }
    }

    /// The request document documented by Sage Intacct Web Services.
    fn build_request(
        &self,
        function_body: &str,
        unique_id: Option<&str>,
        control_id: u64,
    ) -> String {
        let sender = escape_xml(&self.config.sender_id);
        let password = escape_xml(self.config.sender_password.expose());
        let company = escape_xml(&self.config.company_id);
        let user = escape_xml(&self.config.user_id);
        let unique = unique_id.map(escape_xml).unwrap_or_default();
        let control = format!("ctrl-{control_id}");

        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
             <request><control>\
             <senderid>{sender}</senderid>\
             <password>{password}</password>\
             <controlid>{control}</controlid>\
             <uniqueid>{unique}</uniqueid>\
             <dtdversion>3.0</dtdversion>\
             <includewhitespace>false</includewhitespace>\
             </control><operation><authentication><login>\
             <userid>{user}</userid>\
             <companyid>{company}</companyid>\
             <password>{password}</password>\
             </login></authentication><content>\
             <function controlid=\"{control}\">{function_body}</function>\
             </content></operation></request>"
        )
    }
}

/// Reject an object or field name that could break out of the XML element.
fn check_identifier(name: &str, kind: &str) -> Result<(), WirekitError> {
    let valid = !name.is_empty()
        && name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_');

    if valid {
        Ok(())
    } else {
        Err(WirekitError::InvalidConfig {
            message: format!("intacct {kind} name `{name}` must be an ASCII identifier"),
        })
    }
}

/// Escape the five XML entities so a value cannot close its own element.
fn escape_xml(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            other => escaped.push(other),
        }
    }
    escaped
}

/// Parse a gateway response, bounded by `max_bytes` and a fixed depth ceiling.
///
/// The gateway answers HTTP 200 for both success and failure, so the decision is
/// taken from the `<status>` element and never from the HTTP status (CON-213).
fn parse_response(body: &[u8], max_bytes: usize) -> Result<IntacctOutcome, WirekitError> {
    if body.len() > max_bytes {
        return Err(WirekitError::Decode {
            message: format!(
                "intacct response is {} bytes, over the {max_bytes}-byte ceiling",
                body.len()
            ),
            source: None,
        });
    }

    let text = std::str::from_utf8(body).map_err(|_| WirekitError::Decode {
        message: String::from("intacct response is not valid UTF-8"),
        source: None,
    })?;

    let mut reader = Reader::from_str(text);
    reader.config_mut().trim_text(true);

    let mut outcome = IntacctOutcome {
        status: String::new(),
        errorno: None,
        description: None,
        key: None,
        fields: Vec::new(),
    };

    let mut stack: Vec<String> = Vec::new();
    let mut in_data = 0_usize;
    let mut in_result = 0_usize;

    loop {
        match reader.read_event() {
            Ok(Event::Start(start)) => {
                if stack.len() >= DEFAULT_MAX_DEPTH {
                    return Err(WirekitError::Decode {
                        message: format!(
                            "intacct response nests deeper than {DEFAULT_MAX_DEPTH} elements"
                        ),
                        source: None,
                    });
                }
                let name = String::from_utf8_lossy(start.local_name().as_ref()).into_owned();
                if name == "data" {
                    in_data += 1;
                }
                if name == "result" {
                    in_result += 1;
                }
                stack.push(name);
            }
            Ok(Event::End(_)) => {
                if let Some(closed) = stack.pop() {
                    if closed == "data" {
                        in_data = in_data.saturating_sub(1);
                    }
                    if closed == "result" {
                        in_result = in_result.saturating_sub(1);
                    }
                }
            }
            Ok(Event::Text(text)) => {
                // quick-xml 0.41 hands back raw bytes here; 0.42 decoded them for us.
                // Intacct responses are UTF-8, so a lossy conversion is the same text
                // the newer API produced.
                let raw = text.into_inner();
                let value = String::from_utf8_lossy(&raw).into_owned();
                if value.is_empty() {
                    continue;
                }

                let current = stack.last().cloned().unwrap_or_default();
                match current.as_str() {
                    // The control block carries its own `status`; the authoritative
                    // one is inside `<result>`. Reading only the first made a
                    // failed operation look successful (caught by ac_011).
                    "status" if in_result > 0 => outcome.status = value.to_lowercase(),
                    "status" if outcome.status.is_empty() => {
                        outcome.status = value.to_lowercase();
                    }
                    "errorno" | "errno" => outcome.errorno = Some(value),
                    "description" if outcome.description.is_none() => {
                        outcome.description = Some(value);
                    }
                    "key" => outcome.key = Some(value),
                    other if in_data > 0 => outcome.fields.push((other.to_string(), value)),
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(_) => {
                return Err(WirekitError::Decode {
                    message: format!(
                        "intacct response is not well-formed XML near byte {}",
                        reader.buffer_position()
                    ),
                    source: None,
                });
            }
        }
    }

    if outcome.status.is_empty() {
        return Err(WirekitError::Decode {
            message: String::from("intacct response carried no status element"),
            source: None,
        });
    }

    Ok(outcome)
}

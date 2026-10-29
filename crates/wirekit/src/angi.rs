//! Angi lead import: a parser and importer, never a client.
//!
//! Scope: normalize a documented Angi lead email or CSV export into an
//! [`AngiLead`], report one outcome per CSV row, compute a deterministic dedupe
//! key, and verify a partner webhook when a tenant actually has one.
//!
//! Non-goals: any HTTP client, polling, lead storage, retries, or rate limiting.
//! Angi publishes no API, so there is nothing to poll; promising a live
//! integration would be a lie (`catalogue/20_angi.md`). A malformed input is a
//! validation failure, not an API failure, so it surfaces as [`ParseError`] - or
//! as [`WirekitError::InvalidConfig`] once converted.

use std::fmt;

use hmac::Mac;

use crate::{Secret, WirekitError};

/// Largest CSV export this module will parse, in bytes.
///
/// Enforced before any decoding so a hostile or accidental multi-gigabyte file
/// cannot exhaust memory (NFR-007).
pub const MAX_CSV_BYTES: usize = 8 * 1024 * 1024;

/// A postal address as it appeared in the lead.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Address {
    /// Street line, including any unit designator.
    pub line1: String,
    /// City or town.
    pub city: String,
    /// State, province, or region code.
    pub region: String,
    /// Postal or ZIP code.
    pub postal_code: String,
}

impl Address {
    /// The normalized form used in a dedupe key: lowercase, single-spaced,
    /// punctuation removed.
    #[must_use]
    pub fn normalized(&self) -> String {
        [&self.line1, &self.city, &self.region, &self.postal_code]
            .into_iter()
            .map(|part| normalize_token(part))
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

impl fmt::Display for Address {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parts: Vec<&str> = [&self.line1, &self.city, &self.region, &self.postal_code]
            .into_iter()
            .map(String::as_str)
            .filter(|part| !part.trim().is_empty())
            .collect();

        formatter.write_str(&parts.join(", "))
    }
}

/// A lead imported from Angi.
///
/// Every field except [`source`](Self::source) is optional on purpose: an email
/// template that omits a phone number is a real lead, and `None` says so
/// explicitly instead of inventing a value (CON-206).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AngiLead {
    /// Where the lead came from, for example `angi-email` or `angi-csv`.
    pub source: String,
    /// Angi's own lead identifier, when the export or template carries one.
    pub lead_id: Option<String>,
    /// Contact name exactly as supplied.
    pub contact_name: Option<String>,
    /// Phone number, digits and separators as supplied.
    pub phone: Option<String>,
    /// Email address.
    pub email: Option<String>,
    /// Service address.
    pub address: Option<Address>,
    /// Requested service or job type.
    pub service: Option<String>,
    /// When the lead arrived, as the source rendered it.
    pub received_at: Option<String>,
}

impl AngiLead {
    /// The deterministic dedupe key for this lead (FR-004).
    ///
    /// The Angi lead ID wins when present. Otherwise the key is the normalized
    /// phone number, address, and received date joined by `|`, so the same lead
    /// delivered twice - with different spacing or punctuation - collapses to one
    /// key and the result never depends on insertion order.
    ///
    /// ```
    /// # use wirekit::angi::AngiLead;
    /// let lead = AngiLead {
    ///     source: String::from("angi-csv"),
    ///     lead_id: Some(String::from("A-1")),
    ///     contact_name: None,
    ///     phone: None,
    ///     email: None,
    ///     address: None,
    ///     service: None,
    ///     received_at: None,
    /// };
    /// assert_eq!(lead.dedupe_key(), "id:A-1");
    /// ```
    #[must_use]
    pub fn dedupe_key(&self) -> String {
        if let Some(id) = self
            .lead_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
        {
            return format!("id:{id}");
        }

        let phone = normalize_phone(self.phone.as_deref().unwrap_or_default());
        let address = self
            .address
            .as_ref()
            .map(Address::normalized)
            .unwrap_or_default();
        let received = normalize_token(self.received_at.as_deref().unwrap_or_default());

        format!("phy:{phone}|{address}|{received}")
    }
}

/// A field-level validation failure. Never an API failure: Angi has no API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    /// The field that could not be read, using the public field name.
    pub field: String,
    /// What was wrong with it, without echoing the value (CON-214).
    pub reason: String,
}

impl ParseError {
    /// Build a parse error naming the field that failed.
    #[must_use]
    pub fn new(field: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            reason: reason.into(),
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "angi parse error: field `{}` {}",
            self.field, self.reason
        )
    }
}

impl std::error::Error for ParseError {}

/// Converts a parse failure into the shared error type (FR-026).
///
/// The offending field survives in the message, so a caller matching on
/// [`WirekitError`] still learns which field was unusable.
impl From<ParseError> for WirekitError {
    fn from(error: ParseError) -> Self {
        Self::InvalidConfig {
            message: error.to_string(),
        }
    }
}

/// Parse a documented Angi lead email into an [`AngiLead`].
///
/// Recognizes the labelled `Label: value` layout of the documented templates. An
/// unknown template - one with no recognizable label - fails with a
/// [`ParseError`] naming the first expected field rather than returning an empty
/// lead, because an empty lead looks like a lead with no data (AC-002).
pub fn parse_email(body: &str) -> Result<AngiLead, ParseError> {
    let mut lead = AngiLead {
        source: String::from("angi-email"),
        lead_id: None,
        contact_name: None,
        phone: None,
        email: None,
        address: None,
        service: None,
        received_at: None,
    };

    let mut recognized = 0_usize;
    let mut address = Address::default();

    for line in body.lines() {
        let Some((label, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        if value.is_empty() {
            continue;
        }

        match label.trim().to_ascii_lowercase().as_str() {
            "lead id" | "leadid" | "reference" => {
                lead.lead_id = Some(value.to_string());
                recognized += 1;
            }
            "name" | "customer" | "contact" => {
                lead.contact_name = Some(value.to_string());
                recognized += 1;
            }
            "phone" | "telephone" | "mobile" => {
                lead.phone = Some(value.to_string());
                recognized += 1;
            }
            "email" => {
                lead.email = Some(value.to_string());
                recognized += 1;
            }
            "address" | "street" => {
                address.line1 = value.to_string();
                recognized += 1;
            }
            "city" => {
                address.city = value.to_string();
                recognized += 1;
            }
            "state" | "region" => {
                address.region = value.to_string();
                recognized += 1;
            }
            "zip" | "postal code" | "postcode" => {
                address.postal_code = value.to_string();
                recognized += 1;
            }
            "service" | "job type" | "category" => {
                lead.service = Some(value.to_string());
                recognized += 1;
            }
            "received" | "date" | "received at" => {
                lead.received_at = Some(value.to_string());
                recognized += 1;
            }
            _ => {}
        }
    }

    if recognized == 0 {
        return Err(ParseError::new(
            "lead id",
            "was not found; the email does not match a documented Angi template",
        ));
    }

    lead.address = if address.line1.is_empty() && address.city.is_empty() {
        None
    } else {
        Some(address)
    };

    Ok(lead)
}

/// Which CSV column supplies which [`AngiLead`] field.
///
/// The mapping is explicit rather than guessed from header order, because Angi
/// exports vary by template and a positional guess silently misfiles every column
/// after the first difference (FR-003). A `None` column is simply not imported.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ColumnMap {
    /// Header carrying Angi's lead identifier.
    pub lead_id: Option<String>,
    /// Header carrying the contact name.
    pub contact_name: Option<String>,
    /// Header carrying the phone number.
    pub phone: Option<String>,
    /// Header carrying the email address.
    pub email: Option<String>,
    /// Header carrying the street line.
    pub address: Option<String>,
    /// Header carrying the city.
    pub city: Option<String>,
    /// Header carrying the state or region.
    pub region: Option<String>,
    /// Header carrying the postal code.
    pub postal_code: Option<String>,
    /// Header carrying the requested service.
    pub service: Option<String>,
    /// Header carrying the received date.
    pub received_at: Option<String>,
}

/// What happened to one CSV row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportOutcome {
    /// The row became an [`AngiLead`].
    Imported,
    /// The row repeated a lead already seen in this import.
    Skipped,
    /// The row could not become a lead.
    Failed,
}

/// The outcome of one CSV row, always carrying its 1-based file line number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AngiImportOutcome {
    /// 1-based line number in the source file; the header is line 1.
    pub line: usize,
    /// What happened to this row.
    pub outcome: ImportOutcome,
    /// Why, when the outcome is not `Imported`. Never echoes field values.
    pub reason: Option<String>,
}

/// The result of importing an Angi export (FR-003).
///
/// Import never aborts on a single bad row: a failed row is reported and the rest
/// of the file is still imported, so one malformed address cannot lose every lead
/// behind it (AC-003).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AngiImportReport {
    /// The leads that were imported, in file order.
    pub leads: Vec<AngiLead>,
    /// One entry per data row, in file order.
    pub outcomes: Vec<AngiImportOutcome>,
    /// The dedupe key of every imported lead, in file order.
    pub dedupe_keys: Vec<String>,
}

impl AngiImportReport {
    /// How many rows were imported.
    #[must_use]
    pub fn imported(&self) -> usize {
        self.count(ImportOutcome::Imported)
    }

    /// How many rows were skipped as duplicates.
    #[must_use]
    pub fn skipped(&self) -> usize {
        self.count(ImportOutcome::Skipped)
    }

    /// How many rows failed.
    #[must_use]
    pub fn failed(&self) -> usize {
        self.count(ImportOutcome::Failed)
    }

    fn count(&self, outcome: ImportOutcome) -> usize {
        self.outcomes
            .iter()
            .filter(|entry| entry.outcome == outcome)
            .count()
    }
}

/// Import an Angi CSV export through an explicit [`ColumnMap`] (FR-003).
///
/// Fails before touching any row when a mapped column is absent from the header,
/// so a mis-mapped import is refused instead of reported as a partial success
/// (EC-002). Duplicate leads within the file are reported as
/// [`ImportOutcome::Skipped`] using the same [`AngiLead::dedupe_key`] a caller can
/// apply across imports.
pub fn import_csv(export: &[u8], columns: &ColumnMap) -> Result<AngiImportReport, WirekitError> {
    if export.len() > MAX_CSV_BYTES {
        return Err(WirekitError::InvalidConfig {
            message: format!(
                "angi export is {} bytes, over the {MAX_CSV_BYTES}-byte ceiling",
                export.len()
            ),
        });
    }

    let mut reader = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .flexible(true)
        .from_reader(export);

    let headers = reader
        .headers()
        .map_err(|error| WirekitError::InvalidConfig {
            message: format!("angi export header could not be read: {error}"),
        })?
        .clone();

    let index_of = |name: &Option<String>| -> Result<Option<usize>, WirekitError> {
        let Some(wanted) = name else { return Ok(None) };
        match headers.iter().position(|header| header == wanted) {
            Some(index) => Ok(Some(index)),
            None => Err(WirekitError::InvalidConfig {
                message: format!("angi export has no column named `{wanted}`"),
            }),
        }
    };

    let lead_id = index_of(&columns.lead_id)?;
    let contact_name = index_of(&columns.contact_name)?;
    let phone = index_of(&columns.phone)?;
    let email = index_of(&columns.email)?;
    let line1 = index_of(&columns.address)?;
    let city = index_of(&columns.city)?;
    let region = index_of(&columns.region)?;
    let postal_code = index_of(&columns.postal_code)?;
    let service = index_of(&columns.service)?;
    let received_at = index_of(&columns.received_at)?;

    let cell = |record: &csv::StringRecord, index: Option<usize>| -> Option<String> {
        index
            .and_then(|position| record.get(position))
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    };

    let mut report = AngiImportReport::default();
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    for (offset, record) in reader.records().enumerate() {
        let line = offset + 2;

        let record = match record {
            Ok(record) => record,
            Err(error) => {
                report.outcomes.push(AngiImportOutcome {
                    line,
                    outcome: ImportOutcome::Failed,
                    reason: Some(format!("row could not be read: {error}")),
                });
                continue;
            }
        };

        let address =
            if line1.is_some() || city.is_some() || region.is_some() || postal_code.is_some() {
                Some(Address {
                    line1: cell(&record, line1).unwrap_or_default(),
                    city: cell(&record, city).unwrap_or_default(),
                    region: cell(&record, region).unwrap_or_default(),
                    postal_code: cell(&record, postal_code).unwrap_or_default(),
                })
            } else {
                None
            };

        let lead = AngiLead {
            source: String::from("angi-csv"),
            lead_id: cell(&record, lead_id),
            contact_name: cell(&record, contact_name),
            phone: cell(&record, phone),
            email: cell(&record, email),
            address,
            service: cell(&record, service),
            received_at: cell(&record, received_at),
        };

        if lead.lead_id.is_none() && lead.phone.is_none() && lead.email.is_none() {
            report.outcomes.push(AngiImportOutcome {
                line,
                outcome: ImportOutcome::Failed,
                reason: Some(String::from(
                    "row has no lead id, phone, or email, so it cannot be deduplicated",
                )),
            });
            continue;
        }

        let key = lead.dedupe_key();

        if let Some(first_line) = seen.get(&key) {
            report.outcomes.push(AngiImportOutcome {
                line,
                outcome: ImportOutcome::Skipped,
                reason: Some(format!("duplicate of line {first_line}")),
            });
            continue;
        }

        seen.insert(key.clone(), line);
        report.dedupe_keys.push(key);
        report.leads.push(lead);
        report.outcomes.push(AngiImportOutcome {
            line,
            outcome: ImportOutcome::Imported,
            reason: None,
        });
    }

    Ok(report)
}

/// Verifies an Angi partner webhook, when - and only when - one is provisioned.
///
/// Most tenants have no feed, so this type is explicit about both states. Calling
/// [`verify`](Self::verify) on an unprovisioned instance fails with
/// [`WirekitError::InvalidConfig`] instead of "verifying" nothing, which is the
/// difference between an honest limitation and a security hole (FR-005, EC-004).
#[derive(Debug, Clone, Default)]
pub struct AngiWebhook {
    secret: Option<Secret<String>>,
}

impl AngiWebhook {
    /// A webhook backed by the tenant's shared secret.
    #[must_use]
    pub fn provisioned(shared_secret: Secret<String>) -> Self {
        Self {
            secret: Some(shared_secret),
        }
    }

    /// A tenant with no partner feed.
    #[must_use]
    pub fn unprovisioned() -> Self {
        Self { secret: None }
    }

    /// Whether this instance can verify a signature at all.
    #[must_use]
    pub fn is_provisioned(&self) -> bool {
        self.secret.is_some()
    }

    /// Verify a hex-encoded HMAC-SHA256 signature over the **raw** request bytes.
    ///
    /// The body must be the bytes as received: re-serializing before verification
    /// would let a signature cover one payload and authorize another (CON-215).
    pub fn verify(&self, body: &[u8], signature_hex: &str) -> Result<(), WirekitError> {
        let Some(secret) = self.secret.as_ref() else {
            return Err(WirekitError::InvalidConfig {
                message: String::from(
                    "no angi partner feed is provisioned for this tenant; there is nothing to verify",
                ),
            });
        };

        let mut mac =
            <hmac::Hmac<sha2::Sha256> as hmac::Mac>::new_from_slice(secret.expose().as_bytes())
                .map_err(|_| WirekitError::InvalidConfig {
                    message: String::from("angi webhook secret is unusable as an HMAC key"),
                })?;
        mac.update(body);
        let expected = hex_lower(&mac.finalize().into_bytes());

        if constant_time_eq(expected.as_bytes(), signature_hex.trim().as_bytes()) {
            Ok(())
        } else {
            Err(WirekitError::Auth {
                message: String::from("angi webhook signature did not match the request body"),
            })
        }
    }
}

/// Lowercase `token` for comparison: alphanumeric runs joined by single spaces.
fn normalize_token(token: &str) -> String {
    let mut normalized = String::with_capacity(token.len());
    let mut pending_space = false;

    for character in token.chars() {
        if character.is_alphanumeric() {
            if pending_space && !normalized.is_empty() {
                normalized.push(' ');
            }
            pending_space = false;
            normalized.extend(character.to_lowercase());
        } else {
            pending_space = true;
        }
    }

    normalized
}

/// The digits of a phone number, which is what identifies it across templates.
fn normalize_phone(phone: &str) -> String {
    phone.chars().filter(char::is_ascii_digit).collect()
}

/// Lowercase hex encoding, so no extra dependency is needed for signatures.
fn hex_lower(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push_str(&format!("{byte:02x}"));
    }
    encoded
}

/// Compare two byte strings without leaking where they first differ.
fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }

    let mut difference = 0_u8;
    for (a, b) in left.iter().zip(right.iter()) {
        difference |= a ^ b;
    }

    difference == 0
}

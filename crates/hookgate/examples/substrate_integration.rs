//! Composes `platform-core`, `quota-ledger`, and `hookgate` in one flow.
//!
//! The example registers a provider, reserves and commits one unit of quota,
//! verifies a generic HMAC-SHA256 webhook, and inserts the verified delivery
//! into an in-memory store with an atomic outbox enqueue.

use std::sync::Arc;

use bytes::Bytes;
use chrono::Duration;
use hmac::{Hmac, Mac};
use hookgate::{
    DeliveryRecord, DeliveryStore, HmacPreset, InboundRequest, MemoryDeliveryStore, Outbox,
    OutboxMessage, Verifier,
};
use http::{HeaderMap, HeaderName, HeaderValue};
use platform_core::{Capability, Provider, ProviderId, ProvisioningMode, Registry, TenantId};
use quota_ledger::{
    Budget, ExhaustionPolicy, MemoryQuotaStore, Metric, QuotaKey, QuotaStore, ReserveRequest,
    Window,
};
use serde_json::json;
use sha2::Sha256;

struct ExampleProvider {
    id: ProviderId,
}

const CAPABILITIES: &[Capability] = &[Capability::new("messages.send", ProvisioningMode::Api)];

impl Provider for ExampleProvider {
    fn id(&self) -> ProviderId {
        self.id
    }

    fn capabilities(&self) -> &'static [Capability] {
        CAPABILITIES
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let tenant = TenantId::generate();
    let provider = ProviderId::new("acme");

    let mut registry = Registry::new();
    registry.register(Arc::new(ExampleProvider { id: provider }))?;
    let capabilities = registry.capabilities(&provider).unwrap_or(&[]);
    println!(
        "registered {} provider(s); {} capability(ies)",
        registry.len(),
        capabilities.len()
    );

    let quota = MemoryQuotaStore::new();
    let budget = Budget::new(
        tenant,
        provider,
        Metric::new("messages"),
        3,
        Window::rolling(Duration::hours(1)),
        ExhaustionPolicy::HardStop,
    );
    let request = ReserveRequest::new(budget, 1).with_idempotency_key("example-message-1");
    let outcome = quota.reserve(&request).await?;
    let reservation = outcome
        .reservation()
        .ok_or_else(|| "the quota policy denied the reservation".to_owned())?;
    quota.commit(reservation.id).await?;

    let usage = quota
        .usage(&QuotaKey::new(
            tenant,
            provider,
            Metric::new("messages"),
            chrono::Utc::now(),
        ))
        .await?;
    println!(
        "quota: {} used, {} held, {} remaining",
        usage.used,
        usage.held,
        usage.limit.saturating_sub(usage.used + usage.held)
    );

    let secret = b"example-webhook-secret";
    let body = serde_json::to_vec(&json!({
        "id": "evt_example_1",
        "type": "message.created",
        "data": { "text": "hello from the substrate example" }
    }))?;

    let mut mac = Hmac::<Sha256>::new_from_slice(secret)?;
    mac.update(&body);
    let signature = hex::encode(mac.finalize().into_bytes());

    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("x-signature"),
        HeaderValue::from_str(&signature)?,
    );

    let request = InboundRequest::new(tenant, provider, headers, Bytes::from(body));
    let verifier = HmacPreset::new(provider, secret).header(HeaderName::from_static("x-signature"));
    let event = verifier.verify(&request)?;
    println!(
        "verified {} event {}",
        event.event_type,
        event.event_id.as_deref().unwrap_or("<none>")
    );

    let store = MemoryDeliveryStore::new();
    let record = DeliveryRecord::from_event(&event)?;
    let message = OutboxMessage::from_event(&event);
    let inserted = store.insert_if_new(record, message).await?;
    println!("delivery insert: {inserted:?}");

    match store.pop().await? {
        Some(message) => println!(
            "outbox: {} {} {}",
            message.delivery, message.provider, message.event_type
        ),
        None => println!("outbox: empty"),
    }

    Ok(())
}

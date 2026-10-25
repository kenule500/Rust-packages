//! AC-045: the integration flow the shipped example demonstrates, composed
//! here so it runs in the suite as well.

use std::sync::Arc;

use bytes::Bytes;
use chrono::Duration;
use hmac::{Hmac, Mac};
use hookgate::{
    DeliveryRecord, DeliveryStore, HmacPreset, InboundRequest, InsertOutcome, MemoryDeliveryStore,
    Outbox, OutboxMessage, Verifier,
};
use http::{HeaderMap, HeaderName, HeaderValue};
use platform_core::{Capability, Provider, ProviderId, ProvisioningMode, Registry, TenantId};
use quota_ledger::{
    Budget, ExhaustionPolicy, MemoryQuotaStore, Metric, QuotaKey, QuotaStore, ReserveRequest, Window,
};
use serde_json::json;
use sha2::Sha256;

struct ExampleProvider {
    id: ProviderId,
}

static CAPABILITIES: [Capability; 1] =
    [Capability::new("messages.send", ProvisioningMode::Api)];

impl Provider for ExampleProvider {
    fn id(&self) -> ProviderId {
        self.id
    }

    fn capabilities(&self) -> &'static [Capability] {
        &CAPABILITIES
    }
}

/// AC-045: register a provider, reserve and commit quota, verify a signed
/// webhook, and enqueue it through the outbox.
#[tokio::test]
async fn ac_045_the_example_composes_registry_quota_and_webhook_ingress() {
    let tenant = TenantId::generate();
    let provider = ProviderId::new("acme");

    let mut registry = Registry::new();
    registry
        .register(Arc::new(ExampleProvider { id: provider }))
        .expect("AC-045: the example must register its provider");
    let capabilities = registry
        .capabilities(&provider)
        .expect("AC-045: the registered provider must declare capabilities");
    assert_eq!(
        capabilities, CAPABILITIES.as_slice(),
        "AC-045: the example must expose the provider's declared capability"
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
    let reservation = quota
        .reserve(
            &ReserveRequest::new(budget.clone(), 1).with_idempotency_key("example-message-1"),
        )
        .await
        .expect("AC-045: the example must reserve quota")
        .reservation()
        .expect("AC-045: the example's reservation must be allowed")
        .id;
    quota
        .commit(reservation)
        .await
        .expect("AC-045: the example must commit its reservation");
    let usage = quota
        .usage(&QuotaKey::new(
            tenant,
            provider,
            Metric::new("messages"),
            chrono::Utc::now(),
        ))
        .await
        .expect("AC-045: the example must read usage");
    assert_eq!(usage.used, 1, "AC-045: the committed unit must be consumed");
    assert_eq!(usage.held, 0, "AC-045: no unit may remain held");

    let secret = b"example-webhook-secret";
    let body = serde_json::to_vec(&json!({
        "id": "evt_example_1",
        "type": "message.created",
        "data": { "text": "hello from the substrate example" }
    }))
    .expect("AC-045: the example payload must serialize");

    let mut mac = Hmac::<Sha256>::new_from_slice(secret).expect("HMAC accepts any key length");
    mac.update(&body);
    let signature = hex::encode(mac.finalize().into_bytes());

    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("x-signature"),
        HeaderValue::from_str(&signature).expect("the signature must be a visible header"),
    );

    let request = InboundRequest::new(tenant, provider, headers, Bytes::from(body));
    let verifier = HmacPreset::new(provider, secret).header(HeaderName::from_static("x-signature"));
    let event = verifier
        .verify(&request)
        .expect("AC-045: the example must verify its signed webhook");
    assert_eq!(
        event.event_id.as_deref(),
        Some("evt_example_1"),
        "AC-045: the verified event must carry the provider id"
    );

    let store = MemoryDeliveryStore::new();
    let outcome = store
        .insert_if_new(
            DeliveryRecord::from_event(&event).expect("AC-045: the event carries an id"),
            OutboxMessage::from_event(&event),
        )
        .await
        .expect("AC-045: the example must enqueue the delivery");
    assert_eq!(
        outcome,
        InsertOutcome::Inserted,
        "AC-045: the example must insert the delivery once"
    );

    let message = store
        .pop()
        .await
        .expect("AC-045: the outbox must be readable")
        .expect("AC-045: the example must enqueue one outbox message");
    assert_eq!(
        message.delivery, event.delivery,
        "AC-045: the outbox message must reference the verified delivery"
    );
    assert_eq!(
        message.event_type, "message.created",
        "AC-045: the outbox message must carry the event type"
    );
}

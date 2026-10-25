//! AC-052: a webhook flow must emit tracing events for its outcome.
//!
//! This test is expected to expose a production gap: `hookgate` has no
//! `tracing` calls, so the webhook half of AC-052 cannot pass without a
//! production change that this phase may not make.

use std::sync::{Arc, Mutex};

use bytes::Bytes;
use hmac::{Hmac, Mac};
use hookgate::{
    DeliveryRecord, DeliveryStore, HmacPreset, InboundRequest, MemoryDeliveryStore, OutboxMessage,
    ProviderId, TenantId, Verifier,
};
use http::{HeaderMap, HeaderName, HeaderValue};
use sha2::Sha256;
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Metadata, Subscriber};

#[derive(Debug, Clone)]
struct CapturedEvent {
    name: String,
    fields: Vec<(String, String)>,
}

#[derive(Clone, Default)]
struct Collector {
    events: Arc<Mutex<Vec<CapturedEvent>>>,
}

impl CapturedEvent {
    fn message(&self) -> &str {
        self.fields
            .iter()
            .find(|(field, _)| field == "message")
            .map(|(_, value)| value.trim_matches('"'))
            .unwrap_or(&self.name)
    }
}

impl Collector {
    fn names(&self) -> Vec<String> {
        self.events
            .lock()
            .expect("the collector lock must not be poisoned")
            .iter()
            .map(|event| event.message().to_owned())
            .collect()
    }

    fn find(&self, message: &str) -> Option<CapturedEvent> {
        self.events
            .lock()
            .expect("the collector lock must not be poisoned")
            .iter()
            .find(|event| event.message() == message)
            .cloned()
    }
}

struct FieldVisitor(Vec<(String, String)>);

impl Visit for FieldVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.0.push((field.name().to_owned(), format!("{value:?}")));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.0.push((field.name().to_owned(), value.to_owned()));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.0.push((field.name().to_owned(), value.to_string()));
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.0.push((field.name().to_owned(), value.to_string()));
    }
}

impl Subscriber for Collector {
    fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
        true
    }

    fn new_span(&self, _span: &Attributes<'_>) -> Id {
        Id::from_u64(1)
    }

    fn record(&self, _span: &Id, _values: &Record<'_>) {}

    fn record_follows_from(&self, _span: &Id, _follows: &Id) {}

    fn event(&self, event: &Event<'_>) {
        let mut visitor = FieldVisitor(Vec::new());
        event.record(&mut visitor);
        self.events
            .lock()
            .expect("the collector lock must not be poisoned")
            .push(CapturedEvent {
                name: event.metadata().name().to_owned(),
                fields: visitor.0,
            });
    }

    fn enter(&self, _span: &Id) {}

    fn exit(&self, _span: &Id) {}
}

/// AC-052: verification and enqueue emit webhook outcome events.
#[test]
fn ac_052_a_webhook_flow_emits_outcome_events() {
    let collector = Collector::default();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("AC-052: the test runtime must start");

    tracing::subscriber::with_default(collector.clone(), || {
        runtime.block_on(async {
            let secret = b"trace-secret";
            let body: &'static [u8] = br#"{"id":"evt_052","type":"message.created"}"#;

            let mut mac =
                Hmac::<Sha256>::new_from_slice(secret).expect("HMAC accepts any key length");
            mac.update(body);
            let signature = hex::encode(mac.finalize().into_bytes());

            let mut headers = HeaderMap::new();
            headers.insert(
                HeaderName::from_static("x-signature"),
                HeaderValue::from_str(&signature).expect("the signature must be a visible header"),
            );

            let tenant = TenantId::generate();
            let provider = ProviderId::new("acme");
            let request =
                InboundRequest::new(tenant, provider, headers, Bytes::from_static(body));
            let event = HmacPreset::new(provider, secret)
                .verify(&request)
                .expect("AC-052: the signed webhook must verify");

            let store = MemoryDeliveryStore::new();
            store
                .insert_if_new(
                    DeliveryRecord::from_event(&event)
                        .expect("AC-052: the event carries an id"),
                    OutboxMessage::from_event(&event),
                )
                .await
                .expect("AC-052: the verified delivery must be enqueued");
        });
    });

    let names = collector.names();
    assert!(
        names.iter().any(|name| name.starts_with("hookgate.")),
        "AC-052: the webhook flow emitted no tracing event. Expected webhook outcome events \
         (for example `hookgate.delivery.inserted`) from verification and enqueue; captured {names:?}. \
         This is a production gap: `crates/hookgate/src` contains no `tracing` calls."
    );

    let outcome = collector
        .find("hookgate.delivery.inserted")
        .or_else(|| collector.find("hookgate.webhook.outcome"))
        .expect("AC-052: the webhook flow must emit a named outcome event");
    assert!(
        outcome.fields.iter().any(|(field, _)| field == "delivery"),
        "AC-052: the webhook outcome event must identify the delivery, got {outcome:?}"
    );
}

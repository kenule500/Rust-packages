//! AC-030 and AC-031: dead-lettering with the raw body, and replay with a
//! stable delivery identity.

use bytes::Bytes;
use hookgate::{
    DeliveryId, DeliveryRecord, DeliveryStatus, DeliveryStore, InsertOutcome,
    MemoryDeliveryStore, Outbox, OutboxMessage, ProviderId, TenantId, VerifiedEvent,
};
use serde_json::json;

fn event(tenant: TenantId, provider: ProviderId, event_id: &str, body: &'static [u8]) -> VerifiedEvent {
    VerifiedEvent {
        delivery: DeliveryId::generate(),
        tenant,
        provider,
        event_id: Some(event_id.to_owned()),
        event_type: "message.created".to_owned(),
        payload: json!({ "id": event_id }),
        raw: Bytes::from_static(body),
    }
}

/// AC-030: exceeding the attempt ceiling records a dead letter with the raw
/// body and the error text.
#[tokio::test]
async fn ac_030_exhausted_delivery_is_dead_lettered_with_body_and_error() {
    const BODY: &[u8] = br#"{"id":"evt_030","data":{"text":"poison"}}"#;

    let store = MemoryDeliveryStore::with_max_attempts(2);
    let tenant = TenantId::generate();
    let provider = ProviderId::new("acme");
    let verified = event(tenant, provider, "evt_030", BODY);

    store
        .insert_if_new(
            DeliveryRecord::from_event(&verified).expect("AC-030: the event carries an id"),
            OutboxMessage::from_event(&verified),
        )
        .await
        .expect("AC-030: the delivery must be stored");

    let first = store
        .fail(tenant, verified.delivery, "handler failed with token=super-secret")
        .await
        .expect("AC-030: the first failure must be recorded");
    assert!(
        first.is_none(),
        "AC-030: a failure below the ceiling must not dead-letter the delivery"
    );
    let record = store
        .get(tenant, verified.delivery)
        .await
        .expect("AC-030: the delivery must be readable")
        .expect("AC-030: the delivery must exist");
    assert_eq!(record.status, DeliveryStatus::Failed, "AC-030: the first failure must mark the delivery failed");
    assert_eq!(record.attempts, 1, "AC-030: the attempt count must be recorded");

    let dead_letter = store
        .fail(tenant, verified.delivery, "handler failed with token=super-secret")
        .await
        .expect("AC-030: the second failure must be recorded")
        .expect("AC-030: reaching the ceiling must produce a dead-letter record");

    assert_eq!(
        dead_letter.delivery, verified.delivery,
        "AC-030: the dead letter must keep the delivery identity"
    );
    assert_eq!(
        dead_letter.raw.as_ref(),
        BODY,
        "AC-030: the dead letter must contain the raw body"
    );
    assert!(
        dead_letter.error.contains("[REDACTED]"),
        "AC-030: the dead letter must contain the redacted error text, got {:?}",
        dead_letter.error
    );
    assert!(
        !dead_letter.error.contains("super-secret"),
        "AC-030: the dead letter must not leak the secret in the error text"
    );
    let record = store
        .get(tenant, verified.delivery)
        .await
        .expect("AC-030: the delivery must be readable")
        .expect("AC-030: the delivery must exist");
    assert_eq!(
        record.status,
        DeliveryStatus::DeadLettered,
        "AC-030: the delivery must be marked dead-lettered"
    );
    assert_eq!(
        store
            .dead_letters(tenant)
            .await
            .expect("AC-030: dead letters must be readable")
            .len(),
        1,
        "AC-030: exactly one dead-letter record must be stored"
    );
}

/// AC-031: replay preserves the delivery id and reports a processed replay as
/// a duplicate.
#[tokio::test]
async fn ac_031_replay_preserves_delivery_identity_and_detects_duplicates() {
    let store = MemoryDeliveryStore::with_max_attempts(1);
    let tenant = TenantId::generate();
    let provider = ProviderId::new("acme");
    let verified = event(tenant, provider, "evt_031", br#"{"id":"evt_031"}"#);

    store
        .insert_if_new(
            DeliveryRecord::from_event(&verified).expect("AC-031: the event carries an id"),
            OutboxMessage::from_event(&verified),
        )
        .await
        .expect("AC-031: the delivery must be stored");
    store
        .fail(tenant, verified.delivery, "handler panicked")
        .await
        .expect("AC-031: the failure must be recorded")
        .expect("AC-031: one attempt must exceed the configured ceiling");

    let replay = store
        .replay(tenant, verified.delivery)
        .await
        .expect("AC-031: replay must succeed");
    assert_eq!(
        replay,
        InsertOutcome::Inserted,
        "AC-031: a dead-lettered delivery must be re-enqueued"
    );

    let original = store
        .pop()
        .await
        .expect("AC-031: the outbox must be readable")
        .expect("AC-031: the original outbox message must be present");
    let replayed = store
        .pop()
        .await
        .expect("AC-031: the outbox must be readable")
        .expect("AC-031: the replayed outbox message must be present");
    assert_eq!(
        original.delivery, verified.delivery,
        "AC-031: the handler must observe the original delivery id"
    );
    assert_eq!(
        replayed.delivery, verified.delivery,
        "AC-031: the replay must reuse the same delivery id"
    );

    let record = store
        .get(tenant, verified.delivery)
        .await
        .expect("AC-031: the delivery must be readable")
        .expect("AC-031: the delivery must exist");
    assert_eq!(record.status, DeliveryStatus::Pending, "AC-031: replay must reset the delivery to pending");
    assert_eq!(record.attempts, 0, "AC-031: replay must reset the attempt count");

    store
        .mark_processed(tenant, verified.delivery)
        .await
        .expect("AC-031: marking the replay processed must succeed");
    let duplicate = store
        .replay(tenant, verified.delivery)
        .await
        .expect("AC-031: replaying a processed delivery must succeed");
    assert_eq!(
        duplicate,
        InsertOutcome::Duplicate,
        "AC-031: a delivery that already succeeded must report Duplicate"
    );
}

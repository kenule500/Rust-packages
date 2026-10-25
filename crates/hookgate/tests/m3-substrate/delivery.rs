//! AC-026 through AC-029: tenant-scoped deduplication and transactional
//! insert plus enqueue.

use bytes::Bytes;
use hookgate::{
    DeliveryId, DeliveryRecord, DeliveryStore, InsertOutcome, MemoryDeliveryStore, Outbox,
    OutboxMessage, ProviderId, StoreError, TenantId, VerifiedEvent,
};
use serde_json::json;

fn event(
    tenant: TenantId,
    provider: ProviderId,
    event_id: &str,
    body: &'static [u8],
) -> VerifiedEvent {
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

/// AC-026: a repeated provider event id for one tenant is a duplicate.
#[tokio::test]
async fn ac_026_duplicate_event_id_for_one_tenant_is_rejected() {
    let store = MemoryDeliveryStore::new();
    let tenant = TenantId::generate();
    let provider = ProviderId::new("acme");
    let body: &'static [u8] = br#"{"id":"evt_026"}"#;

    let first = event(tenant, provider, "evt_026", body);
    let inserted = store
        .insert_if_new(
            DeliveryRecord::from_event(&first).expect("AC-026: the event carries an id"),
            OutboxMessage::from_event(&first),
        )
        .await
        .expect("AC-026: the first insert must succeed");
    assert_eq!(inserted, InsertOutcome::Inserted, "AC-026: the first delivery must insert");

    let retried = event(tenant, provider, "evt_026", body);
    let duplicate = store
        .insert_if_new(
            DeliveryRecord::from_event(&retried).expect("AC-026: the event carries an id"),
            OutboxMessage::from_event(&retried),
        )
        .await
        .expect("AC-026: the duplicate insert must be handled without error");
    assert_eq!(
        duplicate,
        InsertOutcome::Duplicate,
        "AC-026: a repeated event id for one tenant must report Duplicate"
    );
    assert_eq!(
        store.len().await.expect("AC-026: the outbox must be readable"),
        1,
        "AC-026: a duplicate delivery must not write a second outbox row"
    );
}

/// AC-027: the same event id from two tenants is accepted twice.
#[tokio::test]
async fn ac_027_same_event_id_from_two_tenants_is_accepted() {
    let store = MemoryDeliveryStore::new();
    let provider = ProviderId::new("acme");
    let tenant_a = TenantId::generate();
    let tenant_b = TenantId::generate();
    let body: &'static [u8] = br#"{"id":"evt_027"}"#;

    for tenant in [tenant_a, tenant_b] {
        let verified = event(tenant, provider, "evt_027", body);
        let outcome = store
            .insert_if_new(
                DeliveryRecord::from_event(&verified).expect("AC-027: the event carries an id"),
                OutboxMessage::from_event(&verified),
            )
            .await
            .expect("AC-027: each tenant insert must succeed");
        assert_eq!(
            outcome,
            InsertOutcome::Inserted,
            "AC-027: tenant {tenant} must receive its own delivery"
        );
    }

    assert_eq!(
        store.len().await.expect("AC-027: the outbox must be readable"),
        2,
        "AC-027: both tenants must be enqueued, one row each"
    );
}

/// AC-028: an already-processed delivery is reported as a duplicate.
#[tokio::test]
async fn ac_028_processed_delivery_is_not_re_enqueued() {
    let store = MemoryDeliveryStore::new();
    let tenant = TenantId::generate();
    let provider = ProviderId::new("acme");
    let body: &'static [u8] = br#"{"id":"evt_028"}"#;

    let verified = event(tenant, provider, "evt_028", body);
    store
        .insert_if_new(
            DeliveryRecord::from_event(&verified).expect("AC-028: the event carries an id"),
            OutboxMessage::from_event(&verified),
        )
        .await
        .expect("AC-028: the first insert must succeed");
    store
        .mark_processed(tenant, verified.delivery)
        .await
        .expect("AC-028: marking the delivery processed must succeed");

    let repeated = event(tenant, provider, "evt_028", body);
    let duplicate = store
        .insert_if_new(
            DeliveryRecord::from_event(&repeated).expect("AC-028: the event carries an id"),
            OutboxMessage::from_event(&repeated),
        )
        .await
        .expect("AC-028: the repeated insert must be handled without error");
    assert_eq!(
        duplicate,
        InsertOutcome::Duplicate,
        "AC-028: a processed delivery must be reported as Duplicate"
    );

    let replay = store
        .replay(tenant, verified.delivery)
        .await
        .expect("AC-028: replaying a processed delivery must succeed");
    assert_eq!(
        replay,
        InsertOutcome::Duplicate,
        "AC-028: a processed delivery must not be re-enqueued by replay"
    );
    assert_eq!(
        store.len().await.expect("AC-028: the outbox must be readable"),
        1,
        "AC-028: no additional outbox row may be written"
    );
}

/// AC-029: the delivery and outbox row commit together or not at all.
#[tokio::test]
async fn ac_029_insert_and_enqueue_are_atomic() {
    let tenant = TenantId::generate();
    let provider = ProviderId::new("acme");
    let body: &'static [u8] = br#"{"id":"evt_029"}"#;
    let verified = event(tenant, provider, "evt_029", body);

    let committed = MemoryDeliveryStore::new();
    let outcome = committed
        .insert_if_new(
            DeliveryRecord::from_event(&verified).expect("AC-029: the event carries an id"),
            OutboxMessage::from_event(&verified),
        )
        .await
        .expect("AC-029: a valid insert must succeed");
    assert_eq!(outcome, InsertOutcome::Inserted, "AC-029: the first insert must commit");
    assert!(
        committed
            .get(tenant, verified.delivery)
            .await
            .expect("AC-029: the delivery must be readable")
            .is_some(),
        "AC-029: the delivery row must be visible after commit"
    );
    assert_eq!(
        committed.len().await.expect("AC-029: the outbox must be readable"),
        1,
        "AC-029: the outbox row must be visible after commit"
    );

    let rolled_back = MemoryDeliveryStore::new();
    let other = event(TenantId::generate(), provider, "evt_029_other", body);
    let mismatched_message = OutboxMessage {
        delivery: other.delivery,
        ..OutboxMessage::from_event(&verified)
    };
    let error = rolled_back
        .insert_if_new(
            DeliveryRecord::from_event(&verified).expect("AC-029: the event carries an id"),
            mismatched_message,
        )
        .await
        .expect_err("AC-029: a mismatched outbox message must fail the transaction");
    assert!(
        matches!(error, StoreError::Config(_)),
        "AC-029: a failed enqueue must return a typed error, got {error:?}"
    );
    assert!(
        rolled_back
            .get(tenant, verified.delivery)
            .await
            .expect("AC-029: the delivery lookup must succeed")
            .is_none(),
        "AC-029: a failed enqueue must not leave the delivery visible"
    );
    assert_eq!(
        rolled_back.len().await.expect("AC-029: the outbox must be readable"),
        0,
        "AC-029: a failed enqueue must leave no outbox row"
    );
}

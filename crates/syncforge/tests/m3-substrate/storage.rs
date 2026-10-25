//! AC-032 through AC-035: durable-before-ack enqueue, ordering, storage
//! failures, and UUID v7 action ids.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;
use syncforge::{
    Action, ActionId, MemoryStorage, MockTransport, Operation, Storage, SyncEngine, SyncError,
};
use uuid::Version;

fn engine() -> (SyncEngine, Arc<MemoryStorage>, Arc<MockTransport>) {
    let storage = Arc::new(MemoryStorage::new());
    let transport = Arc::new(MockTransport::new());
    let engine = SyncEngine::new(storage.clone(), transport.clone());
    (engine, storage, transport)
}

/// AC-032: after `enqueue` returns, the action is already in storage.
#[tokio::test]
async fn ac_032_enqueue_is_durable_before_it_returns() {
    let (engine, storage, _transport) = engine();
    let action = Action::new("work_order", Operation::Update, json!({ "status": "done" }));

    let id = engine
        .enqueue(action.clone())
        .await
        .expect("AC-032: enqueue must succeed");

    assert_eq!(id, action.id, "AC-032: enqueue must return the action id");
    assert_eq!(
        storage.len(),
        1,
        "AC-032: the action must be readable from storage after enqueue returns"
    );
    let pending = storage
        .pending()
        .await
        .expect("AC-032: pending actions must be readable");
    assert_eq!(pending.len(), 1, "AC-032: exactly one action must be pending");
    assert_eq!(pending[0].id, action.id, "AC-032: the stored action must be the enqueued one");
    assert_eq!(
        pending[0].payload, action.payload,
        "AC-032: the stored payload must match the enqueued payload"
    );
}

/// AC-033: pending actions are listed in enqueue order.
#[tokio::test]
async fn ac_033_pending_actions_follow_enqueue_order() {
    let (engine, storage, _transport) = engine();
    let mut enqueued = Vec::new();

    for index in 0..4 {
        let action = engine
            .enqueue_new(
                "work_order",
                Operation::Create,
                json!({ "sequence": index }),
            )
            .await
            .expect("AC-033: enqueue must succeed");
        enqueued.push(action.id);
    }

    let pending: Vec<ActionId> = storage
        .pending()
        .await
        .expect("AC-033: pending actions must be readable")
        .into_iter()
        .map(|action| action.id)
        .collect();

    assert_eq!(
        pending, enqueued,
        "AC-033: pending actions must be returned in enqueue order"
    );
}

/// AC-034: a storage write failure returns a typed error and no success.
#[tokio::test]
async fn ac_034_storage_write_failure_returns_a_typed_error() {
    struct FailingStorage {
        append_calls: AtomicUsize,
    }

    #[async_trait]
    impl Storage for FailingStorage {
        async fn append(&self, _action: Action) -> Result<(), SyncError> {
            self.append_calls.fetch_add(1, Ordering::SeqCst);
            Err(SyncError::Storage("disk is full".to_owned()))
        }

        async fn pending(&self) -> Result<Vec<Action>, SyncError> {
            Ok(Vec::new())
        }

        async fn mark_sent(&self, _id: ActionId) -> Result<(), SyncError> {
            Ok(())
        }

        async fn mark_failed(&self, _id: ActionId) -> Result<(), SyncError> {
            Ok(())
        }

        async fn record_attempt(&self, _id: ActionId, _attempts: u32) -> Result<(), SyncError> {
            Ok(())
        }
    }

    let storage = Arc::new(FailingStorage {
        append_calls: AtomicUsize::new(0),
    });
    let engine = SyncEngine::new(storage.clone(), Arc::new(MockTransport::new()));
    let action = Action::new("work_order", Operation::Create, json!({ "status": "new" }));

    let error = engine
        .enqueue(action)
        .await
        .expect_err("AC-034: a storage write failure must not report success");

    assert!(
        matches!(error, SyncError::Storage(_)),
        "AC-034: the failure must be a typed storage error, got {error:?}"
    );
    assert!(
        error.to_string().starts_with("SF-STORAGE"),
        "AC-034: the error must render its stable code, got {error}"
    );
    assert_eq!(
        storage.append_calls.load(Ordering::SeqCst),
        1,
        "AC-034: the engine must have attempted exactly one append"
    );
}

/// AC-035: generated ids are UUID v7 with a monotonic timestamp component.
#[test]
fn ac_035_generated_action_ids_are_monotonic_uuid_v7() {
    let ids: Vec<ActionId> = (0..64).map(|_| ActionId::generate()).collect();

    for id in &ids {
        assert_eq!(
            id.as_uuid().get_version(),
            Some(Version::SortRand),
            "AC-035: every generated action id must be a UUID v7, got {id}"
        );
    }
    for pair in ids.windows(2) {
        assert!(
            pair[1].timestamp_millis() >= pair[0].timestamp_millis(),
            "AC-035: the UUID v7 timestamp component must be monotonic within the process"
        );
    }

    let created = Action::new("work_order", Operation::Create, json!({}));
    assert_eq!(
        created.id.as_uuid().get_version(),
        Some(Version::SortRand),
        "AC-035: an action created without an explicit id must receive a UUID v7"
    );
}

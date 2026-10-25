//! AC-036 through AC-040: per-action replay results, retries, conflicts,
//! dead-lettering, and deterministic double replay.

use std::sync::Arc;

use chrono::Utc;
use serde_json::json;
use syncforge::{
    ActionResult, ActionStatus, ConflictInfo, ConflictStrategy, MemoryStorage,
    MockTransport, Operation, Resolution, Storage, SyncEngine,
};

fn engine() -> (SyncEngine, Arc<MemoryStorage>, Arc<MockTransport>) {
    let storage = Arc::new(MemoryStorage::new());
    let transport = Arc::new(MockTransport::new());
    let engine = SyncEngine::new(storage.clone(), transport.clone());
    (engine, storage, transport)
}

/// AC-036: a fully applied batch yields one Applied result per action.
#[tokio::test]
async fn ac_036_replay_marks_every_applied_action_sent() {
    let (engine, storage, transport) = engine();
    let mut actions = Vec::new();
    for index in 0..3 {
        actions.push(
            engine
                .enqueue_new("work_order", Operation::Create, json!({ "sequence": index }))
                .await
                .expect("AC-036: enqueue must succeed"),
        );
    }

    let result = engine.replay().await.expect("AC-036: replay must succeed");

    assert_eq!(result.len(), 3, "AC-036: replay must return one result per action");
    for action in &actions {
        let action_result = result
            .find(action.id)
            .unwrap_or_else(|| panic!("AC-036: action {} must have a result", action.id));
        assert_eq!(
            action_result.status,
            ActionStatus::Applied,
            "AC-036: every applied action must be reported Applied"
        );
    }
    assert!(
        storage
            .pending()
            .await
            .expect("AC-036: pending actions must be readable")
            .is_empty(),
        "AC-036: applied actions must no longer be pending"
    );
    assert_eq!(
        transport.applied_count(),
        3,
        "AC-036: the transport must apply every action exactly once"
    );
}

/// AC-037: one Retry leaves only that action pending with its attempt count.
#[tokio::test]
async fn ac_037_retry_isolated_to_the_retrying_action() {
    let (engine, storage, transport) = engine();
    let first = engine
        .enqueue_new("work_order", Operation::Create, json!({ "sequence": 1 }))
        .await
        .expect("AC-037: enqueue must succeed");
    let retrying = engine
        .enqueue_new("work_order", Operation::Update, json!({ "sequence": 2 }))
        .await
        .expect("AC-037: enqueue must succeed");
    let last = engine
        .enqueue_new("work_order", Operation::Update, json!({ "sequence": 3 }))
        .await
        .expect("AC-037: enqueue must succeed");

    transport.script_action(ActionResult::retry(retrying.id));

    let result = engine.replay().await.expect("AC-037: replay must succeed");

    assert_eq!(
        result.find(first.id).map(|entry| entry.status),
        Some(ActionStatus::Applied),
        "AC-037: actions before the retry must be applied"
    );
    assert_eq!(
        result.find(retrying.id).map(|entry| entry.status),
        Some(ActionStatus::Retry),
        "AC-037: the retrying action must be reported Retry"
    );
    assert_eq!(
        result.find(last.id).map(|entry| entry.status),
        Some(ActionStatus::Applied),
        "AC-037: actions after the retry must still be applied"
    );

    let pending = storage
        .pending()
        .await
        .expect("AC-037: pending actions must be readable");
    assert_eq!(pending.len(), 1, "AC-037: only the retrying action may stay pending");
    assert_eq!(pending[0].id, retrying.id, "AC-037: the retrying action must stay pending");
    assert_eq!(pending[0].attempts, 1, "AC-037: the attempt count must be incremented");
}

/// AC-038: a reported conflict is resolved by the configured strategy and
/// recorded in the result.
#[tokio::test]
async fn ac_038_conflict_is_resolved_and_recorded() {
    let storage = Arc::new(MemoryStorage::new());
    let transport = Arc::new(MockTransport::new());
    let engine = SyncEngine::new(storage.clone(), transport.clone())
        .with_conflict(ConflictStrategy::ClientWins);

    let first = engine
        .enqueue_new("invoice", Operation::Update, json!({ "total": 10 }))
        .await
        .expect("AC-038: enqueue must succeed");
    let conflicted = engine
        .enqueue_new("invoice", Operation::Update, json!({ "total": 20 }))
        .await
        .expect("AC-038: enqueue must succeed");

    let server_state = json!({ "total": 15 });
    transport.script_action(ActionResult::conflict(
        conflicted.id,
        ConflictInfo::new(
            conflicted.client_timestamp,
            Utc::now(),
            Some(server_state.clone()),
        ),
    ));

    let result = engine.replay().await.expect("AC-038: replay must succeed");

    let entry = result
        .find(conflicted.id)
        .expect("AC-038: the conflicted action must have a result");
    assert_eq!(
        entry.status,
        ActionStatus::Conflict,
        "AC-038: the result must record the conflict status"
    );
    let conflict = entry
        .conflict
        .as_ref()
        .expect("AC-038: the result must record ConflictInfo");
    assert_eq!(
        conflict.resolution,
        Resolution::Client,
        "AC-038: the configured strategy must be applied"
    );
    assert_eq!(
        conflict.server_state.as_ref(),
        Some(&server_state),
        "AC-038: the conflict must record the server state"
    );
    assert_eq!(
        result.find(first.id).map(|entry| entry.status),
        Some(ActionStatus::Applied),
        "AC-038: an unrelated action must still be applied"
    );
    assert_eq!(result.conflicts(), 1, "AC-038: the batch must report one conflict");
    assert!(
        storage
            .pending()
            .await
            .expect("AC-038: pending actions must be readable")
            .is_empty(),
        "AC-038: a resolved conflict must settle the action"
    );
}

/// AC-039: a poison action is dead-lettered at the ceiling and later actions
/// still replay.
#[tokio::test]
async fn ac_039_poison_action_dead_letters_without_blocking_later_actions() {
    let storage = Arc::new(MemoryStorage::new());
    let transport = Arc::new(MockTransport::new());
    let engine = SyncEngine::new(storage.clone(), transport.clone()).with_max_attempts(2);

    let poison = engine
        .enqueue_new("work_order", Operation::Update, json!({ "invalid": true }))
        .await
        .expect("AC-039: enqueue must succeed");
    let healthy = engine
        .enqueue_new("work_order", Operation::Update, json!({ "valid": true }))
        .await
        .expect("AC-039: enqueue must succeed");

    transport.script_action(ActionResult::retry(poison.id));
    transport.script_action(ActionResult::retry(poison.id));

    let first = engine.replay().await.expect("AC-039: the first replay must succeed");
    assert_eq!(
        first.find(poison.id).map(|entry| entry.status),
        Some(ActionStatus::Retry),
        "AC-039: the poison action must retry below the ceiling"
    );
    assert_eq!(
        first.find(healthy.id).map(|entry| entry.status),
        Some(ActionStatus::Applied),
        "AC-039: the later action must replay while the poison action retries"
    );

    let second = engine.replay().await.expect("AC-039: the second replay must succeed");
    assert_eq!(
        second.find(poison.id).map(|entry| entry.status),
        Some(ActionStatus::Rejected),
        "AC-039: reaching the attempt ceiling must mark the action failed"
    );
    assert!(
        storage
            .pending()
            .await
            .expect("AC-039: pending actions must be readable")
            .is_empty(),
        "AC-039: the dead-lettered action must no longer be pending"
    );
    assert!(
        !transport.applied().contains(&poison.id),
        "AC-039: a poison action must never be applied"
    );
    assert!(
        transport.applied().contains(&healthy.id),
        "AC-039: the later action must still have been applied"
    );
}

/// AC-040: replaying the same batch twice never applies an action twice.
#[tokio::test]
async fn ac_040_double_replay_reports_duplicates_and_applies_nothing_twice() {
    let (engine, _storage, transport) = engine();
    let first = engine
        .enqueue_new("work_order", Operation::Create, json!({ "sequence": 1 }))
        .await
        .expect("AC-040: enqueue must succeed");
    let second = engine
        .enqueue_new("work_order", Operation::Create, json!({ "sequence": 2 }))
        .await
        .expect("AC-040: enqueue must succeed");

    let initial = engine.replay().await.expect("AC-040: the first replay must succeed");
    assert_eq!(
        initial
            .results
            .iter()
            .filter(|result| result.status == ActionStatus::Applied)
            .count(),
        2,
        "AC-040: the first replay must apply both actions"
    );
    assert_eq!(transport.applied_count(), 2, "AC-040: the transport applied both actions");

    let redelivered_storage = Arc::new(MemoryStorage::new());
    for action in [first.clone(), second.clone()] {
        redelivered_storage
            .append(action)
            .await
            .expect("AC-040: the redelivered action must be persisted");
    }
    let redelivered_engine = SyncEngine::new(redelivered_storage.clone(), transport.clone());

    let repeated = redelivered_engine
        .replay()
        .await
        .expect("AC-040: the second replay must succeed");
    assert_eq!(repeated.len(), 2, "AC-040: the second replay must report both actions");
    for result in &repeated.results {
        assert_eq!(
            result.status,
            ActionStatus::Duplicate,
            "AC-040: a redelivered action must be reported as a duplicate"
        );
    }
    assert_eq!(
        transport.applied_count(),
        2,
        "AC-040: the second replay must not apply any action twice"
    );
    assert!(
        redelivered_storage
            .pending()
            .await
            .expect("AC-040: pending actions must be readable")
            .is_empty(),
        "AC-040: duplicate results must settle the redelivered actions"
    );
}

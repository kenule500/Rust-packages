//! AC-041 through AC-044: last-write-wins, server-wins, client-wins, and a
//! caller-supplied merger.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use chrono::{Duration, Utc};
use serde_json::{json, Value};
use syncforge::{
    Action, ActionResult, ActionStatus, ConflictInfo, ConflictStrategy, CustomMerger, MemoryStorage,
    MockTransport, Operation, Resolution, SyncEngine, SyncError,
};

fn engine_with(strategy: ConflictStrategy) -> (SyncEngine, Arc<MemoryStorage>, Arc<MockTransport>) {
    let storage = Arc::new(MemoryStorage::new());
    let transport = Arc::new(MockTransport::new());
    let engine = SyncEngine::new(storage.clone(), transport.clone()).with_conflict(strategy);
    (engine, storage, transport)
}

async fn enqueue(
    engine: &SyncEngine,
    payload: Value,
    client_timestamp: chrono::DateTime<Utc>,
) -> Action {
    let action = Action::new("invoice", Operation::Update, payload)
        .with_client_timestamp(client_timestamp);
    let id = engine
        .enqueue(action.clone())
        .await
        .expect("the conflict fixture must enqueue");
    let mut action = action;
    action.id = id;
    action
}

/// AC-041: the later of the client and server timestamps wins and the decision
/// is recorded.
#[tokio::test]
async fn ac_041_last_write_wins_picks_the_later_timestamp() {
    let (engine, _storage, transport) = engine_with(ConflictStrategy::LastWriteWins);
    let now = Utc::now();

    let server_later = enqueue(&engine, json!({ "total": 10 }), now - Duration::hours(1)).await;
    let client_later = enqueue(&engine, json!({ "total": 20 }), now + Duration::minutes(1)).await;

    let server_state = json!({ "total": 15 });
    transport.script_action(ActionResult::conflict(
        server_later.id,
        ConflictInfo::new(server_later.client_timestamp, now, Some(server_state.clone())),
    ));
    transport.script_action(ActionResult::conflict(
        client_later.id,
        ConflictInfo::new(client_later.client_timestamp, now, Some(server_state)),
    ));

    let result = engine.replay().await.expect("AC-041: replay must succeed");

    let server_entry = result
        .find(server_later.id)
        .expect("AC-041: the first conflict must have a result");
    let server_conflict = server_entry
        .conflict
        .as_ref()
        .expect("AC-041: the first conflict must be recorded");
    assert_eq!(
        server_conflict.resolution,
        Resolution::Server,
        "AC-041: the later server timestamp must win"
    );
    assert_eq!(
        server_conflict.client_timestamp, server_later.client_timestamp,
        "AC-041: the recorded decision must keep the client timestamp"
    );

    let client_entry = result
        .find(client_later.id)
        .expect("AC-041: the second conflict must have a result");
    let client_conflict = client_entry
        .conflict
        .as_ref()
        .expect("AC-041: the second conflict must be recorded");
    assert_eq!(
        client_conflict.resolution,
        Resolution::Client,
        "AC-041: the later client timestamp must win"
    );
    assert_eq!(
        result
            .results
            .iter()
            .filter(|entry| entry.status == ActionStatus::Conflict)
            .count(),
        2,
        "AC-041: both conflicts must be reported"
    );
}

/// AC-042: `ServerWins` keeps the server value unchanged.
#[tokio::test]
async fn ac_042_server_wins_selects_the_unchanged_server_value() {
    let (engine, _storage, transport) = engine_with(ConflictStrategy::ServerWins);
    let now = Utc::now();

    let action = enqueue(&engine, json!({ "total": 99 }), now + Duration::hours(1)).await;
    let server_state = json!({ "total": 42, "version": 7 });
    transport.script_action(ActionResult::conflict(
        action.id,
        ConflictInfo::new(action.client_timestamp, now, Some(server_state.clone())),
    ));

    let result = engine.replay().await.expect("AC-042: replay must succeed");
    let entry = result
        .find(action.id)
        .expect("AC-042: the conflicted action must have a result");
    let conflict = entry
        .conflict
        .as_ref()
        .expect("AC-042: the conflict must be recorded");

    assert_eq!(
        conflict.resolution,
        Resolution::Server,
        "AC-042: ServerWins must resolve to the server value even when the client is newer"
    );
    assert_eq!(
        conflict.server_state.as_ref(),
        Some(&server_state),
        "AC-042: the server value must be recorded unchanged"
    );
    assert_eq!(
        entry.status,
        ActionStatus::Conflict,
        "AC-042: the outcome must remain a conflict"
    );
}

/// AC-043: `ClientWins` selects the client payload.
#[tokio::test]
async fn ac_043_client_wins_selects_the_client_payload() {
    let (engine, _storage, transport) = engine_with(ConflictStrategy::ClientWins);
    let now = Utc::now();

    let action = enqueue(&engine, json!({ "total": 99 }), now - Duration::hours(1)).await;
    transport.script_action(ActionResult::conflict(
        action.id,
        ConflictInfo::new(action.client_timestamp, now, Some(json!({ "total": 42 }))),
    ));

    let result = engine.replay().await.expect("AC-043: replay must succeed");
    let entry = result
        .find(action.id)
        .expect("AC-043: the conflicted action must have a result");
    let conflict = entry
        .conflict
        .as_ref()
        .expect("AC-043: the conflict must be recorded");

    assert_eq!(
        conflict.resolution,
        Resolution::Client,
        "AC-043: ClientWins must select the client payload even when the server is newer"
    );
    assert_eq!(
        conflict.client_timestamp, action.client_timestamp,
        "AC-043: the client payload decision must reference the client timestamp"
    );
}

/// AC-044: a custom merger decides the resolution and the result records it.
#[tokio::test]
async fn ac_044_custom_merger_resolution_is_recorded() {
    #[derive(Clone, Default)]
    struct CountingMerger {
        calls: Arc<AtomicUsize>,
    }

    impl CustomMerger for CountingMerger {
        fn resolve(
            &self,
            _action: &Action,
            server_state: Option<&Value>,
        ) -> Result<Resolution, SyncError> {
            assert!(
                server_state.is_some(),
                "AC-044: the merger must receive the server value"
            );
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(Resolution::Merged(json!({ "total": 12, "merged": true })))
        }
    }

    let calls = Arc::new(AtomicUsize::new(0));
    let (engine, _storage, transport) = engine_with(ConflictStrategy::custom(CountingMerger {
        calls: calls.clone(),
    }));
    let now = Utc::now();

    let action = enqueue(&engine, json!({ "total": 10 }), now).await;
    transport.script_action(ActionResult::conflict(
        action.id,
        ConflictInfo::new(action.client_timestamp, now, Some(json!({ "total": 20 }))),
    ));

    let result = engine.replay().await.expect("AC-044: replay must succeed");
    let entry = result
        .find(action.id)
        .expect("AC-044: the conflicted action must have a result");
    let conflict = entry
        .conflict
        .as_ref()
        .expect("AC-044: the conflict must be recorded");

    assert_eq!(
        conflict.resolution,
        Resolution::Merged(json!({ "total": 12, "merged": true })),
        "AC-044: the caller's merged resolution must be recorded in the result"
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "AC-044: the custom merger must be called exactly once"
    );
}

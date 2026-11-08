//! Acceptance tests for syncforge 0.1.
//!
//! One test per acceptance criterion (AC-001 to AC-013). Queue behaviour is
//! driven through the public `Storage` and `Transport` traits, so a custom
//! implementation is exercised the same way a caller would use one.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::{Duration, Utc};
use serde_json::{json, Value};
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Metadata, Subscriber};

use syncforge::{
    decide, Action, ActionId, ActionResult, ActionStatus, Batch, BatchResult, ConflictInfo,
    ConflictStrategy, CustomMerger, EntityType, MemoryStorage, MockTransport, Operation,
    Resolution, Storage, SyncEngine, SyncError, TimestampVerdict, Transport,
};

fn action(entity: &str, payload: Value) -> Action {
    Action::new(entity, Operation::Update, payload)
}

/// A transport that records every batch and answers with scripted per-action
/// statuses (or a default).
struct ScriptedTransport {
    batches: Mutex<Vec<Batch>>,
    statuses: Mutex<HashMap<ActionId, ActionStatus>>,
    default: ActionStatus,
}

impl ScriptedTransport {
    fn new(default: ActionStatus) -> Self {
        Self {
            batches: Mutex::new(Vec::new()),
            statuses: Mutex::new(HashMap::new()),
            default,
        }
    }

    fn set(&self, id: ActionId, status: ActionStatus) {
        self.statuses.lock().expect("status map").insert(id, status);
    }

    fn batches(&self) -> Vec<Batch> {
        self.batches.lock().expect("batch log").clone()
    }
}

#[async_trait]
impl Transport for ScriptedTransport {
    async fn send(&self, batch: &Batch) -> Result<BatchResult, SyncError> {
        self.batches.lock().expect("batch log").push(batch.clone());
        let statuses = self.statuses.lock().expect("status map");
        let results = batch
            .actions
            .iter()
            .map(|action| {
                let status = statuses.get(&action.id).copied().unwrap_or(self.default);
                ActionResult::new(action.id, status)
            })
            .collect();
        Ok(BatchResult::from_results(results))
    }
}

/// A storage whose append always fails, for the durability half of AC-001.
struct RejectingStorage;

#[async_trait]
impl Storage for RejectingStorage {
    async fn append(&self, _action: Action) -> Result<(), SyncError> {
        Err(SyncError::Storage("the disk is full".to_owned()))
    }

    async fn pending(&self) -> Result<Vec<Action>, SyncError> {
        Ok(Vec::new())
    }

    async fn mark_sent(&self, _id: ActionId) -> Result<(), SyncError> {
        Err(SyncError::Storage("the disk is full".to_owned()))
    }

    async fn mark_failed(&self, _id: ActionId) -> Result<(), SyncError> {
        Err(SyncError::Storage("the disk is full".to_owned()))
    }

    async fn record_attempt(&self, _id: ActionId, _attempts: u32) -> Result<(), SyncError> {
        Err(SyncError::Storage("the disk is full".to_owned()))
    }
}

/// A merger that keeps the server value, for the Custom strategy.
struct ServerValueMerger;

impl CustomMerger for ServerValueMerger {
    fn resolve(
        &self,
        _action: &Action,
        server_state: Option<&Value>,
    ) -> Result<Resolution, SyncError> {
        Ok(Resolution::Merged(
            server_state.cloned().unwrap_or(Value::Null),
        ))
    }
}

fn conflict_for(action: &Action, server_after: Duration) -> ConflictInfo {
    ConflictInfo::new(
        action,
        json!({"total": 1}),
        action.client_timestamp + server_after,
    )
}

// AC-001
#[tokio::test]
async fn ac_001_enqueue_persists_before_reporting_success() {
    let storage = Arc::new(MemoryStorage::new());
    let engine = SyncEngine::new(storage.clone(), Arc::new(MockTransport::new()));

    let id = engine
        .enqueue(action("invoice", json!({"total": 10})))
        .await
        .expect("the storage accepted the action");

    let pending = storage.pending().await.expect("the queue is readable");
    assert_eq!(
        pending.len(),
        1,
        "the action is durable when enqueue returns"
    );
    assert_eq!(pending[0].id, id);

    let failing = SyncEngine::new(Arc::new(RejectingStorage), Arc::new(MockTransport::new()));
    let outcome = failing
        .enqueue(action("invoice", json!({"total": 10})))
        .await;
    assert!(
        matches!(outcome, Err(SyncError::Storage(_))),
        "a storage failure is reported, not swallowed: {outcome:?}"
    );
}

// AC-002
#[test]
fn ac_002_actions_get_a_v7_id_and_a_distinct_idempotency_key() {
    let first = action("job", json!({"n": 1}));
    let second = action("job", json!({"n": 2}));

    assert!(
        first.id.timestamp_millis() > 0,
        "the action id carries a v7 timestamp"
    );
    assert!(
        second.id.timestamp_millis() >= first.id.timestamp_millis(),
        "ids are generated in timestamp order"
    );
    assert_ne!(first.id, second.id, "every action has its own id");
    assert_ne!(
        first.idempotency_key, second.idempotency_key,
        "every action has its own idempotency key"
    );
    assert!(!first.idempotency_key.is_nil());
}

// AC-003
#[tokio::test]
async fn ac_003_replay_sends_the_idempotency_key_and_settles_duplicates() {
    let storage = Arc::new(MemoryStorage::new());
    let transport = Arc::new(ScriptedTransport::new(ActionStatus::Duplicate));
    let engine = SyncEngine::new(storage.clone(), transport.clone());

    let first = engine
        .enqueue_new("job", Operation::Update, json!({"n": 1}))
        .await
        .expect("enqueue");
    let second = engine
        .enqueue_new("job", Operation::Update, json!({"n": 2}))
        .await
        .expect("enqueue");

    let result = engine.replay().await.expect("replay");
    assert_eq!(result.len(), 2);
    assert!(
        result
            .results
            .iter()
            .all(|entry| entry.status == ActionStatus::Duplicate),
        "a duplicate is an outcome, not an error"
    );
    assert!(
        storage.pending().await.expect("queue").is_empty(),
        "a duplicate is marked sent, not retried"
    );

    let batches = transport.batches();
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].len(), 2, "both actions went in one batch");
    assert_eq!(batches[0].actions[0].id, first.id);
    assert_eq!(batches[0].actions[1].id, second.id);
    assert_eq!(
        batches[0].actions[0].idempotency_key, first.idempotency_key,
        "the wire carries the action's idempotency key"
    );

    let again = engine.replay().await.expect("second replay");
    assert!(again.is_empty(), "nothing settles twice");
    assert_eq!(transport.batches().len(), 1, "no second transport call");
}

// AC-004
#[tokio::test]
async fn ac_004_a_retryable_action_advances_alone() {
    let storage = Arc::new(MemoryStorage::new());
    let transport = Arc::new(ScriptedTransport::new(ActionStatus::Applied));
    let engine = SyncEngine::new(storage.clone(), transport.clone());

    let retrying = engine
        .enqueue_new("job", Operation::Update, json!({"n": 1}))
        .await
        .expect("enqueue");
    let applied = engine
        .enqueue_new("job", Operation::Update, json!({"n": 2}))
        .await
        .expect("enqueue");
    transport.set(retrying.id, ActionStatus::Retry);

    let result = engine.replay().await.expect("replay");
    assert_eq!(result.len(), 2);

    let retry_result = result.find(retrying.id).expect("a result per action");
    assert_eq!(retry_result.status, ActionStatus::Retry);
    assert_eq!(
        result.find(applied.id).expect("a result per action").status,
        ActionStatus::Applied
    );

    let pending = storage.pending().await.expect("queue");
    assert_eq!(pending.len(), 1, "only the retrying action stays pending");
    assert_eq!(pending[0].id, retrying.id);
    assert_eq!(pending[0].attempts, 1, "the attempt count advanced");
}

// AC-005
#[tokio::test]
async fn ac_005_a_batch_returns_one_result_per_action_in_order() {
    let storage = Arc::new(MemoryStorage::new());
    let transport = Arc::new(ScriptedTransport::new(ActionStatus::Applied));
    let engine = SyncEngine::new(storage.clone(), transport.clone());

    let applied = engine
        .enqueue_new("job", Operation::Create, json!({}))
        .await
        .unwrap();
    let duplicate = engine
        .enqueue_new("job", Operation::Update, json!({}))
        .await
        .unwrap();
    let rejected = engine
        .enqueue_new("job", Operation::Delete, json!({}))
        .await
        .unwrap();
    transport.set(duplicate.id, ActionStatus::Duplicate);
    transport.set(rejected.id, ActionStatus::Rejected);

    let result = engine.replay().await.expect("replay");

    assert_eq!(result.len(), 3, "one result per attempted action");
    let ids: Vec<ActionId> = result.results.iter().map(|entry| entry.action_id).collect();
    assert_eq!(ids, vec![applied.id, duplicate.id, rejected.id]);
    assert_eq!(result.results[0].status, ActionStatus::Applied);
    assert_eq!(result.results[1].status, ActionStatus::Duplicate);
    assert_eq!(result.results[2].status, ActionStatus::Rejected);
    assert!(
        storage.pending().await.expect("queue").is_empty(),
        "every action is settled by its own result"
    );
}

// AC-006
#[tokio::test]
async fn ac_006_batches_are_bounded_by_the_configured_size() {
    let storage = Arc::new(MemoryStorage::new());
    let transport = Arc::new(ScriptedTransport::new(ActionStatus::Applied));
    let engine = SyncEngine::new(storage.clone(), transport.clone()).with_batch_size(2);

    for index in 0..5 {
        engine
            .enqueue_new("job", Operation::Update, json!({"n": index}))
            .await
            .expect("enqueue");
    }

    let result = engine.replay().await.expect("replay");
    assert_eq!(result.len(), 5);

    let batches = transport.batches();
    assert_eq!(batches.len(), 3, "five actions at two per batch");
    let sizes: Vec<usize> = batches.iter().map(Batch::len).collect();
    assert_eq!(sizes, vec![2, 2, 1]);
    assert!(
        batches.iter().all(|batch| batch.len() <= 2),
        "no batch exceeds the configured bound"
    );
}

// AC-007
#[test]
fn ac_007_a_conflict_carries_both_sides_and_resolves_to_a_typed_value() {
    let pending = action("invoice", json!({"total": 2}));
    let server_after = Duration::seconds(30);
    let mut conflict = conflict_for(&pending, server_after);

    assert_eq!(conflict.entity, EntityType::new("invoice"));
    assert_eq!(conflict.client_value, json!({"total": 2}));
    assert_eq!(conflict.server_value, json!({"total": 1}));
    assert_eq!(
        conflict.server_timestamp,
        pending.client_timestamp + server_after
    );
    assert_eq!(conflict.client_timestamp, pending.client_timestamp);

    let decision = ConflictStrategy::ClientWins
        .resolve(&pending, &mut conflict, Duration::minutes(5))
        .expect("the conflict resolves");
    assert_eq!(conflict.resolution, Resolution::Client);
    assert!(decision.within_tolerance);
}

// AC-008
#[test]
fn ac_008_every_strategy_resolves_including_a_custom_merger() {
    let pending = action("invoice", json!({"total": 2}));

    let mut conflict = conflict_for(&pending, Duration::seconds(30));
    ConflictStrategy::ServerWins
        .resolve(&pending, &mut conflict, Duration::minutes(5))
        .expect("resolve");
    assert_eq!(conflict.resolution, Resolution::Server);

    let mut conflict = conflict_for(&pending, Duration::seconds(30));
    ConflictStrategy::ClientWins
        .resolve(&pending, &mut conflict, Duration::minutes(5))
        .expect("resolve");
    assert_eq!(conflict.resolution, Resolution::Client);

    // The client edited five minutes after the server value: the client wins.
    let mut conflict = conflict_for(&pending, Duration::seconds(-300));
    ConflictStrategy::LastWriteWins
        .resolve(&pending, &mut conflict, Duration::minutes(5))
        .expect("resolve");
    assert_eq!(conflict.resolution, Resolution::Client);

    // The server value is newer, so LastWriteWins keeps it.
    let mut conflict = conflict_for(&pending, Duration::seconds(300));
    ConflictStrategy::LastWriteWins
        .resolve(&pending, &mut conflict, Duration::minutes(5))
        .expect("resolve");
    assert_eq!(conflict.resolution, Resolution::Server);

    let mut conflict = conflict_for(&pending, Duration::seconds(30));
    ConflictStrategy::custom(ServerValueMerger)
        .resolve(&pending, &mut conflict, Duration::minutes(5))
        .expect("resolve");
    assert_eq!(conflict.resolution, Resolution::Merged(json!({"total": 1})));
}

// AC-009
#[test]
fn ac_009_timestamp_decisions_accept_clamp_or_reject() {
    let client = Utc::now();
    let server = client + Duration::seconds(30);

    let accepted = decide(client, server, Duration::minutes(5));
    assert_eq!(accepted.verdict, TimestampVerdict::Accepted);
    assert_eq!(accepted.client_timestamp, client);
    assert_eq!(accepted.server_timestamp, server);
    assert_eq!(accepted.effective_client_timestamp(), client);
    assert!(accepted.within_tolerance);

    let ahead_client = client + Duration::minutes(10);
    let clamped = decide(ahead_client, server, Duration::minutes(5));
    assert!(
        matches!(clamped.verdict, TimestampVerdict::Clamped { .. }),
        "a client clock that ran ahead is clamped: {:?}",
        clamped.verdict
    );
    assert_eq!(clamped.effective_client_timestamp(), server);
    assert_eq!(
        clamped.client_timestamp, ahead_client,
        "both times are recorded"
    );

    let behind_client = client - Duration::minutes(10);
    let rejected = decide(behind_client, server, Duration::minutes(5));
    assert!(
        matches!(rejected.verdict, TimestampVerdict::Rejected { .. }),
        "a client clock that ran behind is rejected: {:?}",
        rejected.verdict
    );
    assert_eq!(rejected.effective_client_timestamp(), server);
    assert_eq!(rejected.client_timestamp, behind_client);
}

// AC-010
#[tokio::test]
async fn ac_010_the_attempt_cap_marks_the_action_failed() {
    let storage = Arc::new(MemoryStorage::new());
    let transport = Arc::new(ScriptedTransport::new(ActionStatus::Retry));
    let engine = SyncEngine::new(storage.clone(), transport.clone()).with_max_attempts(2);

    let poisoned = engine
        .enqueue_new("job", Operation::Update, json!({"n": 1}))
        .await
        .expect("enqueue");

    let first = engine.replay().await.expect("first replay");
    assert_eq!(
        first.find(poisoned.id).expect("result").status,
        ActionStatus::Retry
    );

    let second = engine.replay().await.expect("second replay");
    assert_eq!(
        second.find(poisoned.id).expect("result").status,
        ActionStatus::Rejected,
        "the cap surfaces the action instead of retrying forever"
    );
    assert!(
        storage.pending().await.expect("queue").is_empty(),
        "a dead-lettered action leaves the pending queue"
    );
}

// AC-011
#[tokio::test]
async fn ac_011_caller_supplied_storage_and_transport_drive_the_same_logic() {
    let storage: Arc<dyn Storage> = Arc::new(MemoryStorage::new());
    let transport = Arc::new(ScriptedTransport::new(ActionStatus::Applied));
    let dynamic: Arc<dyn Transport> = transport.clone();
    let engine = SyncEngine::new(storage, dynamic);

    let enqueued = engine
        .enqueue_new("job", Operation::Create, json!({"n": 1}))
        .await
        .expect("enqueue");
    let result = engine.replay().await.expect("replay");
    assert_eq!(
        result.find(enqueued.id).expect("result").status,
        ActionStatus::Applied
    );
    assert_eq!(transport.batches().len(), 1);

    // The crate also ships deterministic reference implementations.
    let _reference_storage = MemoryStorage::new();
    let mock = MockTransport::new();
    let batch = Batch::new(vec![enqueued]);
    let mock_result = mock.send(&batch).await.expect("mock transport");
    assert_eq!(mock_result.results[0].status, ActionStatus::Applied);
}

// AC-012
#[test]
fn ac_012_the_core_surface_compiles_without_optional_features() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest = fs::read_to_string(root.join("Cargo.toml")).expect("manifest is readable");
    let features = manifest
        .split_once("[features]")
        .expect("the manifest declares features")
        .1
        .split("\n[")
        .next()
        .expect("a features section");
    assert!(
        features.contains("default = []"),
        "the default feature set is empty"
    );
    let declared: Vec<&str> = features
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();
    assert_eq!(
        declared,
        vec!["default = []"],
        "no optional feature gates the core surface"
    );

    // The core types, the error type, and both test implementations exist with
    // no feature enabled; `cargo check -p syncforge --no-default-features`
    // proves the build.
    let _storage = MemoryStorage::new();
    let _transport = MockTransport::new();
    let _error = SyncError::Config("no feature is required".to_owned());
}

// AC-013
#[tokio::test]
async fn ac_013_crate_hygiene_and_tracing_events() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib = fs::read_to_string(root.join("src/lib.rs")).expect("lib.rs is readable");
    assert!(lib.contains("#![forbid(unsafe_code)]"));
    assert!(lib.contains("#![deny(missing_docs)]"));
    for licence in ["LICENSE-MIT", "LICENSE-APACHE"] {
        assert!(root.join(licence).exists(), "{licence} is shipped");
    }

    let capture = Arc::new(Mutex::new(Capture::default()));
    let _guard = tracing::subscriber::set_default(CaptureSubscriber {
        capture: Arc::clone(&capture),
    });

    let storage = Arc::new(MemoryStorage::new());
    let transport = Arc::new(MockTransport::new());
    let engine = SyncEngine::new(storage.clone(), transport.clone());

    let pending = engine
        .enqueue_new("invoice", Operation::Update, json!({"total": 2}))
        .await
        .expect("enqueue");
    let conflict = conflict_for(&pending, Duration::seconds(10));
    transport.script_action(ActionResult::conflict(pending.id, conflict));

    let result = engine.replay().await.expect("replay");
    assert_eq!(result.results[0].status, ActionStatus::Conflict);

    let events = capture.lock().expect("capture log").events.join("\n");
    assert!(events.contains("syncforge.enqueue"), "enqueue is traced");
    assert!(
        events.contains("syncforge.replay.start"),
        "replay start is traced"
    );
    assert!(
        events.contains("syncforge.replay.finish"),
        "replay finish is traced"
    );
    assert!(
        events.contains("syncforge.conflict"),
        "conflicts are counted"
    );
    assert!(events.contains("queue_depth"), "queue depth is reported");
    assert!(events.contains("duration_ms"), "sync duration is reported");
    assert!(events.contains("conflicts"), "conflict count is reported");
}

#[derive(Default)]
struct Capture {
    events: Vec<String>,
}

struct CaptureSubscriber {
    capture: Arc<Mutex<Capture>>,
}

impl CaptureSubscriber {
    fn record(&self, event: &Event<'_>) {
        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);
        self.capture
            .lock()
            .expect("capture log")
            .events
            .push(visitor.message);
    }
}

impl Subscriber for CaptureSubscriber {
    fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
        true
    }

    fn new_span(&self, _span: &Attributes<'_>) -> Id {
        Id::from_u64(1)
    }

    fn record(&self, _span: &Id, _values: &Record<'_>) {}

    fn record_follows_from(&self, _span: &Id, _follows: &Id) {}

    fn event(&self, event: &Event<'_>) {
        self.record(event);
    }

    fn enter(&self, _span: &Id) {}

    fn exit(&self, _span: &Id) {}
}

#[derive(Default)]
struct MessageVisitor {
    message: String,
}

impl Visit for MessageVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            let _ = write!(self.message, "{value:?}");
        } else {
            let _ = write!(self.message, " {}={value:?}", field.name());
        }
    }
}

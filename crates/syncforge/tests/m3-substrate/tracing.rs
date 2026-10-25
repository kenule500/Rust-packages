//! AC-052: a replay emits queue depth, outcome, conflict count, and duration
//! events.

use std::sync::{Arc, Mutex};

use chrono::Utc;
use serde_json::json;
use syncforge::{
    ActionResult, ActionStatus, ConflictInfo, ConflictStrategy, MemoryStorage, MockTransport,
    Operation, SyncEngine,
};
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
    fn find(&self, message: &str) -> Option<CapturedEvent> {
        self.events
            .lock()
            .expect("the collector lock must not be poisoned")
            .iter()
            .find(|event| event.message() == message)
            .cloned()
    }

    fn count(&self, message: &str) -> usize {
        self.events
            .lock()
            .expect("the collector lock must not be poisoned")
            .iter()
            .filter(|event| event.message() == message)
            .count()
    }

    fn names(&self) -> Vec<String> {
        self.events
            .lock()
            .expect("the collector lock must not be poisoned")
            .iter()
            .map(|event| event.message().to_owned())
            .collect()
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

fn field<'a>(event: &'a CapturedEvent, name: &str, ac: &str) -> &'a str {
    event
        .fields
        .iter()
        .find(|(field, _)| field == name)
        .map(|(_, value)| value.as_str())
        .unwrap_or_else(|| {
            panic!(
                "{ac}: event `{}` must carry the `{name}` field, got {:?}",
                event.name, event.fields
            )
        })
}

/// AC-052: a replay that resolves a conflict emits start, finish, conflict,
/// and per-action outcome events with the required data.
#[test]
fn ac_052_a_replay_emits_queue_depth_outcome_conflicts_and_duration() {
    let storage = Arc::new(MemoryStorage::new());
    let transport = Arc::new(MockTransport::new());
    let engine =
        SyncEngine::new(storage, transport.clone()).with_conflict(ConflictStrategy::ServerWins);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("AC-052: the test runtime must start");

    let collector = Collector::default();
    tracing::subscriber::with_default(collector.clone(), || {
        runtime.block_on(async {
            engine
                .enqueue_new("work_order", Operation::Create, json!({ "sequence": 1 }))
                .await
                .expect("AC-052: enqueue must succeed");
            let conflicted = engine
                .enqueue_new("work_order", Operation::Update, json!({ "sequence": 2 }))
                .await
                .expect("AC-052: enqueue must succeed");
            engine
                .enqueue_new("work_order", Operation::Update, json!({ "sequence": 3 }))
                .await
                .expect("AC-052: enqueue must succeed");
            transport.script_action(ActionResult::conflict(
                conflicted.id,
                ConflictInfo::new(conflicted.client_timestamp, Utc::now(), Some(json!({ "v": 1 }))),
            ));

            let result = engine.replay().await.expect("AC-052: replay must succeed");
            assert_eq!(
                result.find(conflicted.id).map(|entry| entry.status),
                Some(ActionStatus::Conflict),
                "AC-052: the fixture must produce a conflict"
            );
        });
    });

    let names = collector.names();
    let start = collector.find("syncforge.replay.start").unwrap_or_else(|| {
        panic!("AC-052: the replay must emit `syncforge.replay.start`, captured {names:?}")
    });
    assert_eq!(
        field(&start, "pending", "AC-052"),
        "3",
        "AC-052: the start event must report queue depth"
    );

    let finish = collector
        .find("syncforge.replay.finish")
        .unwrap_or_else(|| panic!("AC-052: the replay must emit `syncforge.replay.finish`, captured {names:?}"));
    field(&finish, "duration_ms", "AC-052");
    assert_eq!(
        field(&finish, "results", "AC-052"),
        "3",
        "AC-052: the finish event must report the replay outcome count"
    );
    assert_eq!(
        field(&finish, "conflicts", "AC-052"),
        "1",
        "AC-052: the finish event must report the conflict count"
    );
    assert_eq!(
        field(&finish, "queue_depth", "AC-052"),
        "0",
        "AC-052: the finish event must report the remaining queue depth"
    );

    let conflict = collector.find("syncforge.conflict").unwrap_or_else(|| {
        panic!("AC-052: a resolved conflict must emit `syncforge.conflict`, captured {names:?}")
    });
    assert_eq!(
        field(&conflict, "resolution", "AC-052"),
        "server",
        "AC-052: the conflict event must report the resolution"
    );

    assert_eq!(
        collector.count("syncforge.action.outcome"),
        3,
        "AC-052: every action must emit an outcome event, captured {names:?}"
    );
}

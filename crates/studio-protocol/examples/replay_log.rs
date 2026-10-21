//! Replay a short JSON message log and print the final state.
//!
//! The example touches no network and uses deterministic IDs and timestamps.

use chrono::{TimeZone, Utc};
use studio_protocol::{
    apply, Actor, Envelope, Message, MessageBudget, MessageId, OverlayCommand, OverlayId,
    Participant, ParticipantId, Rejected, Role, SceneId, ScenePatch, SceneRef, StudioState,
    TeleprompterState,
};
use uuid::Uuid;

fn id(value: u128) -> MessageId {
    MessageId::from_uuid(Uuid::from_u128(value))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let director_id = ParticipantId::from_uuid(Uuid::from_u128(1));
    let operator_id = ParticipantId::from_uuid(Uuid::from_u128(2));
    let actor = Actor::new(director_id, Role::Director);
    let mut state = StudioState::new()
        .with_participant(Participant::new(director_id, Role::Director))
        .with_participant(Participant::new(operator_id, Role::Operator));

    let scene_a = SceneRef::new(SceneId::new("scene-a")?, "Scene A");
    let scene_b = SceneRef::new(SceneId::new("scene-b")?, "Scene B");
    let scene_c = SceneRef::new(SceneId::new("scene-c")?, "Scene C");

    let mut envelopes = [
        Envelope::new(director_id, Message::Program(scene_a)),
        Envelope::new(director_id, Message::Preview(scene_b.clone())),
        Envelope::new(director_id, Message::Program(scene_c)),
        Envelope::new(
            director_id,
            Message::Overlay(OverlayCommand {
                id: OverlayId::new("lower-third")?,
                visible: true,
                text: "Live from Studio 4".to_string(),
            }),
        ),
        Envelope::new(
            director_id,
            Message::ScenePatch(ScenePatch {
                scene: scene_b,
                revision: 7,
                changes: vec!["updated camera 2 crop".to_string()],
            }),
        ),
        Envelope::new(
            director_id,
            Message::Teleprompter(TeleprompterState {
                text: "Good evening.".to_string(),
                speed: 1.5,
                running: true,
            }),
        ),
    ];
    for (index, envelope) in envelopes.iter_mut().enumerate() {
        let at = Utc
            .with_ymd_and_hms(2026, 10, 21, 8, 0, index as u32)
            .single()
            .expect("valid timestamp");
        envelope.id = id(index as u128 + 10);
        envelope.at = at;
    }

    let log: Vec<String> = envelopes
        .iter()
        .map(Envelope::to_json)
        .collect::<Result<_, _>>()?;
    for (index, json) in log.iter().enumerate() {
        let envelope = Envelope::from_json(json)?;
        state = apply(&state, &envelope, &actor)?;
        println!("applied {index}: {}", envelope.body.kind());
    }

    println!(
        "program={:?} preview={:?} overlays={} teleprompter_running={} patches={}",
        state.program.as_ref().map(|scene| scene.name.as_str()),
        state.preview.as_ref().map(|scene| scene.name.as_str()),
        state.visible_overlays().count(),
        state.teleprompter.running,
        state.patches.len()
    );

    let duplicate = Envelope::from_json(&log[0])?;
    match apply(&state, &duplicate, &actor) {
        Err(Rejected::Duplicate { .. }) => println!("duplicate rejected"),
        other => println!("unexpected duplicate result: {other:?}"),
    }

    let mut budget = MessageBudget::per_second();
    let now = envelopes[0].at;
    for _ in 0..50 {
        budget.allow(&director_id, now)?;
    }
    match budget.allow(&director_id, now) {
        Err(Rejected::RateLimited { .. }) => println!("rate limit enforced at 50 messages/second"),
        other => println!("unexpected budget result: {other:?}"),
    }
    Ok(())
}

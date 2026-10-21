//! The pure transition function.

use crate::envelope::Envelope;
use crate::error::Rejected;
use crate::message::{Message, Overlay};
use crate::permission;
use crate::state::{Actor, StudioState};
use crate::version;

/// Applies one envelope to a state and returns the next state.
///
/// The function is pure: the input state is only read, and a rejection leaves
/// every caller-visible value unchanged. The checks run in this order:
/// actor identity, protocol major, permission, payload bounds, duplicate ID, and
/// scene/overlay ordering.
pub fn apply(
    state: &StudioState,
    envelope: &Envelope,
    actor: &Actor,
) -> Result<StudioState, Rejected> {
    if envelope.from != actor.id {
        return Err(Rejected::ActorMismatch {
            declared: envelope.from,
            actor: actor.id,
        });
    }
    version::negotiate(envelope.v)?;
    permission::check(actor.role, &envelope.body)?;
    envelope.body.validate()?;

    if state.seen_ids.contains(&envelope.id) {
        return Err(Rejected::Duplicate { id: envelope.id });
    }

    if envelope.body.is_ordered() {
        if let Some(previous) = state.last_at.get(&actor.id) {
            if envelope.at < *previous {
                return Err(Rejected::OutOfOrder { actor: actor.id });
            }
        }
    }

    let mut next = state.clone();
    match &envelope.body {
        Message::Preview(scene) => {
            next.preview = Some(scene.clone());
        }
        Message::Program(scene) => {
            // One tuple assignment: the previous program moves to preview while
            // the incoming scene becomes program. No state value ever holds two
            // programs or the outgoing scene on both buses.
            (next.program, next.preview) = (Some(scene.clone()), next.program.take());
        }
        Message::ScenePatch(patch) => {
            if next
                .preview
                .as_ref()
                .is_some_and(|preview| preview.scene == patch.scene.scene)
            {
                next.preview = Some(patch.scene.clone());
            }
            next.patches.push(patch.clone());
            if next.patches.len() > 64 {
                next.patches.remove(0);
            }
        }
        Message::Overlay(command) => {
            if let Some(existing) = next
                .overlays
                .iter_mut()
                .find(|overlay| overlay.id == command.id)
            {
                existing.text.clone_from(&command.text);
                existing.visible = command.visible;
            } else {
                next.overlays.push(Overlay {
                    id: command.id.clone(),
                    text: command.text.clone(),
                    visible: command.visible,
                });
                if next.overlays.len() > 32 {
                    next.overlays.remove(0);
                }
            }
        }
        Message::Role(change) => {
            let Some(participant) = next.participants.get_mut(&change.participant) else {
                return Err(Rejected::UnknownParticipant {
                    participant: change.participant,
                });
            };
            participant.role = change.role;
        }
        Message::Whisper(whisper) => {
            if !next.participants.contains_key(&whisper.to) {
                return Err(Rejected::UnknownParticipant {
                    participant: whisper.to,
                });
            }
            // Whispers are delivered by the transport, not retained in state.
        }
        Message::Teleprompter(state) => {
            next.teleprompter = state.clone();
        }
        Message::Ping(_) | Message::Pong(_) => {}
    }

    next.seen_ids.insert(envelope.id);
    match next.last_at.get_mut(&actor.id) {
        Some(previous) => {
            if envelope.at > *previous {
                *previous = envelope.at;
            }
        }
        None => {
            next.last_at.insert(actor.id, envelope.at);
        }
    }
    Ok(next)
}

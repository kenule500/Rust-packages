//! Studio state, participants, and the bounded seen-ID set.

use std::collections::{BTreeMap, VecDeque};

use chrono::{DateTime, Utc};

use crate::envelope::{MessageId, ParticipantId};
use crate::message::{Overlay, ScenePatch, SceneRef, TeleprompterState};
use crate::permission::Role;

/// The authenticated actor applying a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Actor {
    /// Participant identifier.
    pub id: ParticipantId,
    /// Server-assigned role.
    pub role: Role,
}

impl Actor {
    /// Creates an actor.
    #[must_use]
    pub fn new(id: ParticipantId, role: Role) -> Self {
        Self { id, role }
    }
}

/// One studio participant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Participant {
    /// Participant identifier.
    pub id: ParticipantId,
    /// Current role.
    pub role: Role,
}

impl Participant {
    /// Creates a participant.
    #[must_use]
    pub fn new(id: ParticipantId, role: Role) -> Self {
        Self { id, role }
    }
}

/// A fixed-capacity insertion-ordered set.
///
/// Inserting past capacity evicts the oldest ID. The bound keeps replay memory
/// flat; duplicate detection is guaranteed inside the retained window, which is
/// documented as the idempotency window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedSet<T> {
    items: VecDeque<T>,
    capacity: usize,
}

impl<T: PartialEq> BoundedSet<T> {
    /// Creates a set with the given capacity. Zero is treated as one.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            items: VecDeque::new(),
            capacity: capacity.max(1),
        }
    }

    /// Inserts a value, returning `false` when it was already present.
    pub fn insert(&mut self, value: T) -> bool {
        if self.items.contains(&value) {
            return false;
        }
        self.items.push_back(value);
        while self.items.len() > self.capacity {
            self.items.pop_front();
        }
        true
    }

    /// Returns whether the value is retained.
    #[must_use]
    pub fn contains(&self, value: &T) -> bool {
        self.items.contains(value)
    }

    /// Returns the number of retained values.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns whether the set is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Returns the capacity.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }
}

impl<T: PartialEq> Default for BoundedSet<T> {
    fn default() -> Self {
        Self::new(512)
    }
}

/// The complete studio state.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StudioState {
    /// Scene currently on program.
    pub program: Option<SceneRef>,
    /// Scene currently on preview.
    pub preview: Option<SceneRef>,
    /// Known participants.
    pub participants: BTreeMap<ParticipantId, Participant>,
    /// Visible and hidden overlays.
    pub overlays: Vec<Overlay>,
    /// Recently applied message IDs.
    pub seen_ids: BoundedSet<MessageId>,
    /// Latest teleprompter state.
    pub teleprompter: TeleprompterState,
    /// Recent scene patches, capped at 64.
    pub patches: Vec<ScenePatch>,
    /// Newest applied timestamp per actor.
    pub last_at: BTreeMap<ParticipantId, DateTime<Utc>>,
}

impl StudioState {
    /// Creates an empty state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a participant.
    #[must_use]
    pub fn with_participant(mut self, participant: Participant) -> Self {
        self.participants.insert(participant.id, participant);
        self
    }

    /// Returns a participant by ID.
    #[must_use]
    pub fn participant(&self, id: &ParticipantId) -> Option<&Participant> {
        self.participants.get(id)
    }

    /// Returns the visible overlays.
    pub fn visible_overlays(&self) -> impl Iterator<Item = &Overlay> {
        self.overlays.iter().filter(|overlay| overlay.visible)
    }
}

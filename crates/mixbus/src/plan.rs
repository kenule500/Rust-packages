//! Mix plan: pre-allocated DSP state swapped at a block boundary.

use std::fmt;

use crate::compressor::Compressor;
use crate::ducker::{Duck, DuckState};
use crate::error::MixError;
use crate::gain::db_to_linear;
use crate::gate::Gate;
use crate::loudness::{Limiter, LimiterConfig, LoudnessMeter};
use crate::meter::MeterBank;
use crate::mixer::MixerConfig;
use crate::pan::constant_power;

/// A validated bus identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BusId(String);

impl BusId {
    /// Validates and creates a bus identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, MixError> {
        let value = value.into();
        if !is_safe_identifier(&value) {
            return Err(MixError::InvalidBusId { value });
        }
        Ok(Self(value))
    }

    /// Returns the identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for BusId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A validated source identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceId(String);

impl SourceId {
    /// Validates and creates a source identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, MixError> {
        let value = value.into();
        if !is_safe_identifier(&value) {
            return Err(MixError::InvalidSourceId { value });
        }
        Ok(Self(value))
    }

    /// Returns the identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SourceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

fn is_safe_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        && value
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric())
}

/// A send from a bus to another bus.
#[derive(Debug, Clone, PartialEq)]
pub struct Send {
    /// Destination bus.
    pub bus: BusId,
    /// Send gain in dB.
    pub gain_db: f32,
}

/// One insert in a bus chain.
#[derive(Debug, Clone, PartialEq)]
pub enum Insert {
    /// Noise gate.
    Gate(crate::gate::GateConfig),
    /// Compressor.
    Compressor(crate::compressor::CompressorConfig),
    /// True-peak limiter.
    Limiter(LimiterConfig),
}

impl Insert {
    /// Returns the insert latency in frames.
    #[must_use]
    pub fn latency_frames(&self, sample_rate: u32) -> u32 {
        match self {
            Self::Limiter(config) => {
                (config.lookahead_ms * sample_rate as f32 / 1000.0).round() as u32
            }
            Self::Gate(_) | Self::Compressor(_) => 0,
        }
    }
}

/// A mixer bus.
#[derive(Debug, Clone, PartialEq)]
pub struct Bus {
    /// Bus identifier.
    pub id: BusId,
    /// Bus gain in dB.
    pub gain_db: f32,
    /// Mute switch.
    pub mute: bool,
    /// Solo switch.
    pub solo: bool,
    /// Insert chain.
    pub inserts: Vec<Insert>,
    /// Sends to other buses.
    pub sends: Vec<Send>,
}

impl Bus {
    /// Creates an empty bus.
    #[must_use]
    pub fn new(id: BusId) -> Self {
        Self {
            id,
            gain_db: 0.0,
            mute: false,
            solo: false,
            inserts: Vec::new(),
            sends: Vec::new(),
        }
    }
}

/// A mixer input source.
#[derive(Debug, Clone, PartialEq)]
pub struct Source {
    /// Source identifier.
    pub id: SourceId,
    /// Source gain in dB.
    pub gain_db: f32,
    /// Pan position in `-1.0..=1.0`.
    pub pan: f32,
    /// Optional sidechain ducker.
    pub duck: Option<Duck>,
}

impl Source {
    /// Creates a centred source at unity gain.
    #[must_use]
    pub fn new(id: SourceId) -> Self {
        Self {
            id,
            gain_db: 0.0,
            pan: 0.0,
            duck: None,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum InsertState {
    Gate(Gate),
    Compressor(Compressor),
    Limiter(Limiter),
}

impl InsertState {
    fn build(insert: &Insert, sample_rate: u32) -> Result<Self, MixError> {
        match insert {
            Insert::Gate(config) => Ok(Self::Gate(Gate::new(config, sample_rate)?)),
            Insert::Compressor(config) => {
                Ok(Self::Compressor(Compressor::new(config, sample_rate)?))
            }
            Insert::Limiter(config) => Ok(Self::Limiter(Limiter::new(config, sample_rate)?)),
        }
    }

    pub(crate) fn process(&mut self, sample: f32) -> f32 {
        match self {
            Self::Gate(gate) => gate.process(sample),
            Self::Compressor(compressor) => compressor.process(sample),
            Self::Limiter(limiter) => limiter.process(sample),
        }
    }

    pub(crate) fn reduction_db(&self) -> f32 {
        match self {
            Self::Gate(gate) => {
                if gate.is_open() {
                    0.0
                } else {
                    -120.0
                }
            }
            Self::Compressor(compressor) => -compressor.gain_reduction_db(),
            Self::Limiter(_) => 0.0,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SourceState {
    pub(crate) gain_linear: f32,
    pub(crate) pan_left: f32,
    pub(crate) pan_right: f32,
    pub(crate) detector_db: f32,
}

#[derive(Debug, Clone)]
pub(crate) struct DuckRuntime {
    pub(crate) config: Duck,
    pub(crate) state: DuckState,
    pub(crate) key: usize,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct DelayLine {
    buffer: Vec<f32>,
    write: usize,
}

impl DelayLine {
    fn new(frames: u32) -> Self {
        Self {
            buffer: vec![0.0; frames as usize * 2],
            write: 0,
        }
    }

    pub(crate) fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        if self.buffer.is_empty() {
            return (left, right);
        }
        let delayed_left = self.buffer[self.write];
        let delayed_right = self.buffer[self.write + 1];
        self.buffer[self.write] = left;
        self.buffer[self.write + 1] = right;
        self.write += 2;
        if self.write >= self.buffer.len() {
            self.write = 0;
        }
        (delayed_left, delayed_right)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct BusPlan {
    pub(crate) gain_linear: f32,
    pub(crate) mute: bool,
    pub(crate) solo: bool,
    pub(crate) inserts: Vec<InsertState>,
    pub(crate) compensation: DelayLine,
    pub(crate) send_targets: Vec<(usize, f32)>,
}

/// Pre-allocated processing state.
///
/// Build a plan on the control thread with [`MixPlan::build`], then install it
/// with [`crate::Mixer::install_plan`] between blocks.
#[derive(Debug)]
pub struct MixPlan {
    pub(crate) sample_rate: u32,
    pub(crate) max_frames: u32,
    pub(crate) latency_frames: u32,
    pub(crate) source_ids: Vec<SourceId>,
    pub(crate) bus_ids: Vec<BusId>,
    pub(crate) sources: Vec<SourceState>,
    pub(crate) ducks: Vec<Option<DuckRuntime>>,
    pub(crate) duck_linear: Vec<f32>,
    pub(crate) buses: Vec<BusPlan>,
    pub(crate) master_gain: f32,
    pub(crate) limiter: Limiter,
    pub(crate) loudness: LoudnessMeter,
    pub(crate) meter: MeterBank,
    pub(crate) source_meters: Vec<MeterBank>,
    pub(crate) bus_meters: Vec<MeterBank>,
    pub(crate) scratch_left: Vec<f32>,
    pub(crate) scratch_right: Vec<f32>,
    pub(crate) mix_left: Vec<f32>,
    pub(crate) mix_right: Vec<f32>,
}

impl MixPlan {
    /// Builds and validates a plan. All buffers are allocated here.
    pub fn build(config: &MixerConfig) -> Result<Self, MixError> {
        config.validate()?;

        let sample_rate = config.sample_rate;
        let mut source_states = Vec::with_capacity(config.sources.len());
        let source_index: Vec<(SourceId, usize)> = config
            .sources
            .iter()
            .enumerate()
            .map(|(index, source)| (source.id.clone(), index))
            .collect();
        for source in &config.sources {
            let (pan_left, pan_right) = constant_power(source.pan);
            source_states.push(SourceState {
                gain_linear: db_to_linear(source.gain_db),
                pan_left,
                pan_right,
                detector_db: -120.0,
            });
        }

        let mut ducks = Vec::with_capacity(config.sources.len());
        for source in &config.sources {
            if let Some(duck) = &source.duck {
                duck.validate()?;
                let Some((_, key)) = source_index.iter().find(|(id, _)| id == &duck.key) else {
                    return Err(MixError::InvalidDynamics {
                        detail: format!(
                            "duck key {:?} is not a configured source",
                            duck.key.to_string()
                        ),
                    });
                };
                ducks.push(Some(DuckRuntime {
                    config: duck.clone(),
                    state: DuckState::new(),
                    key: *key,
                }));
            } else {
                ducks.push(None);
            }
        }

        let bus_index: Vec<(BusId, usize)> = config
            .buses
            .iter()
            .enumerate()
            .map(|(index, bus)| (bus.id.clone(), index))
            .collect();

        let mut insert_states = Vec::with_capacity(config.buses.len());
        let mut bus_latencies = Vec::with_capacity(config.buses.len());
        for bus in &config.buses {
            let mut states = Vec::with_capacity(bus.inserts.len());
            let mut latency = 0_u32;
            for insert in &bus.inserts {
                latency = latency.saturating_add(insert.latency_frames(sample_rate));
                states.push(InsertState::build(insert, sample_rate)?);
            }
            insert_states.push(states);
            bus_latencies.push(latency);
        }
        let max_latency = bus_latencies.iter().copied().max().unwrap_or(0);

        let mut buses = Vec::with_capacity(config.buses.len());
        for (index, bus) in config.buses.iter().enumerate() {
            let mut send_targets = Vec::with_capacity(bus.sends.len());
            for send in &bus.sends {
                let Some((_, target)) = bus_index.iter().find(|(id, _)| id == &send.bus) else {
                    return Err(MixError::UnknownBus {
                        bus: send.bus.to_string(),
                    });
                };
                send_targets.push((*target, db_to_linear(send.gain_db)));
            }
            buses.push(BusPlan {
                gain_linear: db_to_linear(bus.gain_db),
                mute: bus.mute,
                solo: bus.solo,
                inserts: std::mem::take(&mut insert_states[index]),
                compensation: DelayLine::new(max_latency - bus_latencies[index]),
                send_targets,
            });
        }

        let limiter_config = LimiterConfig {
            ceiling_db: config.ceiling_db,
            lookahead_ms: 0.0,
            release_ms: 50.0,
        };

        let max_frames = config.block.frames();
        Ok(Self {
            sample_rate,
            max_frames,
            latency_frames: max_latency,
            source_ids: config
                .sources
                .iter()
                .map(|source| source.id.clone())
                .collect(),
            bus_ids: config.buses.iter().map(|bus| bus.id.clone()).collect(),
            sources: source_states,
            ducks,
            duck_linear: vec![1.0; config.sources.len()],
            buses,
            master_gain: db_to_linear(config.master_gain_db),
            limiter: Limiter::new(&limiter_config, sample_rate)?,
            loudness: LoudnessMeter::new(sample_rate),
            meter: MeterBank::new(),
            source_meters: (0..config.sources.len())
                .map(|_| MeterBank::new())
                .collect(),
            bus_meters: (0..config.buses.len()).map(|_| MeterBank::new()).collect(),
            scratch_left: vec![0.0; max_frames as usize * config.buses.len()],
            scratch_right: vec![0.0; max_frames as usize * config.buses.len()],
            mix_left: vec![0.0; max_frames as usize],
            mix_right: vec![0.0; max_frames as usize],
        })
    }

    /// Returns the plan's reported latency in frames.
    #[must_use]
    pub const fn latency_frames(&self) -> u32 {
        self.latency_frames
    }

    /// Returns the plan's sample rate.
    #[must_use]
    pub const fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Returns the source identifiers in input order.
    #[must_use]
    pub fn source_ids(&self) -> &[SourceId] {
        &self.source_ids
    }

    /// Returns the bus identifiers in plan order.
    #[must_use]
    pub fn bus_ids(&self) -> &[BusId] {
        &self.bus_ids
    }

    /// Reads the latest meter snapshot.
    #[must_use]
    pub fn meter(&self) -> crate::meter::MeterSnapshot {
        self.meter.read()
    }

    pub(crate) fn set_source_gain(&mut self, index: usize, gain_db: f32) {
        if let Some(source) = self.sources.get_mut(index) {
            source.gain_linear = db_to_linear(gain_db);
        }
    }

    pub(crate) fn set_source_pan(&mut self, index: usize, pan: f32) {
        if let Some(source) = self.sources.get_mut(index) {
            let (left, right) = constant_power(pan);
            source.pan_left = left;
            source.pan_right = right;
        }
    }

    pub(crate) fn set_bus_gain(&mut self, index: usize, gain_db: f32) {
        if let Some(bus) = self.buses.get_mut(index) {
            bus.gain_linear = db_to_linear(gain_db);
        }
    }

    pub(crate) fn set_bus_mute(&mut self, index: usize, mute: bool) {
        if let Some(bus) = self.buses.get_mut(index) {
            bus.mute = mute;
        }
    }

    pub(crate) fn set_bus_solo(&mut self, index: usize, solo: bool) {
        if let Some(bus) = self.buses.get_mut(index) {
            bus.solo = solo;
        }
    }
}

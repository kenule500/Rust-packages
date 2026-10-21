//! The mixer, its bounded control queue, and the block processing path.

use crate::block::BlockSize;
use crate::error::MixError;
use crate::gain::{linear_to_db, sanitize};
use crate::meter::{MeterFlags, MeterSnapshot};
use crate::plan::{Bus, BusId, MixPlan, Source, SourceId};

const COMMAND_CAPACITY: usize = 64;

/// Mixer construction parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct MixerConfig {
    /// Sample rate in Hz.
    pub sample_rate: u32,
    /// Maximum block size.
    pub block: BlockSize,
    /// Bus definitions.
    pub buses: Vec<Bus>,
    /// Source definitions.
    pub sources: Vec<Source>,
    /// Master gain in dB.
    pub master_gain_db: f32,
    /// Master limiter ceiling in dBTP.
    pub ceiling_db: f32,
}

impl MixerConfig {
    /// Creates and validates a configuration.
    pub fn new(
        sample_rate: u32,
        block: BlockSize,
        buses: Vec<Bus>,
        sources: Vec<Source>,
    ) -> Result<Self, MixError> {
        let config = Self {
            sample_rate,
            block,
            buses,
            sources,
            master_gain_db: 0.0,
            ceiling_db: -1.0,
        };
        config.validate()?;
        Ok(config)
    }

    /// Validates the whole graph.
    pub fn validate(&self) -> Result<(), MixError> {
        if !(8_000..=384_000).contains(&self.sample_rate) {
            return Err(MixError::InvalidSampleRate {
                sample_rate: self.sample_rate,
            });
        }
        if self.buses.is_empty() || self.sources.is_empty() {
            return Err(MixError::EmptyGraph);
        }
        check_gain(self.master_gain_db)?;
        if !self.ceiling_db.is_finite() || self.ceiling_db > 0.0 || self.ceiling_db < -120.0 {
            return Err(MixError::InvalidGain {
                gain_db: self.ceiling_db,
            });
        }

        let mut bus_ids: Vec<&BusId> = Vec::with_capacity(self.buses.len());
        for bus in &self.buses {
            if bus_ids.contains(&&bus.id) {
                return Err(MixError::DuplicateId {
                    value: bus.id.to_string(),
                });
            }
            bus_ids.push(&bus.id);
            check_gain(bus.gain_db)?;
            for insert in &bus.inserts {
                match insert {
                    crate::plan::Insert::Gate(config) => config.validate()?,
                    crate::plan::Insert::Compressor(config) => config.validate()?,
                    crate::plan::Insert::Limiter(config) => config.validate()?,
                }
            }
            for send in &bus.sends {
                check_gain(send.gain_db)?;
                let Some(target) = self
                    .buses
                    .iter()
                    .position(|candidate| candidate.id == send.bus)
                else {
                    return Err(MixError::UnknownBus {
                        bus: send.bus.to_string(),
                    });
                };
                let origin = self
                    .buses
                    .iter()
                    .position(|candidate| candidate.id == bus.id)
                    .unwrap_or(0);
                if target <= origin {
                    return Err(MixError::InvalidDynamics {
                        detail: "sends must target a bus later in the chain".to_string(),
                    });
                }
            }
        }

        let mut source_ids: Vec<&SourceId> = Vec::with_capacity(self.sources.len());
        for source in &self.sources {
            if source_ids.contains(&&source.id) {
                return Err(MixError::DuplicateId {
                    value: source.id.to_string(),
                });
            }
            source_ids.push(&source.id);
            check_gain(source.gain_db)?;
            if !source.pan.is_finite() || !(-1.0..=1.0).contains(&source.pan) {
                return Err(MixError::InvalidPan { pan: source.pan });
            }
            if let Some(duck) = &source.duck {
                duck.validate()?;
                if duck.key == source.id {
                    return Err(MixError::InvalidDynamics {
                        detail: "a source must not duck itself".to_string(),
                    });
                }
                if !self
                    .sources
                    .iter()
                    .any(|candidate| candidate.id == duck.key)
                {
                    return Err(MixError::InvalidDynamics {
                        detail: "duck key is not a configured source".to_string(),
                    });
                }
            }
        }

        Ok(())
    }
}

fn check_gain(gain_db: f32) -> Result<(), MixError> {
    if !gain_db.is_finite() || !(-120.0..=24.0).contains(&gain_db) {
        return Err(MixError::InvalidGain { gain_db });
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum Command {
    SourceGain(usize, f32),
    SourcePan(usize, f32),
    BusGain(usize, f32),
    BusMute(usize, bool),
    BusSolo(usize, bool),
}

#[derive(Debug)]
struct CommandQueue {
    slots: [Option<Command>; COMMAND_CAPACITY],
    head: usize,
    len: usize,
}

impl CommandQueue {
    fn new() -> Self {
        Self {
            slots: [None; COMMAND_CAPACITY],
            head: 0,
            len: 0,
        }
    }

    fn push(&mut self, command: Command) -> Result<(), MixError> {
        if self.len == COMMAND_CAPACITY {
            return Err(MixError::QueueFull);
        }
        let index = (self.head + self.len) % COMMAND_CAPACITY;
        self.slots[index] = Some(command);
        self.len += 1;
        Ok(())
    }

    fn pop(&mut self) -> Option<Command> {
        if self.len == 0 {
            return None;
        }
        let command = self.slots[self.head].take();
        self.head = (self.head + 1) % COMMAND_CAPACITY;
        self.len -= 1;
        command
    }
}

/// A real-time mixer driven by the caller's audio thread.
#[derive(Debug)]
pub struct Mixer {
    sample_rate: u32,
    block: BlockSize,
    buses: Vec<Bus>,
    sources: Vec<Source>,
    plan: MixPlan,
    queue: CommandQueue,
}

impl Mixer {
    /// Builds a mixer and its initial plan.
    pub fn new(config: MixerConfig) -> Result<Self, MixError> {
        let plan = MixPlan::build(&config)?;
        Ok(Self {
            sample_rate: config.sample_rate,
            block: config.block,
            buses: config.buses,
            sources: config.sources,
            plan,
            queue: CommandQueue::new(),
        })
    }

    /// Returns the sample rate.
    #[must_use]
    pub const fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Returns the configured maximum block size.
    #[must_use]
    pub const fn block(&self) -> BlockSize {
        self.block
    }

    /// Returns the buses.
    #[must_use]
    pub fn buses(&self) -> &[Bus] {
        &self.buses
    }

    /// Returns the sources.
    #[must_use]
    pub fn sources(&self) -> &[Source] {
        &self.sources
    }

    /// Returns the active plan.
    #[must_use]
    pub fn plan(&self) -> &MixPlan {
        &self.plan
    }

    /// Returns the plan's reported latency.
    #[must_use]
    pub const fn latency_frames(&self) -> u32 {
        self.plan.latency_frames()
    }

    /// Reads the master meter.
    #[must_use]
    pub fn meter(&self) -> MeterSnapshot {
        self.plan.meter()
    }

    /// Reads a per-source meter.
    #[must_use]
    pub fn source_meter(&self, index: usize) -> Option<MeterSnapshot> {
        self.plan
            .source_meters
            .get(index)
            .map(crate::meter::MeterBank::read)
    }

    /// Reads a per-bus meter.
    #[must_use]
    pub fn bus_meter(&self, index: usize) -> Option<MeterSnapshot> {
        self.plan
            .bus_meters
            .get(index)
            .map(crate::meter::MeterBank::read)
    }

    /// Installs a plan built on the control thread at a block boundary.
    pub fn install_plan(&mut self, plan: MixPlan) -> Result<(), MixError> {
        if plan.source_ids() != self.plan.source_ids() || plan.bus_ids() != self.plan.bus_ids() {
            return Err(MixError::PlanMismatch);
        }
        self.plan = plan;
        Ok(())
    }

    /// Queues a source gain change for the next block.
    pub fn set_gain(&mut self, source: &SourceId, gain_db: f32) -> Result<(), MixError> {
        check_gain(gain_db)?;
        let Some(index) = self
            .sources
            .iter()
            .position(|candidate| &candidate.id == source)
        else {
            return Err(MixError::InvalidSourceId {
                value: source.to_string(),
            });
        };
        self.queue.push(Command::SourceGain(index, gain_db))
    }

    /// Queues a source pan change for the next block.
    pub fn set_pan(&mut self, source: &SourceId, pan: f32) -> Result<(), MixError> {
        if !pan.is_finite() || !(-1.0..=1.0).contains(&pan) {
            return Err(MixError::InvalidPan { pan });
        }
        let Some(index) = self
            .sources
            .iter()
            .position(|candidate| &candidate.id == source)
        else {
            return Err(MixError::InvalidSourceId {
                value: source.to_string(),
            });
        };
        self.queue.push(Command::SourcePan(index, pan))
    }

    /// Queues a bus gain change for the next block.
    pub fn set_bus_gain(&mut self, bus: &BusId, gain_db: f32) -> Result<(), MixError> {
        check_gain(gain_db)?;
        let Some(index) = self.buses.iter().position(|candidate| &candidate.id == bus) else {
            return Err(MixError::UnknownBus {
                bus: bus.to_string(),
            });
        };
        self.queue.push(Command::BusGain(index, gain_db))
    }

    /// Queues a mute change for the next block.
    pub fn set_mute(&mut self, bus: &BusId, mute: bool) -> Result<(), MixError> {
        let Some(index) = self.buses.iter().position(|candidate| &candidate.id == bus) else {
            return Err(MixError::UnknownBus {
                bus: bus.to_string(),
            });
        };
        self.queue.push(Command::BusMute(index, mute))
    }

    /// Queues a solo change for the next block.
    pub fn set_solo(&mut self, bus: &BusId, solo: bool) -> Result<(), MixError> {
        let Some(index) = self.buses.iter().position(|candidate| &candidate.id == bus) else {
            return Err(MixError::UnknownBus {
                bus: bus.to_string(),
            });
        };
        self.queue.push(Command::BusSolo(index, solo))
    }

    /// Processes one interleaved stereo block.
    ///
    /// The input layout is source-major: for each frame, source 0's left and
    /// right samples, then source 1's, and so on. The output is stereo. The
    /// method allocates nothing, locks nothing, and performs no I/O.
    pub fn process(&mut self, input: &[f32], output: &mut [f32]) -> Result<(), MixError> {
        if output.len() % 2 != 0 {
            return Err(MixError::InvalidOutput {
                expected: output.len() + 1,
                actual: output.len(),
            });
        }
        let frames = output.len() / 2;
        if frames == 0 {
            return Err(MixError::InvalidOutput {
                expected: 2,
                actual: 0,
            });
        }
        if frames > self.plan.max_frames as usize {
            return Err(MixError::BlockTooLarge {
                max_frames: self.plan.max_frames,
                frames,
            });
        }
        let expected = frames * self.sources.len() * 2;
        if input.len() != expected {
            return Err(MixError::InvalidInput {
                expected,
                actual: input.len(),
            });
        }

        self.drain_commands();

        let mut non_finite = false;

        for index in 0..self.sources.len() {
            let mut peak = 0.0_f32;
            for frame in 0..frames {
                let offset = (frame * self.sources.len() + index) * 2;
                let (left, changed_left) = sanitize(input[offset]);
                let (right, changed_right) = sanitize(input[offset + 1]);
                non_finite |= changed_left || changed_right;
                peak = peak.max(left.abs()).max(right.abs());
            }
            self.plan.sources[index].detector_db = linear_to_db(peak);
        }

        for index in 0..self.sources.len() {
            if let Some(runtime) = self.plan.ducks[index].as_mut() {
                let key_db = self.plan.sources[runtime.key].detector_db;
                self.plan.duck_linear[index] =
                    runtime
                        .state
                        .update(key_db, &runtime.config, self.sample_rate, frames);
            } else {
                self.plan.duck_linear[index] = 1.0;
            }
        }

        let mut scratch_left = std::mem::take(&mut self.plan.scratch_left);
        let mut scratch_right = std::mem::take(&mut self.plan.scratch_right);
        let mut mix_left = std::mem::take(&mut self.plan.mix_left);
        let mut mix_right = std::mem::take(&mut self.plan.mix_right);
        scratch_left.fill(0.0);
        scratch_right.fill(0.0);
        mix_left.fill(0.0);
        mix_right.fill(0.0);

        let source_count = self.sources.len();
        let mut max_duck = 0.0_f32;
        for index in 0..source_count {
            let source = &self.plan.sources[index];
            let gain = source.gain_linear * self.plan.duck_linear[index];
            let pan_left = source.pan_left;
            let pan_right = source.pan_right;
            let mut peak = 0.0_f32;
            let mut sum = 0.0_f64;
            for frame in 0..frames {
                let offset = (frame * source_count + index) * 2;
                let (left, _) = sanitize(input[offset]);
                let (right, _) = sanitize(input[offset + 1]);
                let left = left * gain * pan_left;
                let right = right * gain * pan_right;
                scratch_left[frame] += left;
                scratch_right[frame] += right;
                peak = peak.max(left.abs()).max(right.abs());
                sum += f64::from(left) * f64::from(left) + f64::from(right) * f64::from(right);
            }
            let duck_reduction = -linear_to_db(self.plan.duck_linear[index]);
            max_duck = max_duck.max(duck_reduction);
            let snapshot = MeterSnapshot {
                peak,
                rms: rms(sum, frames * 2),
                lufs: -120.0,
                gain_reduction: duck_reduction,
                flags: MeterFlags {
                    non_finite,
                    clipped: false,
                    gate_active: false,
                },
            };
            if let Some(meter) = self.plan.source_meters.get(index) {
                meter.publish(snapshot);
            }
        }

        let any_solo = self.buses.iter().any(|bus| bus.solo);
        let mut max_reduction = 0.0_f32;
        let mut gate_active = false;
        let mut clipped = false;

        for bus_index in 0..self.buses.len() {
            let bus = &mut self.plan.buses[bus_index];
            let muted = bus.mute || (any_solo && !bus.solo);
            let gain = bus.gain_linear;
            let mut peak = 0.0_f32;
            let mut sum = 0.0_f64;
            let offset = bus_index * frames;

            for frame in 0..frames {
                let mut left = scratch_left[offset + frame];
                let mut right = scratch_right[offset + frame];
                for insert in &mut bus.inserts {
                    left = insert.process(left);
                    right = insert.process(right);
                    max_reduction = max_reduction.max(insert.reduction_db());
                    gate_active |=
                        matches!(insert, crate::plan::InsertState::Gate(gate) if gate.is_open());
                }
                left *= gain;
                right *= gain;
                if muted {
                    left = 0.0;
                    right = 0.0;
                }
                let (left, right) = bus.compensation.process(left, right);
                scratch_left[offset + frame] = left;
                scratch_right[offset + frame] = right;
                mix_left[frame] += left;
                mix_right[frame] += right;
                peak = peak.max(left.abs()).max(right.abs());
                sum += f64::from(left) * f64::from(left) + f64::from(right) * f64::from(right);
            }

            for frame in 0..frames {
                let left = scratch_left[offset + frame];
                let right = scratch_right[offset + frame];
                for (target, send_gain) in &bus.send_targets {
                    scratch_left[target * frames + frame] += left * send_gain;
                    scratch_right[target * frames + frame] += right * send_gain;
                }
            }

            let snapshot = MeterSnapshot {
                peak,
                rms: rms(sum, frames * 2),
                lufs: -120.0,
                gain_reduction: max_reduction,
                flags: MeterFlags {
                    non_finite,
                    clipped,
                    gate_active,
                },
            };
            if let Some(meter) = self.plan.bus_meters.get(bus_index) {
                meter.publish(snapshot);
            }
        }

        let ceiling = self.plan.limiter.ceiling();
        let mut peak = 0.0_f32;
        let mut sum = 0.0_f64;
        for frame in 0..frames {
            let left = mix_left[frame] * self.plan.master_gain;
            let right = mix_right[frame] * self.plan.master_gain;
            clipped |= left.abs() > ceiling || right.abs() > ceiling;
            let left = self.plan.limiter.process(left);
            let right = self.plan.limiter.process(right);
            self.plan.loudness.process(left, right);
            output[frame * 2] = left;
            output[frame * 2 + 1] = right;
            peak = peak.max(left.abs()).max(right.abs());
            sum += f64::from(left) * f64::from(left) + f64::from(right) * f64::from(right);
        }

        self.plan.scratch_left = scratch_left;
        self.plan.scratch_right = scratch_right;
        self.plan.mix_left = mix_left;
        self.plan.mix_right = mix_right;

        self.plan.meter.publish(MeterSnapshot {
            peak,
            rms: rms(sum, frames * 2),
            lufs: self.plan.loudness.lufs(),
            gain_reduction: max_reduction.max(max_duck),
            flags: MeterFlags {
                non_finite,
                clipped,
                gate_active,
            },
        });

        Ok(())
    }

    fn drain_commands(&mut self) {
        while let Some(command) = self.queue.pop() {
            match command {
                Command::SourceGain(index, gain_db) => {
                    if let Some(source) = self.sources.get_mut(index) {
                        source.gain_db = gain_db;
                    }
                    self.plan.set_source_gain(index, gain_db);
                }
                Command::SourcePan(index, pan) => {
                    if let Some(source) = self.sources.get_mut(index) {
                        source.pan = pan;
                    }
                    self.plan.set_source_pan(index, pan);
                }
                Command::BusGain(index, gain_db) => {
                    if let Some(bus) = self.buses.get_mut(index) {
                        bus.gain_db = gain_db;
                    }
                    self.plan.set_bus_gain(index, gain_db);
                }
                Command::BusMute(index, mute) => {
                    if let Some(bus) = self.buses.get_mut(index) {
                        bus.mute = mute;
                    }
                    self.plan.set_bus_mute(index, mute);
                }
                Command::BusSolo(index, solo) => {
                    if let Some(bus) = self.buses.get_mut(index) {
                        bus.solo = solo;
                    }
                    self.plan.set_bus_solo(index, solo);
                }
            }
        }
    }
}

fn rms(sum: f64, samples: usize) -> f32 {
    if samples == 0 {
        0.0
    } else {
        (sum / samples as f64).sqrt() as f32
    }
}

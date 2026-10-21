//! Lock-free meter publication.

use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

/// Per-block meter flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MeterFlags {
    /// A non-finite input sample was replaced with zero.
    pub non_finite: bool,
    /// The limiter ceiling was engaged.
    pub clipped: bool,
    /// At least one gate was open.
    pub gate_active: bool,
}

impl MeterFlags {
    fn to_bits(self) -> u32 {
        u32::from(self.non_finite)
            | (u32::from(self.clipped) << 1)
            | (u32::from(self.gate_active) << 2)
    }

    fn from_bits(bits: u32) -> Self {
        Self {
            non_finite: bits & 1 != 0,
            clipped: bits & 2 != 0,
            gate_active: bits & 4 != 0,
        }
    }
}

/// One block's meter values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeterSnapshot {
    /// Absolute peak in the block.
    pub peak: f32,
    /// Root-mean-square level in the block.
    pub rms: f32,
    /// Gated integrated loudness in LUFS.
    pub lufs: f32,
    /// Maximum gain reduction in dB, as a non-positive value.
    pub gain_reduction: f32,
    /// Flags observed in the block.
    pub flags: MeterFlags,
}

impl Default for MeterSnapshot {
    fn default() -> Self {
        Self {
            peak: 0.0,
            rms: 0.0,
            lufs: -120.0,
            gain_reduction: 0.0,
            flags: MeterFlags::default(),
        }
    }
}

/// A double-buffered, lock-free meter slot.
///
/// The audio thread calls [`MeterBank::publish`] once per block; readers call
/// [`MeterBank::read`] from any thread. No reader ever enters the audio path.
#[derive(Debug)]
pub struct MeterBank {
    index: AtomicUsize,
    peak: [AtomicU32; 2],
    rms: [AtomicU32; 2],
    lufs: [AtomicU32; 2],
    gain_reduction: [AtomicU32; 2],
    flags: [AtomicU32; 2],
}

impl Default for MeterBank {
    fn default() -> Self {
        Self::new()
    }
}

impl MeterBank {
    /// Creates an empty bank.
    #[must_use]
    pub fn new() -> Self {
        Self {
            index: AtomicUsize::new(0),
            peak: std::array::from_fn(|_| AtomicU32::new(0)),
            rms: std::array::from_fn(|_| AtomicU32::new(0)),
            lufs: std::array::from_fn(|_| AtomicU32::new((-120.0_f32).to_bits())),
            gain_reduction: std::array::from_fn(|_| AtomicU32::new(0)),
            flags: std::array::from_fn(|_| AtomicU32::new(0)),
        }
    }

    /// Publishes a block snapshot. Only one writer is permitted.
    pub fn publish(&self, snapshot: MeterSnapshot) {
        let next = 1 - self.index.load(Ordering::Relaxed);
        self.peak[next].store(snapshot.peak.to_bits(), Ordering::Relaxed);
        self.rms[next].store(snapshot.rms.to_bits(), Ordering::Relaxed);
        self.lufs[next].store(snapshot.lufs.to_bits(), Ordering::Relaxed);
        self.gain_reduction[next].store(snapshot.gain_reduction.to_bits(), Ordering::Relaxed);
        self.flags[next].store(snapshot.flags.to_bits(), Ordering::Relaxed);
        self.index.store(next, Ordering::Release);
    }

    /// Reads the most recently published snapshot.
    #[must_use]
    pub fn read(&self) -> MeterSnapshot {
        let index = self.index.load(Ordering::Acquire);
        MeterSnapshot {
            peak: f32::from_bits(self.peak[index].load(Ordering::Relaxed)),
            rms: f32::from_bits(self.rms[index].load(Ordering::Relaxed)),
            lufs: f32::from_bits(self.lufs[index].load(Ordering::Relaxed)),
            gain_reduction: f32::from_bits(self.gain_reduction[index].load(Ordering::Relaxed)),
            flags: MeterFlags::from_bits(self.flags[index].load(Ordering::Relaxed)),
        }
    }
}

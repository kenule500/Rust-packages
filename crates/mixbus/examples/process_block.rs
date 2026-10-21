//! Mix two synthetic sources and print the meters.
//!
//! The example opens no device and touches no network.

use mixbus::{BlockSize, Bus, BusId, Duck, Mixer, MixerConfig, Source, SourceId};

fn main() -> Result<(), mixbus::MixError> {
    let block = BlockSize::new(256)?;
    let bus = Bus::new(BusId::new("master")?);
    let mut music = Source::new(SourceId::new("music")?);
    music.duck = Some(Duck {
        key: SourceId::new("voice")?,
        threshold_db: -30.0,
        ratio: 6.0,
        attack_ms: 5.0,
        release_ms: 120.0,
    });
    let voice = Source::new(SourceId::new("voice")?);

    let mut mixer = Mixer::new(MixerConfig::new(
        48_000,
        block,
        vec![bus],
        vec![music, voice],
    )?)?;

    let frames = block.frames() as usize;
    let mut input = vec![0.0_f32; frames * 2 * 2];
    for frame in 0..frames {
        let voice = if frame % 64 < 32 { 0.25 } else { 0.0 };
        input[frame * 4] = 0.1;
        input[frame * 4 + 1] = 0.1;
        input[frame * 4 + 2] = voice;
        input[frame * 4 + 3] = voice;
    }
    let mut output = vec![0.0_f32; frames * 2];

    mixer.process(&input, &mut output)?;
    let meter = mixer.meter();
    println!(
        "peak={:.4} rms={:.4} lufs={:.3} gr={:.3}",
        meter.peak, meter.rms, meter.lufs, meter.gain_reduction
    );
    Ok(())
}

//! Record synthetic packets into a ring and extract a clip.
//!
//! The example writes under the system temp directory and touches no network.

use std::fs;

use replay_recorder::{
    DiskBudget, Packet, PressurePolicy, Recorder, RecorderConfig, RingSink, SessionId, Sink,
    SourceId, Track, TrackCodec, TrackId,
};

fn main() -> Result<(), replay_recorder::RecorderError> {
    let root = std::env::temp_dir().join("m4-replay-recorder-example");
    let _ = fs::remove_dir_all(&root);

    let session = SessionId::new("demo")?;
    let video_id = TrackId::new("video")?;
    let audio_id = TrackId::new("audio")?;
    let video = Track::new(video_id.clone(), SourceId::new("camera")?, TrackCodec::H264);
    let audio = Track::new(audio_id.clone(), SourceId::new("mic")?, TrackCodec::Aac);

    let mut config = RecorderConfig::new(&root, session, vec![video, audio]);
    config.sink = Sink::Ring(RingSink::new(6, 8 * 1024 * 1024));
    config.segment_duration_ms = 1_000;
    config.segment_max_bytes = 1024 * 1024;
    config.budget = DiskBudget::new(64 * 1024 * 1024, 0, PressurePolicy::StopOldest);

    let mut recorder = Recorder::open(config)?;
    recorder.start(&video_id)?;
    recorder.start(&audio_id)?;

    for index in 0..300_i64 {
        let dts = index * 40_000;
        recorder.write_packet(Packet::new(
            video_id.clone(),
            dts,
            dts,
            40_000,
            index % 25 == 0,
            vec![0xAB; 512],
        ))?;
        recorder.write_packet(Packet::new(
            audio_id.clone(),
            dts,
            dts,
            40_000,
            true,
            vec![0xCD; 128],
        ))?;
    }

    recorder.close()?;
    let clip = recorder.clip(&video_id, 3_000)?;
    println!("spool: {}", recorder.spool().root().display());
    println!(
        "clip: {} ({} ms, {} bytes, {} segments)",
        clip.path.display(),
        clip.duration_ms,
        clip.bytes,
        clip.segments
    );
    let recovery = recorder.recovery_report();
    println!(
        "recovery: finalized={} discarded={} repaired={}",
        recovery.finalized, recovery.discarded, recovery.repaired_indexes
    );
    for event in recorder.events() {
        println!("pressure: {:?}", event.action);
    }
    Ok(())
}

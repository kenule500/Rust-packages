//! Validate a destination and plan health handling without connecting.
//!
//! The example performs no DNS lookup, TCP connect, or HTTP request.

use std::time::{Duration, Instant};

use rtmp_publish::{
    BitrateHint, BreakerConfig, CircuitBreaker, Destination, DestinationId, Failover, FailureKind,
    Health, Protocol, RetryPolicy, SecretString,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let primary = Destination::new(
        DestinationId::new("primary")?,
        "Primary RTMPS",
        Protocol::Rtmps,
        "rtmps://live.example.com:443/app",
        SecretString::new("sk_live_example_key")?,
    )?
    .with_region("eu-west-1")?;

    let backup = Destination::new(
        DestinationId::new("backup")?,
        "Backup SRT",
        Protocol::Srt,
        "srt://backup.example.com:9000",
        SecretString::new("backup_example_key")?,
    )?;
    let primary = primary.with_backup(backup)?;

    println!("debug: {primary:?}");
    println!("display: {primary}");
    println!("redacted: {:?}", primary.redacted());

    let policy = RetryPolicy::new(Duration::from_millis(250), 4, Duration::from_secs(4))?;
    let health = Health::Unreachable(FailureKind::Refused);
    let plan = policy.plan(&health, 0.5);
    println!(
        "retry: retry={} delays={:?} total={:?}",
        plan.retry, plan.delays, plan.total_delay
    );

    let config = BreakerConfig::default();
    let mut breaker = CircuitBreaker::new();
    let now = Instant::now();
    for step in 0..config.failure_threshold {
        breaker.record(&health, now + Duration::from_secs(u64::from(step)), &config);
    }
    let allowed = breaker.allow(now, &config);
    println!(
        "breaker: state={:?} failures={} allow_now={}",
        breaker.state, breaker.failures, allowed
    );

    let mut failover = Failover::from_primary(&primary);
    let decision = failover.evaluate(&health, true, now, Duration::from_secs(30));
    println!(
        "failover: decision={:?} active={}",
        decision,
        failover.active_id()
    );

    let hint = BitrateHint::from_throughput(1_500_000, 8_000_000, 2_000_000)?;
    println!(
        "bitrate: observed={} target={} within={}",
        hint.observed,
        hint.target(),
        hint.is_within()
    );
    Ok(())
}

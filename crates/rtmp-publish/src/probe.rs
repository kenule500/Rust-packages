//! Shallow health probes.
//!
//! RTMP, RTMPS, and SRT use a TCP connect. WHIP uses an HTTP `HEAD` request
//! behind the optional `whip` feature. A probe is never a publish attempt and
//! never sends the stream key.
//!
//! TLS classification requires a connector that performs the handshake. The
//! built-in TCP connector only observes the TCP layer, so RTMPS reachability is
//! reported as a TCP result and `FailureKind::Tls` is reserved for injected
//! connectors.

use std::fmt;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

use crate::destination::{Destination, Protocol};
use crate::error::PublishError;

/// Why a probe failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FailureKind {
    /// The host could not be resolved.
    Dns,
    /// The connection was refused.
    Refused,
    /// The connect or HTTP request timed out.
    Timeout,
    /// The TLS handshake failed.
    Tls,
    /// The endpoint rejected authentication.
    Auth,
    /// No probe exists in this build.
    Unsupported,
    /// Any other transport failure.
    Other,
}

impl fmt::Display for FailureKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Dns => "dns-failure",
            Self::Refused => "connection-refused",
            Self::Timeout => "timeout",
            Self::Tls => "tls-failure",
            Self::Auth => "auth-rejected",
            Self::Unsupported => "unsupported",
            Self::Other => "other",
        };
        f.write_str(label)
    }
}

/// The classified probe result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Health {
    /// No probe has run.
    Unknown,
    /// The endpoint answered.
    Reachable {
        /// Observed probe latency.
        latency: Duration,
    },
    /// The endpoint answered and rejected credentials.
    AuthRejected,
    /// The endpoint did not answer.
    Unreachable(FailureKind),
}

impl Health {
    /// Returns whether the endpoint answered successfully.
    #[must_use]
    pub const fn is_reachable(&self) -> bool {
        matches!(self, Self::Reachable { .. })
    }

    /// Returns whether a retry could plausibly help.
    ///
    /// Authentication failures are never retried: the same key would fail
    /// again until configuration changes.
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        matches!(self, Self::Unreachable(kind) if !matches!(kind, FailureKind::Auth | FailureKind::Unsupported))
    }

    /// Returns the observed latency, when reachable.
    #[must_use]
    pub const fn latency(&self) -> Option<Duration> {
        match self {
            Self::Reachable { latency } => Some(*latency),
            _ => None,
        }
    }
}

/// An injected TCP connector.
///
/// Implementations return the observed latency or a classified failure. The
/// trait exists so probes are deterministic in tests and in offline examples.
pub trait Connector {
    /// Probes one host and port.
    fn connect(&self, host: &str, port: u16, timeout: Duration) -> Result<Duration, FailureKind>;
}

/// The standard library TCP connector.
#[derive(Debug, Clone, Copy, Default)]
pub struct TcpConnector;

impl Connector for TcpConnector {
    fn connect(&self, host: &str, port: u16, timeout: Duration) -> Result<Duration, FailureKind> {
        let start = Instant::now();
        let addresses: Vec<_> = (host, port)
            .to_socket_addrs()
            .map_err(|_| FailureKind::Dns)?
            .collect();
        if addresses.is_empty() {
            return Err(FailureKind::Dns);
        }
        let mut timed_out = false;
        let mut refused = false;
        for address in addresses {
            match TcpStream::connect_timeout(&address, timeout) {
                Ok(stream) => {
                    let _ = stream.set_nodelay(true);
                    return Ok(start.elapsed());
                }
                Err(error) => match error.kind() {
                    std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock => {
                        timed_out = true;
                    }
                    std::io::ErrorKind::ConnectionRefused => refused = true,
                    _ => {}
                },
            }
        }
        if timed_out {
            Err(FailureKind::Timeout)
        } else if refused {
            Err(FailureKind::Refused)
        } else {
            Err(FailureKind::Other)
        }
    }
}

/// Probe timeout configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProbeConfig {
    /// Maximum time for one probe.
    pub timeout: Duration,
}

impl Default for ProbeConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(3),
        }
    }
}

/// Probes a destination after re-validating it.
///
/// The key is never transmitted. WHIP returns
/// [`PublishError::ProbeUnsupported`] when built without the `whip` feature.
pub fn probe(
    destination: &Destination,
    config: ProbeConfig,
    connector: &dyn Connector,
) -> Result<Health, PublishError> {
    destination.validate()?;
    let Some(host) = destination.host() else {
        return Err(PublishError::MissingHost);
    };
    let port = destination.port();
    if port == 0 {
        return Err(PublishError::MissingPort);
    }

    if destination.protocol() == Protocol::Whip {
        #[cfg(feature = "whip")]
        {
            return Ok(whip_probe(destination, config.timeout, &ReqwestProbe));
        }
        #[cfg(not(feature = "whip"))]
        {
            return Err(PublishError::ProbeUnsupported { protocol: "whip" });
        }
    }

    Ok(match connector.connect(host, port, config.timeout) {
        Ok(latency) => Health::Reachable { latency },
        Err(kind) => Health::Unreachable(kind),
    })
}

/// An injected HTTP probe for deterministic WHIP checks.
#[cfg(feature = "whip")]
pub trait HttpProbe {
    /// Probes one URL and classifies the result.
    fn probe(&self, url: &url::Url, timeout: Duration) -> Health;
}

/// Probes a WHIP endpoint with an injected HTTP probe.
#[cfg(feature = "whip")]
pub fn whip_probe(destination: &Destination, timeout: Duration, probe: &dyn HttpProbe) -> Health {
    let mut url = destination.url().clone();
    if url.scheme() == "whip+https" {
        let _ = url.set_scheme("https");
    }
    probe.probe(&url, timeout)
}

/// The `reqwest` HTTP probe.
#[cfg(feature = "whip")]
#[derive(Debug, Clone, Copy, Default)]
pub struct ReqwestProbe;

#[cfg(feature = "whip")]
impl HttpProbe for ReqwestProbe {
    fn probe(&self, url: &url::Url, timeout: Duration) -> Health {
        let start = Instant::now();
        let client = match reqwest::blocking::Client::builder()
            .timeout(timeout)
            .build()
        {
            Ok(client) => client,
            Err(_) => return Health::Unreachable(FailureKind::Other),
        };
        match client.head(url.as_str()).send() {
            Ok(response) => match response.status().as_u16() {
                401 | 403 => Health::AuthRejected,
                _ => Health::Reachable {
                    latency: start.elapsed(),
                },
            },
            Err(error) => Health::Unreachable(classify_reqwest(&error)),
        }
    }
}

#[cfg(feature = "whip")]
fn classify_reqwest(error: &reqwest::Error) -> FailureKind {
    if error.is_timeout() {
        return FailureKind::Timeout;
    }
    let text = error.to_string().to_ascii_lowercase();
    if text.contains("dns") || text.contains("resolve") {
        return FailureKind::Dns;
    }
    if text.contains("certificate") || text.contains("tls") || text.contains("handshake") {
        return FailureKind::Tls;
    }
    if text.contains("refused") {
        return FailureKind::Refused;
    }
    FailureKind::Other
}

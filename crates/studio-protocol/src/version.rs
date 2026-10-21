//! Integer protocol-version negotiation.

use serde::{Deserialize, Serialize};

use crate::error::Rejected;

/// Current protocol version: major 1, minor 0.
pub const PROTOCOL_VERSION: u16 = 100;

/// The only supported major version.
pub const SUPPORTED_MAJOR: u16 = 1;

/// A parsed protocol version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Version {
    /// Major version.
    pub major: u16,
    /// Minor version.
    pub minor: u16,
}

/// Returns the major component of a version integer.
#[must_use]
pub const fn major(version: u16) -> u16 {
    version / 100
}

/// Returns the minor component of a version integer.
#[must_use]
pub const fn minor(version: u16) -> u16 {
    version % 100
}

/// Negotiates a version integer.
///
/// An unknown major is rejected. Any minor is accepted: minor releases are
/// additive, and unknown envelope fields are ignored by serde. Payload content
/// remains bounded by [`crate::Message::validate`].
pub fn negotiate(version: u16) -> Result<Version, Rejected> {
    let parsed_major = major(version);
    if parsed_major != SUPPORTED_MAJOR {
        return Err(Rejected::UnknownMajor { version });
    }
    Ok(Version {
        major: parsed_major,
        minor: minor(version),
    })
}

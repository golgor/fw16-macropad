use crate::report::Report;

/// VIA command ids this crate knows how to encode. Kept private so callers go
/// through [`Command`], which is what keeps `0x0B` unreachable.
mod id {
    pub const GET_PROTOCOL_VERSION: u8 = 0x01;
    /// VIA's reply byte for a command the firmware did not handle.
    pub const UNHANDLED: u8 = 0xFF;
}

/// A request the host can send. Add variants here as layers grow; each one owns
/// its encoding in [`Command::encode`] and its decoding in [`Command::decode`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Ask which VIA protocol version the firmware speaks.
    GetProtocolVersion,
}

/// A decoded reply, one variant per [`Command`] variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Response {
    /// Reply to [`Command::GetProtocolVersion`].
    ProtocolVersion(ProtocolVersion),
}

/// VIA protocol version as reported by the firmware (Framework's `fl16-2025`
/// branch reports 12, upstream QMK 13).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProtocolVersion(pub u16);

/// Why a reply could not be decoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ProtoError {
    /// The firmware answered `0xFF`: it does not implement this command.
    #[error("firmware does not handle command {0:?}")]
    Unhandled(Command),
    /// The reply echoes a different command id than the one sent.
    #[error("reply echoes command 0x{got:02x}, expected 0x{expected:02x}")]
    WrongEcho {
        /// Command id we sent.
        expected: u8,
        /// Command id in the reply.
        got: u8,
    },
}

impl Command {
    const fn id(self) -> u8 {
        match self {
            Self::GetProtocolVersion => id::GET_PROTOCOL_VERSION,
        }
    }

    /// Frame this command as one report.
    #[must_use]
    pub fn encode(self) -> Report {
        let mut report = Report::zeroed();
        report.0[0] = self.id();
        report
    }

    /// Decode the firmware's reply to this command.
    ///
    /// # Errors
    /// [`ProtoError`] when the reply is not a valid answer to `self`.
    pub fn decode(self, reply: &Report) -> Result<Response, ProtoError> {
        let bytes = reply.as_bytes();
        match bytes[0] {
            id::UNHANDLED => return Err(ProtoError::Unhandled(self)),
            got if got != self.id() => {
                return Err(ProtoError::WrongEcho {
                    expected: self.id(),
                    got,
                });
            }
            _ => {}
        }
        Ok(match self {
            Self::GetProtocolVersion => {
                Response::ProtocolVersion(ProtocolVersion(u16::from_be_bytes([bytes[1], bytes[2]])))
            }
        })
    }
}

impl Response {
    /// Encode a reply the way firmware would. Used by fake devices and tests so
    /// they share the one byte layout with [`Command::decode`].
    #[must_use]
    pub fn encode(self) -> Report {
        let mut report = Report::zeroed();
        match self {
            Self::ProtocolVersion(ProtocolVersion(v)) => {
                report.0[0] = Command::GetProtocolVersion.id();
                report.0[1..3].copy_from_slice(&v.to_be_bytes());
            }
        }
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_protocol_version_matches_via_bytes() {
        let mut expected = [0u8; 32];
        expected[0] = 0x01;
        assert_eq!(Command::GetProtocolVersion.encode().0, expected);
    }

    #[test]
    fn protocol_version_round_trips() {
        let reply = Response::ProtocolVersion(ProtocolVersion(0x000C)).encode();
        assert_eq!(&reply.0[..3], &[0x01, 0x00, 0x0C]);
        assert_eq!(
            Command::GetProtocolVersion.decode(&reply),
            Ok(Response::ProtocolVersion(ProtocolVersion(12)))
        );
    }

    #[test]
    fn rejects_unhandled_and_mismatched_replies() {
        let mut reply = Report::zeroed();
        reply.0[0] = 0xFF;
        assert_eq!(
            Command::GetProtocolVersion.decode(&reply),
            Err(ProtoError::Unhandled(Command::GetProtocolVersion))
        );
        reply.0[0] = 0x02;
        assert!(matches!(
            Command::GetProtocolVersion.decode(&reply),
            Err(ProtoError::WrongEcho { got: 0x02, .. })
        ));
    }
}

//! Discord voice transport: RTP framing for the UDP audio path.
//!
//! Discord's voice servers expect every UDP datagram to be an RTP packet
//! carrying one 20 ms Opus frame (RFC 3550 header, RFC 7587 payload). Only
//! the Opus payload is encrypted — the RTP header travels in the clear, so
//! the voice server can route on the SSRC and reassemble the stream.
//!
//! The RTP timestamp advances by exactly 960 samples per frame on the 48 kHz
//! Opus clock; `docs/research/lavalink-source-architecture.md` §8 records that
//! constant from the shipped reference's frame poller.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use nexora_error::NexoraResult;

use crate::filters::xsecret_crypt;

/// The RTP timestamp increment for one 20 ms Opus frame at 48 kHz.
pub const RTP_TIMESTAMP_INCREMENT: u32 = 960;

/// The RTP payload type Discord uses for Opus (RFC 7587).
pub const OPUS_PAYLOAD_TYPE: u8 = 120;

/// The length of the RTP header Discord's voice server expects.
pub const RTP_HEADER_LEN: usize = 12;

/// The UDP port Discord's voice servers receive RTP on when the endpoint
/// carries no port of its own.
pub const DEFAULT_VOICE_PORT: u16 = 40100;

/// Splits a voice endpoint into its host and UDP port.
///
/// The endpoint a client hands over is the voice server's address. Discord
/// sends it as a websocket URL (`wss://voice-eu-west-1.discord.gg`), so the
/// scheme and any path are stripped before the host is read. A bare host
/// takes the default RTP port.
pub fn parse_voice_endpoint(endpoint: &str) -> Option<(&str, u16)> {
    let endpoint = endpoint.trim();
    if endpoint.is_empty() {
        return None;
    }
    // Discord's voice endpoint carries a websocket scheme.
    let endpoint = endpoint
        .strip_prefix("wss://")
        .or_else(|| endpoint.strip_prefix("ws://"))
        .or_else(|| endpoint.strip_prefix("https://"))
        .or_else(|| endpoint.strip_prefix("http://"))
        .unwrap_or(endpoint);
    // Any path belongs to the gateway, not to the RTP target.
    let endpoint = endpoint.split(['/', '?', '#']).next().unwrap_or_default();
    if endpoint.is_empty() {
        return None;
    }
    // A bracketed IPv6 literal keeps its colons inside the brackets.
    if let Some(rest) = endpoint.strip_prefix('[') {
        let (host, tail) = rest.split_once(']')?;
        let port = tail
            .strip_prefix(':')
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(DEFAULT_VOICE_PORT);
        return Some((host, port));
    }
    match endpoint.rsplit_once(':') {
        Some((host, port)) => {
            let port = port.parse::<u16>().ok()?;
            if host.is_empty() {
                None
            } else {
                Some((host, port))
            }
        }
        None => Some((endpoint, DEFAULT_VOICE_PORT)),
    }
}

/// The RTP header of one voice frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RtpHeader {
    /// The frame's sequence number; also the XSecret nonce.
    pub sequence: u16,
    /// The 48 kHz sample clock, advancing by 960 per frame.
    pub timestamp: u32,
    /// The synchronisation source the voice server routes on.
    pub ssrc: u32,
}

impl RtpHeader {
    /// Encodes the header into its 12 wire bytes.
    pub fn encode(&self) -> [u8; RTP_HEADER_LEN] {
        let mut header = [0u8; RTP_HEADER_LEN];
        // Version 2, no padding, no extension, no CSRC list.
        header[0] = 0x80;
        // Marker bit clear, payload type 120 (Opus).
        header[1] = OPUS_PAYLOAD_TYPE;
        header[2..4].copy_from_slice(&self.sequence.to_be_bytes());
        header[4..8].copy_from_slice(&self.timestamp.to_be_bytes());
        header[8..12].copy_from_slice(&self.ssrc.to_be_bytes());
        header
    }
}

/// Derives a stable SSRC for a voice stream.
///
/// The value only has to be stable for the life of one playback task and
/// distinct between concurrent streams, so it is derived from the session
/// seed rather than drawn from a random source.
pub fn derive_ssrc(seed: &str) -> u32 {
    let mut hasher = DefaultHasher::new();
    seed.hash(&mut hasher);
    (hasher.finish() & 0xffff_ffff) as u32
}

/// Builds the UDP datagram for one voice frame.
///
/// The datagram is the RTP header followed by the XSecret-encrypted Opus
/// packet. The nonce is the frame's RTP sequence number, which is what the
/// voice server uses to derive the same keystream.
pub fn build_voice_packet(
    header: &RtpHeader,
    key: &[u8; 16],
    payload: &[u8],
) -> NexoraResult<Vec<u8>> {
    let encrypted = xsecret_crypt(key, u32::from(header.sequence), payload)?;
    let mut packet = Vec::with_capacity(RTP_HEADER_LEN + encrypted.len());
    packet.extend_from_slice(&header.encode());
    packet.extend_from_slice(&encrypted);
    Ok(packet)
}

/// The state a voice stream keeps between frames.
#[derive(Debug, Clone, Copy)]
pub struct VoiceStream {
    header: RtpHeader,
}

impl VoiceStream {
    /// Starts a stream with the given SSRC.
    pub fn new(ssrc: u32) -> Self {
        Self {
            header: RtpHeader {
                sequence: 0,
                timestamp: 0,
                ssrc,
            },
        }
    }

    /// The header of the frame that was sent last.
    pub fn header(&self) -> RtpHeader {
        self.header
    }

    /// Builds the next frame's datagram and advances the stream.
    ///
    /// The sequence number and the timestamp both advance by one frame; the
    /// timestamp advances by 960 samples, the 48 kHz clock's step for 20 ms.
    pub fn next_packet(&mut self, key: &[u8; 16], payload: &[u8]) -> NexoraResult<Vec<u8>> {
        let packet = build_voice_packet(&self.header, key, payload)?;
        self.header.sequence = self.header.sequence.wrapping_add(1);
        self.header.timestamp = self.header.timestamp.wrapping_add(RTP_TIMESTAMP_INCREMENT);
        Ok(packet)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filters::xsecret_key;

    #[test]
    fn rtp_header_encodes_the_discord_wire_layout() {
        let header = RtpHeader {
            sequence: 0x0102,
            timestamp: 0x0304_0506,
            ssrc: 0x0708_090a,
        };
        let bytes = header.encode();
        assert_eq!(bytes.len(), RTP_HEADER_LEN);
        // Version 2, no padding/extension/CSRC; payload type 120.
        assert_eq!(bytes[0], 0x80);
        assert_eq!(bytes[1], OPUS_PAYLOAD_TYPE);
        assert_eq!(&bytes[2..4], &[0x01, 0x02]);
        assert_eq!(&bytes[4..8], &[0x03, 0x04, 0x05, 0x06]);
        assert_eq!(&bytes[8..12], &[0x07, 0x08, 0x09, 0x0a]);
    }

    #[test]
    fn the_timestamp_advances_by_960_per_frame() {
        let mut stream = VoiceStream::new(1);
        let key = xsecret_key("token");
        let mut timestamps = Vec::new();
        for _ in 0..3 {
            stream.next_packet(&key, b"opus").unwrap();
            timestamps.push(stream.header().timestamp);
        }
        assert_eq!(timestamps, vec![960, 1920, 2880]);
    }

    #[test]
    fn the_sequence_number_wraps_at_16_bits() {
        let mut stream = VoiceStream::new(1);
        stream.header.sequence = u16::MAX;
        let key = xsecret_key("token");
        stream.next_packet(&key, b"opus").unwrap();
        assert_eq!(stream.header().sequence, 0);
    }

    #[test]
    fn the_datagram_is_the_header_followed_by_the_encrypted_payload() {
        let key = xsecret_key("voice-token");
        let payload = b"opus-payload";
        let header = RtpHeader {
            sequence: 7,
            timestamp: 960,
            ssrc: 42,
        };
        let packet = build_voice_packet(&header, &key, payload).unwrap();

        assert_eq!(&packet[..RTP_HEADER_LEN], &header.encode());
        let body = &packet[RTP_HEADER_LEN..];
        assert_ne!(body, payload);
        // The voice server derives the same keystream from the sequence
        // number, so the payload decrypts back.
        let decrypted = xsecret_crypt(&key, 7, body).unwrap();
        assert_eq!(decrypted, payload);
    }

    #[test]
    fn the_nonce_is_the_rtp_sequence_number() {
        let key = xsecret_key("voice-token");
        let payload = b"opus-payload";
        let mut stream = VoiceStream::new(1);
        let first = stream.next_packet(&key, payload).unwrap();
        let second = stream.next_packet(&key, payload).unwrap();
        // Same payload, different sequence, so the two datagrams differ.
        assert_ne!(first, second);
        // The second datagram's body decrypts with the second sequence.
        let decrypted = xsecret_crypt(&key, 1, &second[RTP_HEADER_LEN..]).unwrap();
        assert_eq!(decrypted, payload);
    }

    #[test]
    fn distinct_seeds_give_distinct_ssrcs() {
        assert_ne!(derive_ssrc("session-a"), derive_ssrc("session-b"));
        assert_eq!(derive_ssrc("session-a"), derive_ssrc("session-a"));
    }

    #[test]
    fn a_bare_host_takes_the_default_voice_port() {
        assert_eq!(
            parse_voice_endpoint("voice-eu-west-1.discord.gg"),
            Some(("voice-eu-west-1.discord.gg", DEFAULT_VOICE_PORT))
        );
    }

    #[test]
    fn an_explicit_port_is_kept() {
        assert_eq!(
            parse_voice_endpoint("voice.example.com:5000"),
            Some(("voice.example.com", 5000))
        );
    }

    #[test]
    fn a_bracketed_ipv6_literal_keeps_its_colons() {
        assert_eq!(
            parse_voice_endpoint("[2001:db8::1]:4040"),
            Some(("2001:db8::1", 4040))
        );
        assert_eq!(
            parse_voice_endpoint("[2001:db8::1]"),
            Some(("2001:db8::1", DEFAULT_VOICE_PORT))
        );
    }

    #[test]
    fn an_empty_or_broken_endpoint_is_rejected() {
        assert_eq!(parse_voice_endpoint(""), None);
        assert_eq!(parse_voice_endpoint("   "), None);
        assert_eq!(parse_voice_endpoint("host:not-a-port"), None);
        assert_eq!(parse_voice_endpoint(":5000"), None);
    }

    #[test]
    fn a_discord_websocket_endpoint_yields_its_host_and_port() {
        // Discord hands the voice endpoint over as a websocket URL.
        assert_eq!(
            parse_voice_endpoint("wss://voice-eu-west-1.discord.gg"),
            Some(("voice-eu-west-1.discord.gg", DEFAULT_VOICE_PORT))
        );
        assert_eq!(
            parse_voice_endpoint("wss://voice.example.com:443"),
            Some(("voice.example.com", 443))
        );
        assert_eq!(
            parse_voice_endpoint("ws://voice.example.com"),
            Some(("voice.example.com", DEFAULT_VOICE_PORT))
        );
        // A path belongs to the gateway, not to the RTP target.
        assert_eq!(
            parse_voice_endpoint("wss://voice.example.com/?v=4"),
            Some(("voice.example.com", DEFAULT_VOICE_PORT))
        );
    }
}

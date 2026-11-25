//! The Lavaplayer encoded-track format, byte for byte.
//!
//! An encoded track is a Base64 string wrapping one framed binary message:
//!
//! ```text
//! [4-byte big-endian header][payload]
//! ```
//!
//! The header packs the frame flags into its top two bits and the payload
//! length into the remaining thirty. The payload is the version-3 track-info
//! record. There is **no** end marker: `MessageOutput.finish()` writes a
//! separate zero-length header that is not part of the encoded track.
//!
//! Ground truth: `docs/research/lavalink-source-architecture.md` §6.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;

use nexora_error::NexoraError;

use super::track::V4TrackInfo;

/// The frame flag marking the payload as versioned.
const TRACK_INFO_VERSIONED: u32 = 1;

/// The payload version NEXORA writes.
const TRACK_INFO_VERSION: u8 = 3;

/// The largest payload Java's `writeUTF` can express.
const MAX_UTF_BYTES: usize = u16::MAX as usize;

/// The source names whose payloads carry a trailing probe descriptor.
const PROBING_SOURCES: [&str; 2] = ["local", "http"];

/// Encode a track into Lavalink's binary encoded-track string.
pub fn encode_binary_track(info: &V4TrackInfo) -> Result<String, NexoraError> {
    let mut payload = Vec::new();
    payload.push(TRACK_INFO_VERSION);
    write_utf(&mut payload, &info.title)?;
    write_utf(&mut payload, &info.author)?;
    payload.extend_from_slice(&(info.length as i64).to_be_bytes());
    write_utf(&mut payload, &info.identifier)?;
    payload.push(u8::from(info.is_stream));
    write_nullable_utf(&mut payload, info.uri.as_deref())?;
    write_nullable_utf(&mut payload, info.artwork_url.as_deref())?;
    write_nullable_utf(&mut payload, info.isrc.as_deref())?;
    write_utf(&mut payload, &info.source_name)?;
    // NEXORA's sources write no source-specific bytes: the identifier already
    // resolves the track, so there is no probe descriptor to carry.
    payload.extend_from_slice(&(info.position as i64).to_be_bytes());

    let mut framed = Vec::with_capacity(payload.len() + 4);
    let header = (TRACK_INFO_VERSIONED << 30) | (payload.len() as u32);
    framed.extend_from_slice(&header.to_be_bytes());
    framed.extend_from_slice(&payload);
    Ok(STANDARD.encode(framed))
}

/// Decode an encoded-track string back into track metadata.
///
/// The decoder accepts payload versions 1, 2 and 3, so tracks encoded by older
/// Lavalink releases still decode.
pub fn decode_binary_track(encoded: &str) -> Result<V4TrackInfo, NexoraError> {
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|error| NexoraError::invalid_input(format!("invalid encoded track: {error}")))?;
    if bytes.len() < 4 {
        return Err(NexoraError::invalid_input(
            "invalid encoded track: truncated frame header",
        ));
    }
    let header = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    let flags = header >> 30;
    let size = (header & 0x3FFF_FFFF) as usize;
    if size == 0 {
        return Err(NexoraError::invalid_input(
            "invalid encoded track: empty frame",
        ));
    }
    if bytes.len() < 4 + size {
        return Err(NexoraError::invalid_input(
            "invalid encoded track: truncated payload",
        ));
    }

    let mut cursor = Cursor::new(&bytes[4..4 + size]);
    let version = if flags & TRACK_INFO_VERSIONED != 0 {
        cursor.u8()?
    } else {
        1
    };
    let title = cursor.utf()?;
    let author = cursor.utf()?;
    let length = cursor.i64()?;
    let identifier = cursor.utf()?;
    let is_stream = cursor.u8()? != 0;
    let uri = if version >= 2 {
        cursor.nullable_utf()?
    } else {
        None
    };
    let artwork_url = if version >= 3 {
        cursor.nullable_utf()?
    } else {
        None
    };
    let isrc = if version >= 3 {
        cursor.nullable_utf()?
    } else {
        None
    };
    let source_name = cursor.utf()?;

    // Lavalink's probing sources append a probe descriptor before the position;
    // NEXORA's own sources write nothing there.
    let position = if cursor.remaining() == 8 {
        cursor.i64()?
    } else if PROBING_SOURCES.contains(&source_name.as_str()) {
        let _probe = cursor.utf()?;
        cursor.i64()?
    } else {
        cursor.i64()?
    };

    Ok(V4TrackInfo {
        is_seekable: !is_stream,
        author,
        length: length.max(0) as u64,
        is_stream,
        position: position.max(0) as u64,
        title,
        uri,
        artwork_url,
        isrc,
        source_name,
        identifier,
        filters: None,
    })
}

/// Writes a Java `writeUTF` string: a `u16` byte-length prefix followed by
/// modified UTF-8.
fn write_utf(output: &mut Vec<u8>, value: &str) -> Result<(), NexoraError> {
    let encoded = modified_utf8(value);
    if encoded.len() > MAX_UTF_BYTES {
        return Err(NexoraError::invalid_input(
            "encoded track field is too long",
        ));
    }
    output.extend_from_slice(&(encoded.len() as u16).to_be_bytes());
    output.extend_from_slice(&encoded);
    Ok(())
}

/// Writes Java's nullable-text form: a presence flag, then the string.
fn write_nullable_utf(output: &mut Vec<u8>, value: Option<&str>) -> Result<(), NexoraError> {
    match value {
        None => output.push(0),
        Some(text) => {
            output.push(1);
            write_utf(output, text)?;
        }
    }
    Ok(())
}

/// Java's modified UTF-8: like standard UTF-8 except that U+0000 becomes the
/// two bytes `0xC0 0x80`, and supplementary characters are written as their
/// two surrogates.
fn modified_utf8(value: &str) -> Vec<u8> {
    let mut encoded = Vec::with_capacity(value.len());
    for character in value.chars() {
        let code = character as u32;
        match code {
            0 => encoded.extend_from_slice(&[0xC0, 0x80]),
            0x01..=0x7F => encoded.push(code as u8),
            0x80..=0x07FF => {
                encoded.push(0xC0 | (code >> 6) as u8);
                encoded.push(0x80 | (code & 0x3F) as u8);
            }
            0x0800..=0xFFFF => {
                encoded.push(0xE0 | (code >> 12) as u8);
                encoded.push(0x80 | ((code >> 6) & 0x3F) as u8);
                encoded.push(0x80 | (code & 0x3F) as u8);
            }
            _ => {
                let adjusted = code - 0x1_0000;
                for surrogate in [0xD800 + (adjusted >> 10), 0xDC00 + (adjusted & 0x3FF)] {
                    encoded.push(0xE0 | (surrogate >> 12) as u8);
                    encoded.push(0x80 | ((surrogate >> 6) & 0x3F) as u8);
                    encoded.push(0x80 | (surrogate & 0x3F) as u8);
                }
            }
        }
    }
    encoded
}

/// A read cursor over one framed payload.
struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], NexoraError> {
        if self.remaining() < count {
            return Err(NexoraError::invalid_input(
                "invalid encoded track: truncated payload",
            ));
        }
        let slice = &self.bytes[self.offset..self.offset + count];
        self.offset += count;
        Ok(slice)
    }

    fn u8(&mut self) -> Result<u8, NexoraError> {
        Ok(self.take(1)?[0])
    }

    fn i64(&mut self) -> Result<i64, NexoraError> {
        let bytes = self.take(8)?;
        let mut buffer = [0u8; 8];
        buffer.copy_from_slice(bytes);
        Ok(i64::from_be_bytes(buffer))
    }

    fn utf(&mut self) -> Result<String, NexoraError> {
        let header = self.take(2)?;
        let length = u16::from_be_bytes([header[0], header[1]]) as usize;
        let bytes = self.take(length)?;
        modified_utf8_decode(bytes)
    }

    fn nullable_utf(&mut self) -> Result<Option<String>, NexoraError> {
        if self.u8()? == 0 {
            Ok(None)
        } else {
            Ok(Some(self.utf()?))
        }
    }
}

/// Decodes Java's modified UTF-8 back into a string.
fn modified_utf8_decode(bytes: &[u8]) -> Result<String, NexoraError> {
    let mut decoded = String::with_capacity(bytes.len());
    let mut index = 0usize;
    while index < bytes.len() {
        let first = bytes[index];
        match first {
            0x00 => {
                return Err(NexoraError::invalid_input(
                    "invalid encoded track: malformed string",
                ));
            }
            0x01..=0x7F => {
                decoded.push(first as char);
                index += 1;
            }
            0xC0..=0xDF => {
                let second = *bytes.get(index + 1).ok_or_else(|| {
                    NexoraError::invalid_input("invalid encoded track: malformed string")
                })?;
                if second & 0xC0 != 0x80 {
                    return Err(NexoraError::invalid_input(
                        "invalid encoded track: malformed string",
                    ));
                }
                let code = (((first & 0x1F) as u32) << 6) | (second & 0x3F) as u32;
                decoded.push(char::from_u32(code).unwrap_or('\u{FFFD}'));
                index += 2;
            }
            0xE0..=0xEF => {
                let second = *bytes.get(index + 1).ok_or_else(|| {
                    NexoraError::invalid_input("invalid encoded track: malformed string")
                })?;
                let third = *bytes.get(index + 2).ok_or_else(|| {
                    NexoraError::invalid_input("invalid encoded track: malformed string")
                })?;
                if second & 0xC0 != 0x80 || third & 0xC0 != 0x80 {
                    return Err(NexoraError::invalid_input(
                        "invalid encoded track: malformed string",
                    ));
                }
                let code = (((first & 0x0F) as u32) << 12)
                    | (((second & 0x3F) as u32) << 6)
                    | (third & 0x3F) as u32;
                index += 3;
                // A supplementary character arrives as a surrogate pair.
                if (0xD800..0xDC00).contains(&code) {
                    let low = *bytes.get(index).ok_or_else(|| {
                        NexoraError::invalid_input("invalid encoded track: malformed string")
                    })?;
                    let low_code = (((low & 0x0F) as u32) << 12)
                        | (((bytes.get(index + 1).copied().unwrap_or(0x80) & 0x3F) as u32) << 6)
                        | (bytes.get(index + 2).copied().unwrap_or(0x80) & 0x3F) as u32;
                    let combined = 0x1_0000 + ((code - 0xD800) << 10) + (low_code - 0xDC00);
                    decoded.push(char::from_u32(combined).unwrap_or('\u{FFFD}'));
                    index += 3;
                } else {
                    decoded.push(char::from_u32(code).unwrap_or('\u{FFFD}'));
                }
            }
            _ => {
                return Err(NexoraError::invalid_input(
                    "invalid encoded track: malformed string",
                ));
            }
        }
    }
    Ok(decoded)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A track encoded by Lavaplayer's own encoder, taken from the reference
    /// blob in `docs/research/lavalink-source-architecture.md` §6.4.
    ///
    /// Decoding it yields: title `TITLE`, author `AUTHOR`, length `212000`,
    /// identifier `dQw4w9WgXcQ`, uri `https://example.com/watch`, artwork
    /// `https://example.com/art.png`, isrc `ISRC123`, source `youtube`.
    const CANONICAL: &str = "QAAAewMABVRJVExFAAZBVVRIT1IAAAAAAAM8IAALZFF3NHc5V2dYY1EAAQAZaHR0cHM6Ly9leGFtcGxlLmNvbS93YXRjaAEAG2h0dHBzOi8vZXhhbXBsZS5jb20vYXJ0LnBuZwEAB0lTUkMxMjMAB3lvdXR1YmUAAAAAAAAAAA==";

    fn canonical_info() -> V4TrackInfo {
        V4TrackInfo {
            identifier: "dQw4w9WgXcQ".into(),
            is_seekable: true,
            author: "AUTHOR".into(),
            length: 212_000,
            is_stream: false,
            position: 0,
            title: "TITLE".into(),
            uri: Some("https://example.com/watch".into()),
            artwork_url: Some("https://example.com/art.png".into()),
            isrc: Some("ISRC123".into()),
            source_name: "youtube".into(),
            filters: None,
        }
    }

    #[test]
    fn encodes_the_canonical_reference_bytes() {
        let encoded = encode_binary_track(&canonical_info()).unwrap();
        assert_eq!(encoded, CANONICAL);
    }

    #[test]
    fn canonical_blob_has_the_reference_length_and_no_end_marker() {
        let bytes = STANDARD.decode(CANONICAL).unwrap();
        let header = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        assert_eq!(header, 0x4000_007B);
        let size = (header & 0x3FFF_FFFF) as usize;
        assert_eq!(size, 123);
        // The blob is exactly the header plus one payload frame: the
        // `finish()` end marker would add four more bytes.
        assert_eq!(bytes.len(), 4 + size);
    }

    #[test]
    fn decodes_the_canonical_reference_bytes() {
        let decoded = decode_binary_track(CANONICAL).unwrap();
        assert_eq!(decoded.title, "TITLE");
        assert_eq!(decoded.author, "AUTHOR");
        assert_eq!(decoded.length, 212_000);
        assert_eq!(decoded.identifier, "dQw4w9WgXcQ");
        assert!(!decoded.is_stream);
        assert_eq!(decoded.uri.as_deref(), Some("https://example.com/watch"));
        assert_eq!(
            decoded.artwork_url.as_deref(),
            Some("https://example.com/art.png")
        );
        assert_eq!(decoded.isrc.as_deref(), Some("ISRC123"));
        assert_eq!(decoded.source_name, "youtube");
        assert_eq!(decoded.position, 0);
    }

    #[test]
    fn round_trips_every_field() {
        let info = V4TrackInfo {
            identifier: "tone.wav".into(),
            is_seekable: true,
            author: "artist".into(),
            length: 1_234,
            is_stream: false,
            position: 56,
            title: "Tone".into(),
            uri: Some("file:///tone.wav".into()),
            artwork_url: Some("https://example.com/a.png".into()),
            isrc: Some("ISRC000".into()),
            source_name: "local-media".into(),
            filters: None,
        };
        let encoded = encode_binary_track(&info).unwrap();
        let decoded = decode_binary_track(&encoded).unwrap();
        assert_eq!(decoded.identifier, info.identifier);
        assert_eq!(decoded.title, info.title);
        assert_eq!(decoded.author, info.author);
        assert_eq!(decoded.length, info.length);
        assert_eq!(decoded.position, info.position);
        assert_eq!(decoded.uri, info.uri);
        assert_eq!(decoded.artwork_url, info.artwork_url);
        assert_eq!(decoded.isrc, info.isrc);
        assert_eq!(decoded.source_name, info.source_name);
    }

    #[test]
    fn nul_is_encoded_as_modified_utf8() {
        let info = V4TrackInfo {
            title: "a\u{0000}b".into(),
            ..canonical_info()
        };
        let encoded = encode_binary_track(&info).unwrap();
        let bytes = STANDARD.decode(&encoded).unwrap();
        // The title's two-byte NUL marker must appear in the payload.
        assert!(bytes.windows(2).any(|pair| pair == [0xC0, 0x80]));
        assert_eq!(decode_binary_track(&encoded).unwrap().title, "a\u{0000}b");
    }

    #[test]
    fn rejects_malformed_input() {
        assert!(decode_binary_track("not base64!!").is_err());
        assert!(decode_binary_track(&STANDARD.encode([0, 0, 0])).is_err());
        assert!(decode_binary_track(&STANDARD.encode([0, 0, 0, 0])).is_err());
        // A header promising more bytes than the blob holds.
        assert!(decode_binary_track(&STANDARD.encode([0x40, 0x00, 0x00, 0x10, 0x03])).is_err());
    }
}

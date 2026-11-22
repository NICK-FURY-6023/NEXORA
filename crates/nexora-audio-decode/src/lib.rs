use std::fs::File;
use std::path::Path;

use nexora_audio_types::{AudioCodec, AudioFormat};
use nexora_error::{NexoraError, NexoraResult};
use symphonia::core::audio::sample::Sample;
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

#[derive(Debug, Clone)]
pub struct DecodedAudio {
    pub format: AudioFormat,
    pub samples: Vec<f32>,
}

/// Decode a local media file into interleaved f32 PCM samples.
pub fn decode_file(path: &Path) -> NexoraResult<DecodedAudio> {
    let file = File::open(path)
        .map_err(|error| NexoraError::io_with_source("cannot open media file", error))?;

    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|value| value.to_str()) {
        hint.with_extension(extension);
    }

    let media_source = MediaSourceStream::new(Box::new(file), Default::default());
    let mut format = symphonia::default::get_probe()
        .probe(
            &hint,
            media_source,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|error| NexoraError::audio_with_source("format probe failed", error))?;

    let track = format
        .default_track(TrackType::Audio)
        .ok_or_else(|| NexoraError::audio("no default audio track found"))?;
    let audio_params = track
        .codec_params
        .as_ref()
        .and_then(|params| params.audio())
        .ok_or_else(|| NexoraError::audio("audio codec parameters missing"))?;
    let sample_rate = audio_params
        .sample_rate
        .ok_or_else(|| NexoraError::audio("sample rate is not set"))?;
    let channels = audio_params
        .channels
        .as_ref()
        .map(|value| value.count())
        .unwrap_or(1);
    if channels == 0 || channels > u16::MAX as usize {
        return Err(NexoraError::audio("unsupported channel count"));
    }

    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(audio_params, &AudioDecoderOptions::default())
        .map_err(|error| NexoraError::audio_with_source("decoder creation failed", error))?;
    let track_id = track.id;

    let mut samples = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            Err(SymphoniaError::ResetRequired) => {
                return Err(NexoraError::audio(
                    "format reset is not supported in Phase 2",
                ));
            }
            Err(error) => return Err(NexoraError::audio_with_source("packet read failed", error)),
        };
        if packet.track_id != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(audio_buffer) => {
                let start = samples.len();
                samples.resize(start + audio_buffer.samples_interleaved(), f32::MID);
                audio_buffer.copy_to_slice_interleaved(&mut samples[start..]);
            }
            Err(SymphoniaError::DecodeError(_)) | Err(SymphoniaError::IoError(_)) => continue,
            Err(error) => return Err(NexoraError::audio_with_source("decode failed", error)),
        }
    }

    Ok(DecodedAudio {
        format: AudioFormat::new(sample_rate, channels as u16, codec_from_path(path)),
        samples,
    })
}

fn codec_from_path(path: &Path) -> AudioCodec {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "flac" => AudioCodec::Flac,
        "mp3" => AudioCodec::Mp3,
        "ogg" | "oga" | "opus" => AudioCodec::Vorbis,
        "aac" | "m4a" => AudioCodec::Aac,
        _ => AudioCodec::Pcm,
    }
}

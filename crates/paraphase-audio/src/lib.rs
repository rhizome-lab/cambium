//! Audio format converters for Cambium
//!
//! Pure Rust audio decoding via Symphonia, WAV encoding via Hound.
//! Currently supports decoding from many formats but encoding only to WAV.

use hound::{WavSpec, WavWriter};
use rhi_paraphase_core::{
    ConvertError, ConvertOutput, Converter, ConverterDecl, Properties, PropertyPattern, Registry,
};
use std::io::Cursor;
use symphonia::core::audio::AudioBufferRef;
use symphonia::core::codecs::{CODEC_TYPE_NULL, DecoderOptions};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Audio formats we can decode from
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AudioFormat {
    Wav,
    Flac,
    Mp3,
    Ogg,
    Aac,
}

impl AudioFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            AudioFormat::Wav => "wav",
            AudioFormat::Flac => "flac",
            AudioFormat::Mp3 => "mp3",
            AudioFormat::Ogg => "ogg",
            AudioFormat::Aac => "aac",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "wav" | "wave" => Some(AudioFormat::Wav),
            "flac" => Some(AudioFormat::Flac),
            "mp3" => Some(AudioFormat::Mp3),
            "ogg" | "vorbis" => Some(AudioFormat::Ogg),
            "aac" | "m4a" => Some(AudioFormat::Aac),
            _ => None,
        }
    }

    pub fn extensions(&self) -> &'static [&'static str] {
        match self {
            AudioFormat::Wav => &["wav", "wave"],
            AudioFormat::Flac => &["flac"],
            AudioFormat::Mp3 => &["mp3"],
            AudioFormat::Ogg => &["ogg", "oga"],
            AudioFormat::Aac => &["aac", "m4a"],
        }
    }

    /// Feature flag name for this format
    pub fn feature(&self) -> &'static str {
        match self {
            AudioFormat::Wav => "wav",
            AudioFormat::Flac => "flac",
            AudioFormat::Mp3 => "mp3",
            AudioFormat::Ogg => "ogg",
            AudioFormat::Aac => "aac",
        }
    }
}

/// Decoded audio data in a common format
struct DecodedAudio {
    samples: Vec<i16>,
    channels: u16,
    sample_rate: u32,
}

/// Decode audio from any supported format
fn decode_audio(input: &[u8], hint: Option<&str>) -> Result<DecodedAudio, ConvertError> {
    let cursor = Cursor::new(input.to_vec());
    let mss = MediaSourceStream::new(Box::new(cursor), Default::default());

    let mut format_hint = Hint::new();
    if let Some(ext) = hint {
        format_hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(
            &format_hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| ConvertError::InvalidInput(format!("Failed to probe audio format: {}", e)))?;

    let mut format = probed.format;

    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| ConvertError::InvalidInput("No audio track found".into()))?;

    let codec_params = &track.codec_params;
    let channels = codec_params.channels.map(|c| c.count() as u16).unwrap_or(2);
    let sample_rate = codec_params.sample_rate.unwrap_or(44100);

    let mut decoder = symphonia::default::get_codecs()
        .make(codec_params, &DecoderOptions::default())
        .map_err(|e| ConvertError::InvalidInput(format!("Failed to create decoder: {}", e)))?;

    let track_id = track.id;
    let mut samples: Vec<i16> = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(symphonia::core::errors::Error::IoError(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(e) => {
                return Err(ConvertError::InvalidInput(format!(
                    "Failed to read packet: {}",
                    e
                )));
            }
        };

        if packet.track_id() != track_id {
            continue;
        }

        let decoded = decoder.decode(&packet).map_err(|e| {
            ConvertError::InvalidInput(format!("Failed to decode audio packet: {}", e))
        })?;

        // Convert to i16 samples
        convert_to_i16(&decoded, &mut samples);
    }

    Ok(DecodedAudio {
        samples,
        channels,
        sample_rate,
    })
}

/// Convert decoded audio buffer to i16 samples
fn convert_to_i16(buffer: &AudioBufferRef, output: &mut Vec<i16>) {
    match buffer {
        AudioBufferRef::S16(buf) => {
            for plane in buf.planes().planes() {
                output.extend_from_slice(plane);
            }
        }
        AudioBufferRef::S32(buf) => {
            for plane in buf.planes().planes() {
                for &sample in plane.iter() {
                    output.push((sample >> 16) as i16);
                }
            }
        }
        AudioBufferRef::F32(buf) => {
            for plane in buf.planes().planes() {
                for &sample in plane.iter() {
                    let clamped = sample.clamp(-1.0, 1.0);
                    output.push((clamped * i16::MAX as f32) as i16);
                }
            }
        }
        AudioBufferRef::F64(buf) => {
            for plane in buf.planes().planes() {
                for &sample in plane.iter() {
                    let clamped = sample.clamp(-1.0, 1.0);
                    output.push((clamped * i16::MAX as f64) as i16);
                }
            }
        }
        AudioBufferRef::U8(buf) => {
            for plane in buf.planes().planes() {
                for &sample in plane.iter() {
                    output.push(((sample as i16) - 128) * 256);
                }
            }
        }
        AudioBufferRef::U16(buf) => {
            for plane in buf.planes().planes() {
                for &sample in plane.iter() {
                    output.push((sample as i16).wrapping_sub(i16::MIN));
                }
            }
        }
        AudioBufferRef::U24(buf) => {
            for plane in buf.planes().planes() {
                for sample in plane.iter() {
                    let val = sample.inner();
                    output.push(((val >> 8) as i16).wrapping_sub(i16::MIN));
                }
            }
        }
        AudioBufferRef::U32(buf) => {
            for plane in buf.planes().planes() {
                for &sample in plane.iter() {
                    output.push(((sample >> 16) as i16).wrapping_sub(i16::MIN));
                }
            }
        }
        AudioBufferRef::S24(buf) => {
            for plane in buf.planes().planes() {
                for sample in plane.iter() {
                    let val = sample.inner();
                    output.push((val >> 8) as i16);
                }
            }
        }
        AudioBufferRef::S8(buf) => {
            for plane in buf.planes().planes() {
                for &sample in plane.iter() {
                    output.push((sample as i16) * 256);
                }
            }
        }
    }
}

/// Encode audio to WAV format
fn encode_wav(audio: &DecodedAudio) -> Result<Vec<u8>, ConvertError> {
    let spec = WavSpec {
        channels: audio.channels,
        sample_rate: audio.sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut buffer = Vec::new();
    {
        let cursor = Cursor::new(&mut buffer);
        let mut writer = WavWriter::new(cursor, spec).map_err(|e| {
            ConvertError::InvalidInput(format!("Failed to create WAV writer: {}", e))
        })?;

        for &sample in &audio.samples {
            writer.write_sample(sample).map_err(|e| {
                ConvertError::InvalidInput(format!("Failed to write WAV sample: {}", e))
            })?;
        }

        writer
            .finalize()
            .map_err(|e| ConvertError::InvalidInput(format!("Failed to finalize WAV: {}", e)))?;
    }

    Ok(buffer)
}

/// Audio converter (any supported format -> WAV)
pub struct AudioToWavConverter {
    decl: ConverterDecl,
    from: AudioFormat,
}

impl AudioToWavConverter {
    pub fn new(from: AudioFormat) -> Self {
        let name = format!("audio.{}-to-wav", from.as_str());

        let decl = ConverterDecl::simple(
            &name,
            PropertyPattern::new().eq("format", from.as_str()),
            PropertyPattern::new().eq("format", "wav"),
        )
        .description(format!("Convert {} to WAV", from.as_str().to_uppercase()));

        Self { decl, from }
    }
}

impl Converter for AudioToWavConverter {
    fn decl(&self) -> &ConverterDecl {
        &self.decl
    }

    fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
        let audio = decode_audio(input, Some(self.from.as_str()))?;

        let output = encode_wav(&audio)?;

        let mut out_props = props.clone();
        out_props.insert("format".into(), "wav".into());
        out_props.insert("channels".into(), (audio.channels as i64).into());
        out_props.insert("sample_rate".into(), (audio.sample_rate as i64).into());
        out_props.insert("bits_per_sample".into(), 16i64.into());

        Ok(ConvertOutput::Single(output, out_props))
    }
}

/// WAV to WAV converter (for resampling/channel conversion in future)
pub struct WavPassthroughConverter {
    decl: ConverterDecl,
}

impl WavPassthroughConverter {
    pub fn new() -> Self {
        let decl = ConverterDecl::simple(
            "audio.wav-to-wav",
            PropertyPattern::new().eq("format", "wav"),
            PropertyPattern::new().eq("format", "wav"),
        )
        .description("Re-encode WAV (normalize format)");

        Self { decl }
    }
}

impl Default for WavPassthroughConverter {
    fn default() -> Self {
        Self::new()
    }
}

impl Converter for WavPassthroughConverter {
    fn decl(&self) -> &ConverterDecl {
        &self.decl
    }

    fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
        let audio = decode_audio(input, Some("wav"))?;
        let output = encode_wav(&audio)?;

        let mut out_props = props.clone();
        out_props.insert("format".into(), "wav".into());
        out_props.insert("channels".into(), (audio.channels as i64).into());
        out_props.insert("sample_rate".into(), (audio.sample_rate as i64).into());
        out_props.insert("bits_per_sample".into(), 16i64.into());

        Ok(ConvertOutput::Single(output, out_props))
    }
}

/// Register all audio converters
pub fn register_all(registry: &mut Registry) {
    // X -> WAV converters
    #[cfg(feature = "flac")]
    registry.register(AudioToWavConverter::new(AudioFormat::Flac));

    #[cfg(feature = "mp3")]
    registry.register(AudioToWavConverter::new(AudioFormat::Mp3));

    #[cfg(feature = "ogg")]
    registry.register(AudioToWavConverter::new(AudioFormat::Ogg));

    #[cfg(feature = "aac")]
    registry.register(AudioToWavConverter::new(AudioFormat::Aac));

    // WAV passthrough (always available with wav feature)
    #[cfg(feature = "wav")]
    registry.register(WavPassthroughConverter::new());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_format_parsing() {
        assert_eq!(AudioFormat::parse("wav"), Some(AudioFormat::Wav));
        assert_eq!(AudioFormat::parse("WAV"), Some(AudioFormat::Wav));
        assert_eq!(AudioFormat::parse("mp3"), Some(AudioFormat::Mp3));
        assert_eq!(AudioFormat::parse("flac"), Some(AudioFormat::Flac));
        assert_eq!(AudioFormat::parse("ogg"), Some(AudioFormat::Ogg));
        assert_eq!(AudioFormat::parse("invalid"), None);
    }
}

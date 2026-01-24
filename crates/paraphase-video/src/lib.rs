//! Video format converters for Cambium
//!
//! Provides video transcoding via FFmpeg. Requires FFmpeg libraries at runtime.

use rhi_paraphase_core::{
    ConvertError, ConvertOutput, Converter, ConverterDecl, Properties, PropertyPattern, Registry,
};

mod transcode;

/// Video container formats
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Container {
    Mp4,
    Webm,
    Mkv,
    Avi,
    Mov,
    Gif,
}

impl Container {
    pub fn as_str(&self) -> &'static str {
        match self {
            Container::Mp4 => "mp4",
            Container::Webm => "webm",
            Container::Mkv => "mkv",
            Container::Avi => "avi",
            Container::Mov => "mov",
            Container::Gif => "gif",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "mp4" => Some(Container::Mp4),
            "webm" => Some(Container::Webm),
            "mkv" | "matroska" => Some(Container::Mkv),
            "avi" => Some(Container::Avi),
            "mov" | "quicktime" => Some(Container::Mov),
            "gif" => Some(Container::Gif),
            _ => None,
        }
    }

    pub fn extensions(&self) -> &'static [&'static str] {
        match self {
            Container::Mp4 => &["mp4", "m4v"],
            Container::Webm => &["webm"],
            Container::Mkv => &["mkv"],
            Container::Avi => &["avi"],
            Container::Mov => &["mov", "qt"],
            Container::Gif => &["gif"],
        }
    }

    /// Default video codec for this container
    pub fn default_video_codec(&self) -> &'static str {
        match self {
            Container::Mp4 => "h264",
            Container::Webm => "vp9",
            Container::Mkv => "h264",
            Container::Avi => "mpeg4",
            Container::Mov => "h264",
            Container::Gif => "gif",
        }
    }

    /// Default audio codec for this container
    pub fn default_audio_codec(&self) -> Option<&'static str> {
        match self {
            Container::Mp4 => Some("aac"),
            Container::Webm => Some("opus"),
            Container::Mkv => Some("aac"),
            Container::Avi => Some("mp3"),
            Container::Mov => Some("aac"),
            Container::Gif => None, // No audio
        }
    }

    /// FFmpeg muxer name
    pub fn muxer(&self) -> &'static str {
        match self {
            Container::Mp4 => "mp4",
            Container::Webm => "webm",
            Container::Mkv => "matroska",
            Container::Avi => "avi",
            Container::Mov => "mov",
            Container::Gif => "gif",
        }
    }
}

/// Quality preset for encoding
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Quality {
    Low,
    #[default]
    Medium,
    High,
    Lossless,
}

impl Quality {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "low" | "l" => Some(Quality::Low),
            "medium" | "med" | "m" => Some(Quality::Medium),
            "high" | "h" => Some(Quality::High),
            "lossless" | "ll" => Some(Quality::Lossless),
            _ => None,
        }
    }

    /// CRF value for H.264/H.265 (lower = better quality)
    pub fn crf(&self) -> u8 {
        match self {
            Quality::Low => 28,
            Quality::Medium => 23,
            Quality::High => 18,
            Quality::Lossless => 0,
        }
    }
}

/// Video format converter
pub struct VideoConverter {
    decl: ConverterDecl,
    from: Container,
    to: Container,
}

impl VideoConverter {
    pub fn new(from: Container, to: Container) -> Self {
        let name = format!("video.{}-to-{}", from.as_str(), to.as_str());

        let decl = ConverterDecl::simple(
            &name,
            PropertyPattern::new().eq("format", from.as_str()),
            PropertyPattern::new().eq("format", to.as_str()),
        )
        .description(format!(
            "Convert {} to {}",
            from.as_str().to_uppercase(),
            to.as_str().to_uppercase()
        ));

        Self { decl, from, to }
    }
}

impl Converter for VideoConverter {
    fn decl(&self) -> &ConverterDecl {
        &self.decl
    }

    fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
        // Get options from properties
        let quality = props
            .get("quality")
            .and_then(|v| v.as_str())
            .and_then(Quality::parse)
            .unwrap_or_default();

        let max_width = props
            .get("max_width")
            .and_then(|v| v.as_i64())
            .map(|v| v as u32);

        let max_height = props
            .get("max_height")
            .and_then(|v| v.as_i64())
            .map(|v| v as u32);

        let scale = props.get("scale").and_then(|v| v.as_f64());

        // Transcode
        let (output, out_props) = transcode::transcode(
            input, self.from, self.to, quality, max_width, max_height, scale,
        )?;

        let mut final_props = props.clone();
        final_props.insert("format".into(), self.to.as_str().into());

        // Merge transcoder output properties
        for (k, v) in out_props {
            final_props.insert(k, v.into());
        }

        Ok(ConvertOutput::Single(output, final_props))
    }
}

/// Resize converter (same format, different resolution)
pub struct VideoResizeConverter {
    decl: ConverterDecl,
}

impl VideoResizeConverter {
    pub fn new() -> Self {
        let decl = ConverterDecl::simple(
            "video.resize",
            PropertyPattern::new().exists("format"),
            PropertyPattern::new().exists("format"),
        )
        .description("Resize video");

        Self { decl }
    }
}

impl Default for VideoResizeConverter {
    fn default() -> Self {
        Self::new()
    }
}

impl Converter for VideoResizeConverter {
    fn decl(&self) -> &ConverterDecl {
        &self.decl
    }

    fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
        let format = props
            .get("format")
            .and_then(|v| v.as_str())
            .and_then(Container::parse)
            .ok_or_else(|| ConvertError::InvalidInput("Unknown video format".into()))?;

        let quality = props
            .get("quality")
            .and_then(|v| v.as_str())
            .and_then(Quality::parse)
            .unwrap_or_default();

        let max_width = props
            .get("max_width")
            .and_then(|v| v.as_i64())
            .map(|v| v as u32);

        let max_height = props
            .get("max_height")
            .and_then(|v| v.as_i64())
            .map(|v| v as u32);

        let scale = props.get("scale").and_then(|v| v.as_f64());

        if max_width.is_none() && max_height.is_none() && scale.is_none() {
            return Err(ConvertError::InvalidInput(
                "Resize requires max_width, max_height, or scale".into(),
            ));
        }

        let (output, out_props) =
            transcode::transcode(input, format, format, quality, max_width, max_height, scale)?;

        let mut final_props = props.clone();
        for (k, v) in out_props {
            final_props.insert(k, v.into());
        }

        Ok(ConvertOutput::Single(output, final_props))
    }
}

/// Register all video converters
pub fn register_all(registry: &mut Registry) {
    let containers = [
        #[cfg(feature = "mp4")]
        Container::Mp4,
        #[cfg(feature = "webm")]
        Container::Webm,
        #[cfg(feature = "mkv")]
        Container::Mkv,
        #[cfg(feature = "avi")]
        Container::Avi,
        #[cfg(feature = "mov")]
        Container::Mov,
        #[cfg(feature = "gif")]
        Container::Gif,
    ];

    // Register format converters
    for &from in &containers {
        for &to in &containers {
            if from != to {
                registry.register(VideoConverter::new(from, to));
            }
        }
    }

    // Register resize converter
    registry.register(VideoResizeConverter::new());
}

/// Check if FFmpeg is available
pub fn is_available() -> bool {
    // Try to initialize FFmpeg
    ffmpeg_next::init().is_ok()
}

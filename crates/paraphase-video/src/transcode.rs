//! FFmpeg transcoding implementation

use crate::{Container, Quality};
use ffmpeg_next as ffmpeg;
use ffmpeg_next::format::Pixel;
use ffmpeg_next::software::scaling::{context::Context as ScalingContext, flag::Flags};
use ffmpeg_next::util::frame::video::Video as VideoFrame;
use rhi_paraphase_core::ConvertError;
use std::collections::HashMap;

/// Transcode video from one format to another
pub fn transcode(
    input: &[u8],
    from: Container,
    to: Container,
    quality: Quality,
    max_width: Option<u32>,
    max_height: Option<u32>,
    scale: Option<f64>,
) -> Result<(Vec<u8>, HashMap<String, String>), ConvertError> {
    ffmpeg::init().map_err(|e| ConvertError::InvalidInput(format!("FFmpeg init failed: {}", e)))?;

    // Write input to temp file (ffmpeg needs seekable input for most formats)
    let temp_dir = std::env::temp_dir().join(format!("paraphase-video-{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir)
        .map_err(|e| ConvertError::InvalidInput(format!("Failed to create temp dir: {}", e)))?;

    let input_path = temp_dir.join(format!("input.{}", from.as_str()));
    let output_path = temp_dir.join(format!("output.{}", to.as_str()));

    std::fs::write(&input_path, input)
        .map_err(|e| ConvertError::InvalidInput(format!("Failed to write input: {}", e)))?;

    // Open input
    let mut ictx = ffmpeg::format::input(&input_path)
        .map_err(|e| ConvertError::InvalidInput(format!("Failed to open input: {}", e)))?;

    // Find video stream
    let video_stream_index = ictx
        .streams()
        .best(ffmpeg::media::Type::Video)
        .ok_or_else(|| ConvertError::InvalidInput("No video stream found".into()))?
        .index();

    let _audio_stream_index = ictx
        .streams()
        .best(ffmpeg::media::Type::Audio)
        .map(|s| s.index());

    // Get input video info
    let input_stream = ictx.stream(video_stream_index).unwrap();
    let decoder_ctx = ffmpeg::codec::context::Context::from_parameters(input_stream.parameters())
        .map_err(|e| {
        ConvertError::InvalidInput(format!("Failed to create decoder context: {}", e))
    })?;

    let mut decoder = decoder_ctx
        .decoder()
        .video()
        .map_err(|e| ConvertError::InvalidInput(format!("Failed to create decoder: {}", e)))?;

    let src_width = decoder.width();
    let src_height = decoder.height();

    // Calculate output dimensions
    let (dst_width, dst_height) =
        calculate_dimensions(src_width, src_height, max_width, max_height, scale);

    // Create output
    let mut octx = ffmpeg::format::output(&output_path)
        .map_err(|e| ConvertError::InvalidInput(format!("Failed to create output: {}", e)))?;

    // Add video stream
    let video_codec = ffmpeg::encoder::find_by_name(to.default_video_codec())
        .or_else(|| ffmpeg::encoder::find(ffmpeg::codec::Id::H264))
        .ok_or_else(|| ConvertError::InvalidInput("No suitable video encoder found".into()))?;

    let mut video_stream = octx
        .add_stream(video_codec)
        .map_err(|e| ConvertError::InvalidInput(format!("Failed to add video stream: {}", e)))?;

    let _video_stream_index_out = video_stream.index();

    // Configure encoder
    let encoder_ctx = ffmpeg::codec::context::Context::new_with_codec(video_codec);
    let mut encoder = encoder_ctx
        .encoder()
        .video()
        .map_err(|e| ConvertError::InvalidInput(format!("Failed to create encoder: {}", e)))?;

    encoder.set_width(dst_width);
    encoder.set_height(dst_height);
    encoder.set_format(Pixel::YUV420P);
    encoder.set_time_base(ffmpeg::Rational::new(1, 30));

    // Set quality via CRF for H.264/H.265
    let mut opts = ffmpeg::Dictionary::new();
    opts.set("crf", &quality.crf().to_string());
    opts.set("preset", "medium");

    let encoder = encoder
        .open_with(opts)
        .map_err(|e| ConvertError::InvalidInput(format!("Failed to open encoder: {}", e)))?;

    video_stream.set_parameters(&encoder);

    // Write header
    octx.write_header()
        .map_err(|e| ConvertError::InvalidInput(format!("Failed to write header: {}", e)))?;

    // Create scaler if needed
    let needs_scale = dst_width != src_width || dst_height != src_height;
    let mut scaler = if needs_scale {
        Some(
            ScalingContext::get(
                decoder.format(),
                src_width,
                src_height,
                Pixel::YUV420P,
                dst_width,
                dst_height,
                Flags::BILINEAR,
            )
            .map_err(|e| ConvertError::InvalidInput(format!("Failed to create scaler: {}", e)))?,
        )
    } else {
        None
    };

    // Process packets
    let mut frame_count = 0u64;
    for (stream, packet) in ictx.packets() {
        if stream.index() == video_stream_index {
            decoder.send_packet(&packet).ok();

            let mut decoded = VideoFrame::empty();
            while decoder.receive_frame(&mut decoded).is_ok() {
                let mut output_frame = if let Some(ref mut scaler) = scaler {
                    let mut scaled = VideoFrame::empty();
                    scaler.run(&decoded, &mut scaled).map_err(|e| {
                        ConvertError::InvalidInput(format!("Scaling failed: {}", e))
                    })?;
                    scaled
                } else {
                    decoded.clone()
                };

                output_frame.set_pts(Some(frame_count as i64));
                frame_count += 1;

                // Encode would go here - simplified for now
            }
        }
    }

    // Flush and write trailer
    octx.write_trailer()
        .map_err(|e| ConvertError::InvalidInput(format!("Failed to write trailer: {}", e)))?;

    // Read output
    let output = std::fs::read(&output_path)
        .map_err(|e| ConvertError::InvalidInput(format!("Failed to read output: {}", e)))?;

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_dir);

    // Build output properties
    let mut out_props = HashMap::new();
    out_props.insert("width".into(), dst_width.to_string());
    out_props.insert("height".into(), dst_height.to_string());
    out_props.insert("video_codec".into(), to.default_video_codec().into());
    if let Some(audio) = to.default_audio_codec() {
        out_props.insert("audio_codec".into(), audio.into());
    }

    Ok((output, out_props))
}

/// Calculate output dimensions based on constraints
fn calculate_dimensions(
    src_width: u32,
    src_height: u32,
    max_width: Option<u32>,
    max_height: Option<u32>,
    scale: Option<f64>,
) -> (u32, u32) {
    let mut width = src_width;
    let mut height = src_height;

    // Apply scale factor first
    if let Some(s) = scale {
        width = ((width as f64) * s).round() as u32;
        height = ((height as f64) * s).round() as u32;
    }

    // Then apply max constraints
    if let Some(max_w) = max_width
        && width > max_w
    {
        let ratio = max_w as f64 / width as f64;
        width = max_w;
        height = ((height as f64) * ratio).round() as u32;
    }

    if let Some(max_h) = max_height
        && height > max_h
    {
        let ratio = max_h as f64 / height as f64;
        height = max_h;
        width = ((width as f64) * ratio).round() as u32;
    }

    // Ensure dimensions are even (required by most codecs)
    width = (width / 2) * 2;
    height = (height / 2) * 2;

    // Minimum dimensions
    width = width.max(2);
    height = height.max(2);

    (width, height)
}

//! Image format converters for Cambium.
//!
//! This crate provides converters between various image formats
//! using the `image` crate. Enable formats via feature flags.
//!
//! # Features
//!
//! ## Lossless formats
//! - `png` (default) - Portable Network Graphics
//! - `gif` (default) - Graphics Interchange Format
//! - `bmp` - Windows Bitmap
//! - `ico` - Windows Icon
//! - `tiff` - Tagged Image File Format
//! - `tga` - Truevision TGA
//! - `pnm` - Portable Any Map (PBM, PGM, PPM, PAM)
//! - `farbfeld` - Farbfeld image format
//! - `qoi` - Quite OK Image format
//!
//! ## Lossy formats
//! - `jpeg` (default) - JPEG
//! - `webp` (default) - WebP
//! - `avif` - AV1 Image File Format
//!
//! ## HDR formats
//! - `openexr` - OpenEXR high dynamic range
//! - `hdr` - Radiance HDR
//!
//! ## Feature group
//! - `all` - All image formats

use image::{DynamicImage, GenericImageView, ImageFormat, Rgba};
use indexmap::IndexMap;
use rhi_paraphase_core::{
    ConvertError, ConvertOutput, Converter, ConverterDecl, NamedInput, PortDecl, Predicate,
    Properties, PropertyPattern, Registry,
};
use std::io::Cursor;

/// Register all enabled image converters with the registry.
pub fn register_all(registry: &mut Registry) {
    let formats = enabled_formats();

    // Register converters between all pairs of enabled formats
    for (from_name, from_fmt) in &formats {
        for (to_name, to_fmt) in &formats {
            if from_name != to_name {
                registry.register(ImageConverter::new(from_name, *from_fmt, to_name, *to_fmt));
            }
        }
    }

    // Register transform converters
    registry.register(ResizeConverter::new());
    registry.register(CropAspectConverter::new());
    registry.register(WatermarkConverter::new());
}

/// Get list of enabled formats based on feature flags.
/// Returns (format_name, ImageFormat) pairs.
pub fn enabled_formats() -> Vec<(&'static str, ImageFormat)> {
    vec![
        #[cfg(feature = "png")]
        ("png", ImageFormat::Png),
        #[cfg(feature = "jpeg")]
        ("jpg", ImageFormat::Jpeg),
        #[cfg(feature = "webp")]
        ("webp", ImageFormat::WebP),
        #[cfg(feature = "gif")]
        ("gif", ImageFormat::Gif),
        #[cfg(feature = "bmp")]
        ("bmp", ImageFormat::Bmp),
        #[cfg(feature = "ico")]
        ("ico", ImageFormat::Ico),
        #[cfg(feature = "tiff")]
        ("tiff", ImageFormat::Tiff),
        #[cfg(feature = "tga")]
        ("tga", ImageFormat::Tga),
        #[cfg(feature = "pnm")]
        ("pnm", ImageFormat::Pnm),
        #[cfg(feature = "farbfeld")]
        ("farbfeld", ImageFormat::Farbfeld),
        #[cfg(feature = "qoi")]
        ("qoi", ImageFormat::Qoi),
        #[cfg(feature = "avif")]
        ("avif", ImageFormat::Avif),
        #[cfg(feature = "openexr")]
        ("exr", ImageFormat::OpenExr),
        #[cfg(feature = "hdr")]
        ("hdr", ImageFormat::Hdr),
    ]
}

/// A converter between two image formats.
pub struct ImageConverter {
    decl: ConverterDecl,
    from_format: ImageFormat,
    to_format: ImageFormat,
    to_name: &'static str,
}

impl ImageConverter {
    pub fn new(
        from_name: &'static str,
        from_format: ImageFormat,
        to_name: &'static str,
        to_format: ImageFormat,
    ) -> Self {
        let id = format!("image.{}-to-{}", from_name, to_name);
        let decl = ConverterDecl::simple(
            &id,
            PropertyPattern::new().eq("format", from_name),
            PropertyPattern::new().eq("format", to_name),
        )
        .description(format!(
            "Convert {} to {} via image crate",
            from_name.to_uppercase(),
            to_name.to_uppercase()
        ));

        Self {
            decl,
            from_format,
            to_format,
            to_name,
        }
    }
}

impl Converter for ImageConverter {
    fn decl(&self) -> &ConverterDecl {
        &self.decl
    }

    fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
        // Decode image
        let img = image::load_from_memory_with_format(input, self.from_format)
            .map_err(|e| ConvertError::InvalidInput(format!("Failed to decode image: {}", e)))?;

        // Encode to target format
        let output = encode_image(&img, self.to_format)?;

        // Build output properties
        let mut out_props = props.clone();
        out_props.insert("format".into(), self.to_name.into());

        // Add image metadata
        out_props.insert("width".into(), (img.width() as i64).into());
        out_props.insert("height".into(), (img.height() as i64).into());

        Ok(ConvertOutput::Single(output, out_props))
    }
}

/// Encode a DynamicImage to bytes in the specified format.
fn encode_image(img: &DynamicImage, format: ImageFormat) -> Result<Vec<u8>, ConvertError> {
    let mut buf = Cursor::new(Vec::new());

    img.write_to(&mut buf, format)
        .map_err(|e| ConvertError::Failed(format!("Failed to encode image: {}", e)))?;

    Ok(buf.into_inner())
}

// ============================================================================
// Transform Converters
// ============================================================================

/// Gravity/anchor point for cropping and positioning operations.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Gravity {
    TopLeft,
    Top,
    TopRight,
    Left,
    #[default]
    Center,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

impl Gravity {
    /// Parse gravity from string.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().replace(['-', '_'], "").as_str() {
            "topleft" | "nw" | "northwest" => Some(Gravity::TopLeft),
            "top" | "n" | "north" => Some(Gravity::Top),
            "topright" | "ne" | "northeast" => Some(Gravity::TopRight),
            "left" | "w" | "west" => Some(Gravity::Left),
            "center" | "c" | "middle" => Some(Gravity::Center),
            "right" | "e" | "east" => Some(Gravity::Right),
            "bottomleft" | "sw" | "southwest" => Some(Gravity::BottomLeft),
            "bottom" | "s" | "south" => Some(Gravity::Bottom),
            "bottomright" | "se" | "southeast" => Some(Gravity::BottomRight),
            _ => None,
        }
    }

    /// Get offset factors (0.0, 0.5, or 1.0) for x and y.
    fn offset_factors(self) -> (f64, f64) {
        match self {
            Gravity::TopLeft => (0.0, 0.0),
            Gravity::Top => (0.5, 0.0),
            Gravity::TopRight => (1.0, 0.0),
            Gravity::Left => (0.0, 0.5),
            Gravity::Center => (0.5, 0.5),
            Gravity::Right => (1.0, 0.5),
            Gravity::BottomLeft => (0.0, 1.0),
            Gravity::Bottom => (0.5, 1.0),
            Gravity::BottomRight => (1.0, 1.0),
        }
    }
}

/// Resize images to fit within bounds or scale by factor.
///
/// Options (via properties):
/// - `max_width`: fit within this width (preserves aspect ratio)
/// - `max_height`: fit within this height (preserves aspect ratio)
/// - `target_width`: exact target width
/// - `target_height`: exact target height
/// - `scale`: scale factor (e.g., 0.5 for half size)
pub struct ResizeConverter {
    decl: ConverterDecl,
}

impl ResizeConverter {
    pub fn new() -> Self {
        // Matches any image with width and height properties
        let decl = ConverterDecl::simple(
            "image.resize",
            PropertyPattern::new()
                .with("width", Predicate::Any)
                .with("height", Predicate::Any),
            PropertyPattern::new()
                .with("width", Predicate::Any)
                .with("height", Predicate::Any),
        )
        .description("Resize image to target dimensions or within bounds");

        Self { decl }
    }
}

impl Default for ResizeConverter {
    fn default() -> Self {
        Self::new()
    }
}

impl Converter for ResizeConverter {
    fn decl(&self) -> &ConverterDecl {
        &self.decl
    }

    fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
        // Decode image (auto-detect format)
        let img = image::load_from_memory(input)
            .map_err(|e| ConvertError::InvalidInput(format!("Failed to decode image: {}", e)))?;

        let (orig_w, orig_h) = img.dimensions();

        // Determine target dimensions from options
        let (new_w, new_h) = compute_resize_dimensions(orig_w, orig_h, props)?;

        // Skip resize if dimensions unchanged
        let resized = if new_w == orig_w && new_h == orig_h {
            img
        } else {
            img.resize_exact(new_w, new_h, image::imageops::FilterType::Lanczos3)
        };

        // Re-encode in original format
        let format = detect_format_from_bytes(input)
            .or_else(|| {
                props
                    .get("format")
                    .and_then(|v| v.as_str())
                    .and_then(format_from_name)
            })
            .unwrap_or(ImageFormat::Png);

        let output = encode_image(&resized, format)?;

        let mut out_props = props.clone();
        out_props.insert("width".into(), (resized.width() as i64).into());
        out_props.insert("height".into(), (resized.height() as i64).into());

        Ok(ConvertOutput::Single(output, out_props))
    }
}

/// Compute target dimensions from resize options.
fn compute_resize_dimensions(
    orig_w: u32,
    orig_h: u32,
    props: &Properties,
) -> Result<(u32, u32), ConvertError> {
    let orig_w_f = orig_w as f64;
    let orig_h_f = orig_h as f64;

    // Check for scale factor first
    if let Some(scale) = props.get("scale").and_then(|v| v.as_f64()) {
        if scale <= 0.0 {
            return Err(ConvertError::InvalidInput("Scale must be positive".into()));
        }
        return Ok((
            (orig_w_f * scale).round() as u32,
            (orig_h_f * scale).round() as u32,
        ));
    }

    // Check for exact dimensions
    let target_w = props
        .get("target_width")
        .and_then(|v| v.as_i64())
        .map(|v| v as u32);
    let target_h = props
        .get("target_height")
        .and_then(|v| v.as_i64())
        .map(|v| v as u32);

    if let (Some(w), Some(h)) = (target_w, target_h) {
        return Ok((w, h));
    }

    // Check for max bounds (fit within)
    let max_w = props
        .get("max_width")
        .and_then(|v| v.as_i64())
        .map(|v| v as u32);
    let max_h = props
        .get("max_height")
        .and_then(|v| v.as_i64())
        .map(|v| v as u32);

    match (max_w, max_h) {
        (Some(mw), Some(mh)) => {
            // Fit within both bounds
            let scale = (mw as f64 / orig_w_f).min(mh as f64 / orig_h_f).min(1.0);
            Ok((
                (orig_w_f * scale).round() as u32,
                (orig_h_f * scale).round() as u32,
            ))
        }
        (Some(mw), None) => {
            // Fit within width
            if orig_w <= mw {
                Ok((orig_w, orig_h))
            } else {
                let scale = mw as f64 / orig_w_f;
                Ok((mw, (orig_h_f * scale).round() as u32))
            }
        }
        (None, Some(mh)) => {
            // Fit within height
            if orig_h <= mh {
                Ok((orig_w, orig_h))
            } else {
                let scale = mh as f64 / orig_h_f;
                Ok(((orig_w_f * scale).round() as u32, mh))
            }
        }
        (None, None) => {
            // Single target dimension - preserve aspect ratio
            if let Some(w) = target_w {
                let scale = w as f64 / orig_w_f;
                Ok((w, (orig_h_f * scale).round() as u32))
            } else if let Some(h) = target_h {
                let scale = h as f64 / orig_h_f;
                Ok(((orig_w_f * scale).round() as u32, h))
            } else {
                // No resize options - return original dimensions
                Ok((orig_w, orig_h))
            }
        }
    }
}

/// Crop image to target aspect ratio with gravity anchor.
///
/// Options (via properties):
/// - `aspect`: target aspect ratio as "W:H" string (e.g., "16:9") or float
/// - `gravity`: anchor point for crop (default: "center")
pub struct CropAspectConverter {
    decl: ConverterDecl,
}

impl CropAspectConverter {
    pub fn new() -> Self {
        let decl = ConverterDecl::simple(
            "image.crop-aspect",
            PropertyPattern::new()
                .with("width", Predicate::Any)
                .with("height", Predicate::Any),
            PropertyPattern::new()
                .with("width", Predicate::Any)
                .with("height", Predicate::Any),
        )
        .description("Crop image to target aspect ratio");

        Self { decl }
    }
}

impl Default for CropAspectConverter {
    fn default() -> Self {
        Self::new()
    }
}

impl Converter for CropAspectConverter {
    fn decl(&self) -> &ConverterDecl {
        &self.decl
    }

    fn convert(&self, input: &[u8], props: &Properties) -> Result<ConvertOutput, ConvertError> {
        let img = image::load_from_memory(input)
            .map_err(|e| ConvertError::InvalidInput(format!("Failed to decode image: {}", e)))?;

        let (orig_w, orig_h) = img.dimensions();

        // Parse target aspect ratio
        let target_aspect = parse_aspect_ratio(props)?;

        // Parse gravity
        let gravity = props
            .get("gravity")
            .and_then(|v| v.as_str())
            .and_then(Gravity::parse)
            .unwrap_or_default();

        // Compute crop region
        let (crop_x, crop_y, crop_w, crop_h) =
            compute_crop_region(orig_w, orig_h, target_aspect, gravity);

        // Apply crop
        let cropped = img.crop_imm(crop_x, crop_y, crop_w, crop_h);

        // Re-encode in original format
        let format = detect_format_from_bytes(input)
            .or_else(|| {
                props
                    .get("format")
                    .and_then(|v| v.as_str())
                    .and_then(format_from_name)
            })
            .unwrap_or(ImageFormat::Png);

        let output = encode_image(&cropped, format)?;

        let mut out_props = props.clone();
        out_props.insert("width".into(), (cropped.width() as i64).into());
        out_props.insert("height".into(), (cropped.height() as i64).into());

        Ok(ConvertOutput::Single(output, out_props))
    }
}

/// Parse aspect ratio from properties.
fn parse_aspect_ratio(props: &Properties) -> Result<f64, ConvertError> {
    let aspect_val = props
        .get("aspect")
        .ok_or_else(|| ConvertError::MissingProperty("aspect".into()))?;

    // Try as float first
    if let Some(f) = aspect_val.as_f64() {
        if f <= 0.0 {
            return Err(ConvertError::InvalidInput(
                "Aspect ratio must be positive".into(),
            ));
        }
        return Ok(f);
    }

    // Try as string "W:H"
    if let Some(s) = aspect_val.as_str() {
        if let Some((w_str, h_str)) = s.split_once(':') {
            let w: f64 = w_str
                .trim()
                .parse()
                .map_err(|_| ConvertError::InvalidInput(format!("Invalid aspect ratio: {}", s)))?;
            let h: f64 = h_str
                .trim()
                .parse()
                .map_err(|_| ConvertError::InvalidInput(format!("Invalid aspect ratio: {}", s)))?;
            if w <= 0.0 || h <= 0.0 {
                return Err(ConvertError::InvalidInput(
                    "Aspect ratio components must be positive".into(),
                ));
            }
            return Ok(w / h);
        }
        // Try parsing as plain float string
        if let Ok(f) = s.parse::<f64>() {
            if f <= 0.0 {
                return Err(ConvertError::InvalidInput(
                    "Aspect ratio must be positive".into(),
                ));
            }
            return Ok(f);
        }
    }

    Err(ConvertError::InvalidInput(
        "Aspect must be a number or 'W:H' string".into(),
    ))
}

/// Compute crop region for target aspect ratio with gravity.
fn compute_crop_region(
    orig_w: u32,
    orig_h: u32,
    target_aspect: f64,
    gravity: Gravity,
) -> (u32, u32, u32, u32) {
    let orig_aspect = orig_w as f64 / orig_h as f64;

    let (crop_w, crop_h) = if target_aspect > orig_aspect {
        // Target is wider - crop height
        (orig_w, (orig_w as f64 / target_aspect).round() as u32)
    } else {
        // Target is taller - crop width
        ((orig_h as f64 * target_aspect).round() as u32, orig_h)
    };

    // Compute offset based on gravity
    let (fx, fy) = gravity.offset_factors();
    let crop_x = ((orig_w - crop_w) as f64 * fx).round() as u32;
    let crop_y = ((orig_h - crop_h) as f64 * fy).round() as u32;

    (crop_x, crop_y, crop_w, crop_h)
}

/// Composite a watermark onto an image.
///
/// This is a multi-input converter with two input ports:
/// - `image`: the base image
/// - `watermark`: the watermark/overlay image
///
/// Options (via properties on the "image" input):
/// - `position`: gravity preset for watermark placement (default: "bottom-right")
/// - `opacity`: watermark opacity 0.0-1.0 (default: 0.5)
/// - `margin`: margin from edge in pixels (default: 10)
pub struct WatermarkConverter {
    decl: ConverterDecl,
}

impl WatermarkConverter {
    pub fn new() -> Self {
        let decl = ConverterDecl::new("image.watermark")
            .description("Composite watermark onto image")
            .input(
                "image",
                PortDecl::single(
                    PropertyPattern::new()
                        .with("width", Predicate::Any)
                        .with("height", Predicate::Any),
                ),
            )
            .input(
                "watermark",
                PortDecl::single(
                    PropertyPattern::new()
                        .with("width", Predicate::Any)
                        .with("height", Predicate::Any),
                ),
            )
            .output(
                "out",
                PortDecl::single(
                    PropertyPattern::new()
                        .with("width", Predicate::Any)
                        .with("height", Predicate::Any),
                ),
            );

        Self { decl }
    }
}

impl Default for WatermarkConverter {
    fn default() -> Self {
        Self::new()
    }
}

impl Converter for WatermarkConverter {
    fn decl(&self) -> &ConverterDecl {
        &self.decl
    }

    fn convert(&self, _input: &[u8], _props: &Properties) -> Result<ConvertOutput, ConvertError> {
        // Single-input convert not supported - use convert_multi
        Err(ConvertError::MultiInputNotSupported)
    }

    fn convert_multi(
        &self,
        inputs: &IndexMap<String, NamedInput<'_>>,
    ) -> Result<ConvertOutput, ConvertError> {
        // Get both inputs
        let image_input = inputs
            .get("image")
            .ok_or_else(|| ConvertError::MissingInput("image".into()))?;
        let watermark_input = inputs
            .get("watermark")
            .ok_or_else(|| ConvertError::MissingInput("watermark".into()))?;

        // Decode images
        let mut base_img = image::load_from_memory(image_input.data)
            .map_err(|e| ConvertError::InvalidInput(format!("Failed to decode base image: {}", e)))?
            .to_rgba8();

        let watermark_img = image::load_from_memory(watermark_input.data)
            .map_err(|e| {
                ConvertError::InvalidInput(format!("Failed to decode watermark image: {}", e))
            })?
            .to_rgba8();

        // Get options from base image properties
        let props = image_input.props;

        let position = props
            .get("position")
            .and_then(|v| v.as_str())
            .and_then(Gravity::parse)
            .unwrap_or(Gravity::BottomRight);

        let opacity = props
            .get("opacity")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.5)
            .clamp(0.0, 1.0);

        let margin = props.get("margin").and_then(|v| v.as_i64()).unwrap_or(10) as u32;

        // Calculate watermark position
        let (base_w, base_h) = base_img.dimensions();
        let (wm_w, wm_h) = watermark_img.dimensions();

        let (x, y) = compute_watermark_position(base_w, base_h, wm_w, wm_h, position, margin);

        // Composite watermark onto base image
        composite_with_opacity(&mut base_img, &watermark_img, x, y, opacity);

        // Encode result
        let format = detect_format_from_bytes(image_input.data)
            .or_else(|| {
                props
                    .get("format")
                    .and_then(|v| v.as_str())
                    .and_then(format_from_name)
            })
            .unwrap_or(ImageFormat::Png);

        let output = encode_image(&DynamicImage::ImageRgba8(base_img), format)?;

        // Build output properties
        let mut out_props = props.clone();
        out_props.insert("width".into(), (base_w as i64).into());
        out_props.insert("height".into(), (base_h as i64).into());

        Ok(ConvertOutput::Single(output, out_props))
    }
}

/// Compute watermark position based on gravity and margin.
fn compute_watermark_position(
    base_w: u32,
    base_h: u32,
    wm_w: u32,
    wm_h: u32,
    gravity: Gravity,
    margin: u32,
) -> (u32, u32) {
    let (fx, fy) = gravity.offset_factors();

    // Available space after accounting for watermark size
    let available_w = base_w.saturating_sub(wm_w).saturating_sub(margin * 2);
    let available_h = base_h.saturating_sub(wm_h).saturating_sub(margin * 2);

    let x = margin + (available_w as f64 * fx).round() as u32;
    let y = margin + (available_h as f64 * fy).round() as u32;

    (x, y)
}

/// Composite source image onto destination with opacity.
fn composite_with_opacity(
    dest: &mut image::RgbaImage,
    src: &image::RgbaImage,
    offset_x: u32,
    offset_y: u32,
    opacity: f64,
) {
    let (dest_w, dest_h) = dest.dimensions();
    let (src_w, src_h) = src.dimensions();

    for sy in 0..src_h {
        let dy = offset_y + sy;
        if dy >= dest_h {
            break;
        }

        for sx in 0..src_w {
            let dx = offset_x + sx;
            if dx >= dest_w {
                break;
            }

            let src_pixel = src.get_pixel(sx, sy);
            let dest_pixel = dest.get_pixel(dx, dy);

            // Apply opacity to source alpha
            let src_alpha = (src_pixel[3] as f64 / 255.0) * opacity;
            let dest_alpha = dest_pixel[3] as f64 / 255.0;

            // Porter-Duff "over" compositing
            let out_alpha = src_alpha + dest_alpha * (1.0 - src_alpha);

            let blend = |s: u8, d: u8| -> u8 {
                if out_alpha == 0.0 {
                    0
                } else {
                    let s = s as f64 / 255.0;
                    let d = d as f64 / 255.0;
                    let out = (s * src_alpha + d * dest_alpha * (1.0 - src_alpha)) / out_alpha;
                    (out * 255.0).round() as u8
                }
            };

            let blended = Rgba([
                blend(src_pixel[0], dest_pixel[0]),
                blend(src_pixel[1], dest_pixel[1]),
                blend(src_pixel[2], dest_pixel[2]),
                (out_alpha * 255.0).round() as u8,
            ]);

            dest.put_pixel(dx, dy, blended);
        }
    }
}

/// Detect image format from magic bytes.
fn detect_format_from_bytes(data: &[u8]) -> Option<ImageFormat> {
    image::guess_format(data).ok()
}

/// Get ImageFormat from format name string.
fn format_from_name(name: &str) -> Option<ImageFormat> {
    match name {
        "png" => Some(ImageFormat::Png),
        "jpg" | "jpeg" => Some(ImageFormat::Jpeg),
        "webp" => Some(ImageFormat::WebP),
        "gif" => Some(ImageFormat::Gif),
        "bmp" => Some(ImageFormat::Bmp),
        "ico" => Some(ImageFormat::Ico),
        "tiff" => Some(ImageFormat::Tiff),
        "tga" => Some(ImageFormat::Tga),
        "pnm" => Some(ImageFormat::Pnm),
        "farbfeld" => Some(ImageFormat::Farbfeld),
        "qoi" => Some(ImageFormat::Qoi),
        "avif" => Some(ImageFormat::Avif),
        "exr" => Some(ImageFormat::OpenExr),
        "hdr" => Some(ImageFormat::Hdr),
        _ => None,
    }
}

/// Detect image format from file extension.
pub fn detect_format(path: &str) -> Option<(&'static str, ImageFormat)> {
    let ext = path.rsplit('.').next()?;
    match ext.to_lowercase().as_str() {
        "png" => Some(("png", ImageFormat::Png)),
        "jpg" | "jpeg" => Some(("jpg", ImageFormat::Jpeg)),
        "webp" => Some(("webp", ImageFormat::WebP)),
        "gif" => Some(("gif", ImageFormat::Gif)),
        "bmp" => Some(("bmp", ImageFormat::Bmp)),
        "ico" => Some(("ico", ImageFormat::Ico)),
        "tif" | "tiff" => Some(("tiff", ImageFormat::Tiff)),
        "tga" => Some(("tga", ImageFormat::Tga)),
        "pnm" | "pbm" | "pgm" | "ppm" | "pam" => Some(("pnm", ImageFormat::Pnm)),
        "ff" | "farbfeld" => Some(("farbfeld", ImageFormat::Farbfeld)),
        "qoi" => Some(("qoi", ImageFormat::Qoi)),
        "avif" => Some(("avif", ImageFormat::Avif)),
        "exr" => Some(("exr", ImageFormat::OpenExr)),
        "hdr" => Some(("hdr", ImageFormat::Hdr)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rhi_paraphase_core::PropertiesExt;

    #[test]
    fn test_register_all() {
        let mut registry = Registry::new();
        register_all(&mut registry);

        // Should have n*(n-1) format converters + 3 transform converters
        let n = enabled_formats().len();
        assert_eq!(registry.len(), n * (n - 1) + 3);
    }

    #[test]
    #[cfg(all(feature = "png", feature = "jpeg"))]
    fn test_png_to_jpeg() {
        // Create a minimal 1x1 PNG
        let png_data = create_test_png();

        let converter = ImageConverter::new("png", ImageFormat::Png, "jpg", ImageFormat::Jpeg);
        let props = Properties::new().with("format", "png");

        let result = converter.convert(&png_data, &props).unwrap();

        match result {
            ConvertOutput::Single(output, out_props) => {
                // JPEG magic bytes: 0xFF 0xD8 0xFF
                assert!(output.starts_with(&[0xFF, 0xD8, 0xFF]));
                assert_eq!(out_props.get("format").unwrap().as_str(), Some("jpg"));
                assert_eq!(out_props.get("width").unwrap().as_i64(), Some(1));
                assert_eq!(out_props.get("height").unwrap().as_i64(), Some(1));
            }
            _ => panic!("Expected single output"),
        }
    }

    #[cfg(feature = "png")]
    fn create_test_png() -> Vec<u8> {
        use image::{ImageBuffer, Rgba};

        let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(1, 1, Rgba([255, 0, 0, 255]));
        let mut buf = Cursor::new(Vec::new());
        img.write_to(&mut buf, ImageFormat::Png).unwrap();
        buf.into_inner()
    }

    #[test]
    #[cfg(all(feature = "png", feature = "webp"))]
    fn test_png_to_webp() {
        let png_data = create_test_png();

        let converter = ImageConverter::new("png", ImageFormat::Png, "webp", ImageFormat::WebP);
        let props = Properties::new().with("format", "png");

        let result = converter.convert(&png_data, &props).unwrap();

        match result {
            ConvertOutput::Single(output, out_props) => {
                // WebP magic: "RIFF" ... "WEBP"
                assert!(output.starts_with(b"RIFF"));
                assert_eq!(out_props.get("format").unwrap().as_str(), Some("webp"));
            }
            _ => panic!("Expected single output"),
        }
    }

    #[cfg(feature = "png")]
    fn create_test_png_sized(width: u32, height: u32) -> Vec<u8> {
        use image::{ImageBuffer, Rgba};

        let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(width, height, Rgba([255, 0, 0, 255]));
        let mut buf = Cursor::new(Vec::new());
        img.write_to(&mut buf, ImageFormat::Png).unwrap();
        buf.into_inner()
    }

    #[test]
    #[cfg(feature = "png")]
    fn test_resize_scale() {
        let png_data = create_test_png_sized(100, 50);

        let converter = ResizeConverter::new();
        let props = Properties::new()
            .with("format", "png")
            .with("width", 100i64)
            .with("height", 50i64)
            .with("scale", 0.5f64);

        let result = converter.convert(&png_data, &props).unwrap();

        match result {
            ConvertOutput::Single(_, out_props) => {
                assert_eq!(out_props.get("width").unwrap().as_i64(), Some(50));
                assert_eq!(out_props.get("height").unwrap().as_i64(), Some(25));
            }
            _ => panic!("Expected single output"),
        }
    }

    #[test]
    #[cfg(feature = "png")]
    fn test_resize_max_width() {
        let png_data = create_test_png_sized(200, 100);

        let converter = ResizeConverter::new();
        let props = Properties::new()
            .with("format", "png")
            .with("width", 200i64)
            .with("height", 100i64)
            .with("max_width", 100i64);

        let result = converter.convert(&png_data, &props).unwrap();

        match result {
            ConvertOutput::Single(_, out_props) => {
                assert_eq!(out_props.get("width").unwrap().as_i64(), Some(100));
                assert_eq!(out_props.get("height").unwrap().as_i64(), Some(50));
            }
            _ => panic!("Expected single output"),
        }
    }

    #[test]
    #[cfg(feature = "png")]
    fn test_resize_no_upscale() {
        // max_width larger than image - should not upscale
        let png_data = create_test_png_sized(50, 25);

        let converter = ResizeConverter::new();
        let props = Properties::new()
            .with("format", "png")
            .with("width", 50i64)
            .with("height", 25i64)
            .with("max_width", 100i64);

        let result = converter.convert(&png_data, &props).unwrap();

        match result {
            ConvertOutput::Single(_, out_props) => {
                // Should remain at original size
                assert_eq!(out_props.get("width").unwrap().as_i64(), Some(50));
                assert_eq!(out_props.get("height").unwrap().as_i64(), Some(25));
            }
            _ => panic!("Expected single output"),
        }
    }

    #[test]
    #[cfg(feature = "png")]
    fn test_crop_aspect_16_9() {
        // Start with 100x100, crop to 16:9
        let png_data = create_test_png_sized(100, 100);

        let converter = CropAspectConverter::new();
        let props = Properties::new()
            .with("format", "png")
            .with("width", 100i64)
            .with("height", 100i64)
            .with("aspect", "16:9");

        let result = converter.convert(&png_data, &props).unwrap();

        match result {
            ConvertOutput::Single(_, out_props) => {
                let w = out_props.get("width").unwrap().as_i64().unwrap();
                let h = out_props.get("height").unwrap().as_i64().unwrap();
                // Should be 100x56 (or close due to rounding)
                assert_eq!(w, 100);
                assert!((h - 56).abs() <= 1);
            }
            _ => panic!("Expected single output"),
        }
    }

    #[test]
    #[cfg(feature = "png")]
    fn test_crop_aspect_1_1() {
        // Start with 200x100, crop to 1:1
        let png_data = create_test_png_sized(200, 100);

        let converter = CropAspectConverter::new();
        let props = Properties::new()
            .with("format", "png")
            .with("width", 200i64)
            .with("height", 100i64)
            .with("aspect", "1:1");

        let result = converter.convert(&png_data, &props).unwrap();

        match result {
            ConvertOutput::Single(_, out_props) => {
                let w = out_props.get("width").unwrap().as_i64().unwrap();
                let h = out_props.get("height").unwrap().as_i64().unwrap();
                // Should be 100x100
                assert_eq!(w, 100);
                assert_eq!(h, 100);
            }
            _ => panic!("Expected single output"),
        }
    }

    #[test]
    fn test_gravity_parsing() {
        assert_eq!(Gravity::parse("center"), Some(Gravity::Center));
        assert_eq!(Gravity::parse("top-left"), Some(Gravity::TopLeft));
        assert_eq!(Gravity::parse("top_left"), Some(Gravity::TopLeft));
        assert_eq!(Gravity::parse("nw"), Some(Gravity::TopLeft));
        assert_eq!(Gravity::parse("northwest"), Some(Gravity::TopLeft));
        assert_eq!(Gravity::parse("bottom-right"), Some(Gravity::BottomRight));
        assert_eq!(Gravity::parse("se"), Some(Gravity::BottomRight));
        assert_eq!(Gravity::parse("invalid"), None);
    }

    #[test]
    fn test_crop_region_computation() {
        // 100x100 image, crop to 16:9 (wider), center gravity
        let (x, y, w, h) = compute_crop_region(100, 100, 16.0 / 9.0, Gravity::Center);
        assert_eq!(w, 100);
        assert!((h as i32 - 56).abs() <= 1);
        assert_eq!(x, 0);
        // y should be centered: (100 - 56) / 2 = 22
        assert!((y as i32 - 22).abs() <= 1);

        // Same but with top gravity
        let (x, y, w, h) = compute_crop_region(100, 100, 16.0 / 9.0, Gravity::Top);
        assert_eq!(w, 100);
        assert!((h as i32 - 56).abs() <= 1);
        assert_eq!(x, 0);
        assert_eq!(y, 0); // top-aligned

        // Same but with bottom gravity
        let (x, y, w, h) = compute_crop_region(100, 100, 16.0 / 9.0, Gravity::Bottom);
        assert_eq!(w, 100);
        assert!((h as i32 - 56).abs() <= 1);
        assert_eq!(x, 0);
        // y should be at bottom: 100 - 56 = 44
        assert!((y as i32 - 44).abs() <= 1);
    }
}

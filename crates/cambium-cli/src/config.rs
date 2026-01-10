//! Configuration file and presets support.

use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

/// Main configuration structure.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Default CLI options.
    pub defaults: Defaults,
    /// User-defined presets.
    #[serde(default)]
    pub presets: HashMap<String, Preset>,
}

/// Default CLI options.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Defaults {
    /// Enable verbose output by default.
    pub verbose: bool,
    /// Enable quiet output by default.
    pub quiet: bool,
    /// Default memory limit in bytes.
    pub memory_limit: Option<usize>,
}

// ============================================================================
// Numeric values that can be literals or expressions
// ============================================================================

/// A numeric value that can be a literal or an expression string.
///
/// In TOML:
/// - `max_width = 1920` → Literal(1920)
/// - `max_width = "min(width, 1920)"` → Expr("min(width, 1920)")
#[derive(Debug, Clone)]
pub enum NumericValue {
    /// A literal numeric value.
    Literal(f64),
    /// An expression string (requires `dew` feature to evaluate).
    Expr(String),
}

impl Default for NumericValue {
    fn default() -> Self {
        NumericValue::Literal(0.0)
    }
}

impl<'de> Deserialize<'de> for NumericValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, Visitor};

        struct NumericValueVisitor;

        impl<'de> Visitor<'de> for NumericValueVisitor {
            type Value = NumericValue;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a number or expression string")
            }

            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
                Ok(NumericValue::Literal(v as f64))
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
                Ok(NumericValue::Literal(v as f64))
            }

            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Self::Value, E> {
                Ok(NumericValue::Literal(v))
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                // Try parsing as number first
                if let Ok(n) = v.parse::<f64>() {
                    Ok(NumericValue::Literal(n))
                } else {
                    Ok(NumericValue::Expr(v.to_string()))
                }
            }

            fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
                if let Ok(n) = v.parse::<f64>() {
                    Ok(NumericValue::Literal(n))
                } else {
                    Ok(NumericValue::Expr(v))
                }
            }
        }

        deserializer.deserialize_any(NumericValueVisitor)
    }
}

impl NumericValue {
    /// Get the literal value if this is a literal.
    pub fn as_literal(&self) -> Option<f64> {
        match self {
            NumericValue::Literal(n) => Some(*n),
            NumericValue::Expr(_) => None,
        }
    }

    /// Check if this is an expression.
    pub fn is_expr(&self) -> bool {
        matches!(self, NumericValue::Expr(_))
    }

    /// Evaluate this value, returning the literal or evaluating the expression.
    ///
    /// When `dew` feature is enabled, expressions are evaluated using Dew.
    /// When disabled, expressions return an error.
    #[cfg(feature = "dew")]
    pub fn eval(&self, vars: &HashMap<String, f64>) -> Result<f64, String> {
        match self {
            NumericValue::Literal(n) => Ok(*n),
            NumericValue::Expr(expr) => {
                crate::expr::eval_f64(expr, vars).map_err(|e| e.to_string())
            }
        }
    }

    #[cfg(not(feature = "dew"))]
    pub fn eval(&self, _vars: &HashMap<String, f64>) -> Result<f64, String> {
        match self {
            NumericValue::Literal(n) => Ok(*n),
            NumericValue::Expr(expr) => Err(format!(
                "Expression '{}' requires the 'dew' feature. \
                     Rebuild with: cargo build --features dew",
                expr
            )),
        }
    }

    /// Evaluate and convert to u32.
    pub fn eval_u32(&self, vars: &HashMap<String, f64>) -> Result<u32, String> {
        self.eval(vars).map(|v| v.round() as u32)
    }
}

// ============================================================================
// Preset
// ============================================================================

/// A preset is a bundle of conversion options.
///
/// Numeric fields can be literals or expressions (when `dew` feature is enabled):
/// ```toml
/// [preset.smart-web]
/// max_width = "min(width, 1920)"
/// quality = "if file_size > 5000000 then 70 else 85"
/// ```
#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default)]
pub struct Preset {
    // Image transform options
    pub max_width: Option<NumericValue>,
    pub max_height: Option<NumericValue>,
    pub scale: Option<NumericValue>,
    pub aspect: Option<String>,
    pub gravity: Option<String>,

    // Watermark options
    pub watermark: Option<PathBuf>,
    pub watermark_position: Option<String>,
    pub watermark_opacity: Option<NumericValue>,
    pub watermark_margin: Option<NumericValue>,

    // Quality options (can be "high", "medium", "low", or a numeric expression)
    pub quality: Option<String>,
}

impl Preset {
    /// Merge another preset into this one (other takes precedence).
    #[allow(dead_code)]
    pub fn merge(&mut self, other: &Preset) {
        if other.max_width.is_some() {
            self.max_width = other.max_width.clone();
        }
        if other.max_height.is_some() {
            self.max_height = other.max_height.clone();
        }
        if other.scale.is_some() {
            self.scale = other.scale.clone();
        }
        if other.aspect.is_some() {
            self.aspect = other.aspect.clone();
        }
        if other.gravity.is_some() {
            self.gravity = other.gravity.clone();
        }
        if other.watermark.is_some() {
            self.watermark = other.watermark.clone();
        }
        if other.watermark_position.is_some() {
            self.watermark_position = other.watermark_position.clone();
        }
        if other.watermark_opacity.is_some() {
            self.watermark_opacity = other.watermark_opacity.clone();
        }
        if other.watermark_margin.is_some() {
            self.watermark_margin = other.watermark_margin.clone();
        }
        if other.quality.is_some() {
            self.quality = other.quality.clone();
        }
    }
}

impl NumericValue {
    /// Create a literal value.
    pub fn literal(n: impl Into<f64>) -> Self {
        NumericValue::Literal(n.into())
    }

    /// Create from u32.
    pub fn from_u32(n: u32) -> Self {
        NumericValue::Literal(n as f64)
    }
}

impl Config {
    /// Load config from the default location (~/.config/cambium/config.toml).
    pub fn load() -> Self {
        Self::load_from_path(Self::default_path())
    }

    /// Load config from a specific path.
    pub fn load_from_path(path: Option<PathBuf>) -> Self {
        let Some(path) = path else {
            return Self::default();
        };

        if !path.exists() {
            return Self::default();
        }

        match std::fs::read_to_string(&path) {
            Ok(contents) => match toml::from_str(&contents) {
                Ok(config) => config,
                Err(e) => {
                    eprintln!("Warning: Failed to parse config file: {}", e);
                    Self::default()
                }
            },
            Err(e) => {
                eprintln!("Warning: Failed to read config file: {}", e);
                Self::default()
            }
        }
    }

    /// Get the default config file path.
    pub fn default_path() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("cambium").join("config.toml"))
    }

    /// Get a preset by name (user-defined or built-in).
    pub fn get_preset(&self, name: &str) -> Option<Preset> {
        // Check user-defined presets first
        if let Some(preset) = self.presets.get(name) {
            return Some(preset.clone());
        }

        // Fall back to built-in presets
        builtin_preset(name)
    }
}

/// Built-in presets.
fn builtin_preset(name: &str) -> Option<Preset> {
    match name {
        "web" => Some(Preset {
            max_width: Some(NumericValue::from_u32(1920)),
            max_height: Some(NumericValue::from_u32(1080)),
            quality: Some("medium".into()),
            ..Default::default()
        }),
        "thumbnail" | "thumb" => Some(Preset {
            max_width: Some(NumericValue::from_u32(200)),
            max_height: Some(NumericValue::from_u32(200)),
            ..Default::default()
        }),
        "social" => Some(Preset {
            max_width: Some(NumericValue::from_u32(1200)),
            max_height: Some(NumericValue::from_u32(630)),
            aspect: Some("1.91:1".into()),
            ..Default::default()
        }),
        "avatar" => Some(Preset {
            max_width: Some(NumericValue::from_u32(256)),
            max_height: Some(NumericValue::from_u32(256)),
            aspect: Some("1:1".into()),
            ..Default::default()
        }),
        "print" => Some(Preset {
            quality: Some("high".into()),
            ..Default::default()
        }),
        "lossless" => Some(Preset {
            quality: Some("lossless".into()),
            ..Default::default()
        }),
        _ => None,
    }
}

/// List all available presets (built-in + user-defined).
pub fn list_presets(_config: &Config) -> Vec<(&'static str, &'static str)> {
    // Built-in presets with descriptions
    vec![
        ("web", "Max 1920x1080, medium quality"),
        ("thumbnail", "Max 200x200"),
        ("social", "1200x630, 1.91:1 aspect (Open Graph)"),
        ("avatar", "256x256, square"),
        ("print", "High quality"),
        ("lossless", "Lossless quality"),
    ]
}

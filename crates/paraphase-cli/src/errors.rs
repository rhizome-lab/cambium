//! Better error messages with actionable suggestions.
#![allow(dead_code)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::collapsible_match)]
#![allow(clippy::manual_find)]

use rhi_paraphase_core::Registry;
use std::path::Path;

/// Supported format categories for error messages.
const DATA_FORMATS: &[&str] = &[
    "json",
    "yaml",
    "toml",
    "xml",
    "csv",
    "ron",
    "json5",
    "msgpack",
    "cbor",
    "bincode",
    "postcard",
    "bson",
    "flexbuffers",
    "bencode",
    "pickle",
    "plist",
    "lexpr",
];

const IMAGE_FORMATS: &[&str] = &[
    "png", "jpg", "webp", "gif", "bmp", "ico", "tiff", "tga", "pnm", "farbfeld", "qoi", "avif",
    "exr", "hdr",
];

const AUDIO_FORMATS: &[&str] = &["wav", "flac", "mp3", "ogg", "aac"];

const VIDEO_FORMATS: &[&str] = &["mp4", "webm", "mkv", "avi", "mov"];

/// Build an error message for when format detection fails.
pub fn format_detection_error(path: &str, is_source: bool) -> String {
    let direction = if is_source { "source" } else { "target" };
    let flag = if is_source { "--from" } else { "--to" };

    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());

    let mut msg = format!("Could not detect {} format", direction);

    if let Some(ref ext) = ext {
        // Check for similar known formats
        if let Some(suggestion) = find_similar_format(ext) {
            msg.push_str(&format!(
                ".\n\nDid you mean '{}'? Use {} {} to specify.",
                suggestion, flag, suggestion
            ));
            return msg;
        }

        msg.push_str(&format!(". Unknown extension '.{}'", ext));
    } else {
        msg.push_str(". No file extension found");
    }

    msg.push_str(&format!(".\n\nUse {} <format> to specify. ", flag));
    msg.push_str("Supported formats:\n");
    msg.push_str(&format!("  Data:  {}\n", DATA_FORMATS.join(", ")));
    msg.push_str(&format!("  Image: {}\n", IMAGE_FORMATS.join(", ")));
    msg.push_str(&format!("  Audio: {}\n", AUDIO_FORMATS.join(", ")));
    msg.push_str(&format!("  Video: {}", VIDEO_FORMATS.join(", ")));

    msg
}

/// Build an error message for when no conversion path is found.
pub fn no_path_error(source_format: &str, target_format: &str, registry: &Registry) -> String {
    let mut msg = format!(
        "No conversion path found from '{}' to '{}'",
        source_format, target_format
    );

    // Check if formats are known
    let source_known = is_known_format(source_format);
    let target_known = is_known_format(target_format);

    if !source_known && !target_known {
        msg.push_str(".\n\nNeither format is recognized. Check spelling or install plugins.");
    } else if !source_known {
        msg.push_str(&format!(
            ".\n\nSource format '{}' is not recognized.",
            source_format
        ));
        if let Some(suggestion) = find_similar_format(source_format) {
            msg.push_str(&format!(" Did you mean '{}'?", suggestion));
        }
    } else if !target_known {
        msg.push_str(&format!(
            ".\n\nTarget format '{}' is not recognized.",
            target_format
        ));
        if let Some(suggestion) = find_similar_format(target_format) {
            msg.push_str(&format!(" Did you mean '{}'?", suggestion));
        }
    } else {
        // Both formats known but no path - suggest what IS possible
        msg.push_str(".\n\n");

        // Find what the source can convert to
        let source_targets = find_reachable_formats(source_format, registry);
        if !source_targets.is_empty() {
            msg.push_str(&format!(
                "'{}' can convert to: {}\n",
                source_format,
                source_targets.join(", ")
            ));
        } else {
            msg.push_str(&format!(
                "'{}' has no registered conversions.\n",
                source_format
            ));
        }

        // Find what can convert to target
        let target_sources = find_source_formats(target_format, registry);
        if !target_sources.is_empty() {
            msg.push_str(&format!(
                "'{}' can be created from: {}",
                target_format,
                target_sources.join(", ")
            ));
        }
    }

    msg
}

/// Build an error message for file read errors.
pub fn file_read_error(path: &str, err: &std::io::Error) -> String {
    use std::io::ErrorKind;

    let mut msg = format!("Failed to read '{}'", path);

    match err.kind() {
        ErrorKind::NotFound => {
            msg.push_str(": file not found");

            // Check for similar files in the same directory
            if let Some(suggestions) = find_similar_files(path) {
                if !suggestions.is_empty() {
                    msg.push_str(&format!(".\n\nDid you mean: {}?", suggestions.join(", ")));
                }
            }
        }
        ErrorKind::PermissionDenied => {
            msg.push_str(": permission denied. Check file permissions.");
        }
        ErrorKind::InvalidData => {
            msg.push_str(": file contains invalid data.");
        }
        _ => {
            msg.push_str(&format!(": {}", err));
        }
    }

    msg
}

/// Check if a format is in our known list.
fn is_known_format(format: &str) -> bool {
    let format = format.to_lowercase();
    DATA_FORMATS.contains(&format.as_str())
        || IMAGE_FORMATS.contains(&format.as_str())
        || AUDIO_FORMATS.contains(&format.as_str())
        || VIDEO_FORMATS.contains(&format.as_str())
}

/// Find a similar format name (for typo suggestions).
fn find_similar_format(input: &str) -> Option<&'static str> {
    let input = input.to_lowercase();

    // Common typos and aliases
    let aliases: &[(&str, &str)] = &[
        ("jpeg", "jpg"),
        ("tif", "tiff"),
        ("yml", "yaml"),
        ("htm", "html"),
        ("wave", "wav"),
        ("mpeg", "mp3"),
        ("msgpk", "msgpack"),
        ("jsonl", "ndjson"),
    ];

    for (alias, canonical) in aliases {
        if input == *alias {
            return Some(canonical);
        }
    }

    // Levenshtein distance 1-2 for short strings
    let all_formats: Vec<&str> = DATA_FORMATS
        .iter()
        .chain(IMAGE_FORMATS)
        .chain(AUDIO_FORMATS)
        .chain(VIDEO_FORMATS)
        .copied()
        .collect();

    for format in all_formats {
        if levenshtein(&input, format) <= 2 && input != format {
            return Some(format);
        }
    }

    None
}

/// Simple Levenshtein distance for short strings.
fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();

    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }

    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0; b.len() + 1];

    for (i, ca) in a.iter().enumerate() {
        curr[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            curr[j + 1] = (prev[j + 1] + 1).min(curr[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[b.len()]
}

/// Find formats reachable from a source format (1-hop only for simplicity).
fn find_reachable_formats(source: &str, registry: &Registry) -> Vec<String> {
    use rhi_paraphase_core::{Properties, PropertiesExt};

    let source_props = Properties::new().with("format", source);
    let mut targets = Vec::new();

    for decl in registry.declarations() {
        if decl.matches_input(&source_props).is_some() {
            // Get output format from first output port
            if let Some((_, port)) = decl.outputs.iter().next() {
                if let Some(pred) = port.pattern.predicates.get("format") {
                    if let rhi_paraphase_core::Predicate::Eq(val) = pred {
                        if let Some(fmt) = val.as_str() {
                            if !targets.contains(&fmt.to_string()) {
                                targets.push(fmt.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    targets.sort();
    targets
}

/// Find formats that can convert to the target format (1-hop only).
fn find_source_formats(target: &str, registry: &Registry) -> Vec<String> {
    let mut sources = Vec::new();

    for decl in registry.declarations() {
        // Check if any output matches target format
        let outputs_target = decl.outputs.values().any(|port| {
            port.pattern
                .predicates
                .get("format")
                .map(|pred| {
                    if let rhi_paraphase_core::Predicate::Eq(val) = pred {
                        val.as_str() == Some(target)
                    } else {
                        false
                    }
                })
                .unwrap_or(false)
        });

        if outputs_target {
            // Get input format
            for port in decl.inputs.values() {
                if let Some(pred) = port.pattern.predicates.get("format") {
                    if let rhi_paraphase_core::Predicate::Eq(val) = pred {
                        if let Some(fmt) = val.as_str() {
                            if !sources.contains(&fmt.to_string()) {
                                sources.push(fmt.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    sources.sort();
    sources
}

/// Find similar files in the same directory (for "did you mean" suggestions).
fn find_similar_files(path: &str) -> Option<Vec<String>> {
    let path = Path::new(path);
    let filename = path.file_name()?.to_str()?;
    let parent = path.parent().unwrap_or(Path::new("."));

    let entries = std::fs::read_dir(parent).ok()?;
    let mut suggestions = Vec::new();

    for entry in entries.flatten() {
        if let Some(name) = entry.file_name().to_str() {
            if levenshtein(filename, name) <= 2 && name != filename {
                suggestions.push(name.to_string());
            }
        }
    }

    suggestions.truncate(3); // Limit suggestions
    Some(suggestions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_similar_format() {
        assert_eq!(find_similar_format("jpeg"), Some("jpg"));
        assert_eq!(find_similar_format("yml"), Some("yaml"));
        assert_eq!(find_similar_format("pngg"), Some("png")); // typo
        assert_eq!(find_similar_format("zzzzz"), None); // nothing close
    }

    #[test]
    fn test_levenshtein() {
        assert_eq!(levenshtein("png", "png"), 0);
        assert_eq!(levenshtein("png", "pngg"), 1);
        assert_eq!(levenshtein("json", "jsn"), 1);
        assert_eq!(levenshtein("abc", "xyz"), 3);
    }

    #[test]
    fn test_is_known_format() {
        assert!(is_known_format("png"));
        assert!(is_known_format("JSON")); // case insensitive
        assert!(!is_known_format("unknown"));
    }
}

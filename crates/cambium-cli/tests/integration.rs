//! Integration tests for cambium CLI.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn cambium_bin() -> PathBuf {
    // Build the binary if needed and return its path
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("../../target/debug/cambium");
    path
}

fn test_data_dir() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/data");
    path
}

/// Get the features string for building the CLI.
/// This uses cfg! to check what features were enabled at compile time.
fn get_features_for_build() -> &'static str {
    // Check features at compile time using cfg!
    #[cfg(feature = "serde-all")]
    {
        return "serde-all";
    }
    #[cfg(all(feature = "serde", not(feature = "serde-all")))]
    {
        return "serde";
    }
    #[cfg(not(any(feature = "serde", feature = "serde-all")))]
    {
        return "";
    }
}

fn setup() {
    // Build the CLI with matching features
    let features = get_features_for_build();

    let status = if features.is_empty() {
        Command::new("cargo")
            .args(["build", "-p", "cambium-cli"])
            .status()
    } else {
        Command::new("cargo")
            .args(["build", "-p", "cambium-cli", "--features", features])
            .status()
    };

    status.expect("Failed to build CLI");

    // Create test data directory
    let data_dir = test_data_dir();
    fs::create_dir_all(&data_dir).ok();
}

#[test]
fn test_help() {
    setup();
    let output = Command::new(cambium_bin())
        .arg("--help")
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Type-driven data transformation"));
}

#[test]
fn test_list() {
    setup();
    let output = Command::new(cambium_bin())
        .arg("list")
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Available converters"));
}

#[test]
fn test_json_to_yaml_conversion() {
    setup();
    let data_dir = test_data_dir();

    // Create test JSON file
    let input = data_dir.join("test.json");
    let output = data_dir.join("test.yaml");
    fs::write(&input, r#"{"name": "test", "value": 42}"#).expect("Failed to write test file");

    // Convert
    let result = Command::new(cambium_bin())
        .args([
            "convert",
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute command");

    assert!(
        result.status.success(),
        "Command failed: {:?}",
        String::from_utf8_lossy(&result.stderr)
    );

    // Verify output exists
    assert!(output.exists(), "Output file not created");

    // Verify content
    let content = fs::read_to_string(&output).expect("Failed to read output");
    assert!(content.contains("name:") || content.contains("name :")); // YAML format
    assert!(content.contains("test"));

    // Cleanup
    fs::remove_file(input).ok();
    fs::remove_file(output).ok();
}

#[test]
fn test_yaml_to_json_conversion() {
    setup();
    let data_dir = test_data_dir();

    // Create test YAML file
    let input = data_dir.join("test2.yaml");
    let output = data_dir.join("test2.json");
    fs::write(&input, "name: hello\ncount: 123\n").expect("Failed to write test file");

    // Convert
    let result = Command::new(cambium_bin())
        .args([
            "convert",
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute command");

    assert!(
        result.status.success(),
        "Command failed: {:?}",
        String::from_utf8_lossy(&result.stderr)
    );

    // Verify output exists
    assert!(output.exists(), "Output file not created");

    // Verify content is JSON
    let content = fs::read_to_string(&output).expect("Failed to read output");
    assert!(content.contains("{"));
    assert!(content.contains("\"name\""));

    // Cleanup
    fs::remove_file(input).ok();
    fs::remove_file(output).ok();
}

#[test]
fn test_format_detection_from_to_flags() {
    setup();
    let data_dir = test_data_dir();

    // Create a file with no extension
    let input = data_dir.join("noext");
    let output = data_dir.join("noext_out");
    fs::write(&input, r#"{"foo": "bar"}"#).expect("Failed to write test file");

    // Convert with explicit format flags
    let result = Command::new(cambium_bin())
        .args([
            "convert",
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
            "--from",
            "json",
            "--to",
            "yaml",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(
        result.status.success(),
        "Command failed: {:?}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(output.exists(), "Output file not created");

    // Cleanup
    fs::remove_file(input).ok();
    fs::remove_file(output).ok();
}

#[test]
fn test_plan_command() {
    setup();
    let output = Command::new(cambium_bin())
        .args(["plan", "input.json", "output.yaml"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Planning:") || stdout.contains("json") || stdout.contains("yaml"));
}

#[test]
fn test_quiet_mode() {
    setup();
    let data_dir = test_data_dir();

    let input = data_dir.join("quiet_test.json");
    let output = data_dir.join("quiet_test.yaml");
    fs::write(&input, r#"{"test": true}"#).expect("Failed to write test file");

    // Convert with -q flag
    let result = Command::new(cambium_bin())
        .args([
            "-q",
            "convert",
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute command");

    assert!(result.status.success());
    // Quiet mode should produce no stdout
    assert!(result.stdout.is_empty(), "Expected no output in quiet mode");

    // Cleanup
    fs::remove_file(input).ok();
    fs::remove_file(output).ok();
}

#[test]
fn test_completions() {
    setup();
    let output = Command::new(cambium_bin())
        .args(["completions", "bash"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("complete") || stdout.contains("cambium"));
}

#[test]
fn test_error_on_missing_input() {
    setup();
    let result = Command::new(cambium_bin())
        .args(["convert", "nonexistent.json", "-o", "out.yaml"])
        .output()
        .expect("Failed to execute command");

    assert!(!result.status.success(), "Expected command to fail");
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("Failed to read") || stderr.contains("error") || stderr.contains("Error")
    );
}

// =============================================================================
// Multi-hop chain tests (3+ steps)
// =============================================================================

#[test]
fn test_3hop_chain_json_yaml_toml() {
    setup();
    let data_dir = test_data_dir();

    // Create test JSON file
    let input = data_dir.join("chain_test.json");
    let intermediate1 = data_dir.join("chain_test.yaml");
    let intermediate2 = data_dir.join("chain_test.toml");

    fs::write(&input, r#"{"name": "chain", "value": 123}"#).expect("Failed to write test file");

    // Step 1: JSON -> YAML
    let result = Command::new(cambium_bin())
        .args([
            "convert",
            input.to_str().unwrap(),
            "-o",
            intermediate1.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute command");
    assert!(
        result.status.success(),
        "JSON->YAML failed: {:?}",
        String::from_utf8_lossy(&result.stderr)
    );

    // Step 2: YAML -> TOML
    let result = Command::new(cambium_bin())
        .args([
            "convert",
            intermediate1.to_str().unwrap(),
            "-o",
            intermediate2.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute command");
    assert!(
        result.status.success(),
        "YAML->TOML failed: {:?}",
        String::from_utf8_lossy(&result.stderr)
    );

    // Verify final output
    let content = fs::read_to_string(&intermediate2).expect("Failed to read output");
    assert!(
        content.contains("name") && content.contains("chain"),
        "TOML content missing expected values"
    );

    // Cleanup
    fs::remove_file(input).ok();
    fs::remove_file(intermediate1).ok();
    fs::remove_file(intermediate2).ok();
}

#[test]
fn test_roundtrip_json_yaml_json() {
    setup();
    let data_dir = test_data_dir();

    let input = data_dir.join("roundtrip_input.json");
    let intermediate = data_dir.join("roundtrip.yaml");
    let output = data_dir.join("roundtrip_output.json");

    let original = r#"{"key": "value", "num": 42, "nested": {"a": 1}}"#;
    fs::write(&input, original).expect("Failed to write test file");

    // JSON -> YAML
    let result = Command::new(cambium_bin())
        .args([
            "convert",
            input.to_str().unwrap(),
            "-o",
            intermediate.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute command");
    assert!(
        result.status.success(),
        "JSON->YAML failed: {:?}",
        String::from_utf8_lossy(&result.stderr)
    );

    // YAML -> JSON
    let result = Command::new(cambium_bin())
        .args([
            "convert",
            intermediate.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute command");
    assert!(
        result.status.success(),
        "YAML->JSON failed: {:?}",
        String::from_utf8_lossy(&result.stderr)
    );

    // Verify roundtrip preserved data
    let final_content = fs::read_to_string(&output).expect("Failed to read output");
    assert!(final_content.contains("\"key\"") && final_content.contains("\"value\""));
    assert!(final_content.contains("42"));
    assert!(final_content.contains("nested"));

    // Cleanup
    fs::remove_file(input).ok();
    fs::remove_file(intermediate).ok();
    fs::remove_file(output).ok();
}

// =============================================================================
// Batch processing tests
// =============================================================================

#[test]
fn test_batch_convert_multiple_files() {
    setup();
    let data_dir = test_data_dir();
    let output_dir = data_dir.join("batch_output");
    fs::create_dir_all(&output_dir).ok();

    // Create multiple JSON files
    let files = ["batch1.json", "batch2.json", "batch3.json"];
    for (i, name) in files.iter().enumerate() {
        let path = data_dir.join(name);
        fs::write(&path, format!(r#"{{"id": {}, "name": "item{}"}}"#, i, i))
            .expect("Failed to write");
    }

    // Batch convert
    let result = Command::new(cambium_bin())
        .args([
            "convert",
            data_dir.join("batch1.json").to_str().unwrap(),
            data_dir.join("batch2.json").to_str().unwrap(),
            data_dir.join("batch3.json").to_str().unwrap(),
            "--output-dir",
            output_dir.to_str().unwrap(),
            "--to",
            "yaml",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(
        result.status.success(),
        "Batch convert failed: {:?}",
        String::from_utf8_lossy(&result.stderr)
    );

    // Verify all outputs exist
    for name in &files {
        let yaml_name = name.replace(".json", ".yaml");
        let output_path = output_dir.join(&yaml_name);
        assert!(output_path.exists(), "Missing output: {}", yaml_name);
    }

    // Cleanup
    for name in &files {
        fs::remove_file(data_dir.join(name)).ok();
        let yaml_name = name.replace(".json", ".yaml");
        fs::remove_file(output_dir.join(&yaml_name)).ok();
    }
    fs::remove_dir(&output_dir).ok();
}

#[test]
fn test_batch_with_progress() {
    setup();
    let data_dir = test_data_dir();
    let output_dir = data_dir.join("batch_progress_output");
    fs::create_dir_all(&output_dir).ok();

    // Create test files
    for i in 0..5 {
        let path = data_dir.join(format!("progress_{}.json", i));
        fs::write(&path, format!(r#"{{"num": {}}}"#, i)).expect("Failed to write");
    }

    // Batch convert (not quiet, should show progress)
    let result = Command::new(cambium_bin())
        .args([
            "convert",
            data_dir.join("progress_0.json").to_str().unwrap(),
            data_dir.join("progress_1.json").to_str().unwrap(),
            data_dir.join("progress_2.json").to_str().unwrap(),
            data_dir.join("progress_3.json").to_str().unwrap(),
            data_dir.join("progress_4.json").to_str().unwrap(),
            "--output-dir",
            output_dir.to_str().unwrap(),
            "--to",
            "yaml",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(
        result.status.success(),
        "Batch failed: {:?}",
        String::from_utf8_lossy(&result.stderr)
    );

    // Verify outputs
    for i in 0..5 {
        assert!(output_dir.join(format!("progress_{}.yaml", i)).exists());
    }

    // Cleanup
    for i in 0..5 {
        fs::remove_file(data_dir.join(format!("progress_{}.json", i))).ok();
        fs::remove_file(output_dir.join(format!("progress_{}.yaml", i))).ok();
    }
    fs::remove_dir(&output_dir).ok();
}

// =============================================================================
// Encoding chain tests (require serde-all feature)
// Run with: cargo test -p cambium-cli --features serde-all
// =============================================================================

// These tests require non-default features. To run them:
// cargo test -p cambium-cli --features serde-all -- test_compression
// cargo test -p cambium-cli --features serde-all -- test_base64

#[test]
#[ignore = "requires serde-all feature: cargo test -p cambium-cli --features serde-all"]
fn test_compression_chain_gzip() {
    setup();
    let data_dir = test_data_dir();

    let input = data_dir.join("compress_test.json");
    let compressed = data_dir.join("compress_test.json.gz");
    let decompressed = data_dir.join("compress_test_out.json");

    let original = r#"{"data": "This is some test data that should compress well when repeated. This is some test data that should compress well when repeated."}"#;
    fs::write(&input, original).expect("Failed to write");

    // Compress: JSON -> gzip
    let result = Command::new(cambium_bin())
        .args([
            "convert",
            input.to_str().unwrap(),
            "-o",
            compressed.to_str().unwrap(),
            "--from",
            "bytes",
            "--to",
            "gzip",
        ])
        .output()
        .expect("Failed to execute command");
    assert!(
        result.status.success(),
        "Compression failed: {:?}",
        String::from_utf8_lossy(&result.stderr)
    );

    // Verify compressed file is smaller
    let original_size = fs::metadata(&input).unwrap().len();
    let compressed_size = fs::metadata(&compressed).unwrap().len();
    assert!(
        compressed_size < original_size,
        "Compressed should be smaller"
    );

    // Decompress: gzip -> bytes
    let result = Command::new(cambium_bin())
        .args([
            "convert",
            compressed.to_str().unwrap(),
            "-o",
            decompressed.to_str().unwrap(),
            "--from",
            "gzip",
            "--to",
            "bytes",
        ])
        .output()
        .expect("Failed to execute command");
    assert!(
        result.status.success(),
        "Decompression failed: {:?}",
        String::from_utf8_lossy(&result.stderr)
    );

    // Verify roundtrip
    let final_content = fs::read_to_string(&decompressed).expect("Failed to read");
    assert_eq!(original, final_content, "Roundtrip should preserve content");

    // Cleanup
    fs::remove_file(input).ok();
    fs::remove_file(compressed).ok();
    fs::remove_file(decompressed).ok();
}

#[test]
#[ignore = "requires serde-all feature: cargo test -p cambium-cli --features serde-all"]
fn test_base64_roundtrip() {
    setup();
    let data_dir = test_data_dir();

    let input = data_dir.join("b64_input.txt");
    let encoded = data_dir.join("b64_encoded.txt");
    let decoded = data_dir.join("b64_decoded.txt");

    let original = "Hello, World! This is a test of base64 encoding.";
    fs::write(&input, original).expect("Failed to write");

    // Encode
    let result = Command::new(cambium_bin())
        .args([
            "convert",
            input.to_str().unwrap(),
            "-o",
            encoded.to_str().unwrap(),
            "--from",
            "bytes",
            "--to",
            "base64",
        ])
        .output()
        .expect("Failed to execute command");
    assert!(
        result.status.success(),
        "Encode failed: {:?}",
        String::from_utf8_lossy(&result.stderr)
    );

    // Verify it's base64 (alphanumeric + /+=)
    let encoded_content = fs::read_to_string(&encoded).expect("Failed to read");
    assert!(encoded_content.chars().all(|c| c.is_alphanumeric()
        || c == '+'
        || c == '/'
        || c == '='
        || c == '\n'));

    // Decode
    let result = Command::new(cambium_bin())
        .args([
            "convert",
            encoded.to_str().unwrap(),
            "-o",
            decoded.to_str().unwrap(),
            "--from",
            "base64",
            "--to",
            "bytes",
        ])
        .output()
        .expect("Failed to execute command");
    assert!(
        result.status.success(),
        "Decode failed: {:?}",
        String::from_utf8_lossy(&result.stderr)
    );

    let final_content = fs::read_to_string(&decoded).expect("Failed to read");
    assert_eq!(original, final_content);

    // Cleanup
    fs::remove_file(input).ok();
    fs::remove_file(encoded).ok();
    fs::remove_file(decoded).ok();
}

// =============================================================================
// Optimize flag tests
// =============================================================================

#[test]
fn test_plan_with_optimize_quality() {
    setup();
    let output = Command::new(cambium_bin())
        .args(["plan", "input.json", "output.yaml", "--optimize", "quality"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("optimize: quality"));
}

#[test]
fn test_plan_with_optimize_speed() {
    setup();
    let output = Command::new(cambium_bin())
        .args(["plan", "input.json", "output.yaml", "--optimize", "speed"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("optimize: speed"));
}

// =============================================================================
// Preset tests
// =============================================================================

#[test]
fn test_presets_command() {
    setup();
    let output = Command::new(cambium_bin())
        .args(["presets"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("web"));
    assert!(stdout.contains("thumbnail"));
    assert!(stdout.contains("social"));
}

// =============================================================================
// Glob and recursive tests
// =============================================================================

#[test]
fn test_glob_pattern_expansion() {
    setup();
    let data_dir = test_data_dir();
    // Use unique subdirectory to avoid picking up files from other tests
    let input_dir = data_dir.join("glob_input");
    let output_dir = data_dir.join("glob_output");
    fs::create_dir_all(&input_dir).ok();
    fs::create_dir_all(&output_dir).ok();

    // Create test files in isolated directory
    for i in 1..=3 {
        let path = input_dir.join(format!("glob_test_{}.json", i));
        fs::write(&path, format!(r#"{{"id": {}}}"#, i)).expect("Failed to write");
    }

    // Use glob pattern on isolated directory
    let pattern = format!("{}/*.json", input_dir.to_string_lossy());
    let result = Command::new(cambium_bin())
        .args([
            "convert",
            &pattern,
            "--output-dir",
            output_dir.to_str().unwrap(),
            "--to",
            "yaml",
        ])
        .output()
        .expect("Failed to execute command");

    // Should succeed and convert exactly 3 files
    assert!(
        result.status.success(),
        "Glob convert failed: {:?}",
        String::from_utf8_lossy(&result.stderr)
    );

    // Verify exactly 3 outputs exist
    for i in 1..=3 {
        let yaml_path = output_dir.join(format!("glob_test_{}.yaml", i));
        assert!(yaml_path.exists(), "Missing output: glob_test_{}.yaml", i);
    }

    // Cleanup
    fs::remove_dir_all(&input_dir).ok();
    fs::remove_dir_all(&output_dir).ok();
}

#[test]
fn test_recursive_directory() {
    setup();
    let data_dir = test_data_dir();
    let tree_dir = data_dir.join("tree_test");
    let sub_dir = tree_dir.join("subdir");
    let output_dir = data_dir.join("tree_output");

    fs::create_dir_all(&sub_dir).ok();
    fs::create_dir_all(&output_dir).ok();

    // Create files at different levels
    fs::write(tree_dir.join("root.json"), r#"{"level": "root"}"#).expect("Failed to write");
    fs::write(sub_dir.join("nested.json"), r#"{"level": "nested"}"#).expect("Failed to write");

    // Recursive convert
    let result = Command::new(cambium_bin())
        .args([
            "convert",
            tree_dir.to_str().unwrap(),
            "-r",
            "--output-dir",
            output_dir.to_str().unwrap(),
            "--to",
            "yaml",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(
        result.status.success(),
        "Recursive convert failed: {:?}",
        String::from_utf8_lossy(&result.stderr)
    );

    // Cleanup
    fs::remove_file(tree_dir.join("root.json")).ok();
    fs::remove_file(sub_dir.join("nested.json")).ok();
    fs::remove_dir(&sub_dir).ok();
    fs::remove_dir(&tree_dir).ok();
    fs::remove_dir_all(&output_dir).ok();
}

#[test]
#[ignore = "requires serde-all feature: cargo test -p cambium-cli --features serde-all"]
fn test_aggregate_to_tar() {
    setup();
    let data_dir = test_data_dir();
    let aggregate_dir = data_dir.join("aggregate_tar");
    fs::create_dir_all(&aggregate_dir).ok();

    // Create test files
    fs::write(aggregate_dir.join("file1.txt"), "Content of file 1").expect("Failed to write");
    fs::write(aggregate_dir.join("file2.txt"), "Content of file 2").expect("Failed to write");
    fs::write(aggregate_dir.join("file3.txt"), "Content of file 3").expect("Failed to write");

    let output_tar = aggregate_dir.join("output.tar");

    // Aggregate to tar
    let result = Command::new(cambium_bin())
        .args([
            "convert",
            aggregate_dir.join("file1.txt").to_str().unwrap(),
            aggregate_dir.join("file2.txt").to_str().unwrap(),
            aggregate_dir.join("file3.txt").to_str().unwrap(),
            "--aggregate",
            "--to",
            "tar",
            "-o",
            output_tar.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute command");

    assert!(
        result.status.success(),
        "Tar aggregation failed: {:?}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(output_tar.exists(), "Tar archive was not created");

    // Verify it's a valid tar (starts with file header)
    let tar_data = fs::read(&output_tar).expect("Failed to read tar");
    assert!(tar_data.len() > 100, "Tar archive is too small");

    // Cleanup
    fs::remove_dir_all(&aggregate_dir).ok();
}

#[test]
#[ignore = "requires serde-all feature: cargo test -p cambium-cli --features serde-all"]
fn test_aggregate_to_zip() {
    setup();
    let data_dir = test_data_dir();
    let aggregate_dir = data_dir.join("aggregate_zip");
    fs::create_dir_all(&aggregate_dir).ok();

    // Create test files
    fs::write(aggregate_dir.join("a.json"), r#"{"id": 1}"#).expect("Failed to write");
    fs::write(aggregate_dir.join("b.json"), r#"{"id": 2}"#).expect("Failed to write");

    let output_zip = aggregate_dir.join("output.zip");

    // Aggregate to zip (auto-detected from --to zip)
    let result = Command::new(cambium_bin())
        .args([
            "convert",
            aggregate_dir.join("a.json").to_str().unwrap(),
            aggregate_dir.join("b.json").to_str().unwrap(),
            "--to",
            "zip",
            "-o",
            output_zip.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute command");

    assert!(
        result.status.success(),
        "Zip aggregation failed: {:?}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(output_zip.exists(), "Zip archive was not created");

    // Verify it's a valid zip (starts with PK signature)
    let zip_data = fs::read(&output_zip).expect("Failed to read zip");
    assert!(zip_data.len() > 50, "Zip archive is too small");
    assert_eq!(&zip_data[0..2], b"PK", "Invalid zip signature");

    // Cleanup
    fs::remove_dir_all(&aggregate_dir).ok();
}

#[test]
#[ignore = "requires serde-all feature: cargo test -p cambium-cli --features serde-all"]
fn test_aggregate_to_tar_gz() {
    setup();
    let data_dir = test_data_dir();
    let aggregate_dir = data_dir.join("aggregate_tar_gz");
    fs::create_dir_all(&aggregate_dir).ok();

    // Create test files
    fs::write(aggregate_dir.join("readme.txt"), "This is a readme").expect("Failed to write");
    fs::write(aggregate_dir.join("data.txt"), "Some data content").expect("Failed to write");

    let output_tgz = aggregate_dir.join("archive.tar.gz");

    // Aggregate to tar.gz (compound format)
    let result = Command::new(cambium_bin())
        .args([
            "convert",
            aggregate_dir.join("readme.txt").to_str().unwrap(),
            aggregate_dir.join("data.txt").to_str().unwrap(),
            "--aggregate",
            "--to",
            "tar.gz",
            "-o",
            output_tgz.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute command");

    assert!(
        result.status.success(),
        "tar.gz aggregation failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(output_tgz.exists(), "tar.gz archive was not created");

    // Verify it's a valid gzip (magic bytes: 1f 8b)
    let gz_data = fs::read(&output_tgz).expect("Failed to read tar.gz");
    assert!(gz_data.len() > 20, "tar.gz archive is too small");
    assert_eq!(
        gz_data[0..2],
        [0x1f, 0x8b],
        "File doesn't have gzip magic bytes"
    );

    // Cleanup
    fs::remove_dir_all(&aggregate_dir).ok();
}

#[test]
#[ignore = "requires serde-all feature: cargo test -p cambium-cli --features serde-all"]
fn test_aggregate_to_tgz_alias() {
    setup();
    let data_dir = test_data_dir();
    let aggregate_dir = data_dir.join("aggregate_tgz");
    fs::create_dir_all(&aggregate_dir).ok();

    // Create test file
    fs::write(aggregate_dir.join("test.txt"), "Test content").expect("Failed to write");

    let output_tgz = aggregate_dir.join("archive.tgz");

    // Aggregate using tgz alias (auto-detected as aggregation)
    let result = Command::new(cambium_bin())
        .args([
            "convert",
            aggregate_dir.join("test.txt").to_str().unwrap(),
            "--from",
            "raw", // txt files need explicit format
            "--to",
            "tgz",
            "-o",
            output_tgz.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute command");

    assert!(
        result.status.success(),
        "tgz aggregation failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(output_tgz.exists(), "tgz archive was not created");

    // Verify gzip magic bytes
    let gz_data = fs::read(&output_tgz).expect("Failed to read tgz");
    assert_eq!(
        gz_data[0..2],
        [0x1f, 0x8b],
        "File doesn't have gzip magic bytes"
    );

    // Cleanup
    fs::remove_dir_all(&aggregate_dir).ok();
}

//! Dew expression evaluation for dynamic presets.
#![allow(dead_code)]
#![allow(clippy::collapsible_if)]
//!
//! This module provides the integration between Cambium's property system
//! and Dew's expression language, enabling dynamic preset values.

use rhi_paraphase_core::Properties;
use rhizome_dew_core::Expr;
use rhizome_dew_scalar::{FunctionRegistry, eval, scalar_registry};
use std::collections::HashMap;

/// Errors that can occur during expression evaluation.
#[derive(Debug)]
pub enum ExprError {
    /// Failed to parse the expression.
    Parse(rhizome_dew_core::ParseError),
    /// Failed to evaluate the expression.
    Eval(rhizome_dew_scalar::Error),
}

impl std::fmt::Display for ExprError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExprError::Parse(e) => write!(f, "expression parse error: {e}"),
            ExprError::Eval(e) => write!(f, "expression eval error: {e}"),
        }
    }
}

impl std::error::Error for ExprError {}

impl From<rhizome_dew_core::ParseError> for ExprError {
    fn from(e: rhizome_dew_core::ParseError) -> Self {
        ExprError::Parse(e)
    }
}

impl From<rhizome_dew_scalar::Error> for ExprError {
    fn from(e: rhizome_dew_scalar::Error) -> Self {
        ExprError::Eval(e)
    }
}

/// Convert Cambium Properties to Dew variable map.
///
/// Only numeric values (Int, Float) are included. Other types are skipped.
pub fn props_to_vars(props: &Properties) -> HashMap<String, f64> {
    props
        .iter()
        .filter_map(|(key, value)| value.as_f64().map(|v| (key.clone(), v)))
        .collect()
}

/// Evaluate a string that is either a literal number or a Dew expression.
///
/// If the string parses as a number, returns that number directly.
/// Otherwise, parses as a Dew expression and evaluates against the variables.
pub fn eval_numeric(expr_str: &str, vars: &HashMap<String, f64>) -> Result<f64, ExprError> {
    // Try parsing as literal number first (fast path)
    if let Ok(n) = expr_str.trim().parse::<f64>() {
        return Ok(n);
    }

    // Parse and evaluate as Dew expression
    let expr = Expr::parse(expr_str)?;
    let registry: FunctionRegistry<f64> = scalar_registry();
    let result = eval(expr.ast(), vars, &registry)?;
    Ok(result)
}

/// Evaluate a numeric expression and convert to u32.
pub fn eval_u32(expr_str: &str, vars: &HashMap<String, f64>) -> Result<u32, ExprError> {
    let value = eval_numeric(expr_str, vars)?;
    Ok(value.round() as u32)
}

/// Evaluate a numeric expression and convert to f64.
pub fn eval_f64(expr_str: &str, vars: &HashMap<String, f64>) -> Result<f64, ExprError> {
    eval_numeric(expr_str, vars)
}

/// Check if a string looks like it might be an expression (vs a plain string value).
///
/// Returns true if the string contains expression-like characters.
pub fn is_expression(s: &str) -> bool {
    let s = s.trim();
    // Contains operators, function calls, or conditionals
    s.contains('+')
        || s.contains('-')
        || s.contains('*')
        || s.contains('/')
        || s.contains('(')
        || s.contains('<')
        || s.contains('>')
        || s.contains("if ")
        || s.contains("then ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_literal_number() {
        let vars = HashMap::new();
        assert_eq!(eval_numeric("42", &vars).unwrap(), 42.0);
        assert_eq!(eval_numeric("3.15", &vars).unwrap(), 3.15);
        assert_eq!(eval_numeric(" 100 ", &vars).unwrap(), 100.0);
    }

    #[test]
    fn test_simple_expression() {
        let vars: HashMap<String, f64> =
            [("width".into(), 1920.0), ("height".into(), 1080.0)].into();

        assert_eq!(eval_numeric("width", &vars).unwrap(), 1920.0);
        assert_eq!(eval_numeric("width + height", &vars).unwrap(), 3000.0);
        assert_eq!(eval_numeric("width * 2", &vars).unwrap(), 3840.0);
    }

    #[test]
    fn test_min_max() {
        let vars: HashMap<String, f64> = [("width".into(), 4000.0)].into();

        assert_eq!(eval_numeric("min(width, 1920)", &vars).unwrap(), 1920.0);
        assert_eq!(eval_numeric("max(width, 1920)", &vars).unwrap(), 4000.0);
    }

    #[test]
    fn test_conditional() {
        let vars: HashMap<String, f64> = [("file_size".into(), 8_000_000.0)].into();

        // if file_size > 5MB then 70 else 85
        let result = eval_numeric("if file_size > 5000000 then 70 else 85", &vars).unwrap();
        assert_eq!(result, 70.0);

        let vars_small: HashMap<String, f64> = [("file_size".into(), 2_000_000.0)].into();
        let result = eval_numeric("if file_size > 5000000 then 70 else 85", &vars_small).unwrap();
        assert_eq!(result, 85.0);
    }

    #[test]
    fn test_eval_u32() {
        let vars: HashMap<String, f64> = [("width".into(), 1920.5)].into();
        assert_eq!(eval_u32("width", &vars).unwrap(), 1921); // rounds
        assert_eq!(eval_u32("1200", &vars).unwrap(), 1200);
    }

    #[test]
    fn test_props_to_vars() {
        use rhi_paraphase_core::PropertiesExt;

        let props = Properties::new()
            .with("width", 1920i64)
            .with("height", 1080i64)
            .with("format", "png") // string, should be skipped
            .with("quality", 0.85f64);

        let vars = props_to_vars(&props);

        assert_eq!(vars.get("width"), Some(&1920.0));
        assert_eq!(vars.get("height"), Some(&1080.0));
        assert_eq!(vars.get("quality"), Some(&0.85));
        assert_eq!(vars.get("format"), None); // string skipped
    }

    #[test]
    fn test_is_expression() {
        assert!(!is_expression("42"));
        assert!(!is_expression("hello"));
        assert!(is_expression("width + 10"));
        assert!(is_expression("min(width, 1920)"));
        assert!(is_expression("if x > 0 then 1 else 0"));
        assert!(is_expression("width * 2"));
    }

    #[test]
    fn test_clamp_expression() {
        let vars: HashMap<String, f64> = [("quality".into(), 150.0)].into();
        assert_eq!(
            eval_numeric("clamp(quality, 0, 100)", &vars).unwrap(),
            100.0
        );
    }

    #[test]
    fn test_complex_expression() {
        let vars: HashMap<String, f64> = [
            ("width".into(), 4000.0),
            ("height".into(), 3000.0),
            ("file_size".into(), 10_000_000.0),
        ]
        .into();

        // Smart resize: limit to 1920 width, scale height proportionally
        let new_width = eval_numeric("min(width, 1920)", &vars).unwrap();
        assert_eq!(new_width, 1920.0);

        // Quality based on original size
        let quality = eval_numeric(
            "if file_size > 5000000 then 75 else if file_size > 1000000 then 85 else 95",
            &vars,
        )
        .unwrap();
        assert_eq!(quality, 75.0);
    }
}

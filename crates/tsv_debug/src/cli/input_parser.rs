//! Common input parsing utilities for CLI commands

use tsv_cli::cli::args::Args;
use tsv_cli::cli::input::{Input, ParserType};

/// Parse input from command-line arguments
///
/// Supports three input modes:
/// 1. File path (positional): auto-detects parser from extension
/// 2. `--content <string> --parser <type>`: explicit content with required parser
/// 3. `--stdin --parser <type>`: stdin with required parser
///
/// # Errors
///
/// Returns error if:
/// - No input provided
/// - `--content` or `--stdin` used without `--parser`
/// - Invalid parser type
/// - File not found
pub fn parse_input_and_parser_type(args: &mut Args) -> Result<(Input, ParserType), String> {
    if let Some(content) = args.option("content") {
        // --content requires --parser
        let parser = args
            .option("parser")
            .ok_or("Error: --parser required when using --content")?
            .parse()?;
        Ok((Input::from_content(content), parser))
    } else if args.flag("stdin") {
        // --stdin requires --parser
        let parser = args
            .option("parser")
            .ok_or("Error: --parser required when using --stdin")?
            .parse()?;
        Ok((Input::from_stdin()?, parser))
    } else if let Some(path) = args.positional() {
        let parser = ParserType::from_extension(&path);
        Ok((Input::from_file(&path)?, parser))
    } else {
        Err("No input provided. Use a file path, --content, or --stdin".to_string())
    }
}

/// Parse optional second input (for commands like ast_diff that can compare two files)
///
/// Returns `None` if no second input is provided.
pub fn parse_optional_second_input(
    args: &mut Args,
    _parser_type: ParserType,
) -> Result<Option<Input>, String> {
    if let Some(path) = args.positional() {
        Ok(Some(Input::from_file(&path)?))
    } else {
        Ok(None)
    }
}

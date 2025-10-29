mod ast;
mod lexer;
mod location;
mod parser;
mod error;
mod span;

use location::LocationTracker;
use parser::{parse_typescript, parse_svelte};

// Re-export AST types
pub use ast::internal;
pub use ast::public;
pub use error::ParseError;
pub use span::Span;

/// Parse TypeScript/JavaScript source and return internal AST
pub fn parse_typescript_ast(source: &str) -> Result<internal::Program, ParseError> {
    // Check file size limit (u32::MAX for span positions)
    if source.len() > u32::MAX as usize {
        return Err(ParseError::FileTooLarge {
            size: source.len(),
            max: u32::MAX as usize,
        });
    }
    parse_typescript(source)
}

/// Parse Svelte source and return internal AST
pub fn parse_svelte_ast(source: &str) -> Result<internal::Root, ParseError> {
    // Check file size limit (u32::MAX for span positions)
    if source.len() > u32::MAX as usize {
        return Err(ParseError::FileTooLarge {
            size: source.len(),
            max: u32::MAX as usize,
        });
    }
    parse_svelte(source)
}

/// Parse and convert to JSON (for testing/compatibility)
/// Automatically detects whether input is Svelte or TypeScript
pub fn parse_to_json(source: &str) -> Result<String, String> {
    parse_to_json_with_options(source, false)
}

/// Parse and convert to JSON with formatting options
pub fn parse_to_json_with_options(source: &str, pretty: bool) -> Result<String, String> {
    // Detect whether this is a Svelte file or TypeScript file
    // Simple heuristic: if it starts with '<', it's Svelte
    let is_svelte = source.trim_start().starts_with('<');

    let json = if is_svelte {
        // Parse as Svelte
        let internal_ast = parse_svelte(source)
            .map_err(|e| e.to_string())?;

        // Convert to public AST
        let public_ast = ast::convert_root(&internal_ast, source);

        // Serialize to JSON
        if pretty {
            serde_json::to_string_pretty(&public_ast)
        } else {
            serde_json::to_string(&public_ast)
        }
    } else {
        // Parse as TypeScript
        let location_tracker = LocationTracker::new(source);
        let internal_ast = parse_typescript(source)
            .map_err(|e| e.to_string())?;

        // Convert to public AST
        let public_ast = ast::convert_program(&internal_ast, &location_tracker);

        // Serialize to JSON
        if pretty {
            serde_json::to_string_pretty(&public_ast)
        } else {
            serde_json::to_string(&public_ast)
        }
    };

    json.map_err(|e| format!("JSON serialization failed: {}", e))
}

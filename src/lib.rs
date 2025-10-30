mod ast;
mod lexer;
mod location;
mod parser;
mod error;
mod span;

use location::LocationTracker;
use parser::{parse_typescript, parse_svelte, parse_css};

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

/// Parse CSS source and return internal AST
/// Can be used standalone or called internally by Svelte parser for <style> tags
pub fn parse_css_ast(source: &str) -> Result<Vec<internal::CssNode>, ParseError> {
    // Check file size limit (u32::MAX for span positions)
    if source.len() > u32::MAX as usize {
        return Err(ParseError::FileTooLarge {
            size: source.len(),
            max: u32::MAX as usize,
        });
    }
    parse_css(source, 0)
}

/// Parse and convert to JSON (for testing/compatibility)
/// Automatically detects whether input is Svelte or TypeScript
pub fn parse_to_json(source: &str) -> Result<String, String> {
    parse_to_json_with_options(source, false)
}

/// Parse and convert to JSON with formatting options
pub fn parse_to_json_with_options(source: &str, pretty: bool) -> Result<String, String> {
    // Detect whether this is a Svelte file, TypeScript file, or CSS file
    // Simple heuristic: if it starts with '<', it's Svelte; otherwise check for CSS-like content
    let trimmed = source.trim_start();
    let is_svelte = trimmed.starts_with('<');
    let is_css = !is_svelte && (trimmed.ends_with('}') || trimmed.contains('{') && trimmed.contains(':'));

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
    } else if is_css {
        // Parse as CSS
        let internal_css = parse_css(source, 0)
            .map_err(|e| e.to_string())?;

        // Convert to public CSS AST (build a CSS Root-like structure)
        let css_json = ast::convert_css_nodes(&internal_css, source);

        // Serialize to JSON
        if pretty {
            serde_json::to_string_pretty(&css_json)
        } else {
            serde_json::to_string(&css_json)
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

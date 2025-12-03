// Error types for parsing

use thiserror::Error;

/// Rich error context with source snippet and position
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorContext {
    /// The source line containing the error
    pub source_line: String,
    /// Column position within the line (0-indexed)
    pub column: usize,
    /// Line number in the source (1-indexed)
    pub line_number: usize,
}

impl ErrorContext {
    /// Extract error context from source code at a given byte position
    ///
    /// Returns None if position is out of bounds or source is empty
    pub fn from_source(source: &str, position: usize) -> Option<Self> {
        if source.is_empty() || position > source.len() {
            return None;
        }

        let position = position.min(source.len());

        // Find line start: search backwards for '\n' and go past it
        let line_start = source[..position].rfind('\n').map_or(0, |i| i + 1);

        // Find line end: search forwards for '\n' or end of string
        let line_end = source[position..]
            .find('\n')
            .map_or(source.len(), |i| position + i);

        // Extract the line
        let source_line = source[line_start..line_end].to_string();

        // Calculate column (bytes from line start to error position)
        let column = position.saturating_sub(line_start);

        // Calculate line number (1-indexed)
        let line_number = source[..line_start].matches('\n').count() + 1;

        Some(ErrorContext {
            source_line,
            column,
            line_number,
        })
    }

    /// Format error context with caret pointer
    pub fn format_with_caret(&self, message: &str) -> String {
        let indent = " ".repeat(format!("{}:", self.line_number).len() + self.column + 1);
        format!(
            "{}\n{}:{} {}\n{}^ here",
            message,
            self.line_number,
            self.column + 1, // Display as 1-indexed for users
            self.source_line,
            indent,
        )
    }
}

/// Format error message with context (caret pointer) or position fallback
fn format_error(base_msg: &str, position: usize, context: Option<&ErrorContext>) -> String {
    if let Some(ctx) = context {
        ctx.format_with_caret(base_msg)
    } else {
        format!("{base_msg} at position {position}")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ParseError {
    #[error("{}", format_error(&format!("Expected {expected}, found {found}"), *position, context.as_ref()))]
    UnexpectedToken {
        expected: String,
        found: String,
        position: usize,
        context: Option<ErrorContext>,
    },
    #[error("{}", format_error("Unexpected end of file", *position, context.as_ref()))]
    UnexpectedEof {
        position: usize,
        context: Option<ErrorContext>,
    },
    #[error("{}", format_error(message, *position, context.as_ref()))]
    InvalidSyntax {
        message: String,
        position: usize,
        context: Option<ErrorContext>,
    },
    #[error("{}", format_error(&format!("Expected expression, found {found}"), *position, context.as_ref()))]
    InvalidExpression {
        found: String,
        position: usize,
        context: Option<ErrorContext>,
    },
    #[error("File too large: {size} bytes (maximum: {max} bytes / 4GB)")]
    FileTooLarge { size: usize, max: usize },
}

/// Result type alias for parsing operations
pub type Result<T> = std::result::Result<T, ParseError>;

impl ParseError {
    /// Add source context to an error
    ///
    /// Call this to enrich errors with source snippets for better debugging.
    /// Example:
    /// ```ignore
    /// let err = ParseError::UnexpectedToken { ... };
    /// let rich_err = err.with_context(source);
    /// ```
    pub fn with_context(self, source: &str) -> Self {
        match self {
            ParseError::UnexpectedToken {
                expected,
                found,
                position,
                context: _,
            } => ParseError::UnexpectedToken {
                expected,
                found,
                position,
                context: ErrorContext::from_source(source, position),
            },
            ParseError::UnexpectedEof {
                position,
                context: _,
            } => ParseError::UnexpectedEof {
                position,
                context: ErrorContext::from_source(source, position),
            },
            ParseError::InvalidSyntax {
                message,
                position,
                context: _,
            } => ParseError::InvalidSyntax {
                message,
                position,
                context: ErrorContext::from_source(source, position),
            },
            ParseError::InvalidExpression {
                found,
                position,
                context: _,
            } => ParseError::InvalidExpression {
                found,
                position,
                context: ErrorContext::from_source(source, position),
            },
            other => other, // FileTooLarge doesn't need context
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_context_at_eof_no_newline() {
        // Position at EOF, source doesn't end with newline
        let source = "hello";
        let ctx = ErrorContext::from_source(source, 5).unwrap();
        assert_eq!(ctx.source_line, "hello");
        assert_eq!(ctx.column, 5);
        assert_eq!(ctx.line_number, 1);
    }

    #[test]
    fn test_error_context_at_eof_with_newline() {
        // Position at EOF, source ends with newline
        let source = "hello\n";
        let ctx = ErrorContext::from_source(source, 6).unwrap();
        assert_eq!(ctx.source_line, ""); // Empty line after newline
        assert_eq!(ctx.column, 0);
        assert_eq!(ctx.line_number, 2);
    }

    #[test]
    fn test_error_context_middle_of_line() {
        let source = "abc\ndef\nghi";
        let ctx = ErrorContext::from_source(source, 5).unwrap(); // 'e' in "def"
        assert_eq!(ctx.source_line, "def");
        assert_eq!(ctx.column, 1);
        assert_eq!(ctx.line_number, 2);
    }

    #[test]
    fn test_error_context_start_of_file() {
        let source = "hello";
        let ctx = ErrorContext::from_source(source, 0).unwrap();
        assert_eq!(ctx.source_line, "hello");
        assert_eq!(ctx.column, 0);
        assert_eq!(ctx.line_number, 1);
    }

    #[test]
    fn test_error_context_empty_source() {
        assert!(ErrorContext::from_source("", 0).is_none());
    }

    #[test]
    fn test_error_context_position_out_of_bounds() {
        assert!(ErrorContext::from_source("hello", 10).is_none());
    }
}

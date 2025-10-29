// Error types for parsing

use crate::lexer::TokenKind;
use std::fmt;

// TODO: Add source context to errors for better user experience:
// - Store source snippet around error position
// - Show "^" pointer under the error
// - Example:
//   ```
//   const x: = 5;
//          ^ Expected type annotation, found '='
//   ```
// Implementation approach:
// 1. Add optional `source_context: Option<String>` to each variant
// 2. In Display impl, format with context when available
// 3. Helper function to extract 1-2 lines around error position
#[derive(Debug, Clone, PartialEq)]
pub enum ParseError {
    UnexpectedToken {
        expected: TokenKind,
        found: TokenKind,
        position: usize,
    },
    UnexpectedEof {
        position: usize,
    },
    InvalidSyntax {
        message: String,
        position: usize,
    },
    InvalidExpression {
        found: TokenKind,
        position: usize,
    },
    FileTooLarge {
        size: usize,
        max: usize,
    },
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::UnexpectedToken { expected, found, position } => {
                write!(f, "Expected {}, found {} at position {}", expected, found, position)
            }
            ParseError::UnexpectedEof { position } => {
                write!(f, "Unexpected end of file at position {}", position)
            }
            ParseError::InvalidSyntax { message, position } => {
                write!(f, "{} at position {}", message, position)
            }
            ParseError::InvalidExpression { found, position } => {
                write!(f, "Expected expression, found {} at position {}", found, position)
            }
            ParseError::FileTooLarge { size, max } => {
                write!(f, "File too large: {} bytes (maximum: {} bytes / 4GB)", size, max)
            }
        }
    }
}

impl std::error::Error for ParseError {}

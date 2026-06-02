// Identifier validation for the TypeScript printer.

use crate::lexer::ident::{is_id_continue, is_id_start};

/// Check if a string is a valid JS identifier (so prettier outputs it unquoted).
///
/// Built on the lexer's identifier grammar (`lexer::ident`) so any key we
/// unquote here can be re-lexed as an identifier (idempotency). Reserved words
/// count as identifiers here (prettier outputs them unquoted).
pub(super) fn is_valid_js_identifier(s: &str) -> bool {
    let mut chars = s.chars();

    match chars.next() {
        Some(c) if is_id_start(c) => {}
        _ => return false,
    }

    chars.all(is_id_continue)
}

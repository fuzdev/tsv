// ECMAScript identifier grammar.
//
// Single source of truth for "is this char a valid identifier start/continue?",
// shared by the lexer (tokenizing identifiers) and the printer (deciding whether
// a quoted object key can be unquoted). Keeping both on these helpers guarantees
// the printer never unquotes a key the lexer couldn't re-lex as an identifier.

use unicode_ident::{is_xid_continue, is_xid_start};

/// `IdentifierStart`: `XID_Start`, plus the ECMAScript `_` and `$` allowances.
///
/// `_` is in `XID_Continue` but not `XID_Start`, so it's checked explicitly.
#[inline]
pub(crate) fn is_id_start(ch: char) -> bool {
    is_xid_start(ch) || ch == '_' || ch == '$'
}

/// `IdentifierPart`: `XID_Continue` plus `$` (`_` and ZWNJ/ZWJ are already in
/// `XID_Continue`).
#[inline]
pub(crate) fn is_id_continue(ch: char) -> bool {
    is_xid_continue(ch) || ch == '$'
}

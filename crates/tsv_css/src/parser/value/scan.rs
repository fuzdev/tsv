// Shared byte-scan machinery for the value scanners.
//
// Three scanners walk a CSS value's bytes with the same paren/quote/escape/comment
// state machine: `ValueParser::fast_scan` (the fused fast path), `ValueCursor::consume_until`
// (the split), and `classify_separators` (the fallback classifier). Their nesting rules
// are deliberately identical — the fused-pass invariant — so the byte set that invariant
// rests on lives here once, rather than being re-spelled per scanner.

/// Bytes that drive a value scanner's state machine: the escape introducer, the
/// nesting/quote triggers, and the comment introducer. All ASCII, so none can occur
/// as a UTF-8 continuation byte.
///
/// This is the set every scanner must inspect no matter what it is looking *for*;
/// each adds its own bytes on top (its separator).
pub(crate) const fn is_value_structural(b: u8) -> bool {
    matches!(b, b'\\' | b'(' | b')' | b'\'' | b'"' | b'/')
}

/// The bytes a value's **separator class** turns on: the comma that makes it a
/// comma list, and the ASCII whitespace that makes it a whitespace list.
///
/// One spelling, two scanners. [`super::parser::ValueParser::fast_scan`] classifies on
/// these directly; the declaration boundary scan
/// (`crate::parser::decl_scan::scan_value_core`) records the same two facts while it walks
/// the value for its terminator, so the class is already known when the value parser
/// starts. Those two answers must agree byte for byte — the boundary scan's is used *in
/// place of* the fused pass — so the set is named here rather than spelled twice.
///
/// ⚠️ `u8::is_ascii_whitespace`, **not** the lexer's `is_ascii_css_whitespace`: the
/// vertical tab is CSS whitespace to `parseCss` but is value *content* to this
/// classification, and both scanners must read it the same way.
pub(crate) const fn is_value_separator(b: u8) -> bool {
    b == b',' || b.is_ascii_whitespace()
}

// The comment-extent primitives live at the crate root ([`crate::comments`]), beside
// [`crate::escapes::escape_len`] — the two answer the same "how far does this reach"
// question for the two constructs a value scanner must step over whole. Re-exported here
// so a scanner imports its whole byte-scan vocabulary from one place.
pub(crate) use crate::comments::{comment_end, comment_run_end, is_comment_start};

/// Build a scanner's 256-entry "this byte cannot possibly matter" table: `true` for a
/// byte that is neither [`is_value_structural`] nor one the scanner is looking for, so
/// its whole loop body collapses to `i += 1`. The overwhelming majority of a value's
/// text is such content (identifier letters, digits, `-`, `#`, `%`, `.`), and one L1
/// load retires it — the per-byte branch chain is the cost these scanners are made of.
///
/// `$must_inspect` names the bytes the scanner cares about *beyond* the structural set.
///
/// **Only the ASCII half is populated** — a byte ≥ `0x80` is always `false`, i.e. never
/// skipped. That is load-bearing for [`super::cursor::ValueCursor::consume_until`],
/// which decodes a non-ASCII lead byte and hands the real `char` to an opaque predicate
/// (`char::is_whitespace` does treat NBSP and friends as delimiters). The pure byte-loop
/// scanners would be free to skip non-ASCII too — for them it is inert — but they take
/// the same rule so the tables mean exactly one thing everywhere, and a non-ASCII byte
/// costs only a walk through arms that all miss.
///
/// A `const fn` can't do this job: stable Rust forbids calling a function pointer in a
/// constant, so the predicate can't be a parameter.
macro_rules! value_skip_table {
    (|$b:ident| $must_inspect:expr) => {{
        let mut t = [false; 256];
        let mut i = 0;
        while i < 128 {
            let $b = i as u8;
            t[i] = !$crate::parser::value::scan::is_value_structural($b) && !($must_inspect);
            i += 1;
        }
        t
    }};
}

pub(crate) use value_skip_table;

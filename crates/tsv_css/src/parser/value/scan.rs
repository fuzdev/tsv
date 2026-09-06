// Shared byte-scan machinery for the value scanners.
//
// Four scanners walk a CSS value's bytes with the same paren/quote/escape/comment
// state machine: `ValueParser::fast_scan` (the fused fast path), `ValueCursor::consume_until`
// (the split), `classify_separators` (the fallback classifier), and [`matching_close_paren`]
// below (the function-extent scan). Their nesting rules are deliberately identical — the
// fused-pass invariant — so the byte set that invariant rests on lives here once, rather
// than being re-spelled per scanner.

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

/// The bytes [`matching_close_paren`] hops between: [`is_value_structural`]'s set **less the
/// comment introducer**, the one member that scan deliberately does not act on (its doc says
/// why). Named once because the hop's needles and the walk's `debug_assert` are two spellings
/// of it, and a needle missing from one is a paren the other never sees.
const FUNCTION_EXTENT_NEEDLES: [u8; 5] = [b'(', b')', b'\'', b'"', b'\\'];

const _: () = {
    let mut i = 0;
    while i < FUNCTION_EXTENT_NEEDLES.len() {
        assert!(
            is_value_structural(FUNCTION_EXTENT_NEEDLES[i]),
            "a function-extent needle outside the structural set"
        );
        i += 1;
    }
};

/// The offset of the `)` that closes the `(` at `open`, or `None` when the run never
/// balances within `text`.
///
/// The **function-extent** member of the scanner family above, and it must be one: a
/// paren is only structure where the other three say it is, so a paren inside a quoted
/// string (`fn("(", …)`) or inside an escape (`fn(a\(b, …)`) closes nothing. A scan that
/// counted those balanced the function somewhere other than its real end — usually
/// nowhere — and the value fell through to an opaque `CssValue::Identifier` that the
/// printer spells back verbatim, which silently switched off **every** normalization the
/// declaration had (quote style, number form, colour case, and the argument list's own
/// break).
///
/// ⚠️ **A block comment is deliberately NOT stepped over here, alone in this family.**
/// The other three only ever *split* a value, so stepping a comment whole inside an
/// unquoted `url()` costs them nothing — the token comes back out as one leaf either way.
/// This one **bounds** the function, and it is entered on any leaf ending in `)`, so it
/// cannot tell `url(foo/*bar)` — where §4.3.6 consumes to the first unescaped `)` and `/*`
/// opens nothing — from `fn(/* ) */ a)`. Reading the url's `/*` as a comment loses its
/// closing paren, drops the value onto the identifier path, and `normalize_css_whitespace`
/// then pads the "comment" (`url(foo /*bar)`), rewriting the URL. The arm buys nothing on
/// the other side of that trade: a value carrying a comment is re-emitted from source
/// whatever this scan answers, so both readings of `fn(/* ) */ a, .10)` are the same bytes.
/// Fixture `css/values/functions/url_comment_chars`.
///
/// `open` must address a `(`, which is what lets the depth counter start at zero and
/// never underflow: the first byte read raises it to one, and the walk returns the moment
/// it comes back down.
///
/// The walk **hops between structural bytes** rather than reading every byte
/// ([`tsv_lang::swar::next_byte_of`]), the rung this site has always sat on — a function's
/// interior runs long between them (a mean of 18.7 bytes across 638 stylesheets) and only
/// about 5% of the bytes it reads can move its state. Widening the needles from the two
/// parens to the five that can now move it costs the word loop about one instruction a byte
/// (`N` = 2 → `N` = 5 on `next_byte_of`'s disassembled table, 2.0 → 3.0, the lane loop
/// vectorizing from `N` = 4) and leaves it well under a 256-entry skip table's flat six —
/// needle count is the weak axis here, run length the strong one. ⚠️ Not re-boarded: the
/// CSS format cell's `value/mod.rs` row pre-dates this widening.
///
/// ⚠️ A one-byte pre-test in front of the hop — `string_end`'s shape, for the
/// adjacent-paren case — is deliberately NOT here. That pre-test pays in proportion to how
/// often the run is empty, and `()` / `))` is only 8.6% of the hops on this surface against
/// roughly half of `string_end`'s; built and measured, it removed marginally fewer
/// instructions and did not separate from this spelling on cycles. Rung by the run length
/// the site actually sees.
pub(crate) fn matching_close_paren(text: &str, open: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    debug_assert_eq!(
        bytes.get(open),
        Some(&b'('),
        "matching_close_paren must be entered on a `(`"
    );

    let mut depth = 0u32;
    let mut in_quote = false;
    let mut quote_char = 0u8;
    let mut i = open;

    while i < bytes.len() {
        let b = bytes[i];
        debug_assert!(
            i == open || FUNCTION_EXTENT_NEEDLES.contains(&b),
            "the hop landed on a byte no arm below can act on"
        );

        // An escape is OPAQUE, in or out of a string — `'a\''` is one string, and `a\(b`
        // is one ident whose paren nests nothing. Probed before the quote arm so an
        // escaped quote cannot close the string it sits in. Kept identical to the twin
        // trackers (see `fast_scan` for the full rationale).
        let resume = if b == b'\\'
            && let Some(len) = crate::escapes::escape_len(text, i)
        {
            i + len
        } else {
            match b {
                b'\'' | b'"' if !in_quote => {
                    in_quote = true;
                    quote_char = b;
                }
                _ if in_quote && b == quote_char => in_quote = false,
                b'(' if !in_quote => depth += 1,
                b')' if !in_quote => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(i);
                    }
                }
                _ => {}
            }
            i + 1
        };

        i = tsv_lang::swar::next_byte_of(bytes, resume, FUNCTION_EXTENT_NEEDLES);
    }

    None
}

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

#[cfg(test)]
mod matching_close_paren_tests {
    use super::matching_close_paren;

    /// A paren that is *content* — inside a string or inside an escape — closes nothing.
    /// The fixtures grade this through the printer; here it is graded at the seam, where
    /// the difference between "no function" and "a function ending elsewhere" is visible.
    #[test]
    fn content_parens_do_not_close_the_run() {
        for (text, want) in [
            // plain
            ("fn(a, b)", Some(7)),
            ("fn()", Some(3)),
            ("fn(fn(a))", Some(8)),
            // a paren inside a string
            ("fn('(', 0.1)", Some(11)),
            ("fn(')', 0.1)", Some(11)),
            ("fn('()', 0.1)", Some(12)),
            ("fn(\"(\", 0.1)", Some(11)),
            ("fn(fn('('), 0.1)", Some(15)),
            ("url('a(b')", Some(9)),
            // a paren inside an escape
            (r"fn(a\(b, 0.1)", Some(12)),
            (r"fn(a\)b, 0.1)", Some(12)),
            (r"url(a\)b)", Some(8)),
            // an escaped quote cannot close the string it sits in
            (r"fn('a\'(b', 0.1)", Some(15)),
            // a hex escape's whitespace terminator belongs to the escape
            (r"fn(a\29 b, 0.1)", Some(14)),
            // never balances
            ("fn(a, b", None),
            ("fn('a)'", None),
            (r"fn(a\)", None),
            // a trailing `\` is not an escape, so the `)` before it still closes
            ("fn(a)\\", Some(4)),
        ] {
            // The sole caller enters on the value's first `(` (`parse_single_value`), so
            // the tests do too rather than hand-counting each offset.
            let open = text
                .bytes()
                .position(|b| b == b'(')
                .expect("a `(` to open on");
            assert_eq!(matching_close_paren(text, open), want, "for {text:?}");
        }
    }

    /// A `/*` is **not** a comment to this scan — see the function's own note. Both
    /// spellings answer as the raw bytes do, which is what keeps an unquoted
    /// `<url-token>`'s content opaque.
    #[test]
    fn a_comment_is_not_stepped_over() {
        assert_eq!(matching_close_paren("url(foo/*bar)", 3), Some(12));
        assert_eq!(matching_close_paren("fn(/* ) */ a)", 2), Some(6));
    }
}

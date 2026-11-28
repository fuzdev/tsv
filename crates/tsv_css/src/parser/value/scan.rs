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

/// Where a value scanner resumes after the quoted string whose opening quote sits at
/// `open`: one past its closing quote, or the end of `text` when the string never closes.
///
/// A string is opaque to every value scanner — nothing inside it is a separator, a
/// paren or a comment — so a scanner standing on an opening quote has one question, where
/// the string ends, and [`crate::lexer::string_end`] answers it a word at a time. A
/// scanner that hops here keeps no quote state at all.
///
/// The hop and the family's byte-at-a-time quote state are ONE string model, and that is
/// an argument about escapes rather than an assumption. The byte walk steps an escape
/// with [`crate::escapes::escape_len`] (a hex escape's digits and terminator included);
/// `string_end` steps `\` and the one byte after it. The two land on the same closing
/// quote because nothing a longer escape reaches past that byte can be a quote or a `\`
/// — hex digits, one whitespace character, a multi-byte char's continuation bytes — and
/// they agree on the three shapes that are no escape at all: a `\` before a newline (the
/// newline is string content either way), a `\` as the text's last byte, and a string
/// that never closes (both run to the end). The assertion below holds every hop to the
/// byte walk's answer in every debug build.
///
/// Out of line on purpose: a string is the rare arm of the loop that hops here, and
/// inlined into `fast_scan` the word scan widened that function's frame — one that sits
/// on the value parser's recursive path (a comma list nested in a function's arguments).
#[inline(never)]
pub(crate) fn quoted_run_end(text: &str, open: usize) -> usize {
    let end = match crate::lexer::string_end(text.as_bytes(), open) {
        Ok(end) => end,
        Err(_) => text.len(),
    };
    // `cfg`, not `debug_assert_eq!` alone: the model is compiled out of a release build,
    // where the macro's arguments are still type-checked.
    #[cfg(debug_assertions)]
    assert_eq!(
        end,
        quoted_run_end_by_bytes(text, open),
        "the string hop left the byte walk's string model in {text:?} at {open}"
    );
    end
}

/// [`quoted_run_end`] as the scanner family's byte-at-a-time quote state reads it — the
/// in-quote arms of `ValueCursor::consume_until` and `classify_separators`, kept as the
/// model the hop is graded against.
#[cfg(any(test, debug_assertions))]
fn quoted_run_end_by_bytes(text: &str, open: usize) -> usize {
    let bytes = text.as_bytes();
    let quote_char = bytes[open];
    let mut i = open + 1;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\\'
            && let Some(len) = crate::escapes::escape_len(text, i)
        {
            i += len;
            continue;
        }
        i += 1;
        if b == quote_char {
            return i;
        }
    }
    bytes.len()
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

/// How many bytes [`table_run_end`] tests behind one bound check.
///
/// Sixteen is chosen for these runs' shape, not in general: a value is short, and a run that
/// starts within a chunk of the value's end walks byte by byte, so a wider chunk hands more
/// runs to that tail, while a narrower one pays the bound check and the loop's back edge more
/// often across the long runs that hold much of a value's bytes. A second, narrower stage after the
/// chunked one, and `chunks_exact`'s remainder bookkeeping, each cost more than they saved.
/// The declaration boundary scan's own walk (`decl_scan::skip_run_end_in`) keeps its width,
/// chosen on its own runs.
const TABLE_RUN_CHUNK: usize = 16;

/// The end of the run of `table` bytes from `from` — `from` itself when none is one — for
/// the value scanners whose skip test is a [`value_skip_table!`] lookup.
///
/// [`TABLE_RUN_CHUNK`] bytes at a time while that many are in bounds, each tested against
/// the table in turn with no bound check of its own, then byte by byte. Every byte is still
/// the table's to classify; only the bound check is shared across the chunk.
#[expect(
    clippy::inline_always,
    reason = "each caller's skip test must fold into its own loop, with a constant table where it has one"
)]
#[inline(always)]
pub(crate) fn table_run_end(bytes: &[u8], from: usize, table: &[bool; 256]) -> usize {
    let mut i = from;
    while let Some(chunk) = bytes.get(i..i + TABLE_RUN_CHUNK) {
        if let Some(k) = chunk.iter().position(|&b| !table[b as usize]) {
            return i + k;
        }
        i += TABLE_RUN_CHUNK;
    }
    while i < bytes.len() && table[bytes[i] as usize] {
        i += 1;
    }
    i
}

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
            // The function-leaf callers (`extract_function_parts`, `plain_function_parts`)
            // enter on the value's first `(` — the operator splitter enters on whichever
            // `(` its run reaches — so the tests take the first rather than hand-counting.
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

#[cfg(test)]
mod quoted_run_end_tests {
    use super::{quoted_run_end, quoted_run_end_by_bytes};

    /// Every string over an alphabet holding each byte either walk acts on — both quotes,
    /// the backslash, a hex digit, the escape terminator, two newlines, a non-ASCII char — and
    /// the bytes a value scanner would act on outside a string, opened by either quote.
    /// Long enough to put a hex escape's digits and terminator against a closing quote and
    /// to leave a string open, escaped shut, or cut off mid-escape.
    #[test]
    fn hop_matches_the_byte_walk() {
        const ALPHABET: [&str; 13] = [
            "\"", "'", "\\", "4", "g", " ", "\n", "\r", "\u{e9}", "(", ")", ",", "/",
        ];
        const MAX_LEN: u32 = 5;
        let mut text = String::new();
        for quote in ["\"", "'"] {
            for len in 0..=MAX_LEN {
                for case in 0..ALPHABET.len().pow(len) {
                    text.clear();
                    text.push_str(quote);
                    let mut rest = case;
                    for _ in 0..len {
                        text.push_str(ALPHABET[rest % ALPHABET.len()]);
                        rest /= ALPHABET.len();
                    }
                    assert_eq!(
                        quoted_run_end(&text, 0),
                        quoted_run_end_by_bytes(&text, 0),
                        "for {text:?}"
                    );
                }
            }
        }
    }

    /// The hop's four endings, by hand: a closed string, an escaped quote that does not
    /// close it, a string left open, and one cut off on its backslash.
    #[test]
    fn hop_endings() {
        for (text, want) in [
            ("'a' b", 3),
            (r"'a\' b' c", 7),
            (r"'\27' b", 5),
            ("'a b", 4),
            ("'a\\", 3),
            ("x \"a,(\" y", 7),
        ] {
            let open = text
                .bytes()
                .position(|b| b == b'"' || b == b'\'')
                .expect("a quote to open on");
            assert_eq!(quoted_run_end(text, open), want, "for {text:?}");
        }
    }
}

#[cfg(test)]
mod table_run_end_tests {
    use super::{TABLE_RUN_CHUNK, is_value_separator, table_run_end};

    /// The tables [`table_run_end`]'s callers walk: a group's interior, the fast scan's, the
    /// comma split's and the whitespace split's.
    const TABLES: [[bool; 256]; 4] = [
        value_skip_table!(|b| false),
        value_skip_table!(|b| is_value_separator(b)),
        value_skip_table!(|b| b == b','),
        value_skip_table!(|b| b.is_ascii_whitespace()),
    ];

    /// The byte-at-a-time walk the chunked one must agree with.
    fn reference(bytes: &[u8], from: usize, table: &[bool; 256]) -> usize {
        let mut i = from;
        while i < bytes.len() && table[usize::from(bytes[i])] {
            i += 1;
        }
        i
    }

    /// [`table_run_end`] walks a chunk at a time and then a byte at a time; either way it
    /// must stop where a byte-at-a-time walk does. Two grids per table. The geometry grid: a
    /// run of every length up to three chunks and one past, entered from every offset up to a
    /// chunk and one past, ended by end of input or by a stopping byte followed by input of
    /// every length up to a chunk. The class grid: every byte value at every position of a
    /// two-chunk-and-one run, entered from either side of a chunk boundary.
    #[test]
    fn runs_end_where_the_table_ends() {
        const CHUNK: usize = TABLE_RUN_CHUNK;
        let mut graded = 0usize;
        let mut chunk_stops = 0usize;
        let mut tail_stops = 0usize;
        let mut grade = |bytes: &[u8], from: usize, table: &[bool; 256]| {
            let want = reference(bytes, from, table);
            assert_eq!(
                table_run_end(bytes, from, table),
                want,
                "{bytes:?} from {from}"
            );
            // A stop inside a whole in-bounds chunk returns from the chunked loop.
            if want < bytes.len() && from + ((want - from) / CHUNK + 1) * CHUNK <= bytes.len() {
                chunk_stops += 1;
            } else {
                tail_stops += 1;
            }
            graded += 1;
        };
        for table in &TABLES {
            assert!(
                table[usize::from(b'a')],
                "`a` is run content in every table"
            );
            let stop = (0..=255u8)
                .find(|&b| !table[usize::from(b)])
                .expect("a byte every table stops at");
            let mut bytes = Vec::new();
            for from in 0..=CHUNK + 1 {
                for run in 0..=3 * CHUNK + 1 {
                    for tail in std::iter::once(None).chain((0..=CHUNK).map(Some)) {
                        bytes.clear();
                        bytes.resize(from, stop);
                        bytes.resize(from + run, b'a');
                        if let Some(tail) = tail {
                            bytes.push(stop);
                            bytes.resize(bytes.len() + tail, b'a');
                        }
                        grade(&bytes, from, table);
                    }
                }
            }
            for byte in 0..=255u8 {
                for at in 0..=2 * CHUNK {
                    bytes.clear();
                    bytes.resize(2 * CHUNK + 1, b'a');
                    bytes[at] = byte;
                    for from in [0, 1, CHUNK - 1, CHUNK, CHUNK + 1] {
                        grade(&bytes, from, table);
                    }
                }
            }
        }
        assert!(graded > 100_000, "{graded}");
        assert!(
            chunk_stops > 0 && tail_stops > 0,
            "{chunk_stops} / {tail_stops}"
        );
    }
}

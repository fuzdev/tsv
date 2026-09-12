// Splitting a value run into its operator tokens and operands.
//
// A leaf reaching [`super::parse_single_value`] is one whitespace/comma-delimited run, and
// for most of a stylesheet that run is one token (`red`, `1px`, `var(--a)`). Where it is
// not — `12px/1.5`, `1.5*2.5`, `f(a:1.5)`, `(1.5)(2.5)` — postcss-values-parser, the
// parser behind prettier's CSS printer, reads it as several nodes, and the printer then
// decides each gap on the two nodes' kinds. tsv had no such token, so the run stayed one
// opaque leaf: the number after the operator never normalized, the colon never spaced, and
// the second group never separated.
//
// This module is the tokenizer half of that model. The separator half — which gap is a
// space and which is glue — is `crate::printer::values`'s `ValueOperator` rule, which
// reads the members this produces.
//
// ⚠️ **The boundary set is not one class of byte.** `*` and `+` end a run wherever they
// occur; `/` and `-` end a **number** (or a closed group / function) and are ordinary
// content inside a word. That asymmetry is postcss's, and it is observable: `1.5/2.5` is
// three nodes and normalizes on both sides, where `a/1.5` is a single word whose `1.5`
// prettier leaves exactly as written.

use super::scan::{comment_end, is_comment_start, matching_close_paren};
use crate::escapes::escape_len;
use crate::lexer::string_end;
use crate::number::number_part_len;

/// What the token just emitted was, which is what decides whether a following `/` or `-`
/// opens an operator or is content.
#[derive(Clone, Copy, PartialEq, Eq)]
enum TokenKind {
    /// A `<number>` / `<dimension>` / `<percentage>` — `1.5`, `12px`, `100%`.
    Number,
    /// An ident-ish run — `red`, `a/1.5`, `#FFF-5`. It **absorbs** `/` and `-`.
    Word,
    /// A run that ends the member outright: a function or parenthesized group closed by
    /// its `)`, or a `<unicode-range-token>`. Like `Number` it releases `/` and `-`, and
    /// unlike either what follows it starts a new member whatever that is
    /// (`(1.5)(2.5)`, `f(1.5)g(2.5)`, `(1.5)aa`, `U+26-1.50` → `U+26-1 0.5`).
    Closed,
}

/// A byte that can open an operator after a [`TokenKind::Number`] or
/// [`TokenKind::Closed`] token — every operator byte there is. `:` is one only inside a
/// group; see [`split_value_run`]'s `in_group`.
const fn releases_operator(b: u8, in_group: bool) -> bool {
    matches!(b, b'*' | b'+' | b'/' | b'-') || (b == b':' && in_group)
}

/// A byte that ends a [`TokenKind::Word`]. The strict subset of
/// [`releases_operator`] that is never an ident code point *and* never glues into a
/// postcss word: `/` and `-` are missing on purpose.
const fn ends_word(b: u8, in_group: bool) -> bool {
    matches!(b, b'*' | b'+') || (b == b':' && in_group)
}

/// A byte that is an operator at a run's START, or straight after another operator.
///
/// `-` is missing on purpose: there it opens an ident (`-webkit-box`) or signs a number,
/// and a leaf that is nothing *but* a `-` needs no operator node — an ordinary member
/// takes the same separator on both sides. `+` is in, since it can open neither (a signed
/// number is claimed by `number_part_len` ahead of this test).
const fn opens_run(b: u8, in_group: bool) -> bool {
    matches!(b, b'*' | b'/' | b'+') || (b == b':' && in_group)
}

/// A byte that could possibly begin a member boundary: any operator byte, plus the `(`
/// that opens a group (a group boundary splits with no operator at all).
///
/// [`split_value_run`]'s refusal pass reads exactly this, so it must be a **superset** of
/// the three position-keyed tests above — a byte one of them releases but this one does
/// not would make the refusal skip a run that really splits. The assertion below is what
/// holds the four together rather than a comment claiming they agree.
const fn may_open_boundary(b: u8) -> bool {
    releases_operator(b, true) || b == b'('
}

const _: () = {
    let mut b = 0u8;
    while b < 128 {
        assert!(
            !(releases_operator(b, true) || ends_word(b, true) || opens_run(b, true))
                || may_open_boundary(b),
            "a boundary byte the refusal pass cannot see"
        );
        b += 1;
    }
};

/// Is the byte at `i` the first of a **doubled** operator, which postcss reads as content
/// rather than as two operators?
///
/// Two pairs, each observable in what prettier leaves alone:
///
/// - `--` opens a custom-property-shaped word, never a subtraction — `1.5--2.5` is one
///   token and neither number normalizes;
/// - `//` takes the whole run with it — `1.50//2.50`, `aa//1.50` and `///1.50` all come
///   back exactly as authored, where a single `/` splits (`/1.50` → `/1.5`). Separate the
///   pair by a space and both sides normalize again (`1.50/ /2.50` → `1.5/ /2.5`), so it
///   is the adjacency that does it.
fn is_content_pair(bytes: &[u8], i: usize) -> bool {
    matches!(bytes[i], b'-' | b'/') && bytes.get(i + 1) == Some(&bytes[i])
}

/// The length of the `<unicode-range-token>` at `i`, or 0 when none starts there.
///
/// css-syntax-3 §"Check if three code points would start a unicode-range": `U`/`u`, then
/// `+`, then a hex digit or `?`. The `+` inside it is part of the token, so the run must
/// be taken whole here — read as an operator it would space the range apart
/// (`U+26` → `U + 26`).
fn unicode_range_len(bytes: &[u8], i: usize) -> usize {
    let is_range_digit = |b: u8| b.is_ascii_hexdigit() || b == b'?';
    if !bytes[i].eq_ignore_ascii_case(&b'u')
        || bytes.get(i + 1) != Some(&b'+')
        || !bytes.get(i + 2).copied().is_some_and(is_range_digit)
    {
        return 0;
    }
    let mut end = i + 2;
    while bytes.get(end).copied().is_some_and(is_range_digit) {
        end += 1;
    }
    // The range's second half (`U+0025-00FF`), whose `-` is the token's, not an operator.
    if bytes.get(end) == Some(&b'-')
        && bytes
            .get(end + 1)
            .copied()
            .is_some_and(|b| b.is_ascii_hexdigit())
    {
        end += 1;
        while bytes
            .get(end)
            .copied()
            .is_some_and(|b| b.is_ascii_hexdigit())
        {
            end += 1;
        }
    }
    end - i
}

/// One member of a split run: its half-open byte range, and whether it is an operator.
///
/// The flag is the splitter's own verdict, carried rather than re-derived. Read back off
/// the bytes instead, a one-byte **operand** that happens to be an operator character
/// (the `-` in `1.5*-`, which at that position opens an ident rather than a subtraction)
/// would be minted as an operator and take a glue rule that is not its own.
pub(crate) struct ValueToken {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) is_operator: bool,
}

/// The members `run` splits into, or `None` when it is a single token —
/// the overwhelmingly common case, which allocates nothing and leaves the caller's
/// existing leaf classification untouched.
///
/// `in_group` says whether the run is inside a function's argument list or a
/// parenthesized group, which is the only place a `:` can stand as an operator: at the top
/// level of a declaration value a `:` is not a value token at all (prettier's parser
/// rejects `a { b: c:d }` outright), so there is no oracle for splitting one and the run
/// is left as the author wrote it.
///
/// Escapes, strings, comments and parenthesized interiors are **opaque**: a `\/` never
/// splits (which is what keeps `inset: a\+b` on its cataloged divergence), a `/*` opens a
/// comment rather than a division, and an operator inside a function's arguments belongs
/// to that function's own run.
pub(crate) fn split_value_run(run: &str, in_group: bool) -> Option<Vec<ValueToken>> {
    let bytes = run.as_bytes();
    let len = bytes.len();
    // A run holding no byte that could open a boundary is the common case, and one pass
    // that can only answer "no" is cheaper than the tokenizer's state machine.
    if !bytes.iter().copied().any(may_open_boundary) {
        return None;
    }

    let mut tokens: Vec<ValueToken> = Vec::new();
    let mut i = 0usize;
    // What the token before this position was. `None` both at the run's start and right
    // after an operator, since an operand is what follows either.
    let mut previous: Option<TokenKind> = None;

    while i < len {
        if operator_stands_here(run, i, previous, in_group) {
            tokens.push(ValueToken {
                start: i,
                end: i + 1,
                is_operator: true,
            });
            i += 1;
            previous = None;
            continue;
        }
        let start = i;
        previous = Some(scan_token(run, &mut i, in_group));
        debug_assert!(i > start, "a token consumed nothing: {run:?} at {start}");
        tokens.push(ValueToken {
            start,
            end: i,
            is_operator: false,
        });
    }

    (tokens.len() > 1).then_some(tokens)
}

/// Is the byte at `i` an operator in its own right, given what came before it?
///
/// Three readings of one question, because what releases an operator is what precedes it:
/// a number or a closed run releases every operator byte, a word releases only the two
/// that are never ident content, and at a run's start (or straight after another
/// operator) only a byte that cannot *begin* an operand can be one.
fn operator_stands_here(run: &str, i: usize, previous: Option<TokenKind>, in_group: bool) -> bool {
    let bytes = run.as_bytes();
    let b = bytes[i];
    // A `/*` opens a comment, never a division (css-syntax-3 §4.3.2).
    if is_comment_start(bytes, i) {
        return false;
    }
    if is_content_pair(bytes, i) {
        return false;
    }
    match previous {
        Some(TokenKind::Number | TokenKind::Closed) => releases_operator(b, in_group),
        Some(TokenKind::Word) => ends_word(b, in_group),
        None => opens_run(b, in_group) && number_part_len(&run[i..]) == 0,
    }
}

/// Consume one token starting at `*i`, returning what it was.
fn scan_token(run: &str, i: &mut usize, in_group: bool) -> TokenKind {
    let bytes = run.as_bytes();

    if bytes[*i] == b'(' {
        return close_group(run, i);
    }

    let range_len = unicode_range_len(bytes, *i);
    if range_len > 0 {
        *i += range_len;
        return TokenKind::Closed;
    }

    let number_len = number_part_len(&run[*i..]);
    if number_len > 0 {
        *i += number_len;
        // The unit. It ends at every operator byte, `/` and `-` included — which is what
        // makes `12px/1.5` a dimension and an operator rather than a dimension whose unit
        // is `px/1.5`.
        return scan_token_body(run, i, in_group, TokenKind::Number);
    }

    scan_token_body(run, i, in_group, TokenKind::Word)
}

/// The shared tail of both token shapes: walk content until a boundary this `kind`
/// releases, stepping every opaque construct whole. A `(` turns the token into a
/// [`TokenKind::Closed`] one (a function name meeting its argument list, or a bare group).
fn scan_token_body(run: &str, i: &mut usize, in_group: bool, kind: TokenKind) -> TokenKind {
    let bytes = run.as_bytes();
    let len = bytes.len();

    while *i < len {
        let b = bytes[*i];

        if b == b'\\'
            && let Some(step) = escape_len(run, *i)
        {
            *i += step;
            continue;
        }
        if b == b'"' || b == b'\'' {
            *i = string_end(bytes, *i).unwrap_or(len);
            continue;
        }
        if is_comment_start(bytes, *i) {
            *i = comment_end(bytes, *i);
            continue;
        }
        if b == b'(' {
            return close_group(run, i);
        }
        // A `[…]` / `{…}` simple block is **content**, where a `(…)` one is a value
        // group: css-syntax-3 gives all three a payload, but prettier's value parser
        // descends only into the paren block and leaves the other two as word text
        // (`[1.50]` keeps its number on both sides). So the run continues through it,
        // operators and all — Tailwind's `--modifier(…, [*])` is one token, not a
        // bracket around a multiplication.
        if b == b'[' || b == b'{' {
            *i = simple_block_end(run, *i);
            continue;
        }
        // A doubled operator is content (see `is_content_pair`), and **both** of its
        // bytes are: stepping only the first would leave the second standing as an
        // operator against the token this one just ended.
        if is_content_pair(bytes, *i) {
            *i += 2;
            continue;
        }
        let boundary = match kind {
            TokenKind::Number | TokenKind::Closed => releases_operator(b, in_group),
            TokenKind::Word => ends_word(b, in_group),
        };
        if boundary {
            return kind;
        }
        *i += 1;
    }
    kind
}

/// One past the `]` / `}` closing the simple block at `open`, or the run's end when it
/// never closes.
///
/// Nesting and the opaque constructs are stepped exactly as [`scan_token_body`] steps
/// them, so a bracket inside a string or an escape closes nothing.
fn simple_block_end(run: &str, open: usize) -> usize {
    let bytes = run.as_bytes();
    debug_assert!(
        matches!(bytes.get(open), Some(b'[' | b'{')),
        "simple_block_end must be entered on a `[` or a `{{`"
    );
    let close = if bytes[open] == b'[' { b']' } else { b'}' };
    let mut depth = 0u32;
    let mut i = open;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\\'
            && let Some(step) = escape_len(run, i)
        {
            i += step;
            continue;
        }
        if b == b'"' || b == b'\'' {
            i = string_end(bytes, i).unwrap_or(bytes.len());
            continue;
        }
        if is_comment_start(bytes, i) {
            i = comment_end(bytes, i);
            continue;
        }
        if b == bytes[open] {
            depth += 1;
        } else if b == close {
            depth -= 1;
            if depth == 0 {
                return i + 1;
            }
        }
        i += 1;
    }
    bytes.len()
}

/// Step over the balanced `(`…`)` at `*i`, leaving `*i` one past its close. An
/// unbalanced `(` swallows the rest of the run, which keeps the leaf opaque exactly as it
/// is today.
fn close_group(run: &str, i: &mut usize) -> TokenKind {
    match matching_close_paren(run, *i) {
        Some(close) => *i = close + 1,
        None => *i = run.len(),
    }
    TokenKind::Closed
}

#[cfg(test)]
mod tests {
    use super::split_value_run;

    /// The members as text. Every member that is an operator is spelled with its own
    /// byte, so the `is_operator` tag is graded by the same assertion as the split.
    fn split(run: &str, in_group: bool) -> Option<Vec<String>> {
        split_value_run(run, in_group).map(|tokens| {
            tokens
                .into_iter()
                .map(|t| {
                    let text = &run[t.start..t.end];
                    if t.is_operator {
                        format!("op {text}")
                    } else {
                        text.to_owned()
                    }
                })
                .collect()
        })
    }

    /// The single-token runs, which must stay a leaf so every existing classification
    /// (colour, dimension, function, opaque identifier) keeps its path.
    #[test]
    fn a_single_token_run_does_not_split() {
        for run in [
            "red",
            "1px",
            "1.5",
            "-1.5",
            "1.5e-2",
            "#FFF",
            "100%",
            "a/1.5",
            "#FFF/1.5",
            "#FFF-5",
            "aa-bb",
            "a/1.5/2.5",
            "1.5--2.5",
            "var(--a)",
            "f(a, b)",
            "(1.5)",
            "url(a/1.5)",
            r"a\+b",
            r"a\/b",
            "1.5/*c*/",
            "'a/1.5'",
            "f(a:1.5)",
            // a doubled operator is content, and takes the whole run with it
            "1.50//2.50",
            "aa//1.50",
            "///1.50",
            "//x",
            // a `<unicode-range-token>` carries its own `+` and `-`
            "U+26",
            "u+4??",
            "U+0025-00FF",
            // a `[…]` / `{…}` simple block is content, operators and all
            "[*]",
            "[1.50]",
            "[a*b]",
            "{a*b}",
            "[integer]",
        ] {
            assert_eq!(split(run, false), None, "{run:?} is one token");
        }
    }

    /// A `/` or `-` ends a number and a closed run; inside a word it is content.
    #[test]
    fn division_and_subtraction_end_only_a_number_or_a_closed_run() {
        assert_eq!(split("1.5/2.5", false).unwrap(), ["1.5", "op /", "2.5"]);
        assert_eq!(split("12px/1.5", false).unwrap(), ["12px", "op /", "1.5"]);
        assert_eq!(split("100%/2", false).unwrap(), ["100%", "op /", "2"]);
        assert_eq!(split("1.5-2.5", false).unwrap(), ["1.5", "op -", "2.5"]);
        assert_eq!(split("f(a)/1.5", false).unwrap(), ["f(a)", "op /", "1.5"]);
        assert_eq!(split("(1.5)/2.5", false).unwrap(), ["(1.5)", "op /", "2.5"]);
        assert_eq!(
            split("1.5/2.5/aa", false).unwrap(),
            ["1.5", "op /", "2.5", "op /", "aa"]
        );
    }

    /// `*` and `+` end every run, a word included.
    #[test]
    fn multiplication_and_addition_end_every_run() {
        assert_eq!(split("1.5*2.5", false).unwrap(), ["1.5", "op *", "2.5"]);
        assert_eq!(split("aa*bb", false).unwrap(), ["aa", "op *", "bb"]);
        assert_eq!(split("aa+bb", false).unwrap(), ["aa", "op +", "bb"]);
        assert_eq!(split("#FFF+5", false).unwrap(), ["#FFF", "op +", "5"]);
        assert_eq!(split("aa*bb/cc", false).unwrap(), ["aa", "op *", "bb/cc"]);
    }

    /// A `:` is an operator only inside a group — at the top level of a declaration
    /// value there is no oracle for splitting one.
    #[test]
    fn a_colon_splits_only_inside_a_group() {
        assert_eq!(split("a:1.5", false), None);
        assert_eq!(split("a:1.5", true).unwrap(), ["a", "op :", "1.5"]);
        assert_eq!(split("1.5:2.5", true).unwrap(), ["1.5", "op :", "2.5"]);
    }

    /// A single `/` still splits where the doubled one does not.
    #[test]
    fn a_lone_slash_still_splits() {
        assert_eq!(split("/1.5", false).unwrap(), ["op /", "1.5"]);
        assert_eq!(split("1.5/", false).unwrap(), ["1.5", "op /"]);
    }

    /// A one-byte member is not an operator just because its byte is one: at a run's
    /// start — or, as here, straight after another operator — a `-` opens an ident, so
    /// the last member of `1.5*-` is an OPERAND. Re-deriving the tag from the byte is
    /// what `ValueToken::is_operator` exists to avoid.
    #[test]
    fn a_one_byte_operand_is_not_tagged_as_an_operator() {
        assert_eq!(split("1.5*-", false).unwrap(), ["1.5", "op *", "-"]);
    }

    /// A unicode range ends its member, so a number glued past it is its own.
    #[test]
    fn a_unicode_range_ends_the_member() {
        assert_eq!(split("U+26-1.50", false).unwrap(), ["U+26-1", ".50"]);
    }

    /// A closed run ends the member outright, so the next one starts against it.
    #[test]
    fn a_closed_run_ends_the_member() {
        assert_eq!(split("(1.5)(2.5)", false).unwrap(), ["(1.5)", "(2.5)"]);
        assert_eq!(split("f(1.5)g(2.5)", false).unwrap(), ["f(1.5)", "g(2.5)"]);
        assert_eq!(split("(1.5)aa", false).unwrap(), ["(1.5)", "aa"]);
        assert_eq!(split("1.5/(2.5)", false).unwrap(), ["1.5", "op /", "(2.5)"]);
    }

    /// An unbalanced `(` swallows the rest of the run, which is what keeps the leaf
    /// opaque rather than inventing a member boundary inside it.
    #[test]
    fn an_unbalanced_paren_swallows_the_run() {
        assert_eq!(split("1.5/(2.5", false).unwrap(), ["1.5", "op /", "(2.5"]);
        assert_eq!(split("(1.5", false), None);
    }
}

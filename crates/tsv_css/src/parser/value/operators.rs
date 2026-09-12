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
//
// Three more members are not operators but end every token they touch, each with its
// own extent, each measured rather than reasoned from a class:
//
// - an **`@`-word** (`@a`, and the empty `@`): postcss's `atword` token, whose end set is
//   its own — whitespace, a quote, `,`, `(`, `)`, `{`, `;`, `/` — so `*`, `+`, `-`, `:`,
//   `#`, `.`, a digit and `[` are its CONTENT (`@a*1.50`, `f(@a:1.50)` come back verbatim
//   where `@a/1.50` is an `@`-word, an operator and a number), and a second `@` starts a
//   second one (`@a@b` → `@a @b`);
// - a **brace**, `{` or `}`, its own one-byte token — reachable only in a custom
//   property's value, where css-syntax-3 keeps the original text whatever it holds (a
//   non-custom property refuses a top-level `{}`-block beside any other value; both
//   parsers reject `b: a{1.5}c`). The word between the braces is a member of its own and
//   normalizes (`a{1.50}c(2.50)` → `a{1.5}c(2.5)`), which is why the brace is not word
//   content the way `[` is (`a[1.50]c` keeps its number: the oracle never descends into
//   a `[…]` block);
// - a **welded `+`**: postcss's `operator()` returns `this.word()` for a `+` that opens
//   the value or follows an operator when the next token is a word, so the sign is the
//   word's own — `+a(2.50)` is the function `+a` and normalizes to `+a(2.5)`, `1.50 / +a`
//   → `1.5 / +a`. Anywhere else (after a member, at the head of a function's arguments
//   or of a group — the `(` is a node there — and after a comma) the `+` is an operator
//   and prints spaced (`f(+a(2.50))` → `f(+ a(2.5))`). Before a group, a string, an
//   `@`-word or another operator nothing welds (`+(2.5)`, `+'x'`, `+ @a`, `+ +a`).

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

/// A byte that ends **every** token kind without being an operator: the `@` that opens an
/// `@`-word and the two braces, each a member of its own (see the module doc).
const fn ends_every_token(b: u8) -> bool {
    matches!(b, b'@' | b'{' | b'}')
}

/// A byte that ends an `@`-word — postcss's `atEnd` set (tokenizer `case 64`), which
/// unlike the word's end set DOES end at `/` and does NOT end at `*`, `+`, `-`, `:`, `[`
/// or `]`. Whitespace and `;` cannot occur in a run; a second `@` ends it because
/// `splitWord` splits a word at every `@`. `\` is in postcss's set but is stepped whole
/// here as an escape (lossless; prettier's own output there rides its unbalanced-value
/// fallback, so no cell pins it).
const fn ends_atword(b: u8) -> bool {
    matches!(
        b,
        b'{' | b'}' | b'(' | b')' | b'\'' | b'"' | b',' | b'/' | b'@'
    )
}

/// Could a `+` weld onto the byte after it? postcss's condition is "the next token is a
/// `word`": not a group, a string, an `@`-word, a brace, a comma, a colon, another
/// operator, or the run's end. Everything else — a letter, `#`, `.`, `\`, `[`, `!`, a
/// digit the number production did not claim — starts a word.
const fn starts_word(b: Option<u8>) -> bool {
    match b {
        None => false,
        Some(b) => !matches!(
            b,
            b'(' | b')'
                | b'\''
                | b'"'
                | b'@'
                | b'{'
                | b'}'
                | b','
                | b':'
                | b'*'
                | b'/'
                | b'+'
                | b'-'
        ),
    }
}

/// A byte that could possibly begin a member boundary: any operator byte, plus the `(`
/// that opens a group (a group boundary splits with no operator at all) and the bytes
/// that end every token.
///
/// [`split_value_run`]'s refusal pass reads exactly this, so it must be a **superset** of
/// the three position-keyed tests above — a byte one of them releases but this one does
/// not would make the refusal skip a run that really splits. The assertion below is what
/// holds the four together rather than a comment claiming they agree.
const fn may_open_boundary(b: u8) -> bool {
    releases_operator(b, true) || b == b'(' || ends_every_token(b)
}

const _: () = {
    let mut b = 0u8;
    while b < 128 {
        assert!(
            !(releases_operator(b, true)
                || ends_word(b, true)
                || opens_run(b, true)
                || ends_every_token(b))
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
/// `head_welds` says whether a `+` opening the run welds onto the word after it (the
/// module doc's third member): true for the first run of a declaration's whole value and
/// for a run straight after an operator member, false at the head of a group's interior
/// and after a comma. A `+` after another operator *inside* the run welds regardless.
///
/// Escapes, strings, comments and parenthesized interiors are **opaque**: a `\/` never
/// splits (which is what keeps `inset: a\+b` on its cataloged divergence), a `/*` opens a
/// comment rather than a division, and an operator inside a function's arguments belongs
/// to that function's own run.
pub(crate) fn split_value_run(
    run: &str,
    in_group: bool,
    head_welds: bool,
) -> Option<Vec<ValueToken>> {
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
        if operator_stands_here(run, i, previous, in_group, head_welds) {
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
/// operator) only a byte that cannot *begin* an operand can be one — and there a `+`
/// that can weld (`head_welds` at the run's start, always after an operator) is the
/// head of the word that follows it, not an operator.
fn operator_stands_here(
    run: &str,
    i: usize,
    previous: Option<TokenKind>,
    in_group: bool,
    head_welds: bool,
) -> bool {
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
        None => {
            // A `-` straight before an `@`-word is an operator here (postcss tokenizes it
            // as one, since `@` cannot continue an ident), and the sign rule then glues it
            // to the word: `-@a` → `-@a`, `f(-@a)`, `1.5 * -@a` — where the same `-` before
            // a letter opens the ident (`-webkit-box`). Measured on prettier's own
            // `css/prefix/prefix.css` (`margin-left: -@leftMargin`).
            if b == b'-' && bytes.get(i + 1) == Some(&b'@') {
                return true;
            }
            opens_run(b, in_group)
                && number_part_len(&run[i..]) == 0
                && !(b == b'+' && (i > 0 || head_welds) && starts_word(bytes.get(i + 1).copied()))
        }
    }
}

/// Consume one token starting at `*i`, returning what it was.
fn scan_token(run: &str, i: &mut usize, in_group: bool) -> TokenKind {
    let bytes = run.as_bytes();

    if bytes[*i] == b'(' {
        return close_group(run, i);
    }
    // A welded `+` (the only way a `+` reaches here — an operator is claimed by
    // `operator_stands_here` first): the sign is the head of the token after it, which
    // `starts_word` guaranteed is there and is not a boundary byte.
    if bytes[*i] == b'+' {
        *i += 1;
        return scan_token(run, i, in_group);
    }
    if bytes[*i] == b'@' {
        return scan_atword(run, i);
    }
    // A brace is a one-byte member. It releases every operator like a closed group
    // (`a{1.5}/2.5` is a `}`, an operator and a number to postcss) and what follows it
    // starts a new member.
    if matches!(bytes[*i], b'{' | b'}') {
        *i += 1;
        return TokenKind::Closed;
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
        // A `[…]` simple block is **content**, where a `(…)` one is a value group and a
        // `{…}` one is two brace members around a run of its own: css-syntax-3 gives all
        // three a payload, but prettier's value parser descends only into the paren
        // block, leaves the bracket block as word text (`[1.50]` keeps its number on
        // both sides) and tokenizes each brace on its own. So the run continues through
        // a bracket block, operators and all — Tailwind's `--modifier(…, [*])` is one
        // token, not a bracket around a multiplication.
        if b == b'[' {
            *i = simple_block_end(run, *i);
            continue;
        }
        if ends_every_token(b) {
            return kind;
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

/// Consume the `@`-word at `*i` (its `@` included), returning what it is to the
/// separator rule: a closed member, since what follows it — an operator, a group, a
/// string, another `@` — always starts a member of its own.
fn scan_atword(run: &str, i: &mut usize) -> TokenKind {
    let bytes = run.as_bytes();
    debug_assert_eq!(bytes[*i], b'@', "scan_atword must be entered on a `@`");
    *i += 1;
    while *i < bytes.len() {
        let b = bytes[*i];
        if b == b'\\'
            && let Some(step) = escape_len(run, *i)
        {
            *i += step;
            continue;
        }
        if is_comment_start(bytes, *i) {
            *i = comment_end(bytes, *i);
            continue;
        }
        if ends_atword(b) {
            break;
        }
        *i += 1;
    }
    TokenKind::Closed
}

/// Is `text` exactly one `{…}` simple block — the **whole-value** block css-syntax-3
/// singles out ("a top-level {}-block is only allowed as the entire value of a non-custom
/// property")? The printer keeps that block's own braces spaced as authored where a brace
/// *inside* a value is glued to its interior.
pub(crate) fn is_whole_value_block(text: &str) -> bool {
    text.starts_with('{') && simple_block_end(text, 0) == text.len()
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
    /// byte, so the `is_operator` tag is graded by the same assertion as the split. The
    /// run is read as a declaration value's first (`head_welds`) unless `in_group`.
    fn split(run: &str, in_group: bool) -> Option<Vec<String>> {
        split_at(run, in_group, !in_group)
    }

    fn split_at(run: &str, in_group: bool, head_welds: bool) -> Option<Vec<String>> {
        split_value_run(run, in_group, head_welds).map(|tokens| {
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
            // a `[…]` simple block is content, operators and all
            "[*]",
            "[1.50]",
            "[a*b]",
            "[integer]",
            // an `@`-word runs through `*`, `+`, `-`, `:`, `#`, `.`, a digit and `[`
            "@a",
            "@",
            "@a*1.50",
            "@a+1.50",
            "@a-1.50",
            "@a1.50",
            "@a#FFF",
            "@a.50",
            "@a[1.50]c",
            r"@a\62 c",
            // a `+` opening the value welds onto the word after it
            "+a",
            "+a(2.50)",
            "+#FFF",
            "+a-1.50",
            "+a/1.50",
            r"+\61",
            "+u+26",
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
    /// value there is no oracle for splitting one. Inside an `@`-word it is content
    /// anywhere.
    #[test]
    fn a_colon_splits_only_inside_a_group() {
        assert_eq!(split("a:1.5", false), None);
        assert_eq!(split("a:1.5", true).unwrap(), ["a", "op :", "1.5"]);
        assert_eq!(split("1.5:2.5", true).unwrap(), ["1.5", "op :", "2.5"]);
        assert_eq!(split("@a:1.5", true), None);
    }

    /// An `@`-word ends every token before it and is ended by its own set: a quote, a
    /// comma, a paren, a brace, a `/` or another `@`.
    #[test]
    fn an_at_word_is_a_member_of_its_own() {
        assert_eq!(split("a@b", false).unwrap(), ["a", "@b"]);
        assert_eq!(split("@a@b", false).unwrap(), ["@a", "@b"]);
        assert_eq!(split("1.5@a", false).unwrap(), ["1.5", "@a"]);
        assert_eq!(split("#FFF@a", false).unwrap(), ["#FFF", "@a"]);
        assert_eq!(split("(1.5)@a", false).unwrap(), ["(1.5)", "@a"]);
        assert_eq!(split("@a(2.5)", false).unwrap(), ["@a", "(2.5)"]);
        assert_eq!(split("a@(2.5)", false).unwrap(), ["a", "@", "(2.5)"]);
        assert_eq!(split("@(2.5)", false).unwrap(), ["@", "(2.5)"]);
        assert_eq!(split("@a/1.5", false).unwrap(), ["@a", "op /", "1.5"]);
        assert_eq!(split("1.5/@a", false).unwrap(), ["1.5", "op /", "@a"]);
        assert_eq!(split("'x'@a", false).unwrap(), ["'x'", "@a"]);
        assert_eq!(split("@a'x'", false).unwrap(), ["@a", "'x'"]);
        assert_eq!(split("@a{1.5}", false).unwrap(), ["@a", "{", "1.5", "}"]);
        // an escaped `@` is ident content, never an `@`-word's start
        assert_eq!(split(r"a\@b", false), None);
        // a `-` straight before an `@`-word is an operator (glued by the sign rule), where
        // inside a word it is content (`a-` then the `@`-word)
        assert_eq!(split("-@a", false).unwrap(), ["op -", "@a"]);
        assert_eq!(
            split("1.5*-@a", false).unwrap(),
            ["1.5", "op *", "op -", "@a"]
        );
        assert_eq!(split("a-@b", false).unwrap(), ["a-", "@b"]);
        assert_eq!(split("-a", false), None);
    }

    /// Each brace is a one-byte member that releases every operator; the word between
    /// two braces is a member of its own.
    #[test]
    fn a_brace_is_a_member_of_its_own() {
        assert_eq!(
            split("a{1.5}c(2.5)", false).unwrap(),
            ["a", "{", "1.5", "}", "c(2.5)"]
        );
        assert_eq!(split("{1.5}", false).unwrap(), ["{", "1.5", "}"]);
        assert_eq!(split("{", false), None);
        assert_eq!(split("}", false), None);
        assert_eq!(
            split("a{{1.5}}c", false).unwrap(),
            ["a", "{", "{", "1.5", "}", "}", "c"]
        );
        assert_eq!(split("a{}c", false).unwrap(), ["a", "{", "}", "c"]);
        assert_eq!(
            split("a{1.5}/2.5", false).unwrap(),
            ["a", "{", "1.5", "}", "op /", "2.5"]
        );
        assert_eq!(
            split("a*{1.5}c", false).unwrap(),
            ["a", "op *", "{", "1.5", "}", "c"]
        );
        assert_eq!(
            split("a{1.5*2.5}c", false).unwrap(),
            ["a", "{", "1.5", "op *", "2.5", "}", "c"]
        );
        // a brace inside a bracket block, a string or an escape is content
        assert_eq!(split("a[{]c", false), None);
        assert_eq!(split("'{'", false), None);
        assert_eq!(split(r"a\{b", false), None);
    }

    /// A `+` welds onto the word after it at the value's head and after an operator;
    /// anywhere else, and before anything but a word, it is an operator.
    #[test]
    fn a_plus_welds_at_the_head_and_after_an_operator() {
        assert_eq!(split("+a*1.5", false).unwrap(), ["+a", "op *", "1.5"]);
        assert_eq!(
            split("1.5/+a(2.5)", false).unwrap(),
            ["1.5", "op /", "+a(2.5)"]
        );
        assert_eq!(split("1.5*+a", false).unwrap(), ["1.5", "op *", "+a"]);
        assert_eq!(
            split("1.5/++a", false).unwrap(),
            ["1.5", "op /", "op +", "+a"]
        );
        assert_eq!(split("++a", false).unwrap(), ["op +", "+a"]);
        assert_eq!(split("+a+b", false).unwrap(), ["+a", "op +", "b"]);
        assert_eq!(split("a+b", false).unwrap(), ["a", "op +", "b"]);
        // before a group, a string, an `@`-word or an operator it stays an operator
        assert_eq!(split("+(2.5)", false).unwrap(), ["op +", "(2.5)"]);
        assert_eq!(split("+'x'", false).unwrap(), ["op +", "'x'"]);
        assert_eq!(split("+@a", false).unwrap(), ["op +", "@a"]);
        assert_eq!(split("+-a", false).unwrap(), ["op +", "-a"]);
        assert_eq!(split("+/1.5", false).unwrap(), ["op +", "op /", "1.5"]);
        assert_eq!(split("1.5*+", false).unwrap(), ["1.5", "op *", "op +"]);
        // at the head of a group's interior, or after a comma, the head does not weld —
        // but a `+` after an operator inside the run still does
        assert_eq!(
            split_at("+a(2.5)", true, false).unwrap(),
            ["op +", "a(2.5)"]
        );
        assert_eq!(split_at("+a", false, false).unwrap(), ["op +", "a"]);
        assert_eq!(
            split_at("1.5/+a", true, false).unwrap(),
            ["1.5", "op /", "+a"]
        );
        // a signed number is a number, never a weld
        assert_eq!(split("+1.5", false), None);
        assert_eq!(split_at("+1.5", true, false), None);
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

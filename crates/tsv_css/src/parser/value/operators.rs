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
// A run that is **one operator token** is a member too, and that is not a special case
// but the same rule read at a whitespace boundary: `1.5 / 2.5`, `1.5 * - 2.5` and
// `--x: a : b` each put the operator in an element of its own, and it reaches the list as
// a `CssValue::Operator` exactly as a glued one does. Only an operand run of one token
// answers "no split".
//
// ⚠️ **The boundary set is not one class of byte.** `*` and `+` end a run wherever they
// occur; `/` and `-` end a **number** (or a closed group / function) and are ordinary
// content inside a word. That asymmetry is postcss's, and it is observable: `1.5/2.5` is
// three nodes and normalizes on both sides, where `a/1.5` is a single word whose `1.5`
// prettier leaves exactly as written.
//
// Where an OPERAND is expected — the run's head, or straight after another operator — the
// question is a third one, and the two signs ask it of **different grammars**
// ([`operator_at_operand_position`]). The `+` asks postcss's: does the next TOKEN come
// back as a word, so that the sign is the word's own ([`starts_word`])? The `-` asks
// css-syntax-3's, of itself: would the `-` and the byte after it start an identifier
// (§4.3.9) or a number (§4.3.10) (the lexer's [`hyphen_starts_own_token`])? The two agree
// on every byte that opens an ident or a number, and part at the bytes postcss's word scan
// runs through but no token production does. That is the RULE; the fifteen bytes it picks
// out of printable ASCII were measured, and the "Hyphen word extent at an operand
// position" entry in `docs/conformance_prettier_css.md` enumerates them.
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
//   normalizes (`a{1.50}c(2.50)` → `a{1.5}c(2.5)`, and its own `:` splits there like any
//   other), which is why the brace is not word content the way `[` is (`a[1.50]c` keeps
//   its number: prettier's value *parser* does not descend into a `[…]` block, though its
//   word tokenizer still ends a word at a `*` or `+` inside the brackets and its printer
//   then spaces the pieces — a cataloged divergence,
//   `css/values/operators/bracket_block_operator_prettier_divergence`);
// - a **welded `+`**: postcss's `operator()` returns `this.word()` for a `+` that opens
//   the value or follows an operator when the next token is a word, so the sign is the
//   word's own — `+a(2.50)` is the function `+a` and normalizes to `+a(2.5)`, `1.50 / +a`
//   → `1.5 / +a`. Anywhere else (after a member, at the head of a function's arguments
//   or of a group — the `(` is a node there — and after a comma) the `+` is an operator
//   and prints spaced (`f(+a(2.50))` → `f(+ a(2.5))`). Before a group, a string, an
//   `@`-word or another operator nothing welds (`+(2.5)`, `+'x'`, `+ @a`, `+ +a`), and
//   nothing welds after a `:` either — a `colon` node is no operator for a sign to bind to
//   (`--x: a:+b` → `a: + b`).

use super::scan::{comment_end, is_comment_start, matching_close_paren};
use crate::escapes::escape_len;
use crate::lexer::{hyphen_starts_own_token, string_end};
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

/// What stands to the left of the position being read — the whole of what the operator
/// question turns on, since a byte's reading is its position's.
///
/// [`Preceding::Head`] and [`Preceding::Operator`] share one arm of that question (an
/// operand is what follows either), but they are not one state: at the head a `+` welds
/// only where the caller says so ([`split_value_run`]'s `head_welds`), and after an
/// operator it welds unless that operator was the `:` — which is no operator for a sign
/// to bind to, the same exclusion the element-level rule makes
/// (`super::parser::ValueParser::member_is_operator`). Read as one state the two rules
/// disagreed, and `--x: a:+b` formatted to `a: +b` and then to `a: + b`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Preceding {
    /// Nothing at all: the run's first byte.
    Head,
    /// An operator member, and whether that operator was the `:`.
    Operator { colon: bool },
    /// An operand token of this kind.
    Token(TokenKind),
}

/// A byte that can open an operator after a [`TokenKind::Number`] or
/// [`TokenKind::Closed`] token — every operator byte there is. The `:` is one only where
/// a `:` can stand as an operator at all; see [`split_value_run`]'s `colon_is_operator`.
const fn releases_operator(b: u8, colon_is_operator: bool) -> bool {
    matches!(b, b'*' | b'+' | b'/' | b'-') || (b == b':' && colon_is_operator)
}

/// A byte that ends a [`TokenKind::Word`]. The strict subset of
/// [`releases_operator`] that is never an ident code point *and* never glues into a
/// postcss word: `/` and `-` are missing on purpose.
const fn ends_word(b: u8, colon_is_operator: bool) -> bool {
    matches!(b, b'*' | b'+') || (b == b':' && colon_is_operator)
}

/// A byte that is an operator at a run's START, or straight after another operator.
///
/// `-` is missing on purpose: there it can be the head of the ident or the number after
/// it (`-webkit-box`, `-1.5`), so whether it is an operator turns on what comes *after*
/// it, which this one-byte test cannot see. [`operator_at_operand_position`] answers the
/// `-` with [`hyphen_starts_own_token`]. `+` is in, since it can open neither an ident nor
/// a number of its own (a signed number is claimed by `number_part_len` ahead of this
/// test) — only WELD onto a word, which the same caller asks with [`starts_word`].
const fn opens_run(b: u8, colon_is_operator: bool) -> bool {
    matches!(b, b'*' | b'/' | b'+') || (b == b':' && colon_is_operator)
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

/// Does the byte after a `+` open a WORD? postcss's condition is "the next token is a
/// `word`": not a group, a string, an `@`-word, a brace, a comma, a colon, another
/// operator, or the run's end. Everything else — a letter, `#`, `.`, `\`, `[`, `!`, a
/// digit the number production did not claim — starts a word, and the `+` WELDS onto it
/// (postcss's `operator()` returning `this.word()`).
///
/// This is the `+`'s question alone. The `-`'s is [`hyphen_starts_own_token`], which asks
/// css-syntax-3 of the `-` itself rather than postcss of the token after it: read this way
/// a `-` before a `[…]` block or a `#` would be the word's first byte, and the same three
/// bytes then had two readings in one document (see [`operator_at_operand_position`]'s
/// ⚠️).
///
/// A `-` is the one byte the answer cannot be read off alone, which is why the byte
/// **after** it is a parameter: a single `-` opens an ident or signs a number and the `+`
/// stays an operator (`+-a` → `+ -a`, `+-1.5` → `+ -1.5`), but a **doubled** one opens the
/// custom-property-shaped word ([`is_content_pair`]'s first pair), and that word the `+`
/// welds onto exactly as it welds onto a letter — `+--a`, `+--`, `+----a` and `+-- @a` are
/// prettier's own output at every position a word welds at (`1.5 * +--a`, `+ +--a`) and at
/// none of the positions none does (`1.5 + --a`, `f(+ --a)`, `(+ --a)`).
const fn starts_word(b: Option<u8>, after: Option<u8>) -> bool {
    match b {
        None => false,
        Some(b'-') => matches!(after, Some(b'-')),
        Some(b) => !matches!(
            b,
            b'(' | b')' | b'\'' | b'"' | b'@' | b'{' | b'}' | b',' | b':' | b'*' | b'/' | b'+'
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
/// the bytes instead, an **operand** that merely opens on an operator character (the `-a`
/// in `1.5*-a`, where the `-` opens the ident rather than a subtraction) would be minted
/// as an operator and take a glue rule that is not its own.
///
/// It is also what a **one-token** run is split on: the same verdict says whether that
/// run is the whitespace-delimited operator element (`1.5 / 2.5`) or an ordinary leaf,
/// so neither the printer's `ValueOperator` nor the weld test
/// (`super::parser::ValueParser::member_is_operator`) has to spell the bytes again.
pub(crate) struct ValueToken {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) is_operator: bool,
}

/// The members `run` splits into, or `None` when it is a single **operand** token —
/// the overwhelmingly common case, which allocates nothing and leaves the caller's
/// existing leaf classification untouched. A run that is a single token which IS an
/// operator still splits, into that one member: a whitespace-delimited `/`, `:` or `-`
/// element (`1.5 / 2.5`, `--x: a : b`) is an operator node to postcss like any other, and
/// the separator rule reads its kind off the node rather than off its bytes.
///
/// `colon_is_operator` says whether a `:` can stand as an operator here: inside a
/// function's argument list or a parenthesized group, and at the top level of a **custom
/// property's** value, whose text css-syntax-3 admits whatever it holds. On a plain
/// property a top-level `:` is not a value token at all (prettier's parser rejects
/// `a { b: c:d }` outright), so there is no oracle for splitting one and the run is left
/// as the author wrote it.
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
    colon_is_operator: bool,
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
    // What stands to the left of `i` (see [`Preceding`]).
    let mut previous = Preceding::Head;

    while i < len {
        if operator_stands_here(run, i, previous, colon_is_operator, head_welds) {
            tokens.push(ValueToken {
                start: i,
                end: i + 1,
                is_operator: true,
            });
            previous = Preceding::Operator {
                colon: bytes[i] == b':',
            };
            i += 1;
            continue;
        }
        let start = i;
        previous = Preceding::Token(scan_token(run, &mut i, colon_is_operator));
        debug_assert!(i > start, "a token consumed nothing: {run:?} at {start}");
        tokens.push(ValueToken {
            start,
            end: i,
            is_operator: false,
        });
    }

    // A one-token run splits only when that token is an operator (the whitespace-delimited
    // element above). The verdict is the splitter's own — read back off the bytes, the
    // `-a` in `1.5*-a` would be minted as an operator it is not (see
    // [`ValueToken::is_operator`]).
    (tokens.len() > 1 || tokens.first().is_some_and(|t| t.is_operator)).then_some(tokens)
}

/// Is the byte at `i` an operator in its own right, given what came before it?
///
/// Three readings of one question, because what releases an operator is what precedes it:
/// a number or a closed run releases every operator byte, a word releases only the two
/// that are never ident content, and at a run's start (or straight after another
/// operator) only a byte that cannot *begin* an operand can be one — and there a `+`
/// that can weld (`head_welds` at the run's start, after any operator but the `:`) is the
/// head of the word that follows it, not an operator.
fn operator_stands_here(
    run: &str,
    i: usize,
    previous: Preceding,
    colon_is_operator: bool,
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
        Preceding::Token(TokenKind::Number | TokenKind::Closed) => {
            releases_operator(b, colon_is_operator)
        }
        Preceding::Token(TokenKind::Word) => ends_word(b, colon_is_operator),
        // The two positions an OPERAND is expected at differ in exactly one input — whether
        // a `+` here welds onto the word after it — so they are one reading with that input
        // named: at the run's head the caller says, and after an operator every operator but
        // the `:` welds.
        Preceding::Head => operator_at_operand_position(run, i, colon_is_operator, head_welds),
        Preceding::Operator { colon } => {
            operator_at_operand_position(run, i, colon_is_operator, !colon)
        }
    }
}

/// [`operator_stands_here`] where an OPERAND is expected — at the run's head, or straight
/// after another operator. Only a byte that cannot *begin* an operand is an operator there.
///
/// `plus_welds` says whether a `+` here is the head of the word after it rather than an
/// operator (postcss's `operator()` returning `this.word()`): a `colon` node is no operator
/// for a sign to bind to, which is the element-level rule's exclusion too
/// (`super::parser::ValueParser::member_is_operator`), so `--x: a:+b` and `f(a:+b)` space the
/// `+` on both sides in one pass where `--x: a: +1.5` keeps the number's own sign.
///
/// A `-` here is the operator wherever it opens neither an IDENT (css-syntax-3 §4.3.9)
/// nor a NUMBER (§4.3.10) of its own, which is the lexer's [`hyphen_starts_own_token`] —
/// the one reading the printer's glue refusal takes too, so the two cannot part. A
/// doubled `-` never reaches it ([`operator_stands_here`] answers `--` as content ahead of
/// this), but §4.3.9 names it and the reading admits it, so they agree by construction.
///
/// ⚠️ **Not [`starts_word`], which is the `+`'s question.** postcss's word runs on to the
/// next byte in a *printer's* end set, so `#`, `.`, `!`, `%`, `[`, `]` and the rest of the
/// fifteen bytes this module's header names are all word content to it, and a `-` in
/// front of any of them would be the word's first byte. Read that way the same `-` had
/// two readings in one document: run-final it is an operator on every reading (`+-` then
/// ` [a]`), the head rule glued it onto the block, and the glued text `-[a]` then read
/// back as one word — two passes, two forms. Asking the byte-level question also keeps it
/// off the token this module would scan, which is a third answer again: a quote and a `[`
/// are both word CONTENT to [`scan_token_body`], so `-'x'` and `-[a]` each scan whole. See
/// `tests/fixtures/css/values/operators/hyphen_word_extent_prettier_divergence`.
fn operator_at_operand_position(
    run: &str,
    i: usize,
    colon_is_operator: bool,
    plus_welds: bool,
) -> bool {
    let bytes = run.as_bytes();
    let b = bytes[i];
    let next = bytes.get(i + 1).copied();
    let after = bytes.get(i + 2).copied();
    // A `-` here is the operator wherever it does not open an IDENT or a NUMBER of its
    // own — a quote, a `(`, a `,` / `:` / `{` / `}` / `@` / `*` / `/` / `+`, a `[…]`
    // block, a `#`, a `!`, and the run's end (`+-'x'`, `+-(2.5)`, `+-/2.5`, `+-*a`,
    // `--x: +-: a`, `1.5 /-`, `+-[a]`, `+-#a`). A letter, a digit, `_`, `é`, an escape
    // and a second `-` are the bound, where the `-` is that token's own head and
    // `-webkit-box`, `-1.5`, `-\61`, `--a` stay whole.
    if b == b'-' {
        return !hyphen_starts_own_token(&run[i + 1..]);
    }
    opens_run(b, colon_is_operator)
        && number_part_len(&run[i..]) == 0
        && !(b == b'+' && plus_welds && starts_word(next, after))
}

/// Consume one token starting at `*i`, returning what it was.
fn scan_token(run: &str, i: &mut usize, colon_is_operator: bool) -> TokenKind {
    let bytes = run.as_bytes();

    if bytes[*i] == b'(' {
        return close_group(run, i);
    }
    // A welded `+` (the only way a `+` reaches here — an operator is claimed by
    // `operator_stands_here` first): the sign is the head of the token after it, which
    // `starts_word` guaranteed is there and is not a boundary byte.
    if bytes[*i] == b'+' {
        *i += 1;
        return scan_token(run, i, colon_is_operator);
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
        return scan_token_body(run, i, colon_is_operator, TokenKind::Number);
    }

    scan_token_body(run, i, colon_is_operator, TokenKind::Word)
}

/// The shared tail of both token shapes: walk content until a boundary this `kind`
/// releases, stepping every opaque construct whole. A `(` turns the token into a
/// [`TokenKind::Closed`] one (a function name meeting its argument list, or a bare group).
fn scan_token_body(
    run: &str,
    i: &mut usize,
    colon_is_operator: bool,
    kind: TokenKind,
) -> TokenKind {
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
            TokenKind::Number | TokenKind::Closed => releases_operator(b, colon_is_operator),
            TokenKind::Word => ends_word(b, colon_is_operator),
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
    /// run is read as a group's interior when `in_group_interior` — where a `:` is an
    /// operator and the head does not weld — and as a plain declaration value's first run
    /// otherwise.
    fn split(run: &str, in_group_interior: bool) -> Option<Vec<String>> {
        split_at(run, in_group_interior, !in_group_interior)
    }

    fn split_at(run: &str, colon_is_operator: bool, head_welds: bool) -> Option<Vec<String>> {
        split_value_run(run, colon_is_operator, head_welds).map(|tokens| {
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

    /// A `:` is an operator only where `colon_is_operator` says it can stand as one —
    /// inside a group, and at the top level of a custom property's value (where the head
    /// still welds, unlike a group's). On a plain property there is no oracle for
    /// splitting one. Inside an `@`-word it is content anywhere.
    #[test]
    fn a_colon_splits_only_where_it_is_an_operator() {
        assert_eq!(split("a:1.5", false), None);
        assert_eq!(split("a:1.5", true).unwrap(), ["a", "op :", "1.5"]);
        assert_eq!(split("1.5:2.5", true).unwrap(), ["1.5", "op :", "2.5"]);
        assert_eq!(split("@a:1.5", true), None);
        // a custom property's value: the colon splits and the head still welds
        assert_eq!(split_at("a:1.5", true, true).unwrap(), ["a", "op :", "1.5"]);
        assert_eq!(split_at("+a(2.5)", true, true), None);
        assert_eq!(split_at("a:1.5", false, true), None);
        // a colon that opens the run, and two in a row, are members of their own
        assert_eq!(split_at(":a", true, true).unwrap(), ["op :", "a"]);
        assert_eq!(
            split_at("a::1.5", true, true).unwrap(),
            ["a", "op :", "op :", "1.5"]
        );
        // the sign the author glued to the digits after a colon is the number's own
        assert_eq!(
            split_at("a:+1.5", true, true).unwrap(),
            ["a", "op :", "+1.5"]
        );
        // a `{…}` block's interior splits at its colon like any other run
        assert_eq!(
            split_at("a{--b:1.5}c", true, true).unwrap(),
            ["a", "{", "--b", "op :", "1.5", "}", "c"]
        );
        // a doubled `/` is content, so a URL-shaped value is a word, a colon and a word
        assert_eq!(
            split_at("https://a.fuz.dev/b", true, true).unwrap(),
            ["https", "op :", "//a.fuz.dev/b"]
        );
    }

    /// A run that is one **operator** token is a member of its own — the
    /// whitespace-delimited element (`1.5 / 2.5`, `--x: a : b`, `1.5 * - 2.5`), which
    /// reaches the list as an operator node rather than as a word spelled with the
    /// operator's byte.
    #[test]
    fn a_one_token_operator_run_is_a_member() {
        assert_eq!(split("/", false).unwrap(), ["op /"]);
        assert_eq!(split("*", false).unwrap(), ["op *"]);
        assert_eq!(split("+", false).unwrap(), ["op +"]);
        assert_eq!(split("-", false).unwrap(), ["op -"]);
        // and the bytes postcss's word scan runs THROUGH but no token production opens on:
        // a `[…]` block, a `#`, a `.` no digit follows, a `!`
        assert_eq!(split("-[a]", false).unwrap(), ["op -", "[a]"]);
        assert_eq!(split("-#a", false).unwrap(), ["op -", "#a"]);
        assert_eq!(split("-.a", false).unwrap(), ["op -", ".a"]);
        assert_eq!(split("-!a", false).unwrap(), ["op -", "!a"]);
        assert_eq!(split(":", true).unwrap(), ["op :"]);
        // …and only where the byte really is one: a plain property's `:` is content
        assert_eq!(split(":", false), None);
        // a one-token OPERAND run still answers `None`, which is nearly every value
        assert_eq!(split("1.5", false), None);
        assert_eq!(split("-a", false), None);
        assert_eq!(split("-1.5", false), None);
    }

    /// A `-` where an OPERAND is expected is the operator wherever it opens neither an
    /// IDENT (css-syntax-3 §4.3.9) nor a NUMBER (§4.3.10) of its own — a quote, a `(`, a
    /// `)`, a `,`, a `:`, a brace, an `@`, a `*`, a `/`, a `+`, a `[…]` block, a `#`, a
    /// `!`, or the run's end — and that token's own head wherever it does. The question is
    /// the byte's, not the token this module would scan: `-'x'` and `-[a]` both scan whole
    /// here, and both of them split.
    #[test]
    fn a_hyphen_is_the_operator_wherever_it_opens_no_token_of_its_own() {
        assert_eq!(split("-'x'", false).unwrap(), ["op -", "'x'"]);
        assert_eq!(split(r#"-"x""#, false).unwrap(), ["op -", r#""x""#]);
        assert_eq!(split("-(2.5)", false).unwrap(), ["op -", "(2.5)"]);
        assert_eq!(split("-,a", false).unwrap(), ["op -", ",a"]);
        assert_eq!(split("-)", false).unwrap(), ["op -", ")"]);
        assert_eq!(split_at("-:a", true, true).unwrap(), ["op -", "op :", "a"]);
        assert_eq!(split("-{1.5}", false).unwrap(), ["op -", "{", "1.5", "}"]);
        assert_eq!(split("-@a", false).unwrap(), ["op -", "@a"]);
        assert_eq!(split("-*a", false).unwrap(), ["op -", "op *", "a"]);
        assert_eq!(split("-/2.5", false).unwrap(), ["op -", "op /", "2.5"]);
        assert_eq!(split("-+a", false).unwrap(), ["op -", "+a"]);
        assert_eq!(split("-+1.5", false).unwrap(), ["op -", "+1.5"]);
        assert_eq!(split("-", false).unwrap(), ["op -"]);
        // and the same reading straight after another operator, where the head's `+` is
        // an operator of its own because a `-` opens no word for it to weld onto
        assert_eq!(split("+-'x'", false).unwrap(), ["op +", "op -", "'x'"]);
        assert_eq!(split("+-(2.5)", false).unwrap(), ["op +", "op -", "(2.5)"]);
        assert_eq!(
            split("1.5*-+a", false).unwrap(),
            ["1.5", "op *", "op -", "+a"]
        );
        // the bound: a byte that DOES open an ident or a number keeps the `-` inside it —
        // an ident (`-webkit-box`), a number, a `.`-opened number, an escape, `_`, a
        // non-ASCII ident code point, and the doubled `-` that opens a
        // custom-property-shaped word
        assert_eq!(split("-webkit-box", false), None);
        assert_eq!(split("-1.5", false), None);
        assert_eq!(split("-.5", false), None);
        assert_eq!(split(r"-\61", false), None);
        assert_eq!(split("-_a", false), None);
        assert_eq!(split("-\u{e9}", false), None);
        assert_eq!(split("--a", false), None);
        // the `+` keeps postcss's word for its own weld, so the two signs part exactly at
        // the bytes above: the `+` is an operator before a `-` that opens no word of its
        // own, and the `-` behind it is one before a `[…]` block where it is not before a
        // letter
        assert_eq!(split("+-[a]", false).unwrap(), ["op +", "op -", "[a]"]);
        assert_eq!(split("+-a", false).unwrap(), ["op +", "-a"]);
        assert_eq!(split("+[a]", false), None);
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

    /// The word a `+` welds onto can be the custom-property-shaped one: a DOUBLED `-`
    /// opens it where a single one opens an ident or signs a number.
    #[test]
    fn a_plus_welds_onto_a_doubled_dash_word() {
        assert_eq!(split("+--a", false), None);
        assert_eq!(split("+--", false), None);
        assert_eq!(split("+----a", false), None);
        assert_eq!(split("+--@a", false).unwrap(), ["+--", "@a"]);
        assert_eq!(split("1.5*+--a", false).unwrap(), ["1.5", "op *", "+--a"]);
        assert_eq!(split("++--a", false).unwrap(), ["op +", "+--a"]);
        // a single `-` after the sign is the ident's or the number's, and the `+` stays
        // an operator
        assert_eq!(split("+-a", false).unwrap(), ["op +", "-a"]);
        assert_eq!(split("+-1.5", false).unwrap(), ["op +", "-1.5"]);
        // and at the positions no word welds at, neither does this one
        assert_eq!(split_at("+--a", true, false).unwrap(), ["op +", "--a"]);
        assert_eq!(split_at("+--a", false, false).unwrap(), ["op +", "--a"]);
        assert_eq!(split("1.5+--a", false).unwrap(), ["1.5", "op +", "--a"]);
    }

    /// A single `/` still splits where the doubled one does not.
    #[test]
    fn a_lone_slash_still_splits() {
        assert_eq!(split("/1.5", false).unwrap(), ["op /", "1.5"]);
        assert_eq!(split("1.5/", false).unwrap(), ["1.5", "op /"]);
    }

    /// A member is not an operator just because it BEGINS with one's byte: at a run's
    /// start — or, as here, straight after another operator — a `-` before an ident code
    /// point opens the ident, so `-a` is an OPERAND however its first byte reads.
    /// Re-deriving the tag from the bytes is what `ValueToken::is_operator` exists to
    /// avoid.
    #[test]
    fn an_operand_opening_on_an_operator_byte_is_not_tagged_as_one() {
        assert_eq!(split("1.5*-a", false).unwrap(), ["1.5", "op *", "-a"]);
        assert_eq!(split("1.5*-1.5", false).unwrap(), ["1.5", "op *", "-1.5"]);
    }

    /// …and a `-` with nothing after it in the run IS the operator, in both of its
    /// positions: the whitespace-delimited element (`-`, the whole run) and a glued run's
    /// tail (`+-`, `1.5/-`, `a:-`). Read the second as a word and the two spellings of one
    /// value print each other forever (`+-` → `+ -` → `+-`).
    #[test]
    fn a_trailing_hyphen_is_the_operator_in_both_positions() {
        assert_eq!(split("-", false).unwrap(), ["op -"]);
        assert_eq!(split("+-", false).unwrap(), ["op +", "op -"]);
        assert_eq!(split("1.5/-", false).unwrap(), ["1.5", "op /", "op -"]);
        assert_eq!(split("1.5*-", false).unwrap(), ["1.5", "op *", "op -"]);
        assert_eq!(split_at("a:-", true, true).unwrap(), ["a", "op :", "op -"]);
    }

    /// A `+` welds onto the word after it wherever the run's head or another operator
    /// stands before it — but never after a `:`, which is no operator for a sign to bind
    /// to. The element-level rule says the same (`ValueParser::member_is_operator`), and
    /// the two must agree or one pass welds and the next splits.
    #[test]
    fn a_plus_does_not_weld_after_a_colon() {
        assert_eq!(
            split_at("a:+b", true, true).unwrap(),
            ["a", "op :", "op +", "b"]
        );
        assert_eq!(split_at(":+a", true, true).unwrap(), ["op :", "op +", "a"]);
        // …while the sign of a NUMBER is the number's own at every position
        assert_eq!(
            split_at("a:+1.5", true, true).unwrap(),
            ["a", "op :", "+1.5"]
        );
        // and after any other operator the weld stands
        assert_eq!(
            split_at("1.5/+b", true, true).unwrap(),
            ["1.5", "op /", "+b"]
        );
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

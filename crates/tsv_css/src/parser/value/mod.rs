// CSS value parsing
//
// Parses CSS values into structured AST (CssValue enum).
// Handles identifiers, strings, numbers/dimensions, colors, functions, and lists.

pub mod colors;
pub(crate) mod cursor;
pub mod dimensions;
pub mod functions;
pub mod lists;
pub(crate) mod operators;
pub(crate) mod parser;
pub(crate) mod scan;
pub mod strings;

use crate::ast::internal::CssValue;
use crate::escapes::{ends_with_live_hex_escape, trim_end_preserving_escape, trim_start_css};
use crate::parser::value::lists::ValueSeparator;
use bumpalo::Bump;
use tsv_lang::Span;

// The per-kind parsers the dispatch below calls. Internal to the value parser — like
// `classify_separators`, none of them is part of the crate's surface.
use colors::{parse_color, parse_color_function};

use dimensions::parse_dimension;
use functions::parse_function_arguments;
use strings::parse_string_literal;

/// Parse a CSS value into a structured CssValue
///
/// Extracts the value directly from source using the provided span, then parses
/// it using ValueParser for accurate span tracking with same-source recursion.
///
/// This ensures that nested value spans are accurate even with multiline formatting,
/// since we're working with the actual source text rather than reconstructed tokens.
///
/// # Arguments
/// * `source` - The CSS source text (may be a substring of the full document)
/// * `source_relative_span` - The span of the value relative to `source` (positions within source)
/// * `base_offset` - Offset to add to spans for absolute positions in full document
/// * `class` - The value's top-level separator class if the declaration's boundary scan
///   already derived it (`crate::parser::decl_scan`), which lets `ValueParser` skip its own
///   classifying pass over the very same bytes. `None` means "not known", never "no
///   separator" — that is `Some(ValueSeparator::None)`.
pub fn parse_value_from_source<'arena>(
    source: &str,
    source_relative_span: Span,
    base_offset: u32,
    class: Option<ValueSeparator>,
    arena: &'arena Bump,
) -> CssValue<'arena> {
    let value_str = source_relative_span.extract(source);
    let absolute_start = base_offset + source_relative_span.start;

    // An all-whitespace (or empty) value keeps the span it was handed.
    let Some((trimmed, leading)) = locate_value(value_str) else {
        return CssValue::Identifier {
            span: Span {
                start: absolute_start,
                end: base_offset + source_relative_span.end,
            },
        };
    };

    // The value's own span: where it starts, plus how long it is. The end needs
    // no separate trailing-whitespace offset — a span that begins at the value
    // and runs its length already excludes what follows it.
    let start = absolute_start + leading as u32;
    let absolute_span = Span {
        start,
        end: start + trimmed.len() as u32,
    };

    // The class was derived over the span the declaration scan measured, so it describes
    // this text only when the trim took nothing off either end. It normally does not (a real
    // stylesheet puts no whitespace at a value's ends, and the scan's own `value_end` is
    // already trimmed); when it does, the class is dropped and the fused pass runs as usual.
    let class = if leading == 0 && trimmed.len() == value_str.len() {
        class
    } else {
        None
    };

    // ValueParser re-parses the same source text, so nested value spans stay
    // accurate through its same-source recursion.
    parser::ValueParser::new(trimmed, absolute_span).parse_classified(arena, class)
}

/// A value's text with its surrounding **CSS** whitespace removed, and how many
/// bytes of that whitespace preceded it. `None` when the value is entirely
/// whitespace.
///
/// The span a declaration hands over is, in practice, already trimmed — real
/// stylesheets do not put a whitespace byte at either end of a value — so the
/// common case answers from two byte comparisons and never walks the text.
///
/// The trim is [`trim_start_css`] / [`trim_end_preserving_escape`], **not**
/// `str::trim`: CSS whitespace is [`is_css_whitespace`](crate::whitespace::is_css_whitespace)'s five ASCII characters, so NBSP and
/// U+3000 are value *content* (`str::trim` would eat them), and the trailing
/// whitespace a `\` escape owns is content too — `50px\ ;` must keep its escaped
/// space or the backslash strands onto the `;` and the output stops parsing.
/// `trimmed` becomes the `ValueParser`'s source and its length is what the leaf
/// spans are derived from, so cutting an escape's payload here is what does it.
///
/// The fast path's guard is therefore exactly `u8::is_ascii_whitespace` — the
/// same set the trim acts on, so the two arms cannot disagree. A boundary byte
/// that is not ASCII whitespace settles the question outright, **including a
/// non-ASCII one**: no byte of a multi-byte char is ASCII whitespace, and the
/// trim would not have touched that char anyway. ⚠️ The set is
/// `u8::is_ascii_whitespace`, **not** `char::is_whitespace` (which would eat NBSP)
/// and not the lexer's `is_ascii_css_whitespace` (which adds the **vertical tab**
/// to match `parseCss`); the vertical tab is content to this trim.
fn locate_value(value_str: &str) -> Option<(&str, usize)> {
    let bytes = value_str.as_bytes();
    let settled = |b: u8| !b.is_ascii_whitespace();
    if let (Some(&first), Some(&last)) = (bytes.first(), bytes.last())
        && settled(first)
        && settled(last)
    {
        return Some((value_str, 0));
    }

    let after_leading = trim_start_css(value_str);
    let trimmed = trim_end_preserving_escape(after_leading);
    if trimmed.is_empty() {
        return None;
    }
    Some((trimmed, value_str.len() - after_leading.len()))
}

// Old parsing functions removed - replaced by ValueParser with same-source recursion
// - parse_value_string() → use parse_value_from_source() instead
// - parse_value_or_list() → handled internally by ValueParser
// See: parser::ValueParser for the new implementation

/// Parse a single CSS value (no lists).
///
/// `s` is expected already trimmed: the sole caller (`ValueParser::build_leaf`)
/// forwards a trimmed range — the fast path passes `self.text()` (the boundary
/// check confirmed it is trimmed) and the two-pass fallback passes
/// `self.text().trim()` — so no `str::trim` is repeated here.
pub(crate) fn parse_single_value<'arena>(
    s: &str,
    span: Span,
    in_group: bool,
    arena: &'arena Bump,
) -> Option<CssValue<'arena>> {
    if s.is_empty() {
        return None;
    }

    // An operator run is several members, not a leaf — `12px/1.5` is a dimension, a `/`
    // and a dimension, and every classification below then runs on each of them. Asked
    // first because the run's own shape is what the rest of this function would otherwise
    // misread: `(1.5)(2.5)` ends in a `)` without being one function, and `1.5/2.5` starts
    // with a number without being one dimension. It answers `None` for a single-token run,
    // which is nearly every value, so the leaf path below is untouched.
    if let Some(tokens) = operators::split_value_run(s, in_group) {
        let mut values = bumpalo::collections::Vec::with_capacity_in(tokens.len(), arena);
        for token in tokens {
            let member = &s[token.start..token.end];
            let member_span = Span {
                start: span.start + token.start as u32,
                end: span.start + token.end as u32,
            };
            values.push(if token.is_operator {
                CssValue::Operator { span: member_span }
            } else {
                // Every operand is re-classified from scratch, which is what gets the
                // number after the operator to its normalizer. The recursion terminates:
                // a member is strictly shorter than the run unless the run was one token,
                // and that case returned `None` above.
                parse_single_value(member, member_span, in_group, arena)
                    .unwrap_or(CssValue::Identifier { span: member_span })
            });
        }
        return Some(CssValue::List {
            values: values.into_bump_slice(),
            span,
        });
    }

    // String literal
    if let Some(val) = parse_string_literal(s, span, arena) {
        return Some(val);
    }

    // Function call or color function.
    //
    // The last byte decides whether either scan below can pay off at all:
    // `extract_function_parts` accepts a value as a function only when the matching `)` is
    // its final byte — the same postcondition the printer's argument-list bounds are read
    // off — so a value ending in anything else has no function to find, and the two walks
    // that would establish that (the search for the opening `(`, then the matching-paren
    // scan from it) answer a question one byte comparison has already answered. Most of a
    // stylesheet's leaves — `red`, `0`, `1px`, `#fff` — end in something else. It is the
    // same kind of pre-filter the vocabulary sets put in front of their hash
    // (`crate::keyword_set`): it refuses only what cannot match, so it can skip work but
    // never change an answer.
    //
    // The refusal is graded by the scan it skips, so every CSS fixture re-proves it.
    let bytes = s.as_bytes();
    debug_assert!(
        bytes.last() == Some(&b')')
            || bytes
                .iter()
                .position(|&b| b == b'(')
                .and_then(|paren_pos| extract_function_parts(s, paren_pos))
                .is_none(),
        "a value not ending in `)` was read as a function: {s:?}"
    );
    // The search for the `(` is a byte-position scan rather than `str::find(char)`, whose
    // CharSearcher state machine outweighs a direct byte loop on this hot per-value-token
    // path (equivalent: `(` is ASCII, self-synchronising).
    if bytes.last() == Some(&b')')
        && let Some(paren_pos) = bytes.iter().position(|&b| b == b'(')
        && let Some((name, args)) = extract_function_parts(s, paren_pos)
    {
        // Try color function first
        if let Some(color) = parse_color_function(name, args) {
            return Some(CssValue::Color { color, span });
        }
        // Fall back to generic function
        // Calculate accurate span for arguments (inside parens)
        // The args string starts at: paren_pos + 1 (after opening paren)
        // The args string ends at: paren_pos + 1 + args.len()
        let args_start = paren_pos + 1;
        let args_span = Span {
            start: span.start + args_start as u32,
            end: span.start + args_start as u32 + args.len() as u32,
        };
        // A comma **closing** the argument list (`var(--a,)`, `rgb(1, 2, 3,)`) terminated
        // no argument, so it is not one — CSS Syntax 3's comma-split stops once the input
        // is empty. It is still authored content the printer must spell back, and it reads
        // that off the source between the last argument and the `)`
        // (`printer::declarations::list_has_closing_comma`) rather than from a synthesized
        // empty argument here: an *escaped* comma (`var(--b, x\,)`) is content inside the
        // last argument, and a synthesized one would double it.
        //
        // The name is a slice of `s`, so it is a slice of the source: hand the printer
        // its span rather than a copy of its bytes (span-for-verbatim). `name` itself
        // still answers `parse_color_function` above, at parse time.
        //
        // Its offset is read off the two heads rather than threaded out of the split,
        // which keeps that tuple at four words (returning it as a fifth cost ~288 bytes
        // of `.text` and a little of the lever).
        //
        // ⚠️ It does NOT buy back the frame. This is the CSS value parser's own
        // recursion, and the name's location has to live across `parse_function_arguments`
        // below, so `build_leaf` grows 16 bytes and `calc(calc(…))` loses ~1,157 levels of
        // its depth budget. Three spellings were measured — the fifth tuple element, this
        // one, and hoisting the recursive call above the struct expression — and all three
        // read exactly that number. It is the shape's price, not a spelling's; don't spend
        // a session re-spelling it.
        let name_start = name.as_ptr() as usize - s.as_ptr() as usize;
        let name_span = Span {
            start: span.start + name_start as u32,
            end: span.start + (name_start + name.len()) as u32,
        };
        return Some(CssValue::Function {
            name_span,
            args: parse_function_arguments(args, args_span, arena),
            span,
        });
    }

    // Hex or named color
    if let Some(color) = parse_color(s) {
        return Some(CssValue::Color { color, span });
    }

    // Dimension (number with optional unit)
    if let Some(dim) = parse_dimension(s, span) {
        return Some(dim);
    }

    // Default to identifier (text recovered from `span` at print time)
    Some(CssValue::Identifier { span })
}

/// Extract function name and arguments, validating balanced parentheses.
/// Both returned strings borrow from `s`, and neither is kept: the caller reads the name's
/// **offset** off the two heads and stores a `name_span` (span-for-verbatim, so an escaped
/// spelling survives), and `args` is re-parsed.
///
/// `Some` means the whole of `s` is the function — the matching close paren is its last
/// byte — which is what lets the printer bound the argument list at `span.end - 1`
/// (`build_value_function_doc`'s closing-comma read). It is also why the sole caller can
/// refuse on `s`'s last byte alone: the close paren this returns on *is* that byte, so a
/// value ending in anything else is never a function.
///
/// ⚠️ **`paren_pos` must address a `(`** — both call sites derive it from `position(|&b| b ==
/// b'(')`, so the first byte [`scan::matching_close_paren`] reads always opens the run, which is
/// what lets its unsigned depth start at zero. A violated precondition trips that walk's
/// `debug_assert`, rather than quietly returning a wrong span.
///
/// ⚠️ **That leading search is quote- and escape-blind, and [`is_function_name`] is what keeps
/// it sound.** A `(` can be *content* in the name region — inside a glued string (`'x'fn(a)`)
/// or an escape (`a\(b(c)`) — and the search stops on it either way. The name validation
/// refuses both, for two different reasons, and neither is a special case:
///
/// - a quote is not word content — the oracle tokenizes a string on its own and prints it as
///   a separate member (`'x' fn(1.5)`), so `'x'fn` is no one word;
/// - the byte before an escaped `(` is the escape's own `\`, so `name_part` ends in a
///   **dangling** `\` — not a valid escape (§4.3.7 needs a code point after it), and so not
///   word content either.
///
/// So a mis-located `(` always yields a name the validation rejects, and the value falls back
/// to an opaque identifier printed verbatim. That is lossless, and on both shapes it is what
/// prettier emits too (it unbalances on the escaped paren and splits the glued string).
/// ⚠️ The dangling-`\` half is **wider than the agreement**: where that escaped `(` happens to
/// *balance* at the value's end, prettier reads its own one-byte word `\` as the name and
/// normalizes inside (`a\(2.50)` → `a\(2.5)`). Keeping the search blind is worth that cell;
/// `css/values/functions/escaped_delimiter_name_prettier_divergence` records it.
fn extract_function_parts(s: &str, paren_pos: usize) -> Option<(&str, &str)> {
    // CSS whitespace only (CSS Syntax 3 §4.2), like every other value-boundary trim: a
    // Unicode `str::trim` would cut a non-ASCII space (an NBSP) out of the name's span, and
    // the printer emits the name from that span, so the character would be dropped. Kept in
    // `name_part`, it fails the validation below and the value stays an opaque identifier.
    let name_region = trim_start_css(&s[..paren_pos]);
    let mut name_part = trim_end_preserving_escape(name_region);

    // ⚠️ A hex escape's optional whitespace **terminator** belongs to the escape (§4.3.7),
    // so a name ending in a live one owns the character the trim just took — the space in
    // `c\41 (1px)` is inside the ident sequence, which therefore ends flush with the `(` and
    // makes the whole run one `<function-token>`. Give it back, the same restore the
    // property name against its colon already makes (`\41 : red`). The printer emits the
    // name from this span, so without it the author's spelling is rewritten to `c\41(1px)`.
    // Not folded into `trim_end_preserving_escape`, which restores an *open* escape's
    // payload (a trailing `\`): at every other site it serves, what follows the trimmed text
    // is the value's own end, where prettier drops the terminator and tsv matches it.
    //
    // Only one character, and only the escape's own: any further whitespace is a real gap,
    // which would have split the value in two before this scan ever saw it.
    if name_part.len() < name_region.len()
        && matches!(name_region.as_bytes()[name_part.len()], b' ' | b'\t')
        && ends_with_live_hex_escape(name_part)
    {
        name_part = &name_region[..=name_part.len()];
    }

    // An **empty** name is the parenthesized group (`(1.5)`, `(100vw - 1.5px)`) — CSS Syntax 3
    // §"component value" calls it a `()`-block, a *simple block* whose associated token is the
    // `(`, and gives it the same payload a function has ("a value consisting of a list of
    // component values"): only the name/token tells the two apart. So its members parse and
    // reach the ordinary number / colour / string / dimension printers, which is also
    // prettier's model — `parseValue` gives a bare `(…)` a `function` node whose `value` is
    // `''`. Read as an opaque identifier instead, the group switched every value-level rule
    // off for its whole interior. The name validation is asked only when there is a name to
    // ask about, so nothing else widens.
    //
    // ⚠️ An empty name can only ever BE the group's own opener, which is what keeps the
    // escape-blind search for `paren_pos` above sound in this arm too: `s` reaches here
    // trimmed, so `s[..paren_pos]` trims to nothing only when `paren_pos` is 0. A `(` found
    // inside an escape or a string always has bytes before it, so it always yields a
    // *non-empty* name — and one the validation rejects, per the ⚠️ on the search.
    //
    // The other two associated tokens are deliberately NOT read this way: `[a]` and `{a}` are
    // left opaque, which is where prettier's own value parser leaves them (`[1.50]` keeps its
    // number on both sides, and a `{}`-block is the whole-value form css-syntax-3 restricts to
    // custom properties — `css/values/variables/block_value_svelte_prettier_divergence`).
    if !name_part.is_empty() && !is_function_name(name_part) {
        return None;
    }

    // Where the run closes is the value scanners' shared question, not this one's: a paren
    // inside a quoted string or inside an escape closes nothing, and a walk that counted
    // them would balance the function somewhere other than its real end. See
    // [`scan::matching_close_paren`], which also carries the hop's rung note, the refused
    // pre-test, and why a block comment is deliberately NOT stepped over.
    let close_pos = scan::matching_close_paren(s, paren_pos)?;

    // Closing paren must be at end of string
    if close_pos != s.len() - 1 {
        return None;
    }

    let args = &s[paren_pos + 1..close_pos];
    Some((name_part, args))
}

/// Whether `name` is a function NAME — the **word** postcss-values-parser reads ahead of a
/// `(`, which is this layer's oracle rather than CSS Syntax 3's.
///
/// ⚠️ Deliberately **not** the spec's ident sequence. A `<function-token>`'s name is one
/// (§"Consume an ident-like token") and an ident code point is only a letter, a digit, `-`,
/// `_` or the non-ASCII set (§"ident code point"), so `aa/`, `50%` and `aa!` are no names to
/// the spec at all — they are to postcss-values-parser, the parser behind prettier's CSS
/// printer, whose `<function-token>` is a `word` token glued to a `(`. The value layer
/// already reads that oracle rather than the spec's tokenizer: the operator run (`12px/1.5`)
/// and the nameless `()`-block are the same transcription. Reading the spec's production here
/// instead dropped every such value onto the verbatim [`CssValue::Identifier`] path, which
/// switches **every** value normalization off for the whole declaration (`aa/(2.50)` kept its
/// `2.50` where `aaf(2.50)` normalized). [`is_value_word_code_point`] holds the class.
///
/// An escape is word content to both readings (§"Consume an ident sequence"), so `\66 n`,
/// `v\61 r` and `a\28 b` are the names `fn`, `var` and `a(b` and each is a real function —
/// the printer then reads the name from its span, so the author's spelling survives. A `\`
/// that starts no valid escape (a dangling one at the name's end, or one before a newline) is
/// not word content and refuses, which is what keeps [`extract_function_parts`]'s
/// escape-blind search for the opening `(` sound — see the ⚠️ there.
///
/// ⚠️ A word whose text **is a number** is a name here and a `value-number` to the oracle,
/// which prints it spaced from the group beside it — a cataloged divergence
/// (`css/values/functions/number_shaped_name_prettier_divergence`), not a miss.
fn is_function_name(name: &str) -> bool {
    // The escape-free name, which is every name a stylesheet really holds: `\` is not word
    // content, so this pass already stops on the first one and the walk below is entered
    // only for a name that has one (or is genuinely not a name at all).
    if name.chars().all(is_value_word_code_point) {
        return true;
    }
    escaped_name_is_value_word(name)
}

/// A code point that is **word content** to the value oracle. Named once so the fast pass
/// above and the escape walk below cannot drift — a name accepted by one and refused by the
/// other would be a function whose recognition depended on whether it carried an escape.
///
/// Stated as what it **excludes**, because that is the finite half and the only half the
/// oracle fixes. Everything else is word content — whether it continues
/// postcss-values-parser's own word (`wordEndRe`'s complement: `#`, `$`, `%`, `.`, `/`, `<`,
/// `=`, `?`, `^`) or becomes a word token of its own that `splitWord` glues back on (`!`,
/// `&`, `>`, `|`, `~`, `` ` ``). The two are indistinguishable here, because tsv takes the
/// whole region as one name and emits it from its span, and prettier prints the glued run
/// back the same way. `[` and `]` are word content for the same reason plus one more: a
/// `[…]` block's interior is word TEXT to the oracle, not a descended-into group
/// (`a[1.50]c(2.50)` → `a[1.50]c(2.5)` — the bracketed number is kept and only the argument
/// normalizes), so the whole region really is the name. Three exclusions, each measured:
///
/// - **the oracle reads across it** — whitespace (`<whitespace-token>`), `,` (comma), `:`
///   (colon), `@` (atword), `*` and `+` (operator), `'` and `"` (string), `{` and `}` (their
///   own token kinds). The first eight put a space or a separate member in prettier's output
///   (`aa @(1.5)`, `aa * (1.5)`, `'x' fn(1.5)`), so a name holding one would print glued
///   against it. The braces are the subtler pair: prettier glues them, but it tokenizes the
///   word *between* them on its own and canonicalizes it, so taking the region whole would
///   freeze content the oracle rewrites (`a{1.50}c(2.50)` → `a{1.5}c(2.5)`, reachable on a
///   custom property). That is a run-split question, not a name-grammar one.
/// - **it cannot occur in the region** — `;`, which terminates the declaration on both sides
///   before any value is read, and `(`, which *is* `paren_pos` by construction.
/// - **soundness** — `\` and `)`. A dangling `\` is what makes
///   [`extract_function_parts`]'s escape-blind search for the opening `(` sound (see the ⚠️
///   there), and prettier unbalances on a stray `)` and emits the declaration verbatim
///   (`a)(2.50)`), so tsv refuses it too. ⚠️ The bracket pair opens no third hazard:
///   `matching_close_paren` is quote- and escape-aware but not bracket-aware, yet a `)`
///   inside a `[…]` keeps the refusal above and an *unbalanced* `(` inside one is rejected by
///   both parsers at the declaration level (`a[(]c(2.50)`), so the reachable names are
///   bracket-closed and paren-free.
///
/// ⚠️ The non-ASCII half is the **lexer's** (`is_non_ascii_identifier_codepoint`, every code
/// point at or above U+00A0) rather than the exclusion above, which is wider than anything
/// measured up there. This position re-reads a token the lexer already read, so a narrower
/// class refuses a name the lexer accepted: `a°` is one identifier to the lexer, and reading
/// it as a non-name dropped the whole value onto the verbatim `Identifier` path, silently
/// switching off every value normalization for that declaration (`a°(1.50)` kept its `1.50`
/// where `aé(1.50)` normalized).
fn is_value_word_code_point(c: char) -> bool {
    if c.is_ascii() {
        return !matches!(
            c,
            // the oracle reads across it
            ' ' | '\t' | '\n' | '\r' | '\u{b}' | '\u{c}'
                | ','
                | ':'
                | '@'
                | '*'
                | '+'
                | '\''
                | '"'
                | '{'
                | '}'
                // cannot occur in the region
                | ';'
                | '('
                // soundness
                | '\\'
                | ')'
        );
    }
    crate::lexer::is_non_ascii_identifier_codepoint(c)
}

/// The escaped tail of [`is_function_name`] — outlined and cold, so the escape walk stays off
/// every function value's path, the same split the printer's own escape-spelled-name question
/// takes (`printer::values::function_name_is`).
#[cold]
#[inline(never)]
fn escaped_name_is_value_word(name: &str) -> bool {
    let bytes = name.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            // Steps the escape whole, terminator included, so `a\29 b` is one word and not
            // a word, a space and another word. The single definition every value scanner
            // shares (`crate::escapes::escape_len`).
            let Some(len) = crate::escapes::escape_len(name, i) else {
                return false;
            };
            i += len;
            continue;
        }
        // `i` is a char boundary: the loop only ever advances by a whole escape or a whole
        // char, and both lengths are measured in code points.
        let Some(ch) = name[i..].chars().next() else {
            return false;
        };
        if !is_value_word_code_point(ch) {
            return false;
        }
        i += ch.len_utf8();
    }
    true
}

#[cfg(test)]
mod function_name_tests {
    use super::{extract_function_parts, is_function_name};

    /// An escape is word content (CSS Syntax 3 §"Consume an ident sequence" — the one half
    /// the oracle and the spec agree on), whatever it spells.
    #[test]
    fn an_escape_is_word_content() {
        // Escape-free, then the hex spellings, then the literal ones — including the four
        // whose payload byte is what prettier's own tokenizer reads as structure.
        for name in [
            "fn", "a-b_1", "fé", r"\66 n", r"v\61 r", r"a\28 b", r"a\29 b", r"\31 fn", r"a\)b",
            r#"a\"b"#, r"a\'b", r"a\\b", r"a\ b", r"\-fn",
        ] {
            assert!(is_function_name(name), "{name:?} is a word");
        }
    }

    /// The name is postcss-values-parser's **word**, not the spec's ident sequence, and this
    /// is the whole character class in one table: every printable ASCII code point, graded
    /// against the exclusion set `is_value_word_code_point` declares. Exhaustive on purpose —
    /// a hand-written pair of "these are in, these are out" lists leaves whatever neither
    /// mentions ungraded, which is how the first cut of this rule shipped with `[` refused for
    /// a reason that turned out to be false.
    ///
    /// `\` is the one code point this sweep cannot grade, because mid-name it is not a bare
    /// character at all: `aa\bb` holds the valid escape `\b`, which IS word content. Its
    /// exclusion is about the **dangling** spelling and
    /// [`a_dangling_backslash_is_not_word_content`] owns it.
    #[test]
    fn the_word_class_is_exactly_this_exclusion_set() {
        /// Every printable-ASCII code point `is_value_word_code_point` refuses, with the
        /// group it belongs to. Grouped exactly as the predicate's doc groups them, so a
        /// change to one has to move the other.
        const EXCLUDED: &[char] = &[
            // the oracle reads across it
            ' ', ',', ':', '@', '*', '+', '\'', '"', '{', '}',
            // cannot occur in the region
            ';', '(',
            // soundness (`\` is graded by the dangling-backslash test, not here)
            ')',
        ];
        for c in (0x20u8..=0x7e).map(char::from) {
            if c == '\\' {
                continue;
            }
            let want = !EXCLUDED.contains(&c);
            // Three positions, so a positional rule cannot creep into what is a pure
            // character class.
            for name in [format!("aa{c}bb"), format!("{c}aa"), format!("aa{c}")] {
                assert_eq!(
                    is_function_name(&name),
                    want,
                    "{name:?}: expected word content = {want}"
                );
            }
        }
        // The ASCII whitespace the sweep's printable range does not reach.
        for c in ['\t', '\n', '\r', '\u{b}', '\u{c}'] {
            assert!(
                !is_function_name(&format!("aa{c}bb")),
                "{c:?} is whitespace"
            );
        }
        // Non-ASCII keeps the lexer's threshold rather than the exclusion set, so that a name
        // the lexer read as one identifier is not refused here (`a°(1.50)`).
        assert!(is_function_name("a\u{b0}"), "U+00B0 is at or above U+00A0");
        assert!(!is_function_name("a\u{85}"), "U+0085 is below it");
    }

    /// The *reasons* behind the exclusion set above, one cell each — the table says which
    /// code points, these say why, on the whole-name shapes the oracle was measured at.
    #[test]
    fn the_exclusion_set_reasons() {
        // Read across: the oracle tokenizes a string, a comma, a colon, an atword or an
        // operator on its own and prints it as a separate member (`'x' fn(1.5)`, `aa @(1.5)`).
        for name in ["'x'fn", "\"x\"fn", "a b", "a,b", "a:b", "a@b", "a*b", "a+b"] {
            assert!(!is_function_name(name), "{name:?} is printed separately");
        }
        // The braces are glued but tokenized apart, and the oracle canonicalizes the word
        // between them (`a{1.50}c(2.50)` → `a{1.5}c(2.5)`), so a region holding one would
        // freeze content prettier rewrites. A `;` cannot reach here at all.
        for name in ["a;b", "a{b", "a}b", "a{1.50}c"] {
            assert!(!is_function_name(name), "{name:?} is read across");
        }
        // Soundness: prettier unbalances on a stray `)` and emits the value verbatim.
        assert!(!is_function_name("a)b"), "a stray close paren");
        // A `[…]` block's interior IS word text to the oracle, kept verbatim while only the
        // argument normalizes (`a[1.50]c(2.50)` → `a[1.50]c(2.5)`), so the whole region is
        // the name — the pair's asymmetry with the braces above is the oracle's.
        for name in ["a[b]c", "a[1.50]c", "a[b]"] {
            assert!(is_function_name(name), "{name:?} is a word");
        }
    }

    /// The whole seam: which values read as a function, and where the name and arguments
    /// are cut. The sole caller enters on the value's first `(` byte, so the tests do too.
    #[test]
    fn the_first_paren_byte_locates_the_function_or_refuses_it() {
        fn parts(s: &str) -> Option<(&str, &str)> {
            let open = s.bytes().position(|b| b == b'(')?;
            extract_function_parts(s, open)
        }
        // An escape-spelled name is a function like any other.
        assert_eq!(parts(r"\66 n(.10)"), Some((r"\66 n", ".10")));
        assert_eq!(parts(r"a\29 b(.10)"), Some((r"a\29 b", ".10")));
        assert_eq!(parts(r"a\)b(.10)"), Some((r"a\)b", ".10")));
        // ⚠️ An escaped `(` is the one payload the blind search stops on. The name then
        // ends in a dangling `\` and refuses, so the value stays a verbatim identifier —
        // which is also what prettier emits, by unbalancing.
        assert_eq!(parts(r"a\(b(.10)"), None);
        assert_eq!(parts(r"a\(b(c))"), None);
        // A quote is not ident content either, so a glued string refuses the same way.
        assert_eq!(parts("'x'fn(.10)"), None);
        // The close paren must be the value's last byte (the printer bounds the argument
        // list at `span.end - 1` on the strength of it).
        assert_eq!(parts("fn(a)b"), None);
        assert_eq!(parts("fn(a"), None);
    }
}

#[cfg(test)]
mod value_span_tests {
    use super::parse_value_from_source;
    use bumpalo::Bump;
    use tsv_lang::Span;

    /// The span `parse_value_from_source` gives the value it parsed.
    fn value_span(source: &str, start: u32, end: u32) -> Span {
        let arena = Bump::new();
        parse_value_from_source(source, Span { start, end }, 0, None, &arena).span()
    }

    /// The already-trimmed fast path must agree with the trimming path on where
    /// the value starts and ends. No corpus can grade this: real declaration
    /// spans arrive pre-trimmed (200K+ of them without one whitespace byte at
    /// either end), so the trimming path is only ever reached by inputs a
    /// stylesheet does not contain — and a span error inside it would sail
    /// through every fixture and corpus diff in the repo.
    #[test]
    fn trims_the_span_the_same_either_way() {
        // Pre-trimmed (the fast path) — the span comes back untouched.
        assert_eq!(value_span("red", 0, 3), Span { start: 0, end: 3 });
        assert_eq!(value_span("a red b", 2, 5), Span { start: 2, end: 5 });

        // CSS whitespace at either end must come off the span identically.
        for (source, span, want) in [
            (" red", (0, 4), (1, 4)),
            ("red ", (0, 4), (0, 3)),
            ("  red  ", (0, 7), (2, 5)),
            ("\tred\t", (0, 5), (1, 4)),
            ("\nred\n", (0, 5), (1, 4)),
            ("\rred\r", (0, 5), (1, 4)),
            ("\x0cred\x0c", (0, 5), (1, 4)),
        ] {
            assert_eq!(
                value_span(source, span.0, span.1),
                Span {
                    start: want.0,
                    end: want.1
                },
                "value span for {source:?}"
            );
        }
    }

    /// Only [`is_css_whitespace`](crate::whitespace::is_css_whitespace)'s five ASCII characters are whitespace to CSS.
    /// Everything else at a boundary is value **content** and keeps its span — which
    /// is also what lets the fast path settle a non-ASCII byte outright.
    ///
    /// The vertical tab is the sharp edge: `char::is_whitespace` accepts it (so
    /// `str::trim` would eat it) and so does the *lexer*'s `is_ascii_css_whitespace`
    /// (which matches `parseCss`), but this trim is the CSS class and must not.
    #[test]
    fn non_css_whitespace_is_content() {
        for (source, span) in [
            ("\u{a0}red\u{a0}", (0, 7)),     // NBSP
            ("\u{3000}red\u{3000}", (0, 9)), // ideographic space
            ("\x0bred\x0b", (0, 5)),         // vertical tab
            ("é", (0, 2)),
        ] {
            assert_eq!(
                value_span(source, span.0, span.1),
                Span {
                    start: span.0,
                    end: span.1
                },
                "value span for {source:?}"
            );
        }
    }

    /// An ASCII space adjacent to a kept non-ASCII space still comes off — only the
    /// ASCII byte is trimmed, the NBSP stays as content.
    #[test]
    fn ascii_space_trims_beside_a_kept_nbsp() {
        assert_eq!(value_span(" \u{a0}red", 0, 6), Span { start: 1, end: 6 });
    }

    /// The trailing whitespace a `\` escape owns is content: trimming it strands
    /// the backslash onto whatever follows (`50px\ ;` → `50px\;`, the `;` now
    /// escaped) and the declaration no longer parses. Padding past the escaped
    /// character is ordinary whitespace and still goes.
    #[test]
    fn an_escapes_payload_stays_in_the_span() {
        assert_eq!(value_span(r"50px\ ", 0, 6), Span { start: 0, end: 6 });
        assert_eq!(value_span(r"50px\   ", 0, 8), Span { start: 0, end: 6 });
        // An even backslash run is a completed `\\` — the space is just padding.
        assert_eq!(value_span(r"50px\\ ", 0, 7), Span { start: 0, end: 6 });
    }
}

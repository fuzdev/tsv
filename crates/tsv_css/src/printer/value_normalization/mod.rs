// Value normalization utilities: semantic value formatting for the CSS printer.
//
// Internal AST stores semantic data + spans. When formatting we usually format
// semantically (normalize spacing, apply prettier rules); raw-source extraction
// uses `span.extract(source)` directly at the callsite when needed.

mod colors;
mod numbers;
mod splitting;

pub(crate) use colors::format_color_from_source;
pub(crate) use numbers::normalize_dimension_from_source;
pub(crate) use splitting::{
    collapse_whitespace_runs, has_closing_comma, normalize_css_whitespace, normalize_is_noop_in,
    split_args_by_comma, split_by_space_preserving_parens,
};

use std::borrow::Cow;

use super::boundary_ws::boundary_run_spelling;
use crate::color::is_hex_color_body;
use crate::escapes::escape_span_at;
use numbers::{canonical_unit, is_known_css_unit, normalize_css_number};
use tsv_lang::printing::format_string_literal;

/// Which of prettier's two prelude readers an at-rule's prelude goes through — the axis
/// every rule in [`normalize_value_text`] is keyed on.
///
/// `parser-postcss.js` routes a prelude by at-rule name: `@media` (and `@custom-media`) to
/// `parseMediaQuery`, and `@supports` — plus every `isModuleRuleName` at-rule, of which
/// `@import` is the one tsv formats — to `parseValue`. tsv's own routing
/// (`parser/atrules/mod.rs`) tests the same pair, so a `@custom-media` prelude reaches this
/// function on the media path like a `@media` one. The two readers tokenize the same
/// text differently and print through different arms, so one authoring can have two
/// canonical forms, and a rule keyed on the wrong one is wrong on three counts at once:
/// the unit gate, the hex fold, and which runs absorb a following number.
///
/// `@container` is on neither list — prettier keeps its params raw — so it never reaches
/// this function and its prelude stays verbatim.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum PreludeReader {
    /// `@supports` and `@import`: `parseValue` (postcss-values-parser) → `value-*` nodes,
    /// printed as `printCssNumber(value) + printUnit(unit)` / `value-word`.
    Value,
    /// `@media` and `@custom-media`: `parseMediaQuery` → a node split (`media-type`, `media-feature`,
    /// `media-colon`, `media-value`, `media-keyword`, `media-url`, `media-unknown`), of which
    /// `media-type` and `media-value` print through `adjustNumbers` (`print/misc.js`), a regex
    /// pass over the raw text.
    ///
    /// ⚠️ **`media-feature` — name position — takes no number pass at all**, only
    /// `maybeToLowerCase(adjustStrings(value.replaceAll(/ +/g, " ")))`. Name position is
    /// everything before the `:`, and the **whole** interior when there is none, so the
    /// boolean `(0.50)` and range `(WIDTH >= 1.50)` / `(1.50 < width < 2.50)` forms are all
    /// name. tsv runs this reader over the whole prelude instead, so it normalizes numbers
    /// prettier keeps there. The boundary is already drawn one pass later by
    /// [`feature_name_end`], which `lowercase_media_feature_names` asks.
    MediaQuery,
}

/// Normalize CSS numbers and string quotes within a raw at-rule prelude string, mirroring
/// whichever of prettier's two prelude readers `path` names. `/* */` comments are copied
/// verbatim, and quoted strings get prettier's quote normalization (prefer single, swapping
/// only to minimize escaping), which is `adjustStrings` on both paths.
///
/// **Numbers.** A number that is its own token normalizes to canonical form (`.50` → `0.5`)
/// on both paths; what follows it is where they part. [`PreludeReader::Value`] prints a
/// `value-number` node as `printCssNumber(value) + printUnit(unit)` with **no gate on the
/// unit** — whatever trails the number is the unit and rides through unchanged, so `1.50abc`
/// is `1.5abc`. [`PreludeReader::MediaQuery`] runs `adjustNumbers`' `(WORD_PART)?(NUMBER)(UNIT)?`
/// regex, which returns the **whole match verbatim** unless the unit it captured is empty,
/// the `<an+b>` `n`, or a unit CSS defines — so `1.50abc` stays. Unit *casing* is
/// [`canonical_unit`]'s question on the value path. On the media path it is not:
/// `adjustNumbers` lowercases the captured unit **before** its gate and prints the lowercased
/// one, so the `<an+b>` `n` folds there whatever [`canonical_unit`] says (`2N` → `2n`, where
/// `@supports` keeps `2N`). ⚠️ tsv asks [`canonical_unit`] on both, so the media `n` is a
/// divergence — see the unit-extent TODO on the number arm.
///
/// **A number abutting the run before it** is that run's own tail, not a token of its own:
/// the canonical form of a `.`-leading number carries a leading `0`, and appending it to an
/// ident code point hands the `0` to the run (`x1` + `.50` → `x10` + `.5`, two different
/// tokens). Such a pair is copied verbatim, unit included. *Which* runs count is the second
/// place the paths part. On the value path a word is one token through to its end, so an
/// ident, a `#`-token and a number's own unit all absorb. ⚠️ "Through to its end" is not a
/// character class, and this module's is too narrow twice over: postcss-values-parser's
/// `splitWord` **glues consecutive `word` tokens**, so punctuation that ends one word and
/// starts the next still leaves them one node (`x!1.50`, `x;1.50`, `x|1.50` all absorb); and
/// the tokenizer's word-end set depends on the word's **first** character (`wordEndRe` for a
/// non-digit start, `numEndRe` — which adds `-` and a bare `/` — for a digit start), so
/// `x-1.50` is one word where `9-1.50` is two, and `x/1.50` is one where `9/1.50` is two.
/// No character class can hold that second axis. On the media path only a
/// `WORD_PART` does — `[$@]?[_a-z\u{80}-\u{FFFF}][\w\u{80}-\u{FFFF}-]*`, so a run whose tail
/// after a `$`/`@` is digits is no word part and the regex reads those digits as the head of
/// the number (`$1.50` → `$1.5`), while a number's unit and a `#` absorb nothing at all
/// (`1a` + `.50` → `1a0.5`, `#1.50` → `#1.5`).
///
/// **`#`-prefixed tokens.** A `#` followed by an ident code point or a valid escape is a
/// `<hash-token>` whose value is the ident sequence after it (CSS Syntax 3 §4.3.1); anything
/// else — `#.50` — leaves a bare `<delim-token>`. On [`PreludeReader::Value`] a hex **color**
/// (`#` + exactly 3, 4, 6, or 8 ASCII hex digits) is lowercased (`#FFF` → `#fff`), matching
/// prettier's `value-word` arm; any other hash token — an off-length run (`#ABCDE`), a
/// non-hex token, or one inside a `selector(...)` group (a case-sensitive ID selector) — is
/// copied verbatim. On [`PreludeReader::MediaQuery`] the `#` is not a token head at all:
/// `adjustNumbers` is a regex over raw text with no hash arm, so the `#` is copied as the
/// delimiter it is and the run after it takes the ordinary ident/number arms.
///
/// An unquoted `url(...)` is a `<url-token>` — opaque per CSS Syntax 3 §4.3.6, so its whole
/// content is copied verbatim (a path like `url(sprite1.50.png)` is never number/unit-normalized).
/// A quoted `url("…")` is a function with a `<string>` arg and takes normal quote normalization.
///
/// ⚠️ That opacity is a **value-reader** rule — `parse-value.js` special-cases the `url` func,
/// while `adjustNumbers` is a regex with no url concept and normalizes straight through one
/// (`@media (a: url(1.50))` is `url(1.5)` to prettier). This arm is unconditional on both
/// paths, so on the media path it is a divergence rather than parity — the mirror of the `#`
/// arm below, which *is* restricted to [`PreludeReader::Value`].
pub(crate) fn normalize_value_text(input: &str, path: PreludeReader) -> String {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    // `url(...)`/`selector(...)` context tracking, consulted only for `#`-tokens:
    // `preserve_from` is `Some(depth)` while inside the outermost such group, so a
    // `#`-token there is preserved rather than lowercased as a hex color.
    let mut paren_depth = 0usize;
    let mut preserve_from: Option<usize> = None;
    // Whether the identifier just emitted was `url`/`selector`, so a `(` that
    // immediately follows opens a preserve group (a CSS function token requires the
    // `(` to abut the name — any char between, incl. whitespace, breaks it).
    let mut prev_ident_preserve_fn = false;
    // Whether the run just emitted ends in a character that would absorb a digit appended
    // to it, so a number abutting it here is that run's own tail rather than a value of
    // its own — see the number arm.
    let mut prev_run_absorbs_digit = false;

    while i < bytes.len() {
        let b = bytes[i];
        let after_preserve_ident = prev_ident_preserve_fn;
        prev_ident_preserve_fn = false;
        let after_absorbing_run = prev_run_absorbs_digit;
        prev_run_absorbs_digit = false;

        // An escape is opaque — its payload is content, never structure (§4.3.7, and
        // `crate::escapes::escape_len` for the extent, terminator included). Copied whole
        // and ahead of every arm below, because each of them would otherwise read the
        // payload byte as the thing it is looking for:
        //
        // - a `\"` / `\'` opened the string arm, which then ran to the *next* real quote
        //   and re-quoted everything between (`(a: x\"y "z")` → `(a: x\'y 'z")`) — the
        //   escape's payload rewritten and the string delimiters moved, output tsv's own
        //   parser then rejects (`Unterminated string`);
        // - a `\#` opened the `#`-token arm, so an escaped hash *inside an ident* was
        //   lowercased as a hex colour (`x\#FFF` → `x\#fff`, a different ident);
        // - a `\/` opened the comment arm, and a hex escape's digits were read as a
        //   number.
        //
        // ⚠️ It does NOT widen the identifier run below, which deliberately stops at an
        // escape. Prettier's prelude passes read an escape's payload as ordinary text too,
        // so stopping here is what keeps the two agreeing: `\41 2.50px` is emitted as
        // `\41 2.5px` by both, where reading the ident sequence's real extent (`A2`, then
        // `.50px`) would hand the number arm a word part to glue against and produce
        // `\41 20.5px` — the ident `A20`. The escape-spelled function name that costs is
        // the `url` arm below, where prettier is blind in the same direction.
        if let Some(end) = escape_span_at(input, i) {
            out.push_str(&input[i..end]);
            i = end;
            continue;
        }

        // Normalize quoted-string quotes (handling backslash escapes). A properly
        // closed string runs through prettier's quote chooser; an unterminated run
        // (malformed input) is copied verbatim.
        if b == b'"' || b == b'\'' {
            let start = i;
            // The extent is `crate::lexer::string_end`'s, not a loop of this scanner's own
            // — see `skip_string` below, which asks the same question for `trivia_span_at`.
            // Either failure arm means the run never closed, and a malformed run is copied
            // verbatim rather than re-quoted.
            let closed = crate::lexer::string_end(bytes, start).ok();
            i = closed.unwrap_or(input.len());
            let literal = &input[start..i];
            if closed.is_some() {
                let content = &literal[1..literal.len() - 1];
                out.push_str(&format_string_literal(content, b as char));
            } else {
                out.push_str(literal);
            }
            continue;
        }

        // Copy block comments verbatim, taking the extent from `crate::comments` — the
        // one definition, shared with the media-feature scanner below and the value
        // scanners.
        if crate::comments::is_comment_start(bytes, i) {
            let start = i;
            i = crate::comments::comment_end(input.as_bytes(), i);
            out.push_str(&input[start..i]);
            continue;
        }

        let Some(ch) = input[i..].chars().next() else {
            break;
        };

        // Copy identifiers verbatim — including any digits they contain, so a
        // number attached to a word (`foo2`, `min-width`) is never normalized.
        if is_ident_start(ch) {
            let start = i;
            i += ch.len_utf8();
            while let Some(c) = input[i..].chars().next() {
                if is_ident_continue(c) {
                    i += c.len_utf8();
                } else {
                    break;
                }
            }
            // How much of the run this path emits, and whether a number abutting it merges
            // into it — the two readers' answers stated side by side in `ident_run_split`.
            // The media path can end the run short of what the loop above read.
            let (emit_len, absorbs) = ident_run_split(&input[start..i], path);
            i = start + emit_len;
            let ident = &input[start..i];
            out.push_str(ident);

            // An unquoted `url(...)` is a `<url-token>` — opaque per CSS Syntax 3
            // §4.3.6, so its content (a path, maybe with number/unit-looking runs like
            // `sprite1.50.png`) is copied verbatim, never number/unit-normalized. A
            // quoted `url("…")` is instead a function with a `<string>` arg; it flows
            // through the normal string branch (quote normalization), so leave it.
            //
            // ⚠️ The name is matched on the run's own bytes, so an **escape-spelled** one
            // (`u\72 l(`, the same `<url-token>`) is missed and its content normalizes.
            // That is **parity, not a gap**: prettier's prelude passes match the name
            // literally too, so `u\72 l(1.50)` is `1.5` and `u\72 l(#FFF)` is `#fff` on
            // both sides. Teaching this arm the ident sequence's real extent would make
            // tsv the only one preserving them. The parser's own recognition, which does
            // decode the escape (`printer::values::function_name_is`), answers a different
            // question — an `@import` prelude the lexer tokenized, not this raw text.
            if ident.eq_ignore_ascii_case("url")
                && bytes.get(i) == Some(&b'(')
                && !url_arg_is_quoted(input, i)
            {
                out.push_str(consume_paren_group(input, &mut i));
                continue;
            }

            // A `selector(...)` that immediately follows opens a hex-preserve group:
            // an ID selector's `#` is case-sensitive (numbers inside stay on the normal
            // path — the documented `selector()` over-normalization).
            prev_ident_preserve_fn = ident.eq_ignore_ascii_case("selector");
            prev_run_absorbs_digit = absorbs;
            continue;
        }

        // A `#`-prefixed token, on the value path only. `#` + an ident sequence is a
        // `<hash-token>` (CSS Syntax 3 §4.3.1), and postcss-values-parser reads that whole
        // word as one `value-word`: a hex color (`#` + 3/4/6/8 hex digits) lowercases,
        // matching prettier, and everything else — an off-length run (`#ABCDE`), a non-hex
        // token, an exponent-looking one (`#1e2`), or a `#` inside a `selector(...)` group
        // (a case-sensitive ID) — is copied verbatim. An unquoted `url(...)` never reaches
        // here (opaque, consumed above).
        //
        // ⚠️ The media path has **no hash arm at all** — `adjustNumbers` is a regex over raw
        // text and knows nothing of hash tokens, so the `#` falls through to the delimiter
        // arm below and the run after it takes the ordinary ident/number arms
        // (`#1.50` → `#1.5`, `#abc1.50` verbatim behind its word part). Giving the media
        // path this arm is what produced `#10.5`: the hash token ended at `#1`, the `.50`
        // then normalized on its own, and its canonical leading `0` joined the token before
        // it — a hash whose value changed from `1` to `10`, matching neither oracle.
        if b == b'#' && path == PreludeReader::Value {
            let start = i;
            i += 1;
            let body_start = i;
            // Two phases, because the two questions have different answers. **Whether** a
            // token starts here is CSS Syntax 3 §4.3.1's: `#` heads a `<hash-token>` only
            // when an ident code point follows, and otherwise is a bare `<delim-token>`
            // whose neighbour is a token of its own (`#.50` is `#` then `.50` → `#0.5`).
            // **How far** it then reaches is postcss-values-parser's, which is wider — see
            // `is_hash_word_char`.
            if let Some(c) = input[i..].chars().next()
                && is_css_ident_code_point(c)
            {
                i += c.len_utf8();
                while let Some(c) = input[i..].chars().next() {
                    if is_hash_word_char(c) {
                        i += c.len_utf8();
                    } else {
                        break;
                    }
                }
            }
            let body = &input[body_start..i];
            // ⚠️ The fold is asked of the **whole** word, not of a hex-shaped prefix of it.
            // A narrower body reads `#FFF.5` as the colour `#FFF` followed by `.5` and folds
            // it to `#fff.5` — recasing a token that is no colour at all, which is exactly
            // what "any other hash token is copied verbatim" exists to prevent.
            if preserve_from.is_none() && is_hex_color_body(body) {
                out.push('#');
                for c in body.chars() {
                    out.push(c.to_ascii_lowercase());
                }
            } else {
                out.push_str(&input[start..i]);
            }
            // A hash token absorbs an abutting number the way an ident run does — the word
            // runs on through it (`#1` + `.50` stays `#1.50`). A `#` that heads no token
            // (`#.50`, a bare `<delim-token>`: the next code point is no ident code point)
            // absorbs nothing, so the number after it normalizes on this path too.
            prev_run_absorbs_digit = !body.is_empty();
            continue;
        }

        // A number (not attached to an identifier — those are consumed above).
        let num_len = crate::number::number_part_len(&input[i..]);
        if num_len > 0 {
            let num = &input[i..i + num_len];
            i += num_len;
            // Trailing unit. ⚠️ The two readers disagree and only one is implemented here:
            // `[a-z]+` is `adjustNumbers`' `STANDARD_UNIT_REGEX`, i.e. the **media** rule, where
            // `%` and operators do end the unit. The value reader's unit is
            // `word.replace(NUMBER_RE, '')` — the whole rest of the glued word — so `50%.5` is
            // the number `50` with unit `%.5` and `.50#FFF` the number `.50` with unit `#FFF`,
            // both of which prettier prints verbatim. `split_number_and_unit` (numbers.rs) is
            // the value rule, already correct, and serves the declaration-value path.
            // TODO: take the value rule from that sibling on `PreludeReader::Value`.
            let unit_start = i;
            while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
                i += 1;
            }
            let unit = &input[unit_start..i];
            // Inside a `selector(...)` group the digits are selector syntax — an
            // `<an+b>` term, an attribute value — not a `<number>` with a canonical
            // serialization, so they are copied verbatim like the `#`-token above.
            let normalizes = preserve_from.is_none()
                && match path {
                    // `[printCssNumber(node.value), printUnit(node.unit)]` — the value node
                    // carries whatever trailed the number as its unit and no gate reads it,
                    // so the number normalizes and the unit rides through (`1.50abc` →
                    // `1.5abc`). `printUnit` is `canonical_unit` below.
                    PreludeReader::Value => true,
                    // `adjustNumbers` returns the whole `(WORD_PART)?(NUMBER)(UNIT)?` match
                    // verbatim unless the captured unit is empty, the `<an+b>` `n`, or one
                    // CSS defines — so `1abc` is left alone.
                    PreludeReader::MediaQuery => {
                        unit.is_empty() || unit.eq_ignore_ascii_case("n") || is_known_css_unit(unit)
                    }
                };
            let normalized = normalizes
                .then(|| normalize_css_number(num))
                // A number abutting the run just emitted must not begin with a character
                // that run would absorb: the canonical form of a `.`-leading number carries
                // a leading `0`, so `x1` + `.50` came back as `x10` + `.5` — two different
                // tokens. The pair is copied through instead, which is what prettier's own
                // scan does (`print/misc.js`: a `(WORD_PART)?(NUMBER)(UNIT)?` match is
                // returned verbatim when the word part matched). A signed number is never at
                // risk, since the sign it keeps merges with nothing — `x+1.50` still
                // normalizes.
                .filter(|n| !(after_absorbing_run && n.starts_with(is_css_ident_code_point)));
            if let Some(normalized) = normalized {
                out.push_str(&normalized);
                // `canonical_unit` lowercases a known unit (`PX`→`px`) and leaves the
                // `n`/empty/unknown cases untouched (none is a known unit).
                out.push_str(&canonical_unit(unit));
            } else {
                out.push_str(num);
                out.push_str(unit);
            }
            // On the value path the number and its unit are one word, so a number abutting
            // *them* is that word's tail too (`1a` + `.50`, `1.5` + `.50`). Asked of what was
            // just emitted, which answers for either branch and lets the unit speak when
            // there is one. The media path's regex re-splits at the next number instead, so
            // a unit absorbs nothing there (`1a` + `.50` → `1a0.5`).
            prev_run_absorbs_digit =
                path == PreludeReader::Value && out.ends_with(is_css_ident_code_point);
            continue;
        }

        // Track `selector(...)` nesting for the `#`-token rule above (an unquoted
        // `url(...)` is consumed opaquely in the ident branch and never reaches here).
        // A `(` right after a `selector` ident opens the outermost hex-preserve group;
        // its matching `)` closes it.
        match ch {
            '(' => {
                paren_depth += 1;
                if after_preserve_ident && preserve_from.is_none() {
                    preserve_from = Some(paren_depth);
                }
            }
            ')' => {
                if preserve_from == Some(paren_depth) {
                    preserve_from = None;
                }
                paren_depth = paren_depth.saturating_sub(1);
            }
            _ => {}
        }
        out.push(ch);
        i += ch.len_utf8();
    }

    out
}

/// Whether the `url(` opening at `open` (the `(` byte index) is immediately
/// followed — after optional whitespace — by a quote, i.e. `url("…")`/`url('…')` (a
/// function with a `<string>` arg) rather than an unquoted `<url-token>`.
fn url_arg_is_quoted(s: &str, open: usize) -> bool {
    let bytes = s.as_bytes();
    let mut j = open + 1;
    while j < bytes.len() && bytes[j].is_ascii_whitespace() {
        j += 1;
    }
    matches!(bytes.get(j), Some(b'"') | Some(b'\''))
}

/// Consume the balanced parenthesized group starting at the `(` at `*i`, advancing
/// `*i` past the matching `)` (or to end of input if unbalanced), and return the
/// consumed slice — used to copy an opaque `url(<url-token>)` verbatim.
///
/// **Escapes step whole.** §4.3.6 "Consume a url token" consumes an escape as url content, so
/// the token ends at the first *unescaped* `)` — a `\)` closes nothing. Counting it does not
/// merely end the copy early: everything past it rejoins the normalizing path and the URL's
/// own bytes are rewritten (`url(x\)1.50)` → `url(x\)1.5)`, `url(x\)y#FFF)` → `url(x\)y#fff)`),
/// which is the §4.3.6 opacity this arm exists to enforce, inverted.
///
/// ⚠️ A block comment is deliberately **not** stepped over, for the reason the value
/// scanners' own `matching_close_paren` gives at length (`parser/value/scan.rs`): this is a
/// *bounding* scan over url content, where `/*` opens nothing (§4.3.6), so reading one as a
/// comment would lose the closing paren of `url(foo/*bar)`. Escapes are the opposite case —
/// §4.3.6 names them explicitly.
fn consume_paren_group<'s>(s: &'s str, i: &mut usize) -> &'s str {
    let bytes = s.as_bytes();
    let start = *i;
    let mut depth = 0usize;
    while *i < s.len() {
        let b = bytes[*i];
        if let Some(end) = escape_span_at(s, *i) {
            *i = end;
            continue;
        }
        *i += 1;
        if b == b'(' {
            depth += 1;
        } else if b == b')' {
            depth -= 1;
            if depth == 0 {
                break;
            }
        }
    }
    &s[start..*i]
}

/// Can `ch` begin a CSS identifier? (letter, `_`, `$`, `@`, or non-ASCII)
fn is_ident_start(ch: char) -> bool {
    ch.is_alphabetic() || ch == '_' || ch == '$' || ch == '@' || !ch.is_ascii()
}

/// Can `ch` continue a CSS identifier? (`is_ident_start` plus digits and `-`)
fn is_ident_continue(ch: char) -> bool {
    is_ident_start(ch) || ch.is_ascii_digit() || ch == '-'
}

/// Is `ch` an **ident code point** as CSS Syntax 3 defines one — a letter, a digit, `-`,
/// `_`, or a non-ASCII code point at the **lexer's** threshold?
///
/// The strict counterpart of [`is_ident_continue`], which additionally admits `$` and `@`
/// so a preprocessor-flavoured prelude reads as one run rather than shattering. Those two
/// are `<delim-token>`s to CSS proper, so they end a token where a real ident code point
/// would extend one — the difference that decides whether appending a digit merges.
///
/// The non-ASCII half is `lexer::is_non_ascii_identifier_codepoint`, the crate's single
/// source for that threshold, rather than a bare `!ch.is_ascii()`: this predicate answers
/// what the *lexer* would join, and the lexer stops at the C1 controls (U+0080–U+009F).
fn is_css_ident_code_point(ch: char) -> bool {
    ch.is_ascii_alphanumeric()
        || ch == '-'
        || ch == '_'
        || crate::lexer::is_non_ascii_identifier_codepoint(ch)
}

/// Can `ch` continue the word a `<hash-token>` heads on the value path — the run whose whole
/// text the hex-colour fold is asked of?
///
/// [`is_css_ident_code_point`] decides whether the token starts at all (§4.3.1) and would
/// also end it, but how far it *reaches* has a different oracle: postcss-values-parser reads
/// a `#` word further than the spec's ident sequence, and the gap is exactly where the fold
/// went wrong. `#FFF.5` is `#FFF` + `.5` to the spec, so a spec-width body found the colour
/// `#FFF` inside a token that is none and recased it to `#fff.5`; prettier keeps the word
/// whole and preserves it.
///
/// ⚠️ **This class is a sample, not the rule, and it is too narrow.** The real oracle is the
/// postcss-values-parser tokenizer plus `splitWord`'s gluing of consecutive `word` tokens: the
/// word ends only where a **non-`word` token** intervenes — whitespace, `,`, `:`, `{`, `}`,
/// an operator (`+` / `-` / `*` / `/`), an atword, a string, a comment — or where a `(` makes
/// the run a function rather than a word. Everything else glues, so `#FFF!5`, `#FFF;5`,
/// `#FFF|5`, `#FFF~5`, `#FFF>5`, `#FFF[5`, `#FFF]5`, `#FFF&5`, `#FFF^5`, `#FFF?5`, `#FFF=5`,
/// `#FFF\5` and `#FFF<5` are each one word to prettier and none of them a colour — where this
/// class ends the word and tsv folds. The two error directions are not symmetric — a class too
/// wide only declines a fold prettier makes, a class too narrow **recases a token** — and
/// every one of those misses is on the recasing side.
///
/// The characters the class does hold (`.`, `/`, non-ASCII, and every ident code point) are
/// right; what it lacks is the ~20 punctuation characters above and the tokenizer's
/// digit-initial axis. TODO: take the extent from the oracle's tokenizer rather than from
/// this class.
fn is_hash_word_char(ch: char) -> bool {
    is_css_ident_code_point(ch) || ch == '.' || ch == '/'
}

/// Can `ch` begin `adjustNumbers`' `WORD_PART` — `[$@]?` **`[_a-z\u{80}-\u{FFFF}]`**
/// `[\w\u{80}-\u{FFFF}-]*`, the media path's notion of "a word precedes this number"?
///
/// [`is_ident_start`] without `$`/`@`: the regex allows those only as the optional single
/// character *before* the start class, never as the start itself.
fn is_media_word_part_start(ch: char) -> bool {
    is_ident_start(ch) && ch != '$' && ch != '@'
}

/// Split the ident run `run` the way `adjustNumbers`' regex reads it, for a run that a
/// number abuts.
///
/// Returns `(emit_len, absorbs)` — how much of `run` is the run proper, the rest being where
/// prettier's `NUMBER` begins, and whether a `WORD_PART` reaches the run's end (which is what
/// makes the regex return the abutting number's match verbatim).
///
/// `$`/`@` are in this module's ident class and in neither of the regex's two classes, so
/// they can only ever be `WORD_PART`'s optional leading character. Everything after the
/// **last** of them is continuation-class by construction — this run holds nothing else — so
/// a word part reaches the run's end exactly when that tail holds one character the start
/// class accepts. When none does, the tail is digits and `-`, and the regex's `NUMBER`
/// begins at the first digit in it: `a$1` + `.50` is the run `a$` then the number `1.50`
/// (→ `a$1.5`), and `$` + `.50` is the run `$` then `.50` (→ `$0.5`).
fn media_word_part_split(run: &str) -> (usize, bool) {
    let tail_start = run
        .char_indices()
        .filter(|&(_, c)| c == '$' || c == '@')
        .map(|(i, c)| i + c.len_utf8())
        .next_back()
        .unwrap_or(0);
    let tail = &run[tail_start..];
    if tail.chars().any(is_media_word_part_start) {
        return (run.len(), true);
    }
    let emit_len = tail
        .char_indices()
        .find(|&(_, c)| c.is_ascii_digit())
        .map_or(run.len(), |(i, _)| tail_start + i);
    (emit_len, false)
}

/// How `path`'s reader takes the ident run `run`: how much of it is the run proper — the
/// rest being where prettier's own `NUMBER` begins — and whether a number abutting it merges
/// into it.
///
/// On the value path postcss-values-parser reads a word through to its end, so the whole run
/// is emitted and the only question is whether its last code point is a CSS **ident** code
/// point (§"ident code point": letter, digit, `-`, `_`, non-ASCII). `$` and `@`, which this
/// module's deliberately permissive ident class also admits so preprocessor-flavoured
/// preludes stay readable, are `<delim-token>`s that absorb nothing — `$` + `.50` may safely
/// become `$0.5`.
///
/// On the media path the question is `adjustNumbers`' `WORD_PART` instead, which can end the
/// run short of what this module read — see [`media_word_part_split`].
fn ident_run_split(run: &str, path: PreludeReader) -> (usize, bool) {
    match path {
        PreludeReader::Value => (run.len(), run.ends_with(is_css_ident_code_point)),
        PreludeReader::MediaQuery => media_word_part_split(run),
    }
}

/// How far does the ident **sequence** starting at `start` reach — [`is_ident_continue`]'s
/// class with escapes stepped whole?
///
/// CSS Syntax 3 §"Consume an ident sequence" appends an escape's code point to the name and
/// keeps going, so an escape both begins and continues a sequence: `a\:b` is the single
/// ident `a:b`, `--A\42 C` the single ident `--AB`. A scanner that stops at the `\` reads
/// one name as two fragments, and any rule keyed on the *name* — the `--` prefix that makes
/// a custom-media name case-sensitive, the name→value `:` — then reads the wrong one.
///
/// Returns `start` when nothing there can extend a sequence (a lone `\` that starts no
/// escape — trailing, or before a newline, §4.3.4), so a caller that advances by the result
/// must handle that case itself. Whether the first code point may *start* an ident is the
/// caller's question; this only measures the reach.
fn ident_sequence_end(s: &str, start: usize) -> usize {
    let bytes = s.as_bytes();
    let mut i = start;
    while i < s.len() {
        if bytes[i] == b'\\' {
            // A `\` that starts no escape extends nothing, so the run ends at it.
            let Some(end) = escape_span_at(s, i) else {
                break;
            };
            i = end;
            continue;
        }
        // `i` is a char boundary: the loop advances only by a whole escape or a whole char.
        let Some(ch) = s[i..].chars().next() else {
            break;
        };
        if !is_ident_continue(ch) {
            break;
        }
        i += ch.len_utf8();
    }
    i
}

/// Lowercase the **feature name** in an `@media`/`@import` media-query string,
/// matching prettier — which lowercases the `media-feature` name (`MIN-WIDTH` →
/// `min-width`) but preserves media types (`SCREEN`), the `and`/`or`/`not`/`only`
/// keywords, and feature *values* (`(orientation: LANDSCAPE)` keeps `LANDSCAPE`).
/// Run **after** [`normalize_value_text`] (numbers/units/strings already
/// canonicalized); this only adjusts identifier case.
///
/// Scope: only a **simple** parenthesized feature expression has its name lowercased —
/// a `(...)` group whose only nested `(`, if any, opens a **function call** in the
/// value (`(min-width: calc(…))`, `(width: min(…))`). A grouped/complex condition — a
/// nested `(` that opens a sub-condition (`(not (hover))`, `((a) and (b))`) — is left
/// verbatim, matching prettier's media-query parser, which treats those as
/// `media-unknown`. (A function-call `(` is told apart from a sub-condition `(` by
/// whether it immediately follows an identifier; see `scan_paren_group`.) (One small
/// divergence:
/// prettier's parser partially lowercases the *first* feature in `((A) and (B))`; tsv
/// preserves the whole grouped condition for consistency — see
/// `media_grouped_feature_case_prettier_divergence`.)
///
/// Within a simple expression the feature name is everything in **name position** — before
/// the `:` (plain feature), or the whole interior where there's no `:` at all (boolean
/// `(hover)` and range `(width >= 600px)` / `(600px <= width)`, whose values are numeric);
/// [`feature_name_end`] draws that line. The value after a `:` is preserved, and so is a
/// name that is case-*sensitive* rather than a keyword — a custom media `--*` above all
/// (see [`feature_name_preserves_case`]).
pub(crate) fn lowercase_media_feature_names(query: &str) -> Cow<'_, str> {
    let bytes = query.as_bytes();
    // Cheap bail: nothing to lowercase without an uppercase ASCII letter.
    if !bytes.iter().any(u8::is_ascii_uppercase) {
        return Cow::Borrowed(query);
    }

    let mut out = String::with_capacity(query.len());
    let mut i = 0;
    while i < query.len() {
        // Comments/strings copy through verbatim (never lowercase their contents).
        if let Some(end) = trivia_span_at(query, i) {
            out.push_str(&query[i..end]);
            i = end;
            continue;
        }
        // So does an escape, whose payload is ident content: a `\(` at depth 0 is part of a
        // media *type*, not the `(` that opens a feature expression, and reading it as one
        // handed the rest of the query to `scan_paren_group` as a single opaque group —
        // `@media a\(b and (MIN-WIDTH: 1px)` kept its uppercase feature name.
        if let Some(end) = escape_span_at(query, i) {
            out.push_str(&query[i..end]);
            i = end;
            continue;
        }
        match bytes[i] {
            b'(' => {
                // A top-level `(` opens a media-feature-expression. A group that
                // contains a nested *sub-condition* `(` is grouped/complex — copy it
                // verbatim (prettier's parser treats it as `media-unknown`); a nested
                // function-call `(` in the value (`calc(`) does not count.
                let (end, has_nested) = scan_paren_group(query, i);
                if has_nested {
                    out.push_str(&query[i..end]);
                } else {
                    lowercase_simple_feature_expr(&query[i..end], &mut out);
                }
                i = end;
            }
            _ => {
                // Depth-0 content (media types, `and`/`or`/`not`/`only`): verbatim.
                let ch = query[i..].chars().next().unwrap_or('\0');
                out.push(ch);
                i += ch.len_utf8();
            }
        }
    }
    Cow::Owned(out)
}

/// Index just past a `'…'`/`"…"` string starting at `start`, or the end of input for an
/// unterminated one (either failure arm — a missing close quote or a trailing `\`).
///
/// The string twin of [`crate::comments::comment_end`]; the two are what
/// [`trivia_span_at`] is built from. Defers to [`crate::lexer::string_end`], the crate's
/// single statement of the string token's extent — a *printer* asking where a string ends
/// must get the same answer the lexer did, or a scanner reading a prelude disagrees with
/// the parse it is printing.
fn skip_string(s: &str, start: usize) -> usize {
    crate::lexer::string_end(s.as_bytes(), start).unwrap_or(s.len())
}

/// If a `/* … */` comment or a `'…'`/`"…"` string starts at byte `i`, return the index
/// just past it (so `s[i..end]` is the whole trivia token); otherwise `None`. The
/// single source of truth for "what is skippable trivia" shared by the media-feature
/// scanners below — each copies it through or skips it, but they must all agree on its
/// extent and on what opens it.
///
/// ⚠️ An **escape** is not trivia and is deliberately not here: it is *content*, and the two
/// media-feature scanners want it for opposite reasons — [`lowercase_media_feature_names`]
/// steps one whole to keep it out of the structure it scans for, while
/// [`lowercase_simple_feature_expr`] runs the ident sequence *through* it
/// ([`ident_sequence_end`]). A shared "skip this span" arm would give the second one the
/// first one's answer and split the name at the escape again.
fn trivia_span_at(s: &str, i: usize) -> Option<usize> {
    let bytes = s.as_bytes();
    match bytes[i] {
        b'/' if crate::comments::is_comment_start(bytes, i) => {
            Some(crate::comments::comment_end(bytes, i))
        }
        b'"' | b'\'' => Some(skip_string(s, i)),
        _ => None,
    }
}

/// Whether `b` is a byte that can end a CSS identifier (a function name, right before
/// its `(`) — ASCII alphanumeric, `-`, `_`, or any non-ASCII byte (part of a
/// multi-byte ident char). Used to tell a function-call `(` (`calc(`, `min(`) from the
/// `(` that opens a grouped sub-condition.
///
/// This mirrors the CSS Syntax 3 tokenizer (§"Consume an ident-like token"): an ident
/// sequence *immediately* followed by `(` is consumed as a `<function-token>`. So a `(`
/// preceded by an ident byte is a function call; a `(` preceded by whitespace/`(`/a
/// connector opens a `( <media-condition> )` per the Media Queries 4 grammar.
fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b >= 0x80
}

/// Scan a parenthesized group starting at the `(` at `open`. Returns
/// `(index_just_past_the_matching_close_paren, contains_a_nested_sub_condition)`,
/// skipping comments/strings. A nested `(` that immediately follows an identifier byte
/// is a function call in the value (`calc(`) and does **not** set the flag; only a `(`
/// opening a sub-condition does. For an unbalanced group, returns end-of-input.
fn scan_paren_group(s: &str, open: usize) -> (usize, bool) {
    let bytes = s.as_bytes();
    let mut i = open + 1;
    let mut depth = 1usize;
    let mut has_nested = false;
    while i < s.len() {
        // A `(`/`)` inside a comment or string doesn't change paren depth.
        if let Some(end) = trivia_span_at(s, i) {
            i = end;
            continue;
        }
        // Nor does one inside an escape: `a\(b` is the single ident `a(b` (§"Consume an ident
        // sequence"), not a nested sub-condition, so the group stays a *simple* feature
        // expression and its name still lowercases.
        if let Some(end) = escape_span_at(s, i) {
            i = end;
            continue;
        }
        match bytes[i] {
            b'(' => {
                // A `(` immediately after an identifier byte is a function call inside
                // a feature value (`calc(`, `min(`, `env(`), not a grouped
                // sub-condition — it must NOT make the feature opaque, since prettier
                // still lowercases the feature name (`(MIN-WIDTH: calc(…))` →
                // `(min-width: calc(…))`). Only a `(` that opens a real sub-condition
                // (preceded by `(`, whitespace, or a connector keyword) marks the group
                // as grouped/complex.
                let is_function_call = i > 0 && is_ident_byte(bytes[i - 1]);
                if !is_function_call {
                    has_nested = true;
                }
                depth += 1;
                i += 1;
            }
            b')' => {
                depth -= 1;
                i += 1;
                if depth == 0 {
                    return (i, has_nested);
                }
            }
            _ => i += 1,
        }
    }
    (s.len(), has_nested)
}

/// Where the feature **name** ends inside `group` (a whole `(…)` slice): the byte index of
/// the name→value `:`, or `group.len()` for a boolean or range feature, which has none.
///
/// Trivia, escapes and a value function call's own parens are all stepped, so the `:` found
/// is one that really separates a name from a value — not one inside a comment or string,
/// not an escaped one (`(A\:B: 1px)` is a name carrying a `:`, per
/// [`ident_sequence_end`]), and not one nested in a call (`(a: url(x:y))`).
fn feature_name_end(group: &str) -> usize {
    debug_assert!(
        group.starts_with('('),
        "a feature expression is its whole parenthesized group"
    );
    let bytes = group.as_bytes();
    let mut i = 0;
    // `group` opens with its own `(`, so the name sits at depth 1.
    let mut depth = 0usize;
    while i < group.len() {
        if let Some(end) = trivia_span_at(group, i) {
            i = end;
            continue;
        }
        if let Some(end) = escape_span_at(group, i) {
            i = end;
            continue;
        }
        match bytes[i] {
            b'(' => depth += 1,
            b')' => depth = depth.saturating_sub(1),
            b':' if depth == 1 => return i,
            _ => {}
        }
        i += 1;
    }
    group.len()
}

/// Does the feature name spelled by `name` keep its case?
///
/// Prettier asks this of the **whole** media-feature node (`maybeToLowerCase` in
/// `src/language-css/utilities/index.js`), and asking it of anything smaller is what let a
/// case-sensitive name half-fold: tsv used to test each identifier run for the `--` prefix,
/// so `(--A B: 1px)` preserved `--A` and lowercased `B`.
///
/// - `--` opens an `<extension-name>` (css-extensions-1), an author-defined name with no
///   canonical casing to fold to — unlike a plain feature name, which is a pre-defined
///   keyword and so ASCII case-insensitive (css-values-4 §"Pre-defined Keywords").
/// - `$`, `@`, `#` and a leading `%` are prettier's preprocessor accommodations (`$var`,
///   `@var`, `#{…}`, `%placeholder`). tsv's parser is permissive enough to reach them, and
///   folding the case of a name it does not understand can only lose information.
/// - A name holding a balanced call is likewise not a keyword.
///
/// Prettier's remaining `:--` arm is unreachable here: the name region ends at the first
/// separator `:`, so a name cannot begin with one.
fn feature_name_preserves_case(name: &str) -> bool {
    name.starts_with("--")
        || name.starts_with('%')
        || name.contains(['$', '@', '#'])
        || (name.contains('(') && name.contains(')'))
}

/// Emit a simple media-feature expression `(…)` (no nested sub-condition), lowercasing the
/// feature name. See [`lowercase_media_feature_names`] for the name-position rule.
fn lowercase_simple_feature_expr(group: &str, out: &mut String) {
    let name_end = feature_name_end(group);
    // Everything after `group`'s own `(`, up to the separator, is the name prettier tests.
    if feature_name_preserves_case(crate::escapes::trim_css(&group[1..name_end])) {
        out.push_str(group);
        return;
    }

    // The name lowercases; the value after the separator is preserved, so only the region
    // below `name_end` is walked for identifiers and the rest is copied through.
    let mut i = 0;
    while i < name_end {
        // Comments/strings copy through verbatim (their contents are never lowercased).
        // Neither can straddle `name_end`: `feature_name_end` steps trivia whole, so the
        // separator it found is never inside one.
        if let Some(end) = trivia_span_at(group, i) {
            out.push_str(&group[i..end]);
            i = end;
            continue;
        }
        let ch = group[i..].chars().next().unwrap_or('\0');
        // An identifier: the ident class, a leading `-` (custom media, vendor prefixes),
        // or a leading escape — an escape begins an ident sequence exactly as an ident
        // code point does, and `ident_sequence_end` runs through the ones inside it too.
        let end = if is_ident_start(ch) || ch == '-' || ch == '\\' {
            ident_sequence_end(group, i)
        } else {
            i
        };
        if end > i {
            out.push_str(&lowercase_feature_name(&group[i..end]));
            i = end;
        } else {
            // Not an ident start, or a lone `\` that starts no escape.
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    debug_assert_eq!(i, name_end, "the name walk overshot the separator");
    out.push_str(&group[name_end..]);
}

/// Lowercase one identifier of a media-feature name. Whether the name is case-*sensitive*
/// is [`feature_name_preserves_case`]'s question, asked once of the whole name before any
/// of this runs; an already-lowercase identifier borrows unchanged.
fn lowercase_feature_name(name: &str) -> Cow<'_, str> {
    if name.bytes().any(|b| b.is_ascii_uppercase()) {
        Cow::Owned(name.to_ascii_lowercase())
    } else {
        Cow::Borrowed(name)
    }
}

/// Extract and normalize property name from declaration source
///
/// Handles property names with comments, preserving raw escapes (Svelte quirk).
/// Normalizes spacing around comments for readability.
///
/// # Arguments
/// * `decl_source` - Full declaration source (e.g., `color /* test */: red;`)
/// * `colon_pos` - Byte offset of the `property : value` colon within `decl_source`
///   (the parser's `CssDeclaration::colon_offset`, minus the declaration span start —
///   no re-scan; a property comment may itself hold a `:` that a naive scan would
///   mis-match, which the parser-recorded colon already skips)
/// * `has_block_comment` - The parser-recorded `CssDeclaration::has_block_comment`. False
///   proves the declaration holds no `/* … */` at all, so the comment path is skipped
///   without scanning the property text (the free O(1) negative gate
///   `has_value_comments_in_decl` uses).
///
/// # Returns
/// * Normalized property name (e.g., `color /* test */`)
///
/// The common case (a bare property name with no comment and no significant
/// trailing whitespace) returns a `Cow::Borrowed` sub-slice of `decl_source` — no
/// per-declaration allocation. Only the comment-bearing and hex-escape-terminator
/// forms reconstruct an owned `String`.
///
/// # Example
/// ```ignore
/// // colon_pos = the parser's recorded colon offset within decl_source
/// let source = "color/* comment */:red;";
/// assert_eq!(extract_property_name(source, 18, true), "color /* comment */");
///
/// // Every gap comment is preserved, joined single-spaced.
/// let source = "color/* a *//* b */:red;";
/// assert_eq!(extract_property_name(source, 19, true), "color /* a */ /* b */");
///
/// let source = "margin: 10px;";
/// assert_eq!(extract_property_name(source, 6, false), "margin");
/// ```
///
/// # Svelte Quirk
/// Property names preserve raw escapes without decoding.
/// Example: `\00e9motion` stays as `\00e9motion`, not `émotion`
///
/// # Formatter Divergence
/// We add spaces around comments for readability:
/// - Input: `color/* comment */:red;`
/// - Output: `color /* comment */` (normalized spacing)
/// - Prettier: `color/* comment */` (no space before comment)
///
/// # Boundary whitespace
/// The gap is an `allow_whitespace()` juncture to `parseCss` (`read_declaration` ends the
/// name at the first JS-`\s` code point), so a boundary run in it — `color<NBSP>: red`,
/// `top <NBSP>: 0`, `left<NBSP> /* c */ : 0` — is the gap's, not the name's, and this head
/// is what carries it back out: each gap's run is spelled where it stood, with the ASCII
/// whitespace beside it kept as one space (`boundary_run_spelling`). The trim that takes
/// the run off the name and the spelling that restores it read one class, so the run is
/// emitted exactly once.
pub(crate) fn extract_property_name(
    decl_source: &str,
    colon_pos: usize,
    has_block_comment: bool,
) -> Cow<'_, str> {
    let property_part = &decl_source[..colon_pos];

    // `has_block_comment` is recorded at parse time and is false iff the declaration
    // carries no `/* … */` anywhere — so the property→colon gap holds none either and the
    // substring scan below is provably fruitless. A literal `/*` cannot occur in a
    // property name except as a comment start (an escaped one is written `\2f\2a`), so
    // the gate can only skip work, never change the branch taken.
    if has_block_comment && let Some(comment_start) = property_part.find("/*") {
        let before_comment = trim_property_part(&property_part[..comment_start]);
        // The declaration's span starts at the property token, so the trim can only have
        // shortened the part's END — which makes the trimmed length the start of the gap
        // ahead of the first comment.
        debug_assert!(property_part.starts_with(before_comment));
        let mut out = String::from(before_comment);
        let mut gap_start = before_comment.len();

        // Collect EVERY comment in the property→colon gap, not just the first: a
        // declaration may carry two or more (`color /* a */ /* b */ :`), and this
        // gap is the only place a CSS declaration comment survives formatting (the
        // parser drops value comments), so emitting only the first is silent
        // content loss. Each comment is preceded by its gap's boundary run — spelled with
        // the author's ASCII presence, `boundary_run_spelling` — then a single space; the
        // gap after the last comment, ahead of the colon, takes the same spelling.
        let mut comment_pos = comment_start;
        loop {
            // `comment_pos` is at a `/*`.
            // `comment_end_checked`, not `comment_end`: a malformed comment abandons the
            // whole reconstruction rather than being taken as reaching end-of-input.
            let rest = &property_part[comment_pos..];
            let Some(comment_len) = crate::comments::comment_end_checked(rest.as_bytes(), 0) else {
                // Malformed comment (no closing `*/`) - just trim the whole part.
                return Cow::Borrowed(trim_property_part(property_part));
            };
            out.push_str(&boundary_run_spelling(
                &property_part[gap_start..comment_pos],
            ));
            out.push(' ');
            out.push_str(&rest[..comment_len]);
            gap_start = comment_pos + comment_len;
            match rest[comment_len..].find("/*") {
                Some(next) => comment_pos = gap_start + next,
                None => break,
            }
        }
        out.push_str(&boundary_run_spelling(&property_part[gap_start..]));
        Cow::Owned(out)
    } else {
        // No comment: trim insignificant whitespace, but a property name ending in a
        // hex escape (`\41`) consumes one following whitespace as the escape's
        // terminator. That whitespace is part of the identifier token, so prettier
        // keeps it before the `:` (`\41 : red`); any extra whitespace is still trimmed
        // (`color : red` → `color: red`).
        let bare = trim_property_part(property_part);
        // An untouched part — every real declaration — settles here without a second scan.
        if bare.len() == property_part.len() {
            return Cow::Borrowed(bare);
        }
        // The gap's own boundary run rides on the trimmed name, spelled with the author's
        // ASCII presence (`color<NBSP>`, `top <NBSP>`): the trim's class is the run's class,
        // so a part the trim shortened is the one place a run can be. The declaration's span
        // starts at the property token, so the trim can only have shortened the part's END —
        // what it took off is the tail, and a shortened part ends in the class by construction.
        debug_assert!(property_part.starts_with(bare));
        let run = boundary_run_spelling(&property_part[bare.len()..]);
        if !run.is_empty() {
            Cow::Owned(format!("{bare}{run}"))
        } else if crate::escapes::ends_with_live_hex_escape(bare) {
            Cow::Owned(format!("{bare} "))
        } else {
            Cow::Borrowed(bare)
        }
    }
}

/// [`str::trim`] over the class the printer's boundary claim scans back across.
///
/// ⚠️ **Not `str::trim`**, whose class is Unicode `White_Space` — which excludes `U+FEFF`,
/// while JS `\s` (and so `parseCss`'s own `.trim()` of the raw property text) includes it.
/// The gap this trims is also the gap `Printer::preserved_boundary_ws` re-emits from, and the
/// two have to agree on where the name ends or the character belongs to both: `color<ZWNBSP>:`
/// kept the run inside the name AND restored it beside it, doubling the run on every pass
/// (`1 → 2 → 4 → …`) — a non-idempotency no fixture can carry and no ratchet was watching.
/// `is_boundary_whitespace` is the class that scan uses, so asking it here is what makes the
/// two halves one answer.
///
/// The searchers run only for a part whose first or last byte could begin or end a run
/// (`byte_may_be_boundary_whitespace`); a real property name is `color`, an ASCII name with
/// nothing to trim at either end, and settles in four compares. The `debug_assert` keeps the
/// gate's claim under the fixture suite rather than in prose.
#[inline]
fn trim_property_part(property_part: &str) -> &str {
    let bytes = property_part.as_bytes();
    if let (Some(&first), Some(&last)) = (bytes.first(), bytes.last())
        && !crate::whitespace::byte_may_be_boundary_whitespace(first)
        && !crate::whitespace::byte_may_be_boundary_whitespace(last)
    {
        debug_assert_eq!(
            trim_property_part_wide(property_part).len(),
            property_part.len(),
            "an ASCII non-whitespace byte at each end proves there is nothing to trim"
        );
        return property_part;
    }
    trim_property_part_wide(property_part)
}

/// The searcher half of [`trim_property_part`] — one outlined copy.
#[inline(never)]
fn trim_property_part_wide(property_part: &str) -> &str {
    property_part.trim_matches(crate::whitespace::is_boundary_whitespace)
}

/// Canonicalize a property name's case: standard CSS property names are ASCII
/// case-insensitive (CSS Syntax 3), and prettier lowercases them (`COLOR`→`color`,
/// `Background-Color`→`background-color`, vendor `-WEBKIT-…`→`-webkit-…`). Returns
/// the input unchanged for the case-sensitive / non-standard / ambiguous forms:
/// - **custom properties** (`--*`) — case-sensitive per CSS Variables 1;
/// - **non-standard property starts** — any name not beginning with an ASCII letter
///   or a vendor-prefix `-`; these aren't CSS property names, so their case is left
///   untouched;
/// - names carrying a **comment** (`color /* c */`) or a **`\` escape** — left
///   verbatim so the lowercasing never touches comment text or an escape's hex
///   digits (both rare in a property position).
///
/// Takes the already-extracted name (see [`extract_property_name`]); only the ASCII
/// letters of a bare identifier are lowercased.
pub(crate) fn lowercase_property_name(name: Cow<'_, str>) -> Cow<'_, str> {
    // Cheapest discriminator first: an all-lowercase name (the overwhelming common
    // case) has nothing to do, so bail before the substring scans below — a word at a
    // time, since every name takes this walk and nearly none has anything to find.
    if !tsv_lang::swar::has_ascii_uppercase(name.as_bytes()) {
        return name;
    }
    // A standard property starts with an ASCII letter or a single vendor-prefix `-`
    // (`--` is a custom property, handled below).
    let standard_start = name
        .as_bytes()
        .first()
        .is_some_and(|&b| b.is_ascii_alphabetic() || b == b'-');
    if !standard_start || name.starts_with("--") || name.contains("/*") || name.contains('\\') {
        return name;
    }
    Cow::Owned(name.to_ascii_lowercase())
}

/// Lowercase an at-rule name (`@MEDIA` → `@media`, `@Font-Face` → `@font-face`),
/// matching prettier — which lowercases **all** at-rule names, including
/// vendor-prefixed ones (`@-WEBKIT-KEYFRAMES` → `@-webkit-keyframes`). An escaped
/// (`\`) name is left verbatim (lowercasing would corrupt an escape's hex digits);
/// an already-lowercase name borrows unchanged. The `@` is written separately by the
/// printer, so `name` is just the keyword.
///
/// Takes the name's **source** slice (`CssAtrule.name_span`), not the decoded
/// `CssAtrule.name`: an escaped name still carries its `\` here, so the escape-guard
/// below is what preserves it (the decoded form would hold a raw control char instead).
pub(crate) fn lowercase_at_rule_name(name: &str) -> Cow<'_, str> {
    if !tsv_lang::swar::has_ascii_uppercase(name.as_bytes()) || name.contains('\\') {
        return Cow::Borrowed(name);
    }
    Cow::Owned(name.to_ascii_lowercase())
}

/// Extract and format string value from declaration source
///
/// Preserves escape sequences by working with original source text.
///
/// # Arguments
/// * `decl_source` - Full declaration source (e.g., `content: "test\n";`)
/// * `colon_pos` - Byte offset of the declaration colon within `decl_source`
///   (the parser's recorded `colon_offset`, so no re-scan)
/// * `quote` - Quote character to use (' or ")
///
/// # Returns
/// * `Some(formatted_string)` if extraction successful
/// * `None` if the value isn't a well-formed quoted string (len < 2)
///
/// # Example
/// ```ignore
/// let source = "content: 'hello\\nworld';";
/// assert_eq!(extract_string_value(source, 7, '\''), Some("'hello\\nworld'".to_string()));
/// ```
pub(crate) fn extract_string_value(
    decl_source: &str,
    colon_pos: usize,
    quote: char,
) -> Option<String> {
    let value_part = decl_source[colon_pos + 1..].trim();
    // String should be quoted
    if value_part.len() >= 2 {
        let raw_content = &value_part[1..value_part.len() - 1];
        let formatted = format_string_literal(raw_content, quote);
        return Some(formatted);
    }

    None
}

/// Extract and normalize value with comments from declaration source
///
/// Extracts the value part of a declaration and normalizes spacing around comments.
///
/// # Arguments
/// * `decl_source` - Full declaration source (e.g., `margin: 10px /* test */ 20px;`)
/// * `colon_pos` - Byte offset of the declaration colon within `decl_source`
///   (the parser's recorded `colon_offset`, so no re-scan)
///
/// # Returns
/// * the normalized post-colon value text (e.g., `10px /* test */ 20px`)
///
/// # Example
/// ```ignore
/// let source = "margin: 10px  /* test */  20px;";
/// assert_eq!(extract_value_with_comments(source, 6), "10px /* test */ 20px");
/// ```
pub(crate) fn extract_value_with_comments(decl_source: &str, colon_pos: usize) -> String {
    let value_with_ws = &decl_source[colon_pos + 1..];
    normalize_css_whitespace(value_with_ws).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Stands in for the parser-recorded `CssDeclaration::has_block_comment` — true iff
    /// the declaration carries a `/* … */` anywhere. These test inputs hold no string or
    /// `url()` that could disguise a literal `/*`, so the substring check agrees with
    /// what the parser would record.
    fn has_block_comment(src: &str) -> bool {
        src.contains("/*")
    }

    #[test]
    fn test_extract_property_name_borrows_common_case() {
        // The overwhelmingly-common case: a bare property name with no comment and
        // no significant trailing whitespace borrows a sub-slice of the input — no
        // per-declaration allocation.
        // `colon_pos` is the parser's recorded `colon_offset` in production; these
        // inputs carry no colon inside a comment, so the first `:` is that colon.
        let colon = |s: &str| s.find(':').unwrap();
        for src in [
            "margin: 10px",
            "color:red",
            "--custom-prop: var(--x)",
            "grid-template-columns: 1fr 2fr",
        ] {
            let got = extract_property_name(src, colon(src), has_block_comment(src));
            assert!(
                matches!(got, Cow::Borrowed(_)),
                "bare property in {src:?} must borrow, got owned {got:?}"
            );
        }
        assert_eq!(
            extract_property_name("margin: 10px", colon("margin: 10px"), false),
            "margin"
        );
        assert_eq!(
            extract_property_name("color:red", colon("color:red"), false),
            "color"
        );
        assert_eq!(
            extract_property_name(
                "--custom-prop: var(--x)",
                colon("--custom-prop: var(--x)"),
                false
            ),
            "--custom-prop"
        );
    }

    #[test]
    fn test_extract_property_name_trims_but_still_borrows() {
        // Insignificant whitespace around the property name is trimmed; the result
        // is still a borrowed sub-slice (trim returns a sub-slice, no alloc).
        let src = "color : red";
        let got = extract_property_name(src, src.find(':').unwrap(), has_block_comment(src));
        assert_eq!(got, "color");
        assert!(matches!(got, Cow::Borrowed(_)));
    }

    #[test]
    fn test_extract_property_name_comment_owns() {
        // A comment in the property name reconstructs an owned, space-normalized form.
        // The comment holds no `:`, so the real colon is the first `:`.
        let src = "color/* comment */:red";
        let got = extract_property_name(src, src.find(':').unwrap(), has_block_comment(src));
        assert_eq!(got, "color /* comment */");
        assert!(matches!(got, Cow::Owned(_)));
    }

    #[test]
    fn test_extract_property_name_hex_escape_terminator_owns() {
        // A property name ending in a hex escape consumes one trailing whitespace as
        // the escape's terminator — that space is preserved, so the result owns.
        let src = "\\41 : red";
        let got = extract_property_name(src, src.find(':').unwrap(), has_block_comment(src));
        assert_eq!(got, "\\41 ");
        assert!(matches!(got, Cow::Owned(_)));

        // A non-escape trailing space is trimmed (borrowed).
        let src = "color : red";
        let got = extract_property_name(src, src.find(':').unwrap(), has_block_comment(src));
        assert_eq!(got, "color");
        assert!(matches!(got, Cow::Borrowed(_)));
    }

    #[test]
    fn test_lowercase_property_name() {
        let low = |s: &str| lowercase_property_name(Cow::Borrowed(s)).into_owned();
        // Standard properties + vendor prefixes lowercase.
        assert_eq!(low("COLOR"), "color");
        assert_eq!(low("Background-Color"), "background-color");
        assert_eq!(low("-WEBKIT-Box-Shadow"), "-webkit-box-shadow");
        // Already-lowercase borrows unchanged (no allocation).
        assert!(matches!(
            lowercase_property_name(Cow::Borrowed("color")),
            Cow::Borrowed("color")
        ));
        // Case-sensitive / non-standard names are preserved.
        assert_eq!(low("--MyVar"), "--MyVar"); // custom property
        assert_eq!(low("$fontFamily"), "$fontFamily"); // non-letter start → not a CSS property
        assert_eq!(low("#{$Foo}"), "#{$Foo}"); // non-letter start → preserved
        assert_eq!(low("COLOR /* C */"), "COLOR /* C */"); // comment-bearing
        assert_eq!(low("\\43OLOR"), "\\43OLOR"); // escaped
    }

    #[test]
    fn test_lowercase_at_rule_name() {
        let low = |s: &str| lowercase_at_rule_name(s).into_owned();
        assert_eq!(low("MEDIA"), "media");
        assert_eq!(low("Font-Face"), "font-face");
        assert_eq!(low("-WEBKIT-KEYFRAMES"), "-webkit-keyframes");
        assert_eq!(low("INCLUDE"), "include"); // non-standard directive name, still lowercased
        // Already-lowercase borrows unchanged (no allocation).
        assert!(matches!(
            lowercase_at_rule_name("media"),
            Cow::Borrowed("media")
        ));
        // Escaped names preserved (lowercasing would corrupt the escape's hex digits).
        assert_eq!(low("\\4D edia"), "\\4D edia");
    }

    #[test]
    fn test_lowercase_media_feature_names() {
        let f = |s: &str| lowercase_media_feature_names(s).into_owned();
        // Simple feature expressions: name lowercased, value preserved.
        assert_eq!(f("(MIN-WIDTH: 100px)"), "(min-width: 100px)");
        assert_eq!(f("(ORIENTATION: LANDSCAPE)"), "(orientation: LANDSCAPE)");
        assert_eq!(f("(HOVER)"), "(hover)"); // boolean feature
        assert_eq!(f("(WIDTH >= 600px)"), "(width >= 600px)"); // range, name-first
        assert_eq!(f("(600px <= WIDTH)"), "(600px <= width)"); // range, value-first
        // Media type + keyword preserved (only the feature name lowercases).
        assert_eq!(
            f("SCREEN and (MIN-WIDTH: 1px)"),
            "SCREEN and (min-width: 1px)"
        );
        // Custom media is case-sensitive → preserved.
        assert_eq!(f("(--SMALL-VIEWPORT)"), "(--SMALL-VIEWPORT)");
        // A function-valued feature is still simple — the name lowercases even though
        // the value carries nested parens (`calc(`/`min(`).
        assert_eq!(
            f("(MIN-WIDTH: calc(100px + 1px))"),
            "(min-width: calc(100px + 1px))"
        );
        assert_eq!(f("(WIDTH: min(50%, 100px))"), "(width: min(50%, 100px))");
        // Grouped/complex condition (nested sub-condition parens) left verbatim — even
        // when a nested feature itself has a function value.
        assert_eq!(
            f("((MIN-WIDTH: 1px) and (MAX-WIDTH: 2px))"),
            "((MIN-WIDTH: 1px) and (MAX-WIDTH: 2px))"
        );
        assert_eq!(
            f("((MIN-WIDTH: calc(1px)) and (b))"),
            "((MIN-WIDTH: calc(1px)) and (b))"
        );
        assert_eq!(f("(NOT (HOVER))"), "(NOT (HOVER))");
        // No uppercase → borrows unchanged (no allocation).
        assert!(matches!(
            lowercase_media_feature_names("(min-width: 100px)"),
            Cow::Borrowed(_)
        ));
        // A comment in the expression is preserved; the name still lowercases.
        assert_eq!(f("(MIN-WIDTH: /* c */ 1px)"), "(min-width: /* c */ 1px)");
    }

    #[test]
    fn test_normalize_value_text_hex() {
        // The value path (`@supports`, `@import`): a hex color of a valid length lowercases.
        let sup = |s: &str| normalize_value_text(s, PreludeReader::Value);
        assert_eq!(sup("(background: #FFF)"), "(background: #fff)"); // 3-digit
        assert_eq!(sup("(a: #ABCD)"), "(a: #abcd)"); // 4-digit RGBA
        assert_eq!(sup("(a: #AABBCC)"), "(a: #aabbcc)"); // 6-digit
        assert_eq!(sup("(a: #AABBCCDD)"), "(a: #aabbccdd)"); // 8-digit RRGGBBAA
        assert_eq!(sup("(a: #1E2)"), "(a: #1e2)"); // exponent-looking but a valid 3-hex color
        // Off-length or non-hex `#`-tokens keep their case (not colors).
        assert_eq!(sup("(a: #ABCDE)"), "(a: #ABCDE)"); // 5 digits
        assert_eq!(sup("(a: #ABCDEFA)"), "(a: #ABCDEFA)"); // 7 digits
        assert_eq!(sup("(a: #GG)"), "(a: #GG)"); // non-hex
        // Inside `url(...)`/`selector(...)`, a `#` is a URL fragment / ID selector →
        // preserved even though it is a valid hex-color shape. Function-name case is
        // irrelevant to the detection (CSS function tokens are case-insensitive).
        assert_eq!(sup("(background: url(#ABC))"), "(background: url(#ABC))");
        assert_eq!(sup("selector(#ABC)"), "selector(#ABC)");
        assert_eq!(sup("URL(#ABC)"), "URL(#ABC)");
        // A hex color OUTSIDE the preserve group still lowercases; the group's is kept.
        assert_eq!(sup("url(#ABC) #FFF"), "url(#ABC) #fff");
        // A non-opaque function (var/color-mix) is a value context → hex lowercases.
        assert_eq!(sup("var(--x, #ABC)"), "var(--x, #abc)");
        // Number normalization is unaffected by the new hex flag.
        assert_eq!(sup("(margin: .5px)"), "(margin: 0.5px)");

        // An unquoted `url(...)` is an opaque `<url-token>`: number/unit-looking
        // content is copied verbatim, never normalized.
        assert_eq!(
            sup("(background: url(sprite1.50.png))"),
            "(background: url(sprite1.50.png))"
        );
        assert_eq!(sup("(width: url(A.5PX))"), "(width: url(A.5PX))");
        assert_eq!(sup("url()"), "url()"); // empty url-token
        // A quoted `url("…")` is a `<string>` arg: quotes normalize, content preserved.
        assert_eq!(
            normalize_value_text("(background: url(\"v2.00.png\"))", PreludeReader::Value),
            "(background: url('v2.00.png'))"
        );
        // url opacity is unconditional — it holds on either path.
        assert_eq!(
            normalize_value_text("(a: url(x1.50))", PreludeReader::MediaQuery),
            "(a: url(x1.50))"
        );

        // The media path (`@media`): the regex has no hash arm, so a `#` is copied as the
        // delimiter it is and its run keeps its case.
        assert_eq!(
            normalize_value_text("(min-width: #FFF)", PreludeReader::MediaQuery),
            "(min-width: #FFF)"
        );
        assert_eq!(
            normalize_value_text("(margin: .5px)", PreludeReader::MediaQuery),
            "(margin: 0.5px)"
        );
    }

    /// A `\)` inside an unquoted `url()` is url content (§4.3.6), not the token's close,
    /// so the whole token is copied verbatim and the bytes past the escape are neither
    /// number- nor hex-normalized.
    ///
    /// Prettier throws `Unbalanced parenthesis` on the single-condition spelling of every
    /// input below, so the fixture that grades this end to end
    /// (`css/at_rules/supports_url_escaped_paren`) has to reach for a two-condition prelude
    /// that happens to balance postcss's own count. These pin the scan directly, including
    /// the **unbalanced** shapes no prettier oracle exists for at all.
    #[test]
    fn an_escaped_paren_does_not_close_a_url_token() {
        // The two normalizations the tail would otherwise take.
        assert_eq!(
            normalize_value_text(r"(a: url(x\)1.50))", PreludeReader::Value),
            r"(a: url(x\)1.50))"
        );
        assert_eq!(
            normalize_value_text(r"(a: url(x\)y#FFF))", PreludeReader::Value),
            r"(a: url(x\)y#FFF))"
        );
        // An escaped OPEN paren nests nothing either — the token still ends at the first
        // unescaped `)`, so `1.50` stays inside it.
        assert_eq!(
            normalize_value_text(r"(a: url(x\(1.50))", PreludeReader::Value),
            r"(a: url(x\(1.50))"
        );
        // A hex escape carries its whitespace terminator, so the `)` here is the escape's
        // payload and not a close.
        assert_eq!(
            normalize_value_text(r"(a: url(x\29 1.50))", PreludeReader::Value),
            r"(a: url(x\29 1.50))"
        );
        // Unbalanced: the group runs to end of input rather than stopping at the escape.
        assert_eq!(
            normalize_value_text(r"(a: url(x\)1.50)", PreludeReader::Value),
            r"(a: url(x\)1.50)"
        );
        // A trailing `\` starts no escape, so the `)` before it still closes.
        assert_eq!(
            normalize_value_text(r"(a: url(x1.50)\)", PreludeReader::Value),
            r"(a: url(x1.50)\)"
        );
        // Control: no escape, so the token is opaque for the ordinary reason.
        assert_eq!(
            normalize_value_text("(a: url(x1.50))", PreludeReader::Value),
            "(a: url(x1.50))"
        );
    }

    /// A `(` or `)` inside an escape is ident content (§"Consume an ident sequence"), so it
    /// neither opens a sub-condition nor moves the group's depth: the feature expression
    /// stays *simple* and its name lowercases.
    ///
    /// Prettier drops the whole prelude on the unbalanced spelling (`@media (a: b\(c)` →
    /// `@media  {`), so only the balanced one is gradable by fixture
    /// (`css/at_rules/media_feature_escaped_paren_value`); both are pinned here.
    #[test]
    fn an_escaped_paren_is_not_a_media_sub_condition() {
        // Balanced raw parens inside escapes — the fixture's shape.
        assert_eq!(
            lowercase_media_feature_names(r"(MIN-WIDTH: a\(b\)c)"),
            r"(min-width: a\(b\)c)"
        );
        // Unbalanced: without the escape arm the group swallowed the rest of the query.
        assert_eq!(
            lowercase_media_feature_names(r"(MIN-WIDTH: a\(b)"),
            r"(min-width: a\(b)"
        );
        assert_eq!(
            lowercase_media_feature_names(r"(MIN-WIDTH: a\)b)"),
            r"(min-width: a\)b)"
        );
        assert_eq!(
            lowercase_media_feature_names(r"(MIN-WIDTH: a\(b) and (MAX-WIDTH: 1px)"),
            r"(min-width: a\(b) and (max-width: 1px)"
        );
        // A real function call in the value still isn't a sub-condition, and a real
        // sub-condition still is — the escape arm moves neither.
        assert_eq!(
            lowercase_media_feature_names("(MIN-WIDTH: calc(1px))"),
            "(min-width: calc(1px))"
        );
        assert_eq!(
            lowercase_media_feature_names("((MIN-WIDTH: 1px))"),
            "((MIN-WIDTH: 1px))"
        );
        // At depth 0 an escaped `(` is part of a media TYPE, not the `(` that opens a
        // feature expression — reading it as one handed the rest of the query to
        // `scan_paren_group` as one opaque group and the feature name never lowercased.
        assert_eq!(
            lowercase_media_feature_names(r"a\(b and (MIN-WIDTH: 1px)"),
            r"a\(b and (min-width: 1px)"
        );
    }

    /// A media-feature name is one **ident sequence**, and an escape is ident content
    /// (CSS Syntax 3 §"Consume an ident sequence"), so the name runs *through* every escape
    /// it carries — including one spelling the `:` that would otherwise look like the
    /// name→value separator.
    ///
    /// Two rules key on the whole name, and a scanner that stopped at the `\` got both
    /// wrong on the fragment after it: a plain name is a pre-defined keyword and so ASCII
    /// case-insensitive (css-values-4 §"Pre-defined Keywords"), while a custom-media name
    /// is an `<extension-name>` (css-extensions-1) with no canonical casing to fold to, so
    /// only the fragment carrying the `--` was preserved.
    ///
    /// The `\:` spelling has no prettier oracle — prettier reads the escape's payload as
    /// the separator and splits the ident (`(a\: B: 1px)`), a cataloged divergence
    /// (`media_feature_escaped_colon_prettier_divergence`).
    #[test]
    fn an_escaped_colon_does_not_end_a_feature_name() {
        // The whole run is the name, so the whole run lowercases.
        assert_eq!(
            lowercase_media_feature_names(r"(A\:B: 1px)"),
            r"(a\:b: 1px)"
        );
        // Case-sensitive per css-extensions-1 — must survive whole, past the escape.
        assert_eq!(
            lowercase_media_feature_names(r"(--A\:B: 1px)"),
            r"(--A\:B: 1px)"
        );
        assert_eq!(
            lowercase_media_feature_names(r"(--A\42 C: 1px)"),
            r"(--A\42 C: 1px)"
        );
        assert_eq!(
            lowercase_media_feature_names(r"(--MIN\-WIDTH: 1px)"),
            r"(--MIN\-WIDTH: 1px)"
        );
        // A boolean feature — no `:` at all, so the whole group is name position.
        assert_eq!(lowercase_media_feature_names(r"(--A\42 C)"), r"(--A\42 C)");
        // An escape elsewhere in a plain name is ordinary content and the run lowercases
        // across it, hex digits included (a hex escape is itself case-insensitive).
        assert_eq!(
            lowercase_media_feature_names(r"(MIN\-WIDTH: 1px)"),
            r"(min\-width: 1px)"
        );
        assert_eq!(
            lowercase_media_feature_names(r"(A\4B C: 1px)"),
            r"(a\4b c: 1px)"
        );
        // A lone `\` starts no escape (§4.3.4) and so extends nothing: the run ends at it
        // and the scanner still advances.
        assert_eq!(
            lowercase_media_feature_names("(A\\\nB: 1px)"),
            "(a\\\nb: 1px)"
        );
        assert_eq!(lowercase_media_feature_names(r"(A: 1px)\"), r"(a: 1px)\");
        // The value side is preserved whether or not it carries an escape.
        assert_eq!(
            lowercase_media_feature_names(r"(A: LAND\53 CAPE)"),
            r"(a: LAND\53 CAPE)"
        );
    }

    /// Case-sensitivity is a property of the **whole** feature name, which is why
    /// [`feature_name_preserves_case`] is asked once rather than per identifier: a name
    /// spelled as several runs kept only the run that carried the marker, so `(--A B: 1px)`
    /// preserved `--A` and lowercased `B`. Prettier asks the same question of the same
    /// region (`maybeToLowerCase`), so every case here matches it.
    #[test]
    fn case_sensitivity_is_a_property_of_the_whole_feature_name() {
        // An `<extension-name>` marks the whole name, however many runs it spans.
        assert_eq!(
            lowercase_media_feature_names(r"(--A B: 1px)"),
            r"(--A B: 1px)"
        );
        assert_eq!(
            lowercase_media_feature_names(r"(--A\Z B: 1px)"),
            r"(--A\Z B: 1px)"
        );
        assert_eq!(
            lowercase_media_feature_names("(--A 1PX: 1px)"),
            "(--A 1PX: 1px)"
        );
        // Leading whitespace does not hide the marker.
        assert_eq!(
            lowercase_media_feature_names("( --A B : 1px)"),
            "( --A B : 1px)"
        );
        // Preprocessor spellings tsv's permissive parser reaches: folding a name it does
        // not understand can only lose information, and prettier preserves them too.
        assert_eq!(lowercase_media_feature_names("(A$B: 1px)"), "(A$B: 1px)");
        assert_eq!(lowercase_media_feature_names("($A: 1px)"), "($A: 1px)");
        assert_eq!(lowercase_media_feature_names("(A@B: 1px)"), "(A@B: 1px)");
        assert_eq!(lowercase_media_feature_names("(A#B: 1px)"), "(A#B: 1px)");
        assert_eq!(lowercase_media_feature_names("(%A: 1px)"), "(%A: 1px)");
        // A `%` that is not leading is not a marker, so the name still folds.
        assert_eq!(lowercase_media_feature_names("(A%B: 1px)"), "(a%b: 1px)");
        // A `:` in the VALUE does not end the name, so a call there cannot mark it.
        assert_eq!(
            lowercase_media_feature_names("(MIN-WIDTH: calc(1px))"),
            "(min-width: calc(1px))"
        );
        assert_eq!(
            lowercase_media_feature_names("(MIN-WIDTH: url(x:y))"),
            "(min-width: url(x:y))"
        );
        // A `:` inside trivia is not the separator either.
        assert_eq!(
            lowercase_media_feature_names("(/* : */ MIN-WIDTH: 1px)"),
            "(/* : */ min-width: 1px)"
        );
        assert_eq!(
            lowercase_media_feature_names(r"(MIN-WIDTH: ':')"),
            r"(min-width: ':')"
        );
    }

    /// A number abutting the identifier before it is that identifier's own tail: the ident
    /// run stops at the first non-ident code point, so what follows can only be a
    /// `.`-leading number, and its canonical leading zero would be swallowed by the ident.
    /// Prettier states the same rule textually and both formatters copy the pair through.
    #[test]
    fn a_number_abutting_an_identifier_is_copied_verbatim() {
        // `x1` + `.50`, not `x10` + `.5`.
        assert_eq!(
            normalize_value_text("(a: x1.50)", PreludeReader::Value),
            "(a: x1.50)"
        );
        // The unit rides along verbatim too — no `canonical_unit` lowercasing.
        assert_eq!(
            normalize_value_text("(a: x.50PX)", PreludeReader::Value),
            "(a: x.50PX)"
        );
        // A leading `--` reaches the ident arm at the first letter, so it is covered.
        assert_eq!(
            normalize_value_text("(a: --x.50)", PreludeReader::Value),
            "(a: --x.50)"
        );
        // Separated by whitespace, the number is a value of its own and normalizes.
        assert_eq!(
            normalize_value_text("(a: x .50)", PreludeReader::Value),
            "(a: x 0.5)"
        );
        // So does one that starts the value.
        assert_eq!(
            normalize_value_text("(a: .50px)", PreludeReader::Value),
            "(a: 0.5px)"
        );
        // A signed number keeps its sign, which merges with nothing, so it normalizes even
        // against an identifier. (`x-1` is one ident run, so `x-1.50` is the glued case.)
        assert_eq!(
            normalize_value_text("(a: x+1.50)", PreludeReader::Value),
            "(a: x+1.5)"
        );
        assert_eq!(
            normalize_value_text("(a: x-1.50)", PreludeReader::Value),
            "(a: x-1.50)"
        );
        // `$` and `@` are `<delim-token>`s, not ident code points, so nothing merges into
        // them even though this module's ident class reads them as run starts.
        assert_eq!(
            normalize_value_text("(a: $.50)", PreludeReader::MediaQuery),
            "(a: $0.5)"
        );
        assert_eq!(
            normalize_value_text("(a: @.50)", PreludeReader::MediaQuery),
            "(a: @0.5)"
        );
        assert_eq!(
            normalize_value_text("(a: $x.50)", PreludeReader::MediaQuery),
            "(a: $x.50)"
        );
        // An escape does not make a word part (prettier's own scan reads no `\` either),
        // so the number after one still normalizes — matching prettier.
        assert_eq!(
            normalize_value_text(r"(a: \41 2.50px)", PreludeReader::Value),
            r"(a: \41 2.5px)"
        );
        assert_eq!(
            normalize_value_text(r"(a: x\41 .50)", PreludeReader::Value),
            r"(a: x\41 0.5)"
        );
        // The ident arm's own `url(` exit clears the word part: what follows the group is
        // not the ident's tail.
        assert_eq!(
            normalize_value_text("(a: url(x).50)", PreludeReader::Value),
            "(a: url(x)0.5)"
        );
    }

    /// The three rules keyed on [`PreludeReader`], each measured against the reader prettier
    /// actually uses: the unit gate, the hex fold, and which runs absorb a number. One
    /// authoring, two canonical forms — so every case is asserted on both paths.
    #[test]
    fn the_two_prelude_readers_answer_a_number_differently() {
        let value = |s: &str| normalize_value_text(s, PreludeReader::Value);
        let media = |s: &str| normalize_value_text(s, PreludeReader::MediaQuery);

        // The unit gate. `printUnit` has none — whatever trails the number is the unit —
        // where `adjustNumbers` keeps the whole match on a unit CSS doesn't define.
        assert_eq!(value("(a: 1.50abc)"), "(a: 1.5abc)");
        assert_eq!(media("(a: 1.50abc)"), "(a: 1.50abc)");
        // A unit both readers accept normalizes on both, casing included.
        assert_eq!(value("(a: 1.50PX)"), "(a: 1.5px)");
        assert_eq!(media("(a: 1.50PX)"), "(a: 1.5px)");

        // A number's own unit absorbs the number after it on the value path (one word), and
        // absorbs nothing on the media path (the regex re-splits at the second number).
        assert_eq!(value("(a: 1a.50)"), "(a: 1a.50)");
        assert_eq!(media("(a: 1a.50)"), "(a: 1a0.5)");
        // The same with no unit between them — a second `.` ends the first number.
        assert_eq!(value("(a: 1.5.50)"), "(a: 1.5.50)");
        assert_eq!(media("(a: 1.5.50)"), "(a: 1.50.5)");

        // A `<hash-token>` absorbs on the value path (postcss-values-parser reads the word
        // whole); the media path has no hash arm at all, so the `#` is a bare delimiter and
        // the run after it takes the ordinary number arm.
        assert_eq!(value("(a: #1.50)"), "(a: #1.50)");
        assert_eq!(media("(a: #1.50)"), "(a: #1.5)");
        assert_eq!(value("(a: #1.50px)"), "(a: #1.50px)");
        assert_eq!(media("(a: #1.50px)"), "(a: #1.5px)");
        // A `#` followed by no ident code point heads no hash token (§4.3.1 leaves a
        // `<delim-token>`), so the number normalizes on both.
        assert_eq!(value("(a: #.50)"), "(a: #0.5)");
        assert_eq!(media("(a: #.50)"), "(a: #0.5)");
        // A hash token's own ident run still absorbs on the media path, via its word part.
        assert_eq!(value("(a: #abc1.50)"), "(a: #abc1.50)");
        assert_eq!(media("(a: #abc1.50)"), "(a: #abc1.50)");
        // The hex fold is the value reader's alone.
        assert_eq!(value("(a: #FFF)"), "(a: #fff)");
        assert_eq!(media("(a: #FFF)"), "(a: #FFF)");

        // `$`/`@` head no `WORD_PART` (`[$@]?` must be followed by a letter or `_`), so the
        // media reader's number starts inside the run this module read as one ident.
        assert_eq!(value("(a: $1.50)"), "(a: $1.50)");
        assert_eq!(media("(a: $1.50)"), "(a: $1.5)");
        assert_eq!(media("(a: x$1.50)"), "(a: x$1.5)");
        // A word part after the `$` absorbs again, on both.
        assert_eq!(value("(a: $a1.50)"), "(a: $a1.50)");
        assert_eq!(media("(a: $a1.50)"), "(a: $a1.50)");
        // …and the word part may start anywhere after the last `$`/`@`.
        assert_eq!(media("(a: $$a.50)"), "(a: $$a.50)");
        // With no word part and no digits in the tail, the run ends where it did.
        assert_eq!(media("(a: $-.50)"), "(a: $-0.5)");
    }

    /// The hex fold is asked of the **whole** hash word, so a colour is a colour only when
    /// the token ends at its digits. A body narrowed to the hex-shaped prefix recased a
    /// token that is no colour (`#FFF.5` → `#fff.5`).
    #[test]
    fn the_hex_fold_reads_the_whole_hash_word() {
        let value = |s: &str| normalize_value_text(s, PreludeReader::Value);

        // The word runs on past the digits — no colour here, so nothing is recased.
        assert_eq!(value("(a: #FFF.5)"), "(a: #FFF.5)");
        assert_eq!(value("(a: #FFF/5)"), "(a: #FFF/5)");
        assert_eq!(value("(a: #FFFé)"), "(a: #FFFé)");
        assert_eq!(value("(a: #FFF-x)"), "(a: #FFF-x)");
        assert_eq!(value("(a: #FFF_x)"), "(a: #FFF_x)");
        assert_eq!(value("(a: #ABCD.5)"), "(a: #ABCD.5)");
        // A separator ends the word, so the colour before it folds.
        assert_eq!(value("(a: #FFF 5)"), "(a: #fff 5)");
        assert_eq!(value("(a: #FFF)"), "(a: #fff)");
        // A word of hex-colour length folds however it got there.
        assert_eq!(value("(a: #FFF9)"), "(a: #fff9)");

        // Whether the token starts at all stays §4.3.1's question, and its answer is
        // narrower than the word class: `.` continues a hash word but heads none, so `#.50`
        // is a bare delimiter and the number after it is its own token.
        assert_eq!(value("(a: #.50)"), "(a: #0.5)");
        // `-` IS an ident code point, so it heads one — and the word then absorbs, which is
        // what keeps `#-` a hash token whose value is `-` rather than `-0`.
        assert_eq!(value("(a: #-.50)"), "(a: #-.50)");
    }
}

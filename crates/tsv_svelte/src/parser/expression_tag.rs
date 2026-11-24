// Expression tag parsing

use crate::ast::*;
use crate::lexer::{BlockOrTagMarker, TokenKind};
use tsv_lang::{Error, Span};
use tsv_ts::__private::BareIsland;
use tsv_ts::ast::{AstView, ExprId};

use super::parser_impl::SvelteParser;

/// The two contexts Svelte's `read_sequence` runs in — a run of text and `{expr}` chunks,
/// where a `{#…}` block or `{@…}` tag is invalid. The strings are Svelte's own `location`
/// argument to `block_invalid_placement` / `tag_invalid_placement`, so the messages
/// [`SvelteParser::check_sequence_placement`] builds read exactly as the canonical parser's.
#[derive(Debug, Clone, Copy)]
pub(crate) enum SequenceLocation {
    /// `<textarea>` RCDATA content — Svelte's sole RCDATA element.
    InsideTextarea,
    /// An attribute value, quoted or not, a directive's and a style directive's included.
    AttributeValue,
}

impl SequenceLocation {
    const fn as_str(self) -> &'static str {
        match self {
            Self::InsideTextarea => "inside <textarea>",
            Self::AttributeValue => "in attribute value",
        }
    }
}

impl<'a> SvelteParser<'a> {
    /// Parse an expression tag `{expression}` at the current lexer position, then
    /// advance the lexer past the closing `}`.
    ///
    /// Used by callers that drive the token stream (template `{expr}` tags,
    /// directive values). Position-based callers that own their own cursor — the
    /// attribute-value sequence readers — use `parse_expression_tag_at`, which runs
    /// the same scan + parse without touching the lexer.
    pub(crate) fn parse_expression_tag(&mut self) -> Result<ExpressionTag, Error> {
        // Verify we're at opening brace — a `{/` included, which the lexer calls a block
        // close but a directive value reads as a regex-led expression
        // (`current_opens_directive_value`).
        if !matches!(
            self.current_kind(),
            TokenKind::LeftBrace | TokenKind::BlockClose
        ) {
            return Err(self.error_expected_found("'{'"));
        }

        let tag = self.parse_expression_tag_at(self.current_start())?;

        // Resume lexing AFTER the closing brace (not at it), preserving tag-vs-template
        // context. Repositioning past `}` means the lexer never tokenizes it, so a `}`
        // in template text stays plain text — matching Svelte, which consumes `}`
        // directly after expression parsing (e.g. `class={expr}>` stays in tag mode,
        // `{expr}</div>` returns to template mode).
        self.advance_to_position(tag.span.end as usize)?;

        Ok(tag)
    }

    /// Parse an expression tag inside a **sequence** — a run of text and `{expr}` chunks —
    /// rejecting a `{#…}` block or `{@…}` tag first, as Svelte's `read_sequence` does.
    ///
    /// This is the entry point every sequence reader takes, rather than
    /// [`Self::parse_expression_tag_at`] plus a guard the next reader can forget: tsv reaches
    /// by five routes what Svelte reaches through one `read_sequence`, and a guard missing
    /// from one of them is the whole bug. The two routes this cannot serve — the directive
    /// arms, which take their `{…}` off the token stream — ask
    /// [`Self::check_sequence_placement`] directly.
    pub(crate) fn parse_sequence_expression_tag_at(
        &mut self,
        brace_pos: usize,
        location: SequenceLocation,
    ) -> Result<ExpressionTag, Error> {
        self.check_sequence_placement(brace_pos, location)?;
        self.parse_expression_tag_at(brace_pos)
    }

    /// Reject a `{#…}` block or `{@…}` tag written where only a text/`{expr}` sequence is
    /// allowed — Svelte's `read_sequence` guard (`1-parse/state/element.js`), which runs
    /// *before* the expression is read.
    ///
    /// Without it the brace contents reach the TypeScript expression parser, which answers a
    /// question nobody asked: `{@debug e}` becomes a decorator (`Expected 'class' after
    /// 'decorator'`) and `{#x in y}` is the one production where a private name is an operand
    /// (the ergonomic brand check), so it *parses* — an over-acceptance in every sequence
    /// context, attribute values included.
    ///
    /// The marker need not be glued to the `{`: [`BlockOrTagMarker::in_sequence_at`] skips the
    /// gap and owns the question of why tsv is wider here than `read_sequence` is. The error is
    /// still reported **at the brace**, matching Svelte's own `block_invalid_placement` index.
    pub(crate) fn check_sequence_placement(
        &self,
        brace_pos: usize,
        location: SequenceLocation,
    ) -> Result<(), Error> {
        let Some((marker, marker_pos)) = BlockOrTagMarker::in_sequence_at(self.source, brace_pos)
        else {
            return Ok(());
        };
        let bytes = self.source.as_bytes();
        // Svelte names the construct with `read_until(/[^a-z]/)` — lowercase ASCII only, so
        // `{@html expr}` names `html` and `{#}` names nothing.
        // `name_start <= bytes.len()`: the marker byte at `marker_pos` was found, so the slice
        // below is in range even when the document ends right after it (`{#`).
        let name_start = marker_pos + 1;
        let rest = &bytes[name_start..];
        let name_len = rest
            .iter()
            .position(|b| !b.is_ascii_lowercase())
            .unwrap_or(rest.len());
        let name = &self.source[name_start..name_start + name_len];
        let sigil = marker.sigil();
        let construct = marker.construct();
        let location = location.as_str();
        Err(self.error_msg_at(
            &format!("{{{sigil}{name} ...}} {construct} cannot be {location}"),
            brace_pos,
        ))
    }

    /// Scan and parse an expression tag `{expression}` starting at byte `brace_pos`
    /// (which must be `{`). The returned tag's span runs from `brace_pos` through the
    /// byte just past the matching `}` (`tag.span.end`).
    ///
    /// Unlike `parse_expression_tag`, this does **not** touch the lexer — the caller
    /// owns the cursor (the raw-byte attribute-value sequence readers reposition once
    /// when the whole value is done). Three readings find the matching `}`, in turn: a bare
    /// name path glued to its `}` (`tsv_ts::__private::parse_bare_name_closed`); then the
    /// island's own parse, run over the rest of the document and stopping at the `}` it
    /// leaves current (`tsv_ts::__private::parse_expression_to_brace`, which refuses an
    /// island holding a `/` the scans' regex rule and the parse may read apart, such as a
    /// division after a `}`); and
    /// last, for every island those refuse, [`scan_to_matching_brace`] (nested braces,
    /// strings, comments, regex and template literals skipped) and a parse of the slice it
    /// bounds, which reports every error.
    pub(crate) fn parse_expression_tag_at(
        &mut self,
        brace_pos: usize,
    ) -> Result<ExpressionTag, Error> {
        debug_assert_eq!(
            self.source.as_bytes().get(brace_pos),
            Some(&b'{'),
            "parse_expression_tag_at must start at `{{`"
        );
        let start = brace_pos;
        let expr_start = brace_pos + 1; // after the '{'

        // The common island, a name path glued to its `}` (`{count}`, `{user.name}`, `{f()}`),
        // is pushed without finding the brace first: the byte after the path is the close.
        let nesting = self.island_nesting();
        if let Some((expression, close)) = tsv_ts::__private::parse_bare_name_closed(
            self.source,
            expr_start,
            b'}',
            BareIsland::Any,
            nesting,
            self.stores_mut(),
        ) {
            return Ok(ExpressionTag {
                expression,
                span: Span {
                    start: start as u32,
                    end: (close + 1) as u32,
                },
            });
        }

        // Every other island is parsed to the `}` it stops at, which the brace scan then need
        // not find — unless the parse refuses it (an island holding a `/` the scan may read
        // apart from the parse among them).
        if let Some(island) = self.with_island_stores(|this, stores| {
            tsv_ts::__private::parse_expression_to_brace(
                this.source,
                expr_start,
                this.island_nesting(),
                stores,
            )
        }) {
            #[cfg(debug_assertions)]
            {
                assert_eq!(
                    scan_to_matching_brace(self.source.as_bytes(), expr_start),
                    Some(island.close),
                    "an island parsed to its `}}` ends where the brace scan does not: {:?}",
                    &self.source[expr_start..island.close]
                );
            }
            self.merge_island_comments(island.comments);
            return Ok(ExpressionTag {
                expression: island.expr,
                span: Span {
                    start: start as u32,
                    end: (island.close + 1) as u32,
                },
            });
        }

        // The island's parse refused it: find the matching closing `}` — the one robust
        // brace matcher — and parse the slice, which owns every error.
        let Some(expr_end) = scan_to_matching_brace(self.source.as_bytes(), expr_start) else {
            return Err(self.error_unclosed_at("expression tag", start));
        };

        // Extract expression content
        let expr_content = &self.source[expr_start..expr_end];

        // Parse expression using TypeScript parser — the shared helper, which
        // collects the comments into `Root.comments`.
        let expression = self.parse_ts_expression(expr_content, expr_start, "'}'")?;

        // The span end is right after the closing brace
        let end = expr_end + 1;

        Ok(ExpressionTag {
            expression,
            span: Span {
                start: start as u32,
                end: end as u32,
            },
        })
    }

    /// The expression of a head that opens on a keyword — `{#if …}`, `{:else if …}`,
    /// `{#key …}`, `{@html …}`, `{@render …}`, `{@attach …}` — parsed to the `}` it stops at,
    /// as [`Self::parse_head_to_brace`] reads it, with the lexer advanced past that `}`.
    /// `head_start` is the first byte after the `{#` / `{:` / `{@` marker, where `keywords`
    /// must stand in turn, each followed by a run of Svelte whitespace (JS `\s`).
    ///
    /// `Ok(None)`, nothing merged and the lexer untouched, for every head that reading
    /// refuses and for every head spelled otherwise — a keyword with no whitespace after it.
    /// The caller then scans to the head's `}` and parses the slice, which owns every error.
    pub(crate) fn parse_keyword_head_to_brace(
        &mut self,
        head_start: usize,
        keywords: &[&[u8]],
        shape: BareIsland,
        accept: impl Fn(&Self, AstView<'_>, ExprId) -> bool,
    ) -> Result<Option<(ExprId, usize)>, Error> {
        let Some(expr_start) = head_expression_start(self.source.as_bytes(), head_start, keywords)
        else {
            return Ok(None);
        };
        let parsed = self.parse_head_to_brace(expr_start, shape, accept)?;
        // The head ends where the bounded path's scan, which starts past the keyword, ends it.
        #[cfg(debug_assertions)]
        if let Some((_, end)) = parsed {
            assert_eq!(
                scan_head_to_matching_brace(self.source.as_bytes(), head_start),
                Some(end - 1),
                "a head parsed to its `}}` ends where the head scan does not: {:?}",
                &self.source[head_start..end]
            );
        }
        Ok(parsed)
    }

    /// A head's expression starting at `expr_start` — past a keyword's whitespace or a
    /// spread's `...` — parsed to the `}` it stops at, as an expression tag's island is
    /// ([`Self::parse_expression_tag_at`]): a bare name path glued to its `}` first, then
    /// the island's own parse over the rest of the document. Returns the expression and the
    /// offset just past its `}`, with the lexer advanced there.
    ///
    /// `Ok(None)`, nothing merged and the lexer untouched, for every island both readings
    /// refuse, which the caller then scans to its `}` and parses as the bounded slice, the
    /// path that owns every error. `accept` is the host's refusal of the parsed island
    /// (`{@render}` takes only a call): an island it refuses falls back the same way, so the
    /// bounded parse can run the refusal inside the island's parse
    /// ([`Self::parse_ts_expression_checked`]). `shape` is the bare path's own answer to it,
    /// since a bare island is pushed without asking.
    pub(crate) fn parse_head_to_brace(
        &mut self,
        expr_start: usize,
        shape: BareIsland,
        accept: impl Fn(&Self, AstView<'_>, ExprId) -> bool,
    ) -> Result<Option<(ExprId, usize)>, Error> {
        let nesting = self.island_nesting();
        if let Some((expression, close)) = tsv_ts::__private::parse_bare_name_closed(
            self.source,
            expr_start,
            b'}',
            shape,
            nesting,
            self.stores_mut(),
        ) {
            let end = close + 1;
            self.advance_to_position(end)?;
            return Ok(Some((expression, end)));
        }
        let Some(island) = self.with_island_stores(|this, stores| {
            tsv_ts::__private::parse_expression_to_brace(
                this.source,
                expr_start,
                this.island_nesting(),
                stores,
            )
        }) else {
            return Ok(None);
        };
        #[cfg(debug_assertions)]
        {
            assert_eq!(
                scan_to_matching_brace(self.source.as_bytes(), expr_start),
                Some(island.close),
                "a head parsed to its `}}` ends where the brace scan does not: {:?}",
                &self.source[expr_start..island.close]
            );
        }
        if !accept(self, self.ts_view(), island.expr) {
            return Ok(None);
        }
        self.merge_island_comments(island.comments);
        let end = island.close + 1;
        self.advance_to_position(end)?;
        Ok(Some((island.expr, end)))
    }
}

/// Where a head's expression starts: past each of `keywords` in turn at `at`, each followed
/// by a run of whitespace ([`SvelteParser::parse_keyword_head_to_brace`]) — the head scan's
/// own run ([`skip_head_ws`]), so the scan the debug assert checks against starts where this
/// reading does. `None` when a keyword is missing or no whitespace follows one.
fn head_expression_start(bytes: &[u8], mut at: usize, keywords: &[&[u8]]) -> Option<usize> {
    for keyword in keywords {
        if !bytes.get(at..)?.starts_with(keyword) {
            return None;
        }
        at += keyword.len();
        let run_start = at;
        at = skip_head_ws(bytes, at);
        if at == run_start {
            return None;
        }
    }
    Some(at)
}

/// Find the `}` that closes the construct opened by a `{` just before
/// `scan_start`, skipping nested braces, strings, line/block comments, regex
/// literals, and (interpolation-aware) template literals. `scan_start` is the
/// first byte to scan (the opening `{` is counted as depth 1). Returns the byte
/// offset of the matching `}`, or `None` if the braces never balance.
///
/// The single robust brace matcher shared by every `{…}` construct — expression
/// tags, `{@…}` tags, `{...spread}`, and block tags — so none reimplements it (and
/// weaker copies can't desync on a `}` inside a regex/comment/string/template).
///
/// A thin wrapper over `tsv_lang::source_scan::scan_to_matching_brace` (the shared
/// expression-context balanced-brace scanner, which the `${…}` template-interpolation
/// skip also uses) with `end = bytes.len()` and the answers to the operand ends bytes
/// cannot settle (`tsv_ts::__private::ACORN_ISLAND_GRAMMAR`), so the scan does not misread a `/`
/// and run past the island's `}`. Whether a `>` closes a type-argument list and whether a `)`
/// closes a statement header are answered as `tsv_ts`, which parses the island, reads them
/// (`f<T> / 2`, `if (c)!/re/`). Whether a `}` closes a braced operand (`{} / 2`) is answered as
/// acorn's tokenizer reads it, since Svelte hands every island's expression to acorn as a parse of
/// its own and the island ends where acorn says: a leading `function` there reads as a statement's
/// keyword, so `{function () {} / 2}` reads the `/` as a regex. The lexer's quoted-attribute arm
/// takes it too, so no brace scan in the crate can be handed a different rule.
#[inline]
pub(crate) fn scan_to_matching_brace(bytes: &[u8], scan_start: usize) -> Option<usize> {
    tsv_lang::source_scan::scan_to_matching_brace(
        bytes,
        scan_start,
        bytes.len(),
        tsv_ts::__private::ACORN_ISLAND_GRAMMAR,
    )
}

/// [`scan_to_matching_brace`]'s paren twin, `tsv_lang::source_scan::scan_to_matching_paren`
/// with the same grammar — for a paren that delimits an EXPRESSION, which may hold a regex
/// literal (`{#each xs as item (/[)]/.test(s))}`), unlike a binding pattern's brackets
/// (`match_bracket`). `scan_start` is the first byte inside the `(`.
pub(crate) fn scan_to_matching_paren(bytes: &[u8], scan_start: usize) -> Option<usize> {
    tsv_lang::source_scan::scan_to_matching_paren(
        bytes,
        scan_start,
        bytes.len(),
        tsv_ts::__private::ACORN_ISLAND_GRAMMAR,
    )
}

/// [`scan_to_matching_brace`] for a block or tag HEAD — `{#if …}`, `{:else if …}`,
/// `{#each …}`, `{@html …}`, `{@const …}`, `{@attach …}`, `{const …}` — whose content opens
/// on a keyword: the scan starts AFTER it, at [`head_keyword_end`].
///
/// The keyword is no operand, but a scan that began on it would read it as one — an
/// identifier byte before the first `/` — and take `{#if /}/.test(s)}`'s regex for a
/// division, `{#if !/}/.test(s)}`'s `!` for a postfix non-null, and `{#if <T>/}/…}`'s
/// assertion for a type-argument list, each ending the head at a `}` inside the regex.
/// Started past the keyword, the head's first `/` stands where an expression begins,
/// which is what it is to the parser that reads the head.
pub(crate) fn scan_head_to_matching_brace(bytes: &[u8], scan_start: usize) -> Option<usize> {
    let Some(from) = head_keyword_end(bytes, scan_start, ascii_head_ws_end) else {
        return scan_wide_head_to_matching_brace(bytes, scan_start);
    };
    scan_from_keyword_end(bytes, from)
}

/// [`scan_head_to_matching_brace`] for a head whose whitespace holds a non-ASCII character
/// before its keyword ends — read again from the start, every run in the full class.
/// Out of line and called last, so the common head's scan saves nothing for it.
#[cold]
#[inline(never)]
fn scan_wide_head_to_matching_brace(bytes: &[u8], scan_start: usize) -> Option<usize> {
    let from = head_keyword_end(bytes, scan_start, |bytes, i| Some(skip_head_ws(bytes, i)))?;
    scan_from_keyword_end(bytes, from)
}

/// The head scan past its keyword, from `from` ([`head_keyword_end`]).
#[inline]
fn scan_from_keyword_end(bytes: &[u8], from: usize) -> Option<usize> {
    // A head that is its keyword alone (`{/if}`, `{:else}`) closes right there — the
    // scan's own first answer, taken without entering it.
    if bytes.get(from) == Some(&b'}') {
        return Some(from);
    }
    scan_to_matching_brace(bytes, from)
}

/// Svelte's whitespace ASCII members — TAB, LF, VT, FF, CR and SPACE, the bytes `is_whitespace`
/// (`1-parse/utils/whitespace.js`) takes below U+0080 — one bit per byte value, so a head's
/// whitespace run asks each ASCII byte one question.
const ASCII_SVELTE_WS: u64 = 0b11_1110_0000_0000 | (1 << b' ');

/// The end of the whitespace run at `i` when an ASCII byte (or the end) ends it, or `None`
/// when a non-ASCII byte does, which may be whitespace too — the caller then reads the head
/// again with [`skip_head_ws`].
#[inline]
fn ascii_head_ws_end(bytes: &[u8], mut i: usize) -> Option<usize> {
    loop {
        match bytes.get(i) {
            Some(&b) if b <= b' ' && (ASCII_SVELTE_WS >> b) & 1 != 0 => i += 1,
            Some(&b) if !b.is_ascii() => return None,
            _ => return Some(i),
        }
    }
}

/// The end of the whitespace run at `i`: [`is_svelte_ws`](crate::whitespace::is_svelte_ws),
/// Svelte's `allow_whitespace` — the class every head reading past a keyword must share, or a
/// separator the canonical parser steps over (`{:else<VT>if …}`, a no-break space, a U+FEFF)
/// ends the run early and the keyword after it is scanned as an operand. A non-ASCII byte
/// hands the rest of the run to [`skip_wide_head_ws`].
#[inline]
fn skip_head_ws(bytes: &[u8], i: usize) -> usize {
    match ascii_head_ws_end(bytes, i) {
        Some(end) => end,
        None => skip_wide_head_ws(bytes, i),
    }
}

/// [`skip_head_ws`] over a run holding a non-ASCII character, decoding each one.
/// `bytes` is a document's text and `i` a character boundary.
#[cold]
#[inline(never)]
fn skip_wide_head_ws(bytes: &[u8], mut i: usize) -> usize {
    while let Some(&b) = bytes.get(i) {
        let width = match b {
            _ if b <= b' ' && (ASCII_SVELTE_WS >> b) & 1 != 0 => 1,
            0xC0..=0xDF => 2,
            0xE0..=0xEF => 3,
            0xF0..=0xF7 => 4,
            _ => break,
        };
        let is_ws = width == 1
            || core::str::from_utf8(&bytes[i..bytes.len().min(i + width)])
                .ok()
                .and_then(|c| c.chars().next())
                .is_some_and(crate::whitespace::is_svelte_ws);
        if !is_ws {
            break;
        }
        i += width;
    }
    i
}

/// Where a block or tag head's keyword ends: past leading whitespace and the keyword's
/// letters, and past a following `if` when the keyword is `else` (`{:else if …}`). A head
/// with no keyword ends where it began. Each whitespace run is `skip`'s — [`skip_head_ws`]'s
/// reading, or `None` where [`ascii_head_ws_end`] leaves a run to it.
#[inline]
fn head_keyword_end(
    bytes: &[u8],
    start: usize,
    skip: impl Fn(&[u8], usize) -> Option<usize>,
) -> Option<usize> {
    let skip_word = |mut i: usize| {
        while bytes.get(i).is_some_and(u8::is_ascii_alphabetic) {
            i += 1;
        }
        i
    };
    let word_start = skip(bytes, start)?;
    let word_end = skip_word(word_start);
    if &bytes[word_start..word_end] == b"else" {
        let if_start = skip(bytes, word_end)?;
        let if_end = skip_word(if_start);
        if &bytes[if_start..if_end] == b"if" {
            return Some(if_end);
        }
    }
    Some(word_end)
}

#[cfg(test)]
mod tests {

    fn parses(source: &str) -> bool {
        crate::__private::parse(source).is_ok()
    }

    /// Every island scan begins as a whole expression (`tsv_ts::__private::ACORN_ISLAND_GRAMMAR`):
    /// Svelte hands acorn each island as a fresh parse, so a leading `function` reads as a
    /// statement's keyword and the `/` after its body opens a regex, which here runs through the
    /// island's `}` to the closing tag's `/`. Each verdict is Svelte's.
    ///
    /// The positions are the scan's entry points: the plain brace scan (a text tag, an
    /// attribute or directive value, a quoted attribute's tag), the head scan past a
    /// keyword, a spread's scan past its `...` (and the whitespace Svelte allows before
    /// it), and an `{#each}` key's paren scan.
    #[test]
    fn an_island_scan_begins_as_a_whole_expression() {
        for (open, close) in [
            ("<div>{", "}</div>"),
            ("<div data-attr={", "}></div>"),
            ("<div data-attr=\"a {", "}\"></div>"),
            ("<div style:color={", "}></div>"),
            ("<div {...", "}></div>"),
            ("<div { ...", "}></div>"),
            ("<div {\n...", "}></div>"),
            ("{#if ", "}text{/if}"),
            ("{#if a}text{:else if ", "}text{/if}"),
            ("{#each ", " as item}text{/each}"),
            ("{#each xs as item (", ")}text{/each}"),
            ("{#key ", "}text{/key}"),
            ("{#await ", "}text{/await}"),
        ] {
            let island = |expression: &str| format!("{open}{expression}{close}");
            for leading in ["function () {} / 2", "class {} / 2"] {
                assert!(!parses(&island(leading)), "{}", island(leading));
            }
            for operand in ["(function () {}) / 2", "x + function () {} / 2", "{} / 2"] {
                assert!(parses(&island(operand)), "{}", island(operand));
            }
        }
    }

    /// A binding pattern's template literal is scanned as an island's is: an object
    /// literal's `}` in its interpolation ends an operand. Each verdict is Svelte's.
    #[test]
    fn a_binding_pattern_template_reads_a_brace_as_an_island_does() {
        for source in [
            "{#each xs as { a = `${{} / 2}` }}x{/each}",
            "{#each xs as { a = `${x + function () {} / 2}` }}x{/each}",
            "{#each xs as [a = `${{} / 2}`]}x{/each}",
            "{#await p then { a = `${{} / 2}` }}x{/await}",
        ] {
            assert!(parses(source), "{source}");
        }
    }

    /// A template literal nested in an interpolation, a `} /` at every level: each level is
    /// scanned once. A scan that asked again about a nested template's `} /` every time it
    /// passed the template would double its work per level, and this depth would not finish.
    #[test]
    fn nested_templates_are_scanned_once_per_level() {
        let mut expression = String::from("{} / 1");
        for _ in 0..40 {
            expression = format!("`${{{expression}}}` + {{}} / 1");
        }
        assert!(parses(&format!("<div>{{{expression}}}</div>")));
    }

    /// One island holding a great many `}` + `/` pairs: the scan asks about each and the
    /// walk behind the answers resumes from the last, so the island is walked once. Walked
    /// from its start at each pair, this many would not finish.
    #[test]
    fn an_island_of_many_braced_operands_is_scanned_once() {
        let elements = vec!["{} / 1"; 100_000].join(", ");
        assert!(parses(&format!("<div>{{[{elements}]}}</div>")));
    }

    /// The head run's byte mask and cold arm are Svelte's whitespace class, at every code point.
    #[test]
    fn the_head_run_is_svelte_whitespace() {
        for c in (0..=0x10_FFFF).filter_map(char::from_u32) {
            let source = format!("{c}x");
            let expected = if crate::whitespace::is_svelte_ws(c) {
                c.len_utf8()
            } else {
                0
            };
            assert_eq!(super::skip_head_ws(source.as_bytes(), 0), expected, "{c:?}");
        }
    }
}

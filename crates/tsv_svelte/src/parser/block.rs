// Control flow block parsing
//
// Handles: {#if}, {#each}, {#await}, {#key} blocks

use crate::ast::internal::*;
use crate::lexer::TokenKind;
use crate::parser::element::ParsedElement;
use crate::whitespace::is_svelte_ws;
use bumpalo::Bump;
use tsv_lang::source_scan::{TriviaProfile, skip_trivia_run};
use tsv_lang::{ParseError, Span};
use tsv_ts::{Expression, TopLevelAs};

use super::expression_tag::scan_to_matching_brace;
use super::parser_impl::{EmbeddedParseMark, SvelteParser};
use super::{match_bracket, subslice_offset};

/// Strip a **mid-head keyword separator** — optional leading whitespace, the keyword,
/// then a *required* whitespace run — returning the value that follows.
///
/// The separator every block head puts *between* its expression and a binding:
/// `{#each … as x}`, `{#await … then v}`, `{#await … catch e}`. Mirrors canonical's
/// `allow_whitespace()` / `eat(kw)` / `require_whitespace()`, so `(items)as x` is a
/// separator (no leading whitespace needed) while `as/* c */x` and a head-final `as` are
/// not. A separator with nothing after it (`as `) still matches — canonical's
/// `require_whitespace` is satisfied there and it is `read_pattern` that reports the
/// missing binding, so the empty value is the caller's error rather than a non-match.
///
/// Both whitespace runs are of **any width**, not the single spaces the canonical form
/// happens to print: an authored break or tab is legal in either, and so is the wrapped
/// head tsv's own formatter emits (`items as A[]⏎as item}`) — which a single-space match
/// rejected, leaving tsv unable to re-parse its own output. The class is [`is_svelte_ws`],
/// the `\s` canonical itself tests, not Rust's `trim_start`, whose Unicode `White_Space`
/// set disagrees at `U+0085` and `U+FEFF`.
///
/// [`SvelteParser::strip_keyword_value`] is the *leading*-keyword counterpart — same rule
/// one position earlier, where a missing keyword is an error rather than a `None`.
fn strip_head_keyword<'s>(rest: &'s str, keyword: &str) -> Option<&'s str> {
    let after_kw = rest
        .trim_start_matches(is_svelte_ws)
        .strip_prefix(keyword)?;
    let value = after_kw.trim_start_matches(is_svelte_ws);
    (value.len() < after_kw.len()).then_some(value)
}

/// The words of a `{:...}` continuation run, split on [`is_svelte_ws`] — Svelte's class,
/// not Rust's.
///
/// ⚠️ NOT `str::split_whitespace`, which splits on `char::is_whitespace` (Unicode
/// White_Space). That set lacks **U+FEFF**, so it welds `else<ZWNBSP>if` into a single word
/// and loses the `{:else if}` match that [`SvelteParser::continuation_keyword_at`]'s own cut
/// just took care to preserve — the two would then disagree about where a word ends, which is
/// the bug the shared class exists to rule out. It also *adds* U+0085 NEL, which Svelte reads
/// as junk rather than as a separator.
///
/// Empty pieces are dropped so a run of separators (`{:else  if}`) yields two words, matching
/// `split_whitespace`'s one useful property.
fn continuation_words(keyword: &str) -> impl Iterator<Item = &str> {
    keyword.split(is_svelte_ws).filter(|w| !w.is_empty())
}

/// [`strip_head_keyword`], plus the **head-final** form the keyword-with-no-value case
/// needs: `{#await p then}` names its clause and stops, so nothing follows the keyword for
/// the required whitespace run to be found in.
///
/// The `{#await}` spelling of the same question `{#each}` asks with `strip_head_keyword`
/// alone, and the difference is the whole reason both exist: `{#each xs as}` must be
/// REJECTED (canonical's `require_whitespace` after `eat('as')` fails, and it is the `as`
/// that would otherwise bind an absent pattern), while `{#await p then}` is valid — its
/// clause simply has no binding. The caller's `content` stops before the head's `}`
/// (`scan_block_tag_content`), so "ends the head" is just "nothing but whitespace left".
///
/// Returns the same empty value the with-whitespace form does (`{#await p then }`), so the
/// caller reads one answer rather than distinguishing two spellings of "no binding".
fn strip_head_keyword_or_final<'s>(rest: &'s str, keyword: &str) -> Option<&'s str> {
    strip_head_keyword(rest, keyword).or_else(|| {
        let after_kw = rest
            .trim_start_matches(is_svelte_ws)
            .strip_prefix(keyword)?;
        after_kw
            .trim_start_matches(is_svelte_ws)
            .is_empty()
            .then_some("")
    })
}

/// The two `{#await}` clauses — the phases `{:then}` / `{:catch}` name, and that a
/// shorthand head (`{#await p then v}`) fills in advance. Each fills at most one
/// [`AwaitSlot`], which is what [`SvelteParser::parse_await_block`]'s continuation loop
/// guards.
#[derive(Clone, Copy)]
enum AwaitClause {
    Then,
    Catch,
}

impl AwaitClause {
    /// The keyword as it is spelled in both the head (`{#await p then v}`) and the
    /// continuation (`{:then v}`) — one spelling drives the scan, the parse and the
    /// duplicate error.
    fn keyword(self) -> &'static str {
        match self {
            Self::Then => "then",
            Self::Catch => "catch",
        }
    }
}

/// What ends any `{#await}` body — the pending phase's and each clause's alike. An
/// unrecognized continuation ends a body regardless (`parse_block_children` breaks on
/// one), so the list is not per-clause; the continuation loop is what decides whether
/// the one that ended it is legal there.
const AWAIT_BODY_STOPS: &[&str] = &["then", "catch", "await"];

/// What one `{#await}` clause holds: its fragment and its binding pattern, kept together
/// so filling a clause and asking whether it is filled cannot name different slots.
type AwaitSlot<'arena> = (Option<Fragment<'arena>>, Option<Expression<'arena>>);

/// The TypeScript assertion keywords, each paired with whether it can BE a `{#each}`
/// binding separator. Both continue an assertion chain; only `as` is spelled the way
/// Svelte spells its separator. Neither prefix can shadow the other (`satisfies` does not
/// start with `as`), so the order here is immaterial.
const ASSERTION_KEYWORDS: [(&str, bool); 2] = [("as", true), ("satisfies", false)];

/// Step over a run of whitespace and comments at the start of `s`.
///
/// The gap between a type assertion and the `{#each}` binding separator can hold both
/// (`items as A[] /* c */ as item`, which canonical Svelte accepts), and a type's own
/// `span.end` deliberately stops *before* trailing trivia — so the separator scan has to
/// cross it explicitly. Comments only, never strings: nothing but whitespace and comments
/// can sit between a complete type and the token after it.
fn skip_ws_and_comments(s: &str) -> &str {
    &s[skip_trivia_run(s, 0, TriviaProfile::COMMENTS, is_svelte_ws)..]
}

/// What a `{#each}` head's assertion run says about its binding — the three answers
/// canonical's unwind can give, which strips the head expression's **outermost** node and
/// only when that node is a `TSAsExpression`.
///
/// Every offset is ABSOLUTE, so the caller indexes `content` by one rule (subtract its
/// offset) instead of summing the head's parts a second time.
#[derive(Debug, PartialEq, Eq)]
enum EachHeadSplit {
    /// The head's first `as` — the one the caller already stripped — is the binding
    /// separator: `{#each xs as item}`.
    FirstAs,
    /// A LATER `as` is: `{#each xs as A[] as item}`. `separator` is where that keyword
    /// begins and `binding` where the binding after it does, with nothing but the keyword
    /// and its whitespace between them.
    LaterAs { separator: usize, binding: usize },
    /// The run holds no separator at all — it ends on `satisfies`, so the whole run is
    /// the iterable and the head has NO binding, the same shape a head with no `as` at
    /// all produces: `{#each xs as A satisfies B}`, which canonical reads as a
    /// binding-less each block. `run_end` is where the run's last type ends.
    NoBinding { run_end: usize },
}

/// Split a `{#each}` head's assertion run, given `s` — the head text that follows its
/// FIRST `as`, at absolute offset `s_offset`.
///
/// `{#each xs as A[] as item}` reads as one assertion chain, and only the **type grammar**
/// can say where TypeScript's assertions stop and Svelte's binding begins — so this walks
/// the chain by parsing a type after each assertion keyword. It stops at the first item
/// that is not a type, which is precisely the binding: a destructuring default
/// (`{#each xs as A[] as { a = 1 }}`) is a pattern and no type, so the `as` before it is
/// Svelte's.
///
/// ⚠️ The separator is the run's **LAST** keyword, and only when that keyword is `as` —
/// *not* the last `as` anywhere in the run. Canonical unwinds the head by stripping the
/// expression's outermost node and only when it is a `TSAsExpression`, so a `satisfies`
/// **cancels** every `as` before it: `{#each xs as A[] as item satisfies B}` has no
/// binding at all — `item` is a type — where "keep the last `as`" hands `item` to the
/// binding parser and rejects a head canonical accepts.
///
/// ⚠️ **Both** assertion keywords continue the chain; only `as` can BE the separator. The
/// chain is `as`/`satisfies` interleaved (`xs as A satisfies B as item`), and a walk that
/// recognized `as` alone stopped dead at the first `satisfies`, handing its whole tail to
/// the binding parser. That is an over-rejection the byte scan this replaced did *not*
/// have — a scan looking for the last ` as ` steps over an intervening `satisfies` without
/// having to know it exists — so it is the one axis where modelling the grammar costs
/// something, and the cost is paid by naming both keywords rather than by scanning.
///
/// ⚠️ Asking the type grammar is the point, and a bracket-depth byte scan cannot replace
/// it. `<`/`>` are not a bracket pair, so an arrow's `>` closes a depth nothing opened:
/// a signed depth counter goes **negative** at `A<() => string>`, and every later candidate
/// dies against a `depth == 0` test that can no longer be true — rejecting `{#each fns as
/// A<() => string> as item}`. A mapped type is the same question one step subtler: its
/// `[K in T as U]` spells an `as` that is no separator, and it survives a depth scan only
/// because brackets happen to nest it — the type grammar knows it outright.
///
/// The probe is read-only: it collects no comments and keeps no node (see
/// [`tsv_ts::parse_type_extent`]) — the caller re-reads both regions with the real
/// iterable and binding parses, so a probe that registered anything would double it.
fn each_binding_separator(s: &str, s_offset: usize, arena: &Bump) -> EachHeadSplit {
    // Every position the walk names is a subslice of `s`, so one rule converts them all
    // and no offset is ever summed from parts. Via [`subslice_offset`] rather than a
    // length subtraction, which is the same value only while the slice stays a SUFFIX —
    // a precondition each site would have to re-establish, and that a `trim_matches`
    // added later silently breaks.
    let offset_of = |part: &str| s_offset + subslice_offset(s, part);

    // The run's last keyword, as a separator: `Some` while that keyword is an `as`, reset
    // to `None` by a `satisfies` after it.
    let mut separator = None;
    // Where the run ends, set ONLY when the walk stops on a complete type — i.e. the head
    // ran out of items rather than reaching one that is a pattern. Only then can the
    // answer be `NoBinding`; an item that is no type IS the binding.
    let mut run_end = None;
    // Whether the walk consumed a keyword of its own, which is what tells a run that
    // ENDED on `satisfies` from a head whose first `as` was already the separator:
    // `{#each xs as item}` also walks one type and finds no keyword after it, and its
    // `item` is the binding, not a run end.
    let mut linked = false;
    let mut item = s;
    loop {
        // No type here ⇒ this item is the binding, and the keyword before it is the
        // separator — the one recorded below, or the head's first `as` when none is.
        let item_at = offset_of(item);
        let Ok(type_end) = tsv_ts::parse_type_extent(item, item_at, arena) else {
            break;
        };
        let after_type = skip_ws_and_comments(&item[type_end - item_at..]);
        // An assertion keyword after a complete type makes that type an assertion and the
        // chain continue; nothing else can, so anything else ends the run on this type.
        let Some((is_separator, rest)) =
            ASSERTION_KEYWORDS.iter().find_map(|&(kw, is_separator)| {
                strip_head_keyword(after_type, kw).map(|rest| (is_separator, rest))
            })
        else {
            run_end = Some(type_end);
            break;
        };
        // The LAST keyword decides, so `satisfies` clears the candidate rather than being
        // skipped over — see the outermost-node rule above.
        separator = is_separator.then(|| (offset_of(after_type), offset_of(rest)));
        linked = true;
        // `strip_head_keyword` consumed the keyword plus a required whitespace run, so
        // `rest` is strictly shorter than `item` and the walk always terminates.
        item = rest;
    }

    match (separator, run_end) {
        (Some((separator, binding)), _) => EachHeadSplit::LaterAs { separator, binding },
        (None, Some(run_end)) if linked => EachHeadSplit::NoBinding { run_end },
        _ => EachHeadSplit::FirstAs,
    }
}

/// Return type for parse_each_binding: (context, index, key, consumed_end).
/// `consumed_end` is the absolute source offset just past the last token the binding
/// consumed — the caller rejects any non-whitespace between it and the closing `}`.
type EachBindingResult<'arena> = (
    Expression<'arena>,
    Option<&'arena str>,
    Option<EachKey<'arena>>,
    usize,
);

/// Return type for parse_index_and_key_after_context: (index, key, consumed_end).
type IndexAndKeyResult<'arena> = (Option<&'arena str>, Option<EachKey<'arena>>, usize);

impl<'a, 'arena> SvelteParser<'a, 'arena> {
    /// Parse a control flow block starting with {#
    ///
    /// Dispatches to specific block parsers based on the keyword.
    pub(crate) fn parse_block(&mut self) -> Result<FragmentNode<'arena>, ParseError> {
        let start = self.current_start;

        // We're at {#, consume it
        if !self.check(TokenKind::BlockOpen) {
            return Err(self.error_expected_found("'{#'"));
        }

        // After {# we expect a block keyword: if, each, await, key, snippet
        let keyword = self.keyword_at(self.current_end);

        match keyword {
            "if" => self.parse_if_block(start),
            "each" => self.parse_each_block(start),
            "await" => self.parse_await_block(start),
            "key" => self.parse_key_block(start),
            "snippet" => self.parse_snippet_block(start),
            _ => Err(self.error_unknown_at("block type", &format!("{{#{keyword}}}"), start)),
        }
    }

    /// Parse an if block: {#if test}...{:else if test}...{:else}...{/if}
    fn parse_if_block(&mut self, start: usize) -> Result<FragmentNode<'arena>, ParseError> {
        self.parse_if_block_inner(start, false)
    }

    /// Inner parser for if blocks (handles both {#if} and {:else if})
    fn parse_if_block_inner(
        &mut self,
        start: usize,
        is_elseif: bool,
    ) -> Result<FragmentNode<'arena>, ParseError> {
        // Get the content start position (after {# or {:)
        let tag_content_start = self.current_end;

        // Scan to find closing } and extract content
        let (expr_content, content_start) = self.scan_block_tag_content(tag_content_start)?;

        // Extract the expression (skip "if " or "else if " prefix, handling
        // variable whitespace). Svelte requires whitespace after the `if`
        // keyword, so `{#if(x)}` / `{:else if(x)}` are rejected.
        let expr_str = if is_elseif {
            // {:else if expr} - skip "else", whitespace, "if", whitespace
            let after_else = expr_content
                .strip_prefix("else")
                .unwrap_or(expr_content)
                .trim_start_matches(is_svelte_ws);
            self.strip_block_keyword(after_else, "if", tag_content_start)?
                .trim_start_matches(is_svelte_ws)
        } else {
            // {#if expr} - skip "if", whitespace
            self.strip_block_keyword(expr_content, "if", tag_content_start)?
                .trim_start_matches(is_svelte_ws)
        };

        // Parse the test expression (with comments)
        let expr_offset = tag_content_start + subslice_offset(expr_content, expr_str);

        let test = self.parse_ts_expression(expr_str, expr_offset)?;

        // Opening tag span is from start to content_start (includes the closing })
        let opening_tag_span = Span {
            start: start as u32,
            end: content_start as u32,
        };

        // Parse consequent (content until {:else}, {:else if}, or {/if})
        let consequent = self.parse_block_children(&["else", "if"], content_start)?;

        // Check for alternate branch
        let alternate = if self.check(TokenKind::BlockContinue) {
            // Peek at what follows {:. Match the first two whitespace-delimited
            // words allocation-free (the old `.take(2).join(" ")` normalized
            // "else  if" -> "else if" only to compare against these two forms).
            let keyword = self.continuation_keyword_at(self.current_end);
            let mut words = continuation_words(keyword);
            let first = words.next();
            let second = words.next();
            let is_else_if = first == Some("else") && second == Some("if");
            let is_else = first == Some("else") && second.is_none();

            if is_else_if {
                // {:else if} - parse as nested if block
                let elseif_start = self.current_start;
                let elseif_block = self.parse_if_block_inner(elseif_start, true)?;
                let mut nodes = self.bvec();
                nodes.push(elseif_block);
                Some(Fragment {
                    nodes: nodes.into_bump_slice(),
                })
            } else if is_else {
                // {:else} - parse else branch
                let else_tag_start = self.current_end;
                let (else_tag_content, else_content_start) =
                    self.scan_block_tag_content(else_tag_start)?; // consume "else}"
                // Only whitespace may follow `else` before the `}` — Svelte's
                // `allow_whitespace` then `eat('}')`. `continuation_keyword_at` cannot
                // answer this: it stops the run at the first char that is neither
                // alphabetic nor a SPACE, so a U+0085 (whitespace to Rust, not to JS `\s`)
                // ended the run at `else` and was then silently dropped.
                let after_else = &else_tag_content["else".len()..];
                self.reject_trailing_tag_content(after_else, else_tag_start + "else".len())?;
                let else_content = self.parse_block_children(&["if"], else_content_start)?;
                self.reject_duplicate_else()?;
                Some(else_content)
            } else {
                None
            }
        } else {
            None
        };

        // Determine end position
        // For {:else if}, the nested IfBlock already consumed {/if}, so use its end position
        // For all other cases (no alternate, {:else}, {:else}{#if}), consume {/if} ourselves
        let elseif_end = alternate.as_ref().and_then(|alt| {
            if let Some(FragmentNode::IfBlock(inner)) = alt.nodes.first()
                && inner.elseif
            {
                return Some(inner.span.end as usize);
            }
            None
        });

        let end = if let Some(end_pos) = elseif_end {
            end_pos
        } else {
            self.expect_block_close_keyword("if", start)?
        };

        Ok(FragmentNode::IfBlock(IfBlock {
            elseif: is_elseif,
            test,
            consequent,
            alternate,
            span: Span {
                start: start as u32,
                end: end as u32,
            },
            opening_tag_span,
        }))
    }

    /// Re-parse a `{#each}` iterable over `iterable` — the wider slice the head's
    /// assertion run turned out to cover, once [`each_binding_separator`] has said where
    /// the run really ends.
    ///
    /// The slice STARTS where the partial parse did and reaches further, so it re-registers
    /// everything that parse registered — rewinding to `mark` first is what keeps each
    /// registration single. Without it a comment here was listed twice in the root
    /// `comments` array and printed twice by whichever emitter owned its gap, and the
    /// iterable's `AcornRegion` was pushed twice. Both re-parsing arms share this, so the
    /// rewind cannot be done at one and forgotten at the other.
    fn reparse_each_iterable(
        &mut self,
        iterable: &str,
        expr_offset: usize,
        mark: EmbeddedParseMark,
    ) -> Result<Expression<'arena>, ParseError> {
        self.rewind_embedded_parses(mark);
        // Leading whitespace only, and `expr_offset` is already the first non-whitespace byte:
        // this slice ends at the head's SECOND `as`, so its trailing run may be a line
        // comment's own text (`Parser::parse_ts_expression`).
        self.parse_ts_expression(iterable.trim_start_matches(is_svelte_ws), expr_offset)
    }

    /// Parse an each block: {#each expression as context, index (key)}...{:else}...{/each}
    fn parse_each_block(&mut self, start: usize) -> Result<FragmentNode<'arena>, ParseError> {
        // Get the content start position (after {#)
        let tag_content_start = self.current_end;

        // Scan to find closing } and extract content
        let (tag_content, content_start) = self.scan_block_tag_content(tag_content_start)?;

        // Parse: "each expression as context, index (key)" — Svelte requires
        // whitespace after the keyword. The remainder keeps its leading
        // whitespace; `content_offset` points just past the keyword and the
        // `trim_start()` below recovers the expression's exact offset.
        let content = self.strip_block_keyword(tag_content, "each", tag_content_start)?;
        let content_offset = tag_content_start + subslice_offset(tag_content, content);

        // Use partial parsing for the iterable expression, which is also what keeps
        // `getItems(" as ")` from splitting on the `as` inside its string.
        //
        // A top-level `as` is the HOST's here, unlike `{#await}`: this head's separator IS
        // `as`, so the keyword has to survive the parse for the split below to find it
        // ([`TopLevelAs`]) — and giving it away is what obliges that split to walk the
        // assertion run itself (`each_binding_separator`). `satisfies` is not in question
        // and stays TypeScript's, so `{#each xs satisfies A[] as item}` parses here.
        //
        // The mark is what the type-assertion branch below rewinds the embedded-parse ledgers
        // to before re-parsing the same region — see `reparse_each_iterable`; `expr_offset` is
        // where BOTH parses of the iterable start, so it is derived once rather than twice.
        let mark_before_expr = self.embedded_parse_mark();
        let expr_str = content.trim_start_matches(is_svelte_ws);
        let expr_offset = content_offset + subslice_offset(content, expr_str);
        let (expression, expr_end_pos) =
            self.parse_ts_expression_partial(expr_str, expr_offset, TopLevelAs::HostSeparator)?;

        // Opening tag span is from start to content_start (includes the closing })
        let opening_tag_span = Span {
            start: start as u32,
            end: content_start as u32,
        };

        // After the expression, check for the `as` separator, `, index`, or just the head end
        let after_expr = &content[expr_end_pos - content_offset..];

        // Where the binding is, if this head has one at all. A SECOND `as` in the binding
        // means the first was a TypeScript type assertion (`items as A[] as item`), not
        // the Svelte binding separator; a run ENDING on `satisfies` (`items as A satisfies
        // B`) means there is no binding at all and the whole run is the iterable — see
        // [`EachHeadSplit`]. Only the slice each side owns differs between the cases; the
        // binding is read once whichever it is.
        let mut expression = expression;
        let binding = match strip_head_keyword(after_expr, "as") {
            None => None,
            Some(binding_str) => {
                // `binding_str` is a suffix of `content`, so its offset follows the same
                // one rule the separator walk uses — and `content[abs - content_offset]`
                // is how every absolute position below indexes back into the head.
                let binding_offset = content_offset + subslice_offset(content, binding_str);
                match each_binding_separator(binding_str, binding_offset, self.arena) {
                    EachHeadSplit::FirstAs => Some((binding_str, binding_offset)),
                    EachHeadSplit::LaterAs { separator, binding } => {
                        let iterable = &content[..separator - content_offset];
                        expression =
                            self.reparse_each_iterable(iterable, expr_offset, mark_before_expr)?;
                        Some((&content[binding - content_offset..], binding))
                    }
                    EachHeadSplit::NoBinding { run_end } => {
                        let iterable = &content[..run_end - content_offset];
                        expression =
                            self.reparse_each_iterable(iterable, expr_offset, mark_before_expr)?;
                        None
                    }
                }
            }
        };

        let (context, index, key, binding_end) = match binding {
            Some((binding_str, binding_offset)) => {
                let (ctx, idx, k, b_end) = self.parse_each_binding(binding_str, binding_offset)?;
                (Some(ctx), idx, k, b_end)
            }
            None => {
                // No binding: the remainder is the optional `, index` and/or `(key)` —
                // the same grammar as after a context, just without one — so route it through
                // the shared parser (context stays `None`). Svelte allows index/key without
                // `as` (`{#each items, i}`, `{#each items, i (key)}`); the shared parser also
                // bounds the key with the trivia-aware bracket scanner and reports a precise
                // `consumed_end`, so trailing junk is rejected below instead of swallowed.
                // A `satisfies`-terminated run reaches here too, which is the point of
                // routing both through one tail: the two heads produce one shape.
                //
                // Read from the expression's SEMANTIC end, not `expr_end_pos` (the partial
                // parser's stop, which swallows a trailing comment as trivia) — mirroring
                // Svelte's `parser.index = expression.end`. This way a comment after the
                // iterable with no binding (`{#each items /* c */}`) becomes trailing junk
                // rejected below, not a silently kept comment. (The binding branch keeps
                // using `after_expr` so a comment *before* `as` — `{#each items /* c */ as
                // item}`, which Svelte accepts — still resolves to the binding.)
                let semantic_end = expression.span().end as usize;
                let after_semantic = &content[semantic_end - content_offset..];
                let (idx, k, b_end) =
                    self.parse_index_and_key_after_context(after_semantic, semantic_end)?;
                (None, idx, k, b_end)
            }
        };

        // The opening tag must end at `}` immediately after the binding (Svelte's final
        // `eat('}')`): only whitespace may remain. A stray comment, leftover index/key
        // fragment, or junk here is rejected — not silently dropped (content loss).
        let brace_pos = content_start - 1;
        self.reject_trailing_tag_content(&self.source[binding_end..brace_pos], binding_end)?;

        // Parse body
        let body = self.parse_block_children(&["else", "each"], content_start)?;

        // Check for fallback. Only `{:else}` is an each continuation — any other
        // `{:keyword}` (e.g. `{:catch}`, `{:then}`) is left unconsumed so it
        // surfaces as an orphan-continuation error, matching the canonical parser.
        let fallback = if self.check(TokenKind::BlockContinue)
            && self.continuation_keyword_at(self.current_end) == "else"
        {
            let else_tag_start = self.current_end;
            let (_, else_content_start) = self.scan_block_tag_content(else_tag_start)?; // consume "else}"
            let fallback_content = self.parse_block_children(&["each"], else_content_start)?;
            self.reject_duplicate_else()?;
            Some(fallback_content)
        } else {
            None
        };

        // Expect closing {/each}
        let end = self.expect_block_close_keyword("each", start)?;

        Ok(FragmentNode::EachBlock(EachBlock {
            expression,
            context,
            index,
            key,
            body,
            fallback,
            span: Span {
                start: start as u32,
                end: end as u32,
            },
            opening_tag_span,
        }))
    }

    /// Parse each binding: "context, index (key)" using the TypeScript expression parser.
    ///
    /// Uses partial expression parsing which correctly handles:
    /// - Simple identifiers: `item`
    /// - Object destructuring: `{ a, b }` (commas inside braces don't split)
    /// - Array destructuring: `[a, b]` (commas inside brackets don't split)
    /// - Strings with brackets: `{ a: "}" }` (braces inside strings don't count)
    /// - Template literals: `` { a: `${x}` } ``
    ///
    /// The TS parser stops at top-level commas, so `{ a, b }, i` parses `{ a, b }` and leaves `, i`.
    ///
    /// Returns (context, index, key); the key carries the span of its parentheses.
    fn parse_each_binding(
        &mut self,
        binding: &str,
        binding_offset: usize,
    ) -> Result<EachBindingResult<'arena>, ParseError> {
        // Calculate leading whitespace and adjust offset accordingly
        let leading_ws = binding.len() - binding.trim_start_matches(is_svelte_ws).len();
        // A trailing trim here cannot clip a line comment the way a head's does
        // (`Parser::parse_ts_expression`): every sub-parse below is bounded by the grammar —
        // the pattern by its bracket or identifier run, the annotation by its own end — and a
        // comment trailing the annotation is REJECTED, as canonical rejects it.
        let trimmed = binding.trim_matches(is_svelte_ws);
        let adjusted_offset = binding_offset + leading_ws;

        // Parse context as a PATTERN (like Svelte does), not as expression
        // Patterns are: identifiers OR destructuring {..}/[..]
        // This naturally stops at whitespace/comma/paren, avoiding the
        // `item (key)` being parsed as a function call
        let (context, pattern_end) = self.parse_context_pattern(trimmed, adjusted_offset)?;

        // Parse remaining: ", index" and/or "(key)"
        let consumed = pattern_end - adjusted_offset;
        let remaining = &trimmed[consumed..];
        let (index, key, consumed_end) =
            self.parse_index_and_key_after_context(remaining, pattern_end)?;

        Ok((context, index, key, consumed_end))
    }

    /// Parse a context pattern: identifier or destructuring pattern, each with an
    /// optional `: Type` annotation (`{#each xs as x: T}`,
    /// `{#each xs as { a }: T}`).
    ///
    /// Like Svelte's `read_pattern`, this stops at whitespace/comma/paren for
    /// identifiers and at the matching bracket for a destructuring pattern — the
    /// bound slice is what keeps `{#each xs as { a } (a.id)}` from parsing
    /// `{ a } (a.id)` as a *call*. The annotation therefore has to be consumed
    /// here, after the pattern, rather than by handing the whole remainder to the
    /// expression parser.
    ///
    /// `tsv_ts::attach_pattern_type_annotation` owns the span convention — one
    /// definition, so the two Svelte block readers can't drift. It leaves the span
    /// on the **bare** binding for every kind (only a destructuring pattern's wire
    /// `end` widens at emit time — the one kind whose byte range and `loc` genuinely
    /// disagree; an annotated identifier stays bare on the wire too, per Svelte's
    /// `read_pattern`), so `pattern.span().end` is the bare end and
    /// `tsv_ts::pattern_binding_end` the end past any annotation. A reader that
    /// needs the gap between them — the trailing-comment gates — must ask for both.
    fn parse_context_pattern(
        &mut self,
        input: &str,
        offset: usize,
    ) -> Result<(Expression<'arena>, usize), ParseError> {
        let trimmed = input.trim_start_matches(is_svelte_ws);
        let ws_len = input.len() - trimmed.len();
        let adjusted = offset + ws_len;

        // The pattern's extent, bounded so the annotation (and any `, index` /
        // `(key)` tail) stays out of the expression parse.
        // This split IS canonical's: `read_pattern` (`1-parse/read/context.js`) opens with
        // `parser.read_identifier()` and only falls to acorn when that reads nothing — i.e.
        // on the `{`/`[` destructuring branch. So the identifier arm goes through the same
        // reader (reserved-word rule included) and the bracket arm keeps the deferral.
        let end = if trimmed.starts_with('{') || trimmed.starts_with('[') {
            self.find_matching_bracket(trimmed)?
        } else {
            let Some(name) = self.read_identifier(trimmed, adjusted)? else {
                return Err(self.error_expected_at("identifier or pattern", offset));
            };
            name.len()
        };
        // `parse_ts_pattern` yields ObjectPattern/ArrayPattern (not the Object/Array
        // *Expression* the plain expression parser would).
        let mut expr = self.parse_ts_pattern(&trimmed[..end], adjusted)?;

        let after_pattern = trimmed[end..].trim_start_matches(is_svelte_ws);
        if !after_pattern.starts_with(':') {
            return Ok((expr, adjusted + end));
        }
        let ws_before_colon = trimmed.len() - end - after_pattern.len();
        let colon_offset = adjusted + end + ws_before_colon;
        // The annotation is its own sub-parse of the host document, so its comments
        // exist nowhere else — dropping them dropped the comment outright
        // (`{#each xs as x: /* c */ T}`), and with no registration the print-once
        // ledger could not see the loss either. `parse_ts_type_annotation` collects
        // them the way every sibling sub-parse does; they keep their true columns,
        // since the synthetic-`(` shift belongs to the destructure parse alone (see
        // `parse_ts_pattern`).
        let ta = self.parse_ts_type_annotation(after_pattern, colon_offset)?;
        // The consumed extent is the ANNOTATION's own end. Reporting where the
        // sub-parse's lexer stopped hands the tail back with a trailing comment
        // silently eaten — canonical Svelte rejects one there
        // (`{#each xs as x: T /* c */}` → `expected_token`), and accepting it here
        // would drop the comment.
        let annotation_end = ta.span.end as usize;
        tsv_ts::attach_pattern_type_annotation(&mut expr, ta, self.arena)?;
        Ok((expr, annotation_end))
    }

    /// Find the matching closing bracket for a string starting with `{` or `[`,
    /// returning the byte offset just past the close (so `&input[..end]` is the whole
    /// bracketed run). Comment- and string-aware via the shared cursor.
    fn find_matching_bracket(&self, input: &str) -> Result<usize, ParseError> {
        let bytes = input.as_bytes();
        let (open, close) = match bytes.first() {
            Some(b'{') => (b'{', b'}'),
            Some(b'[') => (b'[', b']'),
            _ => {
                return Err(ParseError::invalid_syntax("Expected { or [".to_string(), 0));
            }
        };

        match_bracket(bytes, 0, bytes.len(), open, close, TriviaProfile::JS)
            .map(|close_pos| close_pos + 1) // include the closing bracket
            .ok_or_else(|| ParseError::invalid_syntax("Unmatched bracket".to_string(), 0))
    }

    /// Parse ", index" and/or "(key)" after the context pattern
    ///
    /// Returns (index, key); the key carries the span of its parentheses.
    fn parse_index_and_key_after_context(
        &mut self,
        remaining: &str,
        remaining_offset: usize,
    ) -> Result<IndexAndKeyResult<'arena>, ParseError> {
        let trimmed = remaining.trim_start_matches(is_svelte_ws);
        let ws_len = remaining.len() - trimmed.len();
        let offset = remaining_offset + ws_len;

        let mut rest = trimmed;
        let mut rest_offset = offset;
        let mut index = None;
        // Absolute offset just past the last token the binding consumed. Starts at the
        // context end (`remaining_offset`): with no index/key the binding ends there, so
        // everything in `remaining` is trailing and the caller rejects it.
        let mut consumed_end = remaining_offset;

        // Check for ", index" — the index is a bare identifier (Svelte's `read_identifier`).
        if let Some(after_comma) = rest.strip_prefix(',') {
            let after_comma_trimmed = after_comma.trim_start_matches(is_svelte_ws);
            let comma_ws = after_comma.len() - after_comma_trimmed.len();

            // The index is a bare identifier read the way Svelte reads it — so a reserved
            // word (`{#each x as y, if}`) is rejected here rather than stored. A non-start
            // (`, /* c */ i`, `, 5`) leaves the index unread and the comma unconsumed, so
            // the caller's trailing check reports it, matching Svelte's "Expected an
            // identifier".
            let idx_offset = offset + 1 + comma_ws;
            if let Some(name) = self.read_identifier(after_comma_trimmed, idx_offset)? {
                index = Some(self.alloc_str_in(name));
                rest = &after_comma_trimmed[name.len()..];
                rest_offset = idx_offset + name.len();
                consumed_end = rest_offset;
            }
        }

        // Check for "(key)" — match the `)` with the trivia-aware bracket scanner so a
        // `)` inside a string/comment in the key can't end it early, and any trailing
        // junk after the real `)` is left for the caller's trailing check (not swallowed).
        let rest_trimmed = rest.trim_start_matches(is_svelte_ws);
        let key = if rest_trimmed.starts_with('(') {
            let key_ws = rest.len() - rest_trimmed.len();
            let paren_start = rest_offset + key_ws; // absolute offset of '('
            let close = match_bracket(
                rest_trimmed.as_bytes(),
                0,
                rest_trimmed.len(),
                b'(',
                b')',
                TriviaProfile::JS,
            )
            .ok_or_else(|| self.error_expected_at("')'", paren_start + rest_trimmed.len()))?;
            let key_str = &rest_trimmed[1..close];
            let key_offset = paren_start + 1; // after '('
            // Leading whitespace only — the trailing run may be a line comment's own text
            // (`Parser::parse_ts_expression`).
            let key_expr_str = key_str.trim_start_matches(is_svelte_ws);
            let key_expr = self.parse_ts_expression(
                key_expr_str,
                key_offset + (key_str.len() - key_expr_str.len()),
            )?;
            // Span includes the parentheses: from '(' to after ')'.
            let span = Span::new(paren_start as u32, (paren_start + close + 1) as u32);
            consumed_end = span.end as usize;
            Some(EachKey {
                expression: key_expr,
                span,
            })
        } else {
            None
        };

        Ok((index, key, consumed_end))
    }

    /// Parse an await block: {#await expression}...{:then value}...{:catch error}...{/await}
    fn parse_await_block(&mut self, start: usize) -> Result<FragmentNode<'arena>, ParseError> {
        // Get the content start position (after {#)
        let tag_content_start = self.current_end;

        // Scan to find closing } and extract content
        let (tag_content, content_start) = self.scan_block_tag_content(tag_content_start)?;

        // Parse: "await expression" or "await expression then value" — Svelte
        // requires whitespace after the keyword. The remainder keeps its leading
        // whitespace; `content_offset` points just past the keyword and the
        // `trim_start()` below recovers the expression's exact offset.
        let content = self.strip_block_keyword(tag_content, "await", tag_content_start)?;
        let content_offset = tag_content_start + subslice_offset(tag_content, content);

        // Use partial parsing for the promise expression, which is also what keeps
        // `fetch(" then ")` from splitting on the `then` inside its string.
        //
        // A top-level `as` is TypeScript's here, unlike `{#each}`: this head's separators
        // are `then` / `catch`, no part of type syntax, so an `as` in an await head is
        // always an assertion and the parse ends on its own at the clause keyword
        // ([`TopLevelAs`]) — no assertion-run walk needed, which is the whole difference
        // between the two heads. Giving the keyword away rejected every unparenthesized
        // assertion canonical accepts (`{#await p as T then v}`) and — since the printer
        // strips the parens the authored form needs — turned `{#await (p as T) then v}`
        // into output tsv itself could not re-read.
        let expr_str = content.trim_start_matches(is_svelte_ws);
        let (expression, expr_end_pos) = self.parse_ts_expression_partial(
            expr_str,
            content_offset + subslice_offset(content, expr_str),
            TopLevelAs::Assertion,
        )?;

        // Opening tag span is from start to content_start (includes the closing })
        let opening_tag_span = Span {
            start: start as u32,
            end: content_start as u32,
        };

        // Check what follows the expression
        let expr_consumed = expr_end_pos - content_offset;
        let after_expr = &content[expr_consumed..];

        // Check for shorthand: {#await promise then value} / {#await promise catch error}.
        let shorthand_then = strip_head_keyword_or_final(after_expr, "then");
        let shorthand_catch = strip_head_keyword_or_final(after_expr, "catch");

        // Which clause the HEAD fills, and with what binding text. The block form fills
        // none — its first body is the pending phase — while each shorthand fills its
        // own, which is exactly the state canonical's `next` guards the continuations
        // against. Naming it here is what lets all three forms share one continuation
        // loop below: with a copy per shorthand arm, a duplicate-clause guard added
        // to one copy would silently not apply to the others.
        let head_clause = match (shorthand_then, shorthand_catch) {
            (Some(value_str), _) => Some((AwaitClause::Then, value_str)),
            (_, Some(error_str)) => Some((AwaitClause::Catch, error_str)),
            (None, None) => {
                // No `then`/`catch` shorthand matched, so the opening tag must end
                // right after the promise expression. Reject trailing content like
                // `{#await p garbage}` or a shorthand jammed against the expression
                // (`{#await p then(v)}`) — the canonical parser rejects both.
                self.reject_trailing_tag_content(after_expr, expr_end_pos)?;
                None
            }
        };

        // The block form (`{#await x}…{/await}`) always carries a pending Fragment —
        // empty or not — unlike the inline `then`/`catch` shorthand (no pending
        // phase); the writer emits `{Fragment, []}` vs `null` from this.
        let pending_block = head_clause.is_none();

        // The head's binding, if it carries one. The text starts right after
        // "expression <keyword> "; the pattern parser trims its own leading whitespace,
        // so pass the raw slice plus that offset. The valueless form (`{#await p then}`)
        // yields an empty slice and no binding.
        let head_binding = match head_clause {
            Some((_, s)) if !s.is_empty() => {
                let keyword_end = expr_end_pos + subslice_offset(after_expr, s);
                Some(self.parse_await_value_pattern(s, keyword_end)?)
            }
            _ => None,
        };

        // One `(fragment, binding)` pair per clause, so the fill and the filled-check
        // always name the same slot. Keeping them as four independent locals is what let
        // the guard be added to one arm and not the other.
        let mut then_slot: AwaitSlot<'arena> = (None, None);
        let mut catch_slot: AwaitSlot<'arena> = (None, None);
        let mut pending = None;

        // The first body belongs to whichever clause the head named.
        let first_body = self.parse_block_children(AWAIT_BODY_STOPS, content_start)?;
        match head_clause {
            Some((AwaitClause::Then, _)) => then_slot = (Some(first_body), head_binding),
            Some((AwaitClause::Catch, _)) => catch_slot = (Some(first_body), head_binding),
            None => pending = (!first_body.nodes.is_empty()).then_some(first_body),
        }

        // Read `{:then}` / `{:catch}` continuations in either order. A slot may be
        // filled once — canonical's `next` guards each arm with `if (block.then)` /
        // `if (block.catch)` and raises `block_duplicate_clause`. Without the guard the
        // assignment OVERWRITES, and the discarded fragment's markup is gone from the
        // AST: a formatter reprinting it would silently delete a branch of the document.
        loop {
            let Some(clause) = [AwaitClause::Then, AwaitClause::Catch]
                .into_iter()
                .find(|c| self.check_await_continuation(c.keyword()))
            else {
                break;
            };
            let slot = match clause {
                AwaitClause::Then => &mut then_slot,
                AwaitClause::Catch => &mut catch_slot,
            };
            if slot.0.is_some() {
                return Err(self.error_duplicate_clause(clause.keyword()));
            }
            *slot = self.parse_await_continuation(clause.keyword(), AWAIT_BODY_STOPS)?;
        }

        let (then_fragment, value) = then_slot;
        let (catch_fragment, error) = catch_slot;
        let end = self.expect_block_close_keyword("await", start)?;

        Ok(FragmentNode::AwaitBlock(AwaitBlock {
            expression,
            value,
            error,
            pending,
            pending_block,
            then: then_fragment,
            catch: catch_fragment,
            span: Span {
                start: start as u32,
                end: end as u32,
            },
            opening_tag_span,
        }))
    }

    /// Reject a second `{:else}` / `{:else if}` once the block's alternate is taken.
    ///
    /// Canonical's `next` (`1-parse/state/tag.js`) writes `block.alternate` /
    /// `block.fallback` **unguarded**, so a repeat continuation replaces the fragment
    /// and the first branch's markup is gone from the AST — unlike its own `{#await}`
    /// arm, which raises `block_duplicate_clause` for exactly this. tsv applies the
    /// `{#await}` rule to all three continuations: reproducing the overwrite means a
    /// formatter that silently deletes a branch of the author's document.
    ///
    /// A deliberate over-rejection, cataloged in `docs/conformance_svelte.md`
    /// §Block Continuation Corrections. Called where the alternate has just been
    /// parsed, so the current token — the stray `{:` — carries the error's position.
    /// Any other continuation keyword (`{:catch}` after an `{:else}`) is left alone:
    /// canonical rejects those too, so the verdict already matches.
    ///
    /// The two spellings take different messages because only one of them is a
    /// **repeat**: a second `{:else}` is a duplicate, while an `{:else if}` after an
    /// `{:else}` is the block's *first* `{:else if}` landing on an alternate the
    /// `{:else}` already took. Calling that a duplicate names a clause the author
    /// never wrote twice.
    fn reject_duplicate_else(&self) -> Result<(), ParseError> {
        if !self.check(TokenKind::BlockContinue) {
            return Ok(());
        }
        let mut words = continuation_words(self.continuation_keyword_at(self.current_end));
        if words.next() != Some("else") {
            return Ok(());
        }
        Err(if words.next() == Some("if") {
            self.error_clause_after("else if", "else")
        } else {
            self.error_duplicate_clause("else")
        })
    }

    /// Check if the next token is a BlockContinue with the given keyword (e.g., "catch", "then").
    fn check_await_continuation(&self, keyword: &str) -> bool {
        self.check(TokenKind::BlockContinue)
            && self
                .continuation_keyword_at(self.current_end)
                .starts_with(keyword)
    }

    /// Parse a `{:then value}` / `{:catch error}` continuation block within an await block.
    /// `keyword` is `"then"` or `"catch"`; `stop_keywords` are the continuations that end this
    /// block's body (`{:catch}` always stops only at `{/await}`; `{:then}` also stops at a
    /// following `{:catch}` in the full form). Returns `(fragment, binding_pattern)`.
    fn parse_await_continuation(
        &mut self,
        keyword: &str,
        stop_keywords: &[&str],
    ) -> Result<(Option<Fragment<'arena>>, Option<Expression<'arena>>), ParseError> {
        let tag_start = self.current_end;
        let (tag_content, content_start) = self.scan_block_tag_content(tag_start)?;
        let binding_str = self
            .strip_keyword_value(tag_content, keyword, tag_start)?
            .trim_matches(is_svelte_ws);

        let binding = if !binding_str.is_empty() {
            let offset = tag_start + subslice_offset(tag_content, binding_str);
            Some(self.parse_await_value_pattern(binding_str, offset)?)
        } else {
            None
        };

        let fragment = self.parse_block_children(stop_keywords, content_start)?;
        Ok((Some(fragment), binding))
    }

    /// Read the leading alphabetic keyword at `pos` in the source — the `if` in
    /// `{#if}`, the `each` in `{/each}`, the `html` in `{@html}`. Stops at the
    /// first non-alphabetic byte (space, `}`, …); returns `""` when there is none.
    pub(super) fn keyword_at(&self, pos: usize) -> &'a str {
        let remaining = &self.source[pos..];
        let end = remaining
            .find(|c: char| !c.is_alphabetic())
            .unwrap_or(remaining.len());
        &remaining[..end]
    }

    /// Read the continuation keyword-run at `pos` — the alphabetic-and-space run
    /// after `{:`, trimmed. Unlike `keyword_at` this keeps internal spaces so the
    /// two-word `else if` survives; callers compare against `"else"`, `"else if"`,
    /// `"catch"`, etc. Trailing content makes the run miss every keyword (e.g.
    /// `{:else garbage}` yields `"else garbage"`, which is neither `else` nor
    /// `else if`), so it is left unconsumed and surfaces as an error.
    fn continuation_keyword_at(&self, pos: usize) -> &'a str {
        let remaining = &self.source[pos..];
        let end = remaining
            .find(|c: char| !c.is_alphabetic() && !is_svelte_ws(c))
            .unwrap_or(remaining.len());
        // [`is_svelte_ws`] on BOTH the cut and the trim, and they must stay the same class:
        // the cut decides which characters are inside the run, so a trim answering a
        // different question would either leave a member in or take a non-member out.
        //
        // ⚠️ A cut of `c != ' '` — a LITERAL SPACE — would make this the narrowest
        // whitespace class in the parser and break the two-word `{:else if}` in both
        // directions at once. A separator Svelte's `allow_whitespace` crosses but a space
        // isn't (a tab, a NEWLINE, or any non-ASCII JS `\s`) ended the run at `else`, so
        // `{:else⏎if x}` read as a plain `{:else}` and the `if x` then failed the head's `}`
        // — an OVER-REJECTION of a wrapped else-if, the one spelling here a human actually
        // writes. The same cut swallowed junk in the other direction: `{:else⇥junk}` also
        // reduced to `else`, so `{#each}`'s exact `== "else"` test took the branch and
        // ACCEPTED a head canonical rejects, where `{:else junk}` (space) was correctly
        // refused. One class, both bugs.
        //
        // ⚠️ Not `char::is_whitespace` / `str::trim` either — that is Rust's White_Space,
        // which disagrees with JS `\s` in both directions (it has U+0085 NEL, it lacks
        // U+FEFF). NEL must stay OUTSIDE the run so it reads as trailing junk, exactly as
        // canonical treats it.
        remaining[..end].trim_matches(is_svelte_ws)
    }

    /// Strip a leading block/tag keyword, enforcing the whitespace Svelte
    /// requires between the keyword and any value that follows. The value may be
    /// absent (`{:then}` → `Ok("")`), but a value jammed against the keyword
    /// (`{:then(v)}`, `{:thenx}`) is rejected — matching the canonical parser,
    /// which emits `expected_whitespace`. Any whitespace counts (space, tab,
    /// newline), so the returned remainder is left untrimmed; callers trim it and
    /// recover span offsets with `subslice_offset`.
    fn strip_keyword_value(
        &self,
        content: &'a str,
        keyword: &str,
        keyword_start: usize,
    ) -> Result<&'a str, ParseError> {
        let rest = content.strip_prefix(keyword).unwrap_or(content);
        if rest.is_empty() || rest.starts_with(is_svelte_ws) {
            Ok(rest)
        } else {
            Err(self.error_expected_at(&format!("whitespace after `{keyword}`"), keyword_start))
        }
    }

    /// Like `strip_keyword_value`, but the value is mandatory: the keyword
    /// standing alone (`{#each}`, `{@html}`) is also rejected. Used by the blocks
    /// and tags whose expression or name is required.
    pub(super) fn strip_block_keyword(
        &self,
        content: &'a str,
        keyword: &str,
        keyword_start: usize,
    ) -> Result<&'a str, ParseError> {
        let rest = self.strip_keyword_value(content, keyword, keyword_start)?;
        if rest.is_empty() {
            return Err(
                self.error_expected_at(&format!("whitespace after `{keyword}`"), keyword_start)
            );
        }
        Ok(rest)
    }

    /// Require that `region` — whose first byte is at absolute source offset `region_start` —
    /// holds only whitespace before the tag's closing `}`. This is Svelte's `allow_whitespace`
    /// then `eat('}')` after a tag's payload: a stray comment, leftover binding fragment, or
    /// junk is rejected (erroring at the first non-whitespace byte), never silently dropped.
    /// Shared by every block whose tag ends right after its payload — `{#each}`'s binding,
    /// `{#await}`'s promise, `{#snippet}`'s `)`, and every `{/block}` close.
    fn reject_trailing_tag_content(
        &self,
        region: &str,
        region_start: usize,
    ) -> Result<(), ParseError> {
        let trailing = region.trim_start_matches(is_svelte_ws);
        if !trailing.is_empty() {
            let trailing_start = region_start + subslice_offset(region, trailing);
            return Err(self.error_expected_at("'}'", trailing_start));
        }
        Ok(())
    }

    /// Parse a `{#await}` `then`/`catch` binding pattern (the value/error), rejecting a
    /// comment immediately BEFORE the pattern or BETWEEN the binding and its `}`.
    /// Svelte reads these with `read_pattern` — acorn at the current index, having skipped
    /// only whitespace — so a comment before the pattern fails ("Expected identifier or
    /// destructure pattern") and the following `eat()` rejects one between the binding and
    /// the next token. A comment INSIDE a destructure (`{ a /* c */ }`) or INSIDE the type
    /// annotation (`value: /* c */ number`) stays valid — it's acorn trivia within the
    /// pattern/type. tsv's `parse_ts_pattern` is comment-tolerant (it would relocate or drop
    /// a surrounding comment), so this gate restores Svelte's strictness. `region_offset` is
    /// the absolute source offset of `region[0]`.
    ///
    /// ⚠️ An annotation gives the region **two** edges and the gate needs both. A bare-span
    /// reading alone sees the `:` first, calls the whole tail an annotation, and lets
    /// `{:then a: A /* c */}` through — accepted where canonical rejects, with the comment
    /// silently eaten by the sub-parse's lookahead. A `pattern_binding_end` reading alone
    /// steps past the `:` and re-opens `{:then a /* c */: A}`, which the bare reading had
    /// covered. Both, for every kind. The `{#each}` and `{@const}` readers gate the same
    /// two edges, `{#each}` by bounding the pattern before it parses the annotation.
    fn parse_await_value_pattern(
        &mut self,
        region: &str,
        region_offset: usize,
    ) -> Result<Expression<'arena>, ParseError> {
        let lead = region.len() - region.trim_start_matches(is_svelte_ws).len();
        let value_start = region_offset + lead;
        // The trailing trim is safe here for the reason the doc below gives: a comment at
        // either edge of this region is REJECTED, so none can be standing at the end for the
        // trim to clip (contrast a head's, `Parser::parse_ts_expression`).
        let trimmed = region.trim_matches(is_svelte_ws);
        // `{:then p}` / `{:catch p}` are `read_pattern` positions like `{#each … as p}`, so
        // a PLAIN-IDENTIFIER binding takes the reserved-word rule here too — canonical's
        // `read_pattern` reads it with `parser.read_identifier()` before any acorn call.
        // Asked BEFORE `parse_ts_pattern` rather than of the parsed node because a reserved
        // word must not reach the TypeScript parser at all: it defers exactly this as a
        // strict-mode early error, so the shape would come back as a valid binding. The
        // reader's own answer is discarded — the annotation (`{:then v: T}`) means the
        // pattern's extent is the TS parser's to find, not this reader's.
        if !trimmed.starts_with(['{', '[']) {
            self.read_identifier(trimmed, value_start)?;
        }
        let pattern = self.parse_ts_pattern(trimmed, value_start)?;
        let span = pattern.span();
        // Leading comment: the pattern would start past `value_start`.
        if span.start as usize != value_start {
            return Err(self.error_expected_at("identifier or destructure pattern", value_start));
        }
        // TWO gaps in this region can hold a comment canonical rejects, one on each side of
        // the annotation, and neither reading covers the other: after the BARE pattern,
        // before its `:`/`}` (`then a /* c */: A`), and after the whole BINDING, before its
        // `}` (`then a: A /* c */`). Each is a legitimate leftover otherwise — a `: type`
        // annotation, or nothing — so the reject is only on a tail that *starts* with a
        // comment; one INSIDE the type (`value: /* c */ number`) leaves `:` first and is
        // allowed. The two ends coincide for an unannotated binding.
        //
        // The bare end is the pattern node's own span for EVERY kind — `attach_pattern_type_
        // annotation` leaves the span on the bare binding; only a destructuring pattern's
        // wire `end` widens, at emit time — so only the far end needs `pattern_binding_end`.
        for edge in [
            span.end as usize,
            tsv_ts::pattern_binding_end(&pattern) as usize,
        ] {
            let tail = trimmed[edge - value_start..].trim_start_matches(is_svelte_ws);
            if tail.starts_with("/*") || tail.starts_with("//") {
                return Err(self.error_expected_at("identifier or destructure pattern", edge));
            }
        }
        Ok(pattern)
    }

    /// Consume the closing `{/expected}` tag and return the position after it.
    /// `block_start` is the byte offset of the opening `{#expected`, used to
    /// locate the unclosed-block error.
    ///
    /// Three failure modes, all rejected by the canonical parser:
    /// - the block is left unclosed (`{#if x}a`) — reported at `block_start`;
    /// - the close names a different block — a mismatch like `{#if x}…{/each}`;
    /// - the close carries trailing junk (`{/each foo}`) — only whitespace may
    ///   follow the keyword before `}`.
    fn expect_block_close_keyword(
        &mut self,
        expected: &str,
        block_start: usize,
    ) -> Result<usize, ParseError> {
        // Unclosed block: Svelte requires a matching `{/expected}`.
        if !self.check(TokenKind::BlockClose) {
            return Err(self.error_unclosed_at(&format!("{{#{expected}}} block"), block_start));
        }

        // The keyword after `{/` must match the open block.
        if self.keyword_at(self.current_end) != expected {
            return Err(self.error_expected_at(&format!("{{/{expected}}}"), self.current_start));
        }

        let close_tag_start = self.current_end;
        let (close_content, after_close) = self.scan_block_tag_content(close_tag_start)?;

        // Only whitespace may follow the keyword: `{/each foo}` is rejected.
        // The keyword matched above, so `close_content` starts with `expected`.
        self.reject_trailing_tag_content(
            &close_content[expected.len()..],
            close_tag_start + expected.len(),
        )?;

        Ok(after_close)
    }

    /// Parse a key block: {#key expression}...{/key}
    fn parse_key_block(&mut self, start: usize) -> Result<FragmentNode<'arena>, ParseError> {
        // Get the content start position (after {#)
        let tag_content_start = self.current_end;

        // Scan to find closing } and extract content
        let (tag_content, content_start) = self.scan_block_tag_content(tag_content_start)?;

        // Parse: "key expression" — Svelte requires whitespace after the keyword. Leading
        // whitespace only — the trailing run may be a line comment's own text
        // (`Parser::parse_ts_expression`).
        let expr_str = self
            .strip_block_keyword(tag_content, "key", tag_content_start)?
            .trim_start_matches(is_svelte_ws);

        let expr_offset = tag_content_start + subslice_offset(tag_content, expr_str);
        let expression = self.parse_ts_expression(expr_str, expr_offset)?;

        // Opening tag span is from start to content_start (includes the closing })
        let opening_tag_span = Span {
            start: start as u32,
            end: content_start as u32,
        };

        // Parse fragment
        let fragment = self.parse_block_children(&["key"], content_start)?;

        // Expect closing {/key}
        let end = self.expect_block_close_keyword("key", start)?;

        Ok(FragmentNode::KeyBlock(KeyBlock {
            expression,
            fragment,
            span: Span {
                start: start as u32,
                end: end as u32,
            },
            opening_tag_span,
        }))
    }

    /// Parse a snippet block: {#snippet name(params)}...{/snippet}
    /// Also handles TypeScript generics: `{#snippet name<T>(params)}`
    fn parse_snippet_block(&mut self, start: usize) -> Result<FragmentNode<'arena>, ParseError> {
        // Get the content start position (after {#)
        let tag_content_start = self.current_end;

        // Scan to find closing } and extract content
        let (tag_content, content_start) = self.scan_block_tag_content(tag_content_start)?;

        // Parse: "snippet name(params)" or "snippet name<T>(params)" — Svelte
        // requires whitespace after the keyword.
        let content = self
            .strip_block_keyword(tag_content, "snippet", tag_content_start)?
            .trim_matches(is_svelte_ws);
        let content_bytes = content.as_bytes();
        // Absolute offset of `content[0]` (the name's first byte) in the source, the
        // base for every span and error position below.
        let content_offset = tag_content_start + subslice_offset(tag_content, content);

        // Mirror Svelte's snippet-head grammar (`1-parse/state/tag.js`): read the name,
        // then an optional `<…>` generic via the naive `<`/`>` matcher, then REQUIRE a
        // `(`. Svelte's generic matcher tracks only angle depth (never parens), so a `>`
        // from a `=>` / `>=` / `>>` closes the generic early and the required `(` can't be
        // found — Svelte rejects, and we reject in lockstep. A function type (or any stray
        // `>`) in a snippet generic is invalid Svelte, so corrupting it on format would be
        // worse than a parse error. See `find_matching_angle_bracket`.

        // Name: the leading identifier run, read the way Svelte's `read_identifier` reads
        // it. `content` is trimmed, so it starts at the name.
        let Some(name_str) = self.read_identifier(content, content_offset)? else {
            return Err(self.error_expected_at("snippet name", content_offset));
        };
        let name_len = name_str.len();
        let expression = self.parse_ts_expression(name_str, content_offset)?;
        // The reserved-word filter above is what makes this an `Identifier` rather than
        // whatever `this` / `null` / `true` / `super` would have parsed to — a
        // `ThisExpression`, a `Literal`, a `Super`, none of which the wire's
        // `SnippetBlock.expression` may hold. Stated rather than assumed, so the filter and
        // the node shape can never drift apart silently.
        if !matches!(expression, Expression::Identifier(_)) {
            return Err(self.error_expected_at("snippet name", content_offset));
        }

        // Optional `<…>` generic. `head_start` is the `<` (or, with no generic, the `(`)
        // where the parseable signature head begins — the wrapper slice below spans from
        // there through the matching `)`.
        let after_name = content[name_len..].trim_start_matches(is_svelte_ws);
        let head_start = content.len() - after_name.len();
        let (after_generic, type_params_raw): (usize, Option<&'arena str>) =
            if after_name.starts_with('<') {
                // `type_params_raw` is the raw inner text — feeds the public AST's `typeParams`
                // string (Svelte stores it raw too) and the parse-failure fallback.
                let close_pos = self.find_matching_angle_bracket(content, head_start)?;
                (
                    close_pos + 1,
                    Some(self.alloc_str_in(&content[head_start + 1..close_pos])),
                )
            } else {
                (head_start, None)
            };

        // Require `(` after only whitespace — Svelte's `allow_whitespace` then
        // `eat('(', true)`. Crucially this skips whitespace but NOT comments, so
        // `<T> /* c */ (…)` is rejected exactly as Svelte rejects it.
        let after_generic_str = &content[after_generic..];
        let paren_pos = after_generic
            + (after_generic_str.len() - after_generic_str.trim_start_matches(is_svelte_ws).len());
        if !content[paren_pos..].starts_with('(') {
            return Err(self.error_expected_at("'('", content_offset + paren_pos));
        }

        // Opening tag span is from start to content_start (includes the closing })
        let opening_tag_span = Span {
            start: start as u32,
            end: content_start as u32,
        };

        // The `)` matching the opening `(` — depth- and trivia-aware, so a `)` inside a
        // string/comment in a param default can't end the list early. Svelte requires the
        // close (`eat(')', true)`); an unmatched `(` is rejected.
        let close_paren = match_bracket(
            content_bytes,
            paren_pos,
            content.len(),
            b'(',
            b')',
            TriviaProfile::JS,
        )
        .ok_or_else(|| self.error_expected_at("')'", content_offset + content.len()))?;

        // Only whitespace may follow `)` before the closing `}` — Svelte's
        // `allow_whitespace` then `eat('}', true)`. `{#snippet fn() junk}` is rejected.
        self.reject_trailing_tag_content(
            &content[close_paren + 1..],
            content_offset + close_paren + 1,
        )?;

        // Absolute source span of the parens (`start` = `(`, `end` = `)`), for comment
        // lookup when printing the parameter list.
        let params_paren = Some(Span {
            start: (content_offset + paren_pos) as u32,
            end: (content_offset + close_paren) as u32,
        });
        let params_str = &content[paren_pos + 1..close_paren];

        // Parse the signature head `<TP>(PARAMS)` as `function f<TP>(PARAMS) {}` so every
        // position — type parameters (constraints/defaults/modifiers/comments),
        // typed/destructured params, comments anywhere — goes through the canonical
        // comment-collecting parser. Wrapping a *contiguous* source slice (from the `<` or
        // `(` through the matching `)`) keeps the single `base` offset valid across both
        // `<…>` and `(…)`. Collected comments merge into the root buffer (the printer
        // locates them by position).
        //
        // A parse failure REJECTS the component, in lockstep with Svelte: its own reader
        // hands the same slice to `parse_expression_at` as `(PARAMS) => {}` and lets the
        // throw out (`1-parse/state/tag.js`). Swallowing it here instead — keeping the raw
        // text — accepted every malformed head (`fn(a b)`, `fn(,,)`, `fn<T extends>()`) and
        // was lossy twice over: the wire emits no parameters at all, and the printer
        // reflowed the kept text.
        let mut type_parameters: Option<tsv_ts::TSTypeParameterDeclaration<'arena>> = None;
        let mut parameters: &'arena [Expression<'arena>] = &[];
        if type_params_raw.is_some() || !params_str.trim_matches(is_svelte_ws).is_empty() {
            // The head runs from where the signature begins (`<` or `(`) through the `)`.
            let head_slice = &content[head_start..=close_paren];
            const WRAPPER_PREFIX: &str = "function f";
            let wrapper = format!("{WRAPPER_PREFIX}{head_slice} {{}}");
            let base = (content_offset + head_start).saturating_sub(WRAPPER_PREFIX.len());
            // Snippet parameters preserve grouping parens (acorn's `preserveParens`,
            // without Svelte's `remove_parens`), so a default like `c = (2, 3)` keeps
            // its `ParenthesizedExpression` — matching Svelte's snippet-param AST.
            // Svelte's own prelude is `replace(/\S/g, ' ')` — it blanks the
            // non-whitespace and keeps every terminator — so acorn counted the
            // ECMAScript class over the whole prefix, exactly as for the raw
            // template the expression islands get.
            // The extent is the head slice itself — from the `<` or `(` through the
            // matching `)` — not the wrapper the parse actually runs over, whose
            // `function f` prefix sits at synthetic offsets outside the document.
            self.record_acorn_region(
                content_offset + head_start,
                &content[head_start..=close_paren],
                // `replace(/\S/g, ' ')` blanks the non-whitespace ONLY: the author's tab
                // reaches acorn intact, and the blanked columns after it EXTEND the run the
                // dedent measures past anything the document has — and it leaves every
                // ECMAScript terminator standing, which is the line class it derives.
                AcornPrefixText::WhitespaceKept,
            );
            let program = tsv_ts::parse_embedded_preserve_parens(&wrapper, base, self.arena)?;
            self.expression_comments.extend_from_slice(program.comments);
            // The wrapper is literally a `function` declaration, so the match holds by
            // construction — stated as an error rather than an `if let` so a head whose
            // pieces went nowhere can never reach the printer as a silently empty
            // signature (the shape a raw-text fallback would produce).
            let Some(tsv_ts::Statement::FunctionDeclaration(func)) = program.body.first() else {
                return Err(
                    self.error_expected_at("snippet signature", content_offset + head_start)
                );
            };
            type_parameters.clone_from(&func.type_parameters);
            parameters = func.params;
        }

        // Parse body
        let body = self.parse_block_children(&["snippet"], content_start)?;

        // Expect closing {/snippet}
        let end = self.expect_block_close_keyword("snippet", start)?;

        Ok(FragmentNode::SnippetBlock(SnippetBlock {
            expression,
            type_parameters,
            type_params_raw,
            parameters,
            params_paren,
            body,
            span: Span {
                start: start as u32,
                end: end as u32,
            },
            opening_tag_span,
        }))
    }

    /// Find the matching closing angle bracket for generics like `<T>` (the byte
    /// offset of the `>`). Used for TypeScript generics in snippet declarations.
    /// Comment- and string-aware via the shared cursor.
    ///
    /// Deliberately a naive `<`/`>` depth count, mirroring Svelte's own snippet-generic
    /// scanner (`match_bracket` with `pointy_bois`): a `>` from a `=>` / `>=` / `>>`
    /// decrements depth and closes the generic early. `parse_snippet_block` then requires
    /// a `(` immediately after, so such a head (a function type — `<T extends () => void>`,
    /// `<T = () => void>` — or any stray `>`) is rejected exactly as Svelte rejects it,
    /// rather than mis-sliced and corrupted on format.
    fn find_matching_angle_bracket(
        &self,
        content: &str,
        open_pos: usize,
    ) -> Result<usize, ParseError> {
        match_bracket(
            content.as_bytes(),
            open_pos,
            content.len(),
            b'<',
            b'>',
            TriviaProfile::JS,
        )
        .ok_or_else(|| ParseError::unexpected_eof(content.len()))
    }

    /// Scan source from a position until we find the closing } of a block tag
    /// Returns (content between start and }, position after })
    pub(super) fn scan_block_tag_content(
        &mut self,
        start: usize,
    ) -> Result<(&'a str, usize), ParseError> {
        // Find the block tag's closing `}` (skips strings/comments/regex). `start`
        // is just after the `{#…`/`{@…` keyword, so the opening `{` is the depth-1
        // brace that `scan_to_matching_brace` matches.
        let Some(end) = scan_to_matching_brace(self.source.as_bytes(), start) else {
            return Err(self.error_unclosed_at("block tag", start));
        };

        let content = &self.source[start..end];

        // Reposition the lexer past `}`. Block tags only occur in template content,
        // so `inside_tag` is already `false` (template mode) and stays that way for
        // the block body, which `advance_to_position` preserves.
        let after_close = end + 1; // Skip past the }
        self.advance_to_position(after_close)?;

        Ok((content, after_close))
    }

    /// Parse children of a block until we hit a closing or intermediate tag
    /// stop_keywords: keywords that should stop parsing (e.g., ["else", "if"] for if blocks)
    /// content_start: position to start capturing text from (position after opening tag's `}`)
    fn parse_block_children(
        &mut self,
        stop_keywords: &[&str],
        content_start: usize,
    ) -> Result<Fragment<'arena>, ParseError> {
        let mut nodes = self.bvec();
        let mut last_end = content_start;

        loop {
            // Capture text gaps
            self.capture_text_if_gap(last_end, &mut nodes)?;

            if self.check(TokenKind::Eof) {
                break;
            }

            // Check for block close {/keyword}
            if self.check(TokenKind::BlockClose)
                && stop_keywords.contains(&self.keyword_at(self.current_end))
            {
                break;
            }

            // Check for block continue {:keyword}
            if self.check(TokenKind::BlockContinue) {
                let keyword = self.continuation_keyword_at(self.current_end);

                // Stop when the continuation keyword begins with a stop keyword,
                // so the two-word `{:else if}` matches the `else` stop.
                let should_stop = stop_keywords.iter().any(|sk| keyword.starts_with(sk));

                if should_stop {
                    break;
                }
            }

            // Parse child nodes
            if self.check(TokenKind::Comment) {
                let comment = self.parse_comment()?;
                last_end = comment.span.end_usize();
                nodes.push(FragmentNode::Comment(comment));
            } else if self.check(TokenKind::LeftAngle) {
                // Check if closing tag
                if self.is_next_token(TokenKind::Slash)? {
                    break;
                }
                match self.parse_element_or_special()? {
                    ParsedElement::Element(elem) => {
                        last_end = elem.span.end_usize();
                        nodes.push(FragmentNode::Element(elem));
                    }
                    ParsedElement::SpecialElement(elem) => {
                        last_end = elem.span.end_usize();
                        nodes.push(FragmentNode::SpecialElement(elem));
                    }
                }
            } else if self.check(TokenKind::LeftBrace) {
                let tag = self.parse_brace_tag()?;
                last_end = tag.span().end_usize();
                nodes.push(tag);
            } else if self.check(TokenKind::BlockOpen) {
                let block = self.parse_block()?;
                last_end = block.span().end_usize();
                nodes.push(block);
            } else if self.check(TokenKind::TagOpen) {
                let tag = self.parse_template_tag()?;
                last_end = tag.span().end_usize();
                nodes.push(tag);
            } else {
                // Unknown token - might be text content that wasn't captured
                break;
            }
        }

        Ok(Fragment {
            nodes: nodes.into_bump_slice(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{EachHeadSplit, each_binding_separator, is_svelte_ws, strip_head_keyword};
    use bumpalo::Bump;

    /// A mid-head separator is its keyword wearing a whitespace RUN of any width, in
    /// canonical's own [`is_svelte_ws`](super::is_svelte_ws) class — not the single ASCII
    /// spaces the canonical printed form happens to use. A newline is what tsv's own
    /// wrapped `{#each}` head emits (`items as A[]⏎as item}`), so a narrower match made
    /// the formatter's output unparseable by its own parser; the non-ASCII members are
    /// reachable only from authored source, and no fixture can carry them legibly.
    ///
    /// Leading whitespace is OPTIONAL (canonical's `allow_whitespace`) and trailing is
    /// REQUIRED (`require_whitespace`), which is what keeps `as/* c */x` and a head-final
    /// `as` out — the latter having been an out-of-bounds panic while the keyword's width
    /// was assumed rather than measured. All three keywords take the one rule, so
    /// `{#await … then v}` cannot drift from `{#each … as x}` the way they had.
    #[test]
    fn head_separator_takes_a_whitespace_run_of_any_width() {
        for kw in ["as", "then", "catch"] {
            for ws in [
                " ", "  ", "\t", "\n", "\r\n", "\u{a0}", "\u{2028}", "\u{feff}",
            ] {
                let rest = format!("{ws}{kw}{ws}v");
                assert_eq!(strip_head_keyword(&rest, kw), Some("v"), "{rest:?}");
                assert_eq!(
                    strip_head_keyword(&rest[ws.len()..], kw),
                    Some("v"),
                    "{rest:?}"
                );
            }
            // No keyword, no trailing run, or the keyword glued to its value.
            for rest in [
                String::new(),
                format!(" {kw}"),
                kw.to_string(),
                format!(" {kw}v"),
                format!(" {kw}/* c */v"),
                format!(" {kw}}}"),
            ] {
                assert_eq!(strip_head_keyword(&rest, kw), None, "{rest:?}");
            }
            // A separator followed by NOTHING is still a separator — canonical's
            // `require_whitespace` is satisfied and it is the pattern reader that then
            // reports the missing binding, so the empty value is the caller's error.
            assert_eq!(strip_head_keyword(&format!(" {kw} "), kw), Some(""));
        }
        assert_eq!(strip_head_keyword(" a s item", "as"), None);
    }

    /// The run's LAST keyword is what splits `items as A[] as item` into an assertion plus
    /// a binding, and only when that keyword is `as`. The reported binding offset is exact
    /// — the caller slices with it — and everything between the two offsets is the
    /// separator itself.
    ///
    /// The chain is walked with the type grammar rather than by counting brackets, so the
    /// cases worth listing are the ones a byte scan cannot state as a rule: an arrow
    /// inside a type argument (whose `>` closes nothing), a `satisfies` link the chain has
    /// to step over, a mapped type carrying its own `as`, and a pattern-only binding that
    /// is no type at all.
    #[test]
    fn each_binding_separator_reports_the_binding_offset() {
        let arena = Bump::new();
        for (input, binding) in [
            ("A[] as item", "item"),
            ("A[]\tas\nitem", "item"),
            ("A[]  as   item", "item"),
            ("A as B[] as item", "item"),
            ("A[] as { a, b }", "{ a, b }"),
            ("A[] as item, i (key)", "item, i (key)"),
            // The `>` of an arrow return type closes no bracket — a signed depth counter
            // went NEGATIVE here, so every later ` as ` failed the scan's `depth == 0`
            // test and the whole head was rejected.
            ("A<() => string> as item", "item"),
            ("(() => string)[] as item", "item"),
            ("A<{ a: () => string }> as item", "item"),
            ("unknown as A<() => string> as item", "item"),
            // `satisfies` is a link in the chain, never a separator: the walk steps over
            // it and keeps looking for an `as`. Stopping at it instead handed the tail to
            // the binding parser, rejecting heads canonical accepts. An `as` AFTER the
            // link is still the run's last keyword, so it still separates.
            ("A satisfies B as item", "item"),
            ("A[] satisfies B[] as item, i", "item, i"),
            ("A satisfies B satisfies C as item", "item"),
            // A mapped type spells `as` INSIDE a bracket run. A depth scan survives it
            // only because brackets nest it; the type grammar knows it outright — and the
            // two disagree as soon as an arrow has already broken the depth count.
            ("{ [K in T as U]: V } as item", "item"),
            ("{ [K in keyof A as string]: () => string } as item", "item"),
            // The gap before the separator may hold comments, which canonical accepts and
            // the caller keeps inside its expression slice.
            ("A[] /* c */ as item", "item"),
            // A type's own end is the token anchor, so no whitespace is required before
            // the keyword — canonical accepts this and the old leading-run anchor did not.
            ("A[]as item", "item"),
            // The binding need not be a type; the walk stops at the last one that is.
            ("A[] as { a = 1 }", "{ a = 1 }"),
            ("A[] as [a = 1]", "[a = 1]"),
        ] {
            let EachHeadSplit::LaterAs {
                separator,
                binding: start,
            } = each_binding_separator(input, 0, &arena)
            else {
                panic!("expected a later `as` in {input:?}");
            };
            assert_eq!(&input[start..], binding, "{input:?}");
            // Between the two offsets lies the keyword and the whitespace after it — the
            // caller trims both off its expression slice.
            assert_eq!(
                input[separator..start].trim_matches(is_svelte_ws),
                "as",
                "{input:?}"
            );
        }

        // No SECOND `as`: the head's first one was already the separator. A binding that
        // is no type at all takes this arm too — a destructuring default is a pattern,
        // and an arrow inside one would drop a bracket-depth scan to a false zero and
        // split the head at the `as` buried in it. So does a run that consumed a
        // `satisfies` link and THEN reached a pattern: the item is still the binding.
        for input in [
            "item",
            "{ a } as", // head-final: not a separator, and never sliced past the end
            "Array<A as B>",
            "item /* as x */",
            "item // as x",
            "item['as x']",
            "Aas x",
            "{ a = 1 }",
            "{ a = (x) => x as T }",
            "[a = 1]",
            "A satisfies { a = 1 }",
        ] {
            assert_eq!(
                each_binding_separator(input, 0, &arena),
                EachHeadSplit::FirstAs,
                "{input:?}"
            );
        }

        // A run ENDING on `satisfies` has no separator at all — canonical strips only the
        // outermost assertion and only when it is an `as`, so the whole run is the
        // iterable and the head is binding-less. The reported end is the run's last TYPE,
        // not the end of the text: what follows is the head's own `, index` / `(key)`.
        for (input, run) in [
            ("A satisfies B", "A satisfies B"),
            ("A satisfies item", "A satisfies item"),
            // A `satisfies` cancels an `as` before it, however binding-shaped the type it
            // cancels looks. Keeping the last `as` instead split these heads and rejected
            // them.
            ("A[] as item satisfies B", "A[] as item satisfies B"),
            ("A as B satisfies C", "A as B satisfies C"),
            ("A[] satisfies B[], i", "A[] satisfies B[]"),
            ("A satisfies B, i (i)", "A satisfies B"),
        ] {
            assert_eq!(
                each_binding_separator(input, 0, &arena),
                EachHeadSplit::NoBinding { run_end: run.len() },
                "{input:?}"
            );
            assert_eq!(&input[..run.len()], run, "{input:?}");
        }
    }
}

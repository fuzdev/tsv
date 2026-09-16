//! Per-input properties an audit checks — the panic-safe primitives that turn a
//! source string into a verdict.
//!
//! This is the shared home for the **input → property** layer that every audit
//! in the [`audit`](crate::audit) substrate builds on:
//!
//! - **reparse** — [`tsv_parse_to_value`] (parse to the wire `Value`), [`tsv_parses`] (the
//!   parse-only twin — does the text parse at all, no wire built),
//!   [`compare_reparsed`] (the one input-wire-vs-output-wire verdict every round-trip
//!   consumer asks, layering [`node_conservation_diff`] / [`node_census`] (the
//!   zero-tolerance node-count census — a node the formatter DROPPED),
//!   [`structurally_equivalent`] (the structural-skeleton compare) and
//!   [`leaf_conservation_diff`] / [`leaf_value_multiset`] (the complementary
//!   decode-invariant leaf-value check that the skeleton, erasing every scalar,
//!   is blind to)) — the round-trip primitives the `roundtrip_audit` / `fuzz` /
//!   `blank_audit` commands share.
//! - **directive pre-scan** — [`source_has_ignore_directive`], the coarse "does this
//!   source freeze a region" question every audit whose property does not survive a
//!   verbatim region asks of its seed.
//! - **base fixed point** — [`base_fixed_point`], the precondition the two mutation audits
//!   rest on: a document with no fixed point has nothing for its authorings to converge ON.
//! - **ledger** (behind the `comment_check` feature) — [`ledger_format`] /
//!   [`ledger_format_with_comments`] / [`pristine_format`] drive `format_source`
//!   with the print-once comment ledger armed, and the [`Verdict`] /
//!   [`VerifyOutcome`] / [`VerifySummary`] verdict types turn a ledger claim
//!   into a falsifiable, self-verified outcome. `gap_audit` and `blank_audit` are
//!   the consumers.
//!
//! The shared property set includes [`f1_check`], which lives here — the
//! core that (wrapped in `catch_unwind` by its callers) drives the no-panic
//! guard, the F1 idempotency fixed point, and the reparse compare — and `fuzz`
//! consumes it (as does `blank_audit`, for its pristine pass). `roundtrip_audit`
//! and `blank_audit`'s per-injection grade share the reparse compare through
//! [`compare_reparsed`] rather than through [`f1_check`]: the round-trip keeps
//! the verbose AST diff (which [`f1_check`] discards) and must *not* run the
//! idempotency step, and the blank grade reuses an output the ledger already
//! paid for — so the shared piece is the smaller reparse-compare core, and the
//! orchestration around it stays each consumer's own.

use std::collections::BTreeMap;

use serde_json::Value;

use tsv_cli::cli::format_source::format_source;
use tsv_cli::cli::input::ParserType;

use crate::diff::{DiffOptions, diff_to_string};
use crate::render_normalize::{normalize_pair, structural_skeleton};

/// The seed's own fixed point, or why it has none — the precondition every **mutation** audit
/// rests on and neither can state for itself.
///
/// An authoring-independence audit asks "does every spelling of this document reach ONE
/// output?". That question is only meaningful against a base that IS an output: if
/// `format(format(x)) != format(x)`, the document has no fixed point to converge on and a
/// converge/diverge verdict per site would be noise. So both `authoring_audit` and
/// `paren_audit` open the same way — format, then format again — and both treat a
/// [`Self::NonIdempotent`](BaseFixedPoint::NonIdempotent) seed as excluded-from-the-analysis
/// but still a **failure of the run**: excluding it is not a reason to pass, or a whole-file
/// reflow could sit reported-but-green.
///
/// Returned as a verdict rather than an `Option` because the two failure reasons are counted
/// separately in every consumer's report, and collapsing them loses which one fired.
pub(crate) enum BaseFixedPoint {
    /// `format(source)`, verified to be a fixed point of itself.
    Ok(String),
    /// The parser rejected the seed — ordinary over a fixture tree (a `tsv_rejects` fixture, a
    /// prettier-rejects input) and not this audit's business.
    ParseError,
    /// The seed formats, but its output reformats to something else: F1 broken as authored.
    NonIdempotent,
}

/// Format `source` and verify the result is a fixed point. See [`BaseFixedPoint`].
pub(crate) fn base_fixed_point(source: &str, parser: ParserType) -> BaseFixedPoint {
    let Ok(f) = format_source(source, parser) else {
        return BaseFixedPoint::ParseError;
    };
    match format_source(&f, parser) {
        Ok(f2) if f2 == f => BaseFixedPoint::Ok(f),
        _ => BaseFixedPoint::NonIdempotent,
    }
}

/// Whether `source` bears an ignore directive anywhere — a coarse substring pre-scan for the
/// `format-ignore` / `prettier-ignore` families (the exact recognizer is
/// `tsv_lang::is_format_ignore_directive`, on a comment's trimmed text). Shared by every audit
/// whose property does not hold across a frozen region: `blank_audit` exempts such a file from
/// its blank-run invariant (locating the verbatim ignore range from the output alone is
/// fragile), `ignore_audit` skips it as a seed (an injected directive interacting with a
/// pre-existing one is fragile), and `paren_audit` skips it because a frozen region reproduces
/// the parens it injects verbatim.
///
/// Ungated, and here rather than in [`sites`](crate::audit::sites) (where it was born, beside
/// the injection audits that were its only callers) because it is pure text over an input with
/// no dependency on the ledger those audits arm — and `sites` is compiled out of a default
/// build.
pub(crate) fn source_has_ignore_directive(source: &str) -> bool {
    source.contains("format-ignore") || source.contains("prettier-ignore")
}

/// Parse `source` with tsv's own parser and convert to the wire-JSON `Value`
/// (the same shape the canonical ASTs use). `None` on a tsv parse error.
///
/// The parse-to-wire primitive shared across the audit substrate — the
/// `roundtrip_audit` / `fuzz` round-trips and the gap audit's Svelte region walk
/// ([`sites::code_regions`](crate::audit::sites)) all reduce a source string to
/// this `Value`.
///
/// TypeScript is read **exactly as [`format_source`] reads it** — no source type
/// named, so the module grammar with a script retry
/// (`tsv_ts::parse_with_goal_or_fallback`). Every caller here is an audit that
/// paired this with a `format_source` call, and a re-read stricter than the
/// formatter's own would report a sloppy script's perfectly good output as
/// unreparseable. This is the audit substrate's reader, not the `parse` command's:
/// the wire it builds is compared against another wire from the same reader, never
/// published as a `Program.sourceType` claim.
pub(crate) fn tsv_parse_to_value(source: &str, parser: ParserType) -> Option<Value> {
    let arena = bumpalo::Bump::new();
    match parser {
        ParserType::TypeScript => {
            let ast = tsv_ts::parse_with_goal_or_fallback(source, None, &arena).ok()?;
            Some(crate::json::wire_value(&tsv_ts::convert_ast_json_bytes(
                &ast, source,
            )))
        }
        ParserType::Svelte => {
            let ast = tsv_svelte::parse(source, &arena).ok()?;
            Some(crate::json::wire_value(
                &tsv_svelte::convert_ast_json_bytes(&ast, source),
            ))
        }
        ParserType::Css => {
            let ast = tsv_css::parse(source, &arena).ok()?;
            Some(crate::json::wire_value(&tsv_css::convert_ast_json_bytes(
                &ast, source,
            )))
        }
    }
}

/// Does `source` parse at all — the parse-only twin of [`tsv_parse_to_value`], for the
/// per-injection hot paths that ask nothing of the tree.
///
/// Same grammar as the format entry (`format_source` parses through the same three
/// functions, the TypeScript arm with the same unnamed-goal fallback), so "the formatter's
/// own output does not parse" is graded against exactly the parse a second format would
/// run. Builds no wire: the JSON write + read is the cost that made a full [`f1_check`]
/// per injection unaffordable for `gap_audit`, and a bare parse is ~a third of a format.
///
/// Its two callers (`gap_audit`, `ignore_audit`) are `comment_check` modules, so it is
/// gated the same way — a default-feature build would otherwise warn it unused.
#[cfg(feature = "comment_check")]
pub(crate) fn tsv_parses(source: &str, parser: ParserType) -> bool {
    let arena = bumpalo::Bump::new();
    match parser {
        ParserType::TypeScript => tsv_ts::parse_with_goal_or_fallback(source, None, &arena).is_ok(),
        ParserType::Svelte => tsv_svelte::parse(source, &arena).is_ok(),
        ParserType::Css => tsv_css::parse(source, &arena).is_ok(),
    }
}

/// Translates the wire AST's positions into byte offsets.
///
/// The wire emits **UTF-16 code-unit** offsets (`tsv_lang::location::ByteToCharMap`), not
/// byte offsets — they coincide on ASCII and diverge the moment a file holds a `é` or an
/// emoji. Slicing `source` with a raw wire offset is then off by the multi-byte count:
/// wrong regions, or a panic on a non-char-boundary. Nothing downstream can catch that —
/// an ASCII-only corpus grades identical either way — so the map is unit-tested against a
/// direct `char_indices` walk instead.
///
/// The shared wire→byte primitive of the audit substrate: the gap-injection region walk
/// ([`sites::code_regions`](crate::audit::sites) / `node_edge_key`) and the
/// `binding_audit` re-binding gate both key wire node spans against byte offsets through it,
/// so there is one map, not two inverse copies.
pub(crate) struct Utf16ToByte {
    /// `None` for an all-ASCII source, where the two spaces are identical and the table is
    /// pure overhead (the overwhelmingly common case).
    table: Option<Vec<usize>>,
    len: usize,
}

impl Utf16ToByte {
    pub(crate) fn new(source: &str) -> Self {
        if source.is_ascii() {
            return Self {
                table: None,
                len: source.len(),
            };
        }
        // One entry per UTF-16 code unit; an astral char spans two units and both map to
        // the char's byte start, so a boundary offset always lands on a char boundary.
        let mut table = Vec::with_capacity(source.len() + 1);
        for (byte, ch) in source.char_indices() {
            for _ in 0..ch.len_utf16() {
                table.push(byte);
            }
        }
        table.push(source.len());
        Self {
            table: Some(table),
            len: source.len(),
        }
    }

    /// The byte offset for a wire offset, or `None` if it is out of range.
    pub(crate) fn byte(&self, wire: usize) -> Option<usize> {
        match &self.table {
            None => (wire <= self.len).then_some(wire),
            Some(t) => t.get(wire).copied(),
        }
    }

    /// The byte-space `[start, end)` of a wire node, or `None` when it carries no span, has a
    /// malformed `end < start`, or an offset lands out of range. The wire's own positions are
    /// UTF-16, so both ends are translated through the map. The one wire-node→byte-span
    /// primitive the audit walkers share (`sites`'s region/edge collection, `node_edge`'s child
    /// walk, `blank_audit`'s verbatim-skip walk) — all `comment_check`-gated, so the method is
    /// too, else it reads as dead code in a default build (`Utf16ToByte` itself stays
    /// always-compiled for `binding_audit`, which uses only `byte`).
    #[cfg(feature = "comment_check")]
    pub(crate) fn node_byte_span(&self, node: &Value) -> Option<(usize, usize)> {
        let s = node.get("start")?.as_u64()? as usize;
        let e = node.get("end")?.as_u64()? as usize;
        if e < s {
            return None;
        }
        Some((self.byte(s)?, self.byte(e)?))
    }
}

/// Compare two ASTs for **structural** equivalence — the corruption-hunt basis.
///
/// Both are [`normalize_pair`]'d (render-normalized when `render`, then
/// location-stripped) and compared as [`structural_skeleton`]s, so legitimate
/// leaf reformatting doesn't read as corruption while an injected / dropped /
/// re-typed node still does (see `structural_skeleton` for what the skeleton
/// keeps vs erases). Char-dropping *value* corruption stays covered by the
/// complementary `corpus:compare:format` SAFETY (differential char-frequency),
/// which this deliberately does not duplicate. A **value** change that neither drops
/// characters nor changes the shape (a mis-decoded string, a miscanonicalized number,
/// a mangled multi-line comment) is invisible to both this skeleton and the SAFETY
/// frequency check — that class is [`leaf_conservation_diff`]'s, which the same two
/// commands run as a refinement **when this returns equal** (a shape change is already
/// a divergence; a shape-equal leaf change is the skeleton-blind corruption).
///
/// Returns `(structurally_equal, diff)` — the diff (only with `verbose`) shows the
/// full location-stripped values, not the skeleton, so it's readable for triage.
///
/// Shared by the `roundtrip_audit` and `fuzz` commands.
pub(crate) fn structurally_equivalent(
    a: Value,
    b: Value,
    render: bool,
    verbose: bool,
) -> (bool, Option<String>) {
    let (a, b) = normalize_pair(a, b, render);
    if structural_skeleton(&a) == structural_skeleton(&b) {
        return (true, None);
    }
    let diff = if verbose {
        match (
            serde_json::to_string_pretty(&a),
            serde_json::to_string_pretty(&b),
        ) {
            (Ok(pa), Ok(pb)) => Some(diff_to_string(&pa, &pb, &DiffOptions::ast_diff())),
            _ => None,
        }
    } else {
        None
    };
    (false, diff)
}

/// The multiset of **decode-invariant leaf values** in a wire AST — the semantically
/// conserved scalars a legitimate reformat must never change, keyed so that an equal multiset
/// means every such leaf survived the format.
///
/// [`structural_skeleton`] erases *every* scalar leaf to `Null`, so a format that still
/// parses but corrupts a leaf value — a mis-decoded string, a number canonicalized to a
/// *different* value, a mangled multi-line comment — reparses to an equal skeleton and slips
/// past [`structurally_equivalent`]. This is the complementary check: conserve the leaves
/// whose value carries meaning, ignore the ones a formatter legitimately rewrites.
///
/// ## Invariant table — conserve vs ignore
///
/// | wire field | verdict | key | why |
/// | --- | --- | --- | --- |
/// | `Literal.value` — string | **conserve** | `s:` | the decoded text; `raw` reformats, the value must not. Shares the `s:` tag with `Identifier.name` (see below) |
/// | `Literal.value` — number / bool / null | **conserve** | `v:` | the decoded value; distinct tag so a string `"1"` and a number `1` never cancel |
/// | `Literal.bigint` | **conserve** | `bigint:` | the bigint digits (`value` is null / lossy in JSON) |
/// | `Literal.regex.pattern` | **conserve** | `re.pattern:` | the regex body is opaque, must survive verbatim |
/// | `Literal.regex.flags` | **conserve** (order-free) | `re.flags:` | a set — tsv/prettier canonicalize the order, so flags are sorted before comparing; an add/remove still differs |
/// | `Identifier.name` / `PrivateIdentifier.name` | **conserve** | `s:` | the decoded identifier; **shares `s:` with a string `value`** so a quote-props key flip (`{"a": 1}` ↔ `{a: 1}`, Literal ↔ Identifier, same text) conserves |
/// | `TemplateElement.value.cooked` | **conserve** | `cooked:` | the decoded chunk (`raw` reformats) |
/// | every `raw` | **ignore** | — | the source spelling — quotes, digit separators, escapes are the formatter's to rewrite |
/// | `loc` / `start` / `end` | **ignore** | — | positions move under formatting |
/// | `extra` | **ignore** | — | acorn's source-metadata bag (trailing-comma / `parenthesized`), whose key presence itself flips |
/// | whitespace / `Text` `data` / formatting | **ignore** | — | reflow is the point |
///
/// The **string-value/name conflation** is deliberate: a value ↔ name *node-type* flip is a
/// **shape** change, which the structural skeleton owns, so the leaf check conserves the text
/// across it rather than double-reporting a legitimate quote-props rewrite as gate-fatal. It
/// stays precise on its own mandate — a **same-shape** scalar change keeps a Literal a Literal
/// and an Identifier an Identifier, so a mis-decoded string, a renamed identifier, and a
/// miscanonicalized number are all still caught.
///
/// Walks **generically** on the `type` discriminator plus field names — it does not enumerate
/// a full node set. An **unrecognized** node contributes nothing, so the check is a graceful
/// no-op over a wire shape with none of these nodes (CSS: no `Literal` / `Identifier` /
/// `TemplateElement`). CSS-value leaf conservation is a documented future extension, out of
/// scope today.
///
/// Shared by the `roundtrip_audit` and `fuzz` commands, compared input-parse vs output-parse
/// under the same parser (both tsv, or both canonical) so a leaf's representation is
/// consistent across the pair.
pub(crate) fn leaf_value_multiset(wire: &Value) -> BTreeMap<String, usize> {
    let mut leaves: Vec<String> = Vec::new();
    collect_conserved_leaves(wire, &mut leaves);
    let mut ms: BTreeMap<String, usize> = BTreeMap::new();
    for leaf in leaves {
        *ms.entry(leaf).or_insert(0) += 1;
    }
    ms
}

/// Walk `v`, pushing one role-tagged key per conserved leaf. The explicit extraction handles
/// each recognized node's scalar leaf; the generic recursion visits child *nodes*, so every
/// `Identifier` / `Literal` / `TemplateElement` in the tree contributes exactly once (a
/// scalar leaf recursed into yields nothing, so there is no double-count).
fn collect_conserved_leaves(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Object(map) => {
            match map.get("type").and_then(Value::as_str) {
                Some("Literal") => {
                    // A regex / bigint literal carries its value elsewhere than `value`
                    // (which is `{}` / null), so branch before falling back to `value`.
                    if let Some(regex) = map.get("regex").and_then(Value::as_object) {
                        if let Some(pattern) = regex.get("pattern") {
                            out.push(format!("re.pattern:{}", scalar_key(pattern)));
                        }
                        if let Some(flags) = regex.get("flags") {
                            out.push(format!("re.flags:{}", sorted_flags(flags)));
                        }
                    } else if let Some(bigint) = map.get("bigint") {
                        out.push(format!("bigint:{}", scalar_key(bigint)));
                    } else if let Some(value) = map.get("value") {
                        out.push(literal_value_key(value));
                    }
                }
                Some("Identifier" | "PrivateIdentifier") => {
                    if let Some(name) = map.get("name") {
                        out.push(format!("s:{}", scalar_key(name)));
                    }
                }
                Some("TemplateElement") => {
                    if let Some(cooked) = map.get("value").and_then(|value| value.get("cooked")) {
                        out.push(format!("cooked:{}", scalar_key(cooked)));
                    }
                }
                _ => {}
            }
            // Recurse into child nodes. `extra` / `loc` are metadata / position bags with no
            // conserved leaves — skipping them avoids walking their scalars for nothing.
            for (k, child) in map {
                if k == "extra" || k == "loc" {
                    continue;
                }
                collect_conserved_leaves(child, out);
            }
        }
        Value::Array(arr) => {
            for child in arr {
                collect_conserved_leaves(child, out);
            }
        }
        _ => {}
    }
}

/// A canonical, type-tagged key for a scalar leaf — its exact JSON serialization, so a string
/// `"1"` and a number `1` never collide and two equal values always match.
fn scalar_key(v: &Value) -> String {
    v.to_string()
}

/// A `Literal.value`'s multiset key. A **string** value shares the `s:` tag with an
/// `Identifier` name, because a property key legitimately flips between a string Literal and a
/// bare Identifier under quote normalization (`{"a": 1}` ↔ `{a: 1}`) — same text, different
/// node. That node-type flip is a *shape* change, which the structural skeleton owns; the leaf
/// check conserves the text and so must not read the flip as corruption. A **non-string**
/// value (number / bool / null) keeps a distinct `v:` tag, so a string `"1"` and a number `1`
/// never cancel.
fn literal_value_key(value: &Value) -> String {
    if value.is_string() {
        format!("s:{}", scalar_key(value))
    } else {
        format!("v:{}", scalar_key(value))
    }
}

/// Regex flags are an unordered set — tsv (like prettier) canonicalizes their order, so the
/// leaf check compares them **order-independently**: a pure reorder (`mgi` → `gim`) conserves,
/// while an added or removed flag still differs.
fn sorted_flags(flags: &Value) -> String {
    match flags.as_str() {
        Some(s) => {
            let mut chars: Vec<char> = s.chars().collect();
            chars.sort_unstable();
            chars.into_iter().collect()
        }
        // A non-string flags field is unexpected; fall back to the raw scalar key rather than
        // silently dropping the leaf.
        None => scalar_key(flags),
    }
}

/// `None` when every conserved leaf survives `input` → `output`; otherwise a compact
/// description of the divergence (leaves lost from the input, leaves gained in the output — a
/// *mangle* shows as both). The gate-fatal signal `roundtrip_audit` / `fuzz` file as a
/// leaf-value-corruption finding, distinct from the render-noisy structural-divergence bucket.
pub(crate) fn leaf_conservation_diff(input: &Value, output: &Value) -> Option<String> {
    let before = leaf_value_multiset(input);
    let after = leaf_value_multiset(output);
    if before == after {
        return None;
    }
    Some(format!(
        "leaf-value not conserved — {}",
        multiset_delta(&before, &after)
    ))
}

/// The multiset of **conserved nodes** in a wire AST — every node the formatter must carry
/// from input to output, keyed by `type`, plus the words of every template text — so an equal
/// multiset means no node was dropped, duplicated or re-typed and no word of prose was lost.
///
/// The zero-tolerance sibling of [`leaf_value_multiset`] on the OTHER axis: the leaf check
/// conserves scalar values across an equal shape, this conserves the **population** of the
/// tree across any shape. [`structural_skeleton`] already sees a dropped node — but as one more
/// entry in the render-noisy `divergent` bucket (a whitespace `Text` appearing or vanishing
/// beside a block element is a divergence too), which every consumer holds report-only. A
/// dropped `<b>x</b>` filed there was reported into noise, so this census names the class on
/// its own, where it can gate: a formatter that reorders, re-quotes or reflows never changes
/// how many `RegularElement`s a document has.
///
/// ## What is counted vs erased
///
/// Every exclusion below is a rewrite the formatter makes BY DESIGN and prettier makes too —
/// measured over the fixture tree and the prettier suites, where the census is born green.
/// The invariant is zero-tolerance, so an exclusion must name a sanctioned rewrite, never a
/// bug: a new kind of difference is a finding until it is understood.
///
/// | wire node | verdict | why |
/// | --- | --- | --- |
/// | every object with a string `type` | **count** `n:<type>` | the population |
/// | `Text` whose `data` is whitespace-only | **erase** | the formatter's own separator — appears and vanishes at every block boundary (the skeleton's noise) |
/// | `Text` in a fragment `nodes` array | **count each whitespace-split word** of `data` as `w:<word>`, not the node | a text node's WORDS are what a content drop loses, and its node count is not conserved: hoisting a `<script>` / `<style>` out from between two texts leaves them adjacent, and they reparse as one. Excluded from the word count (node counted instead): the text of a `<script>` / `<style>` the parser keeps as an element (a non-JS `type=` script, a `lang=` style), whose JS / CSS the formatter reformats as code; attribute-value text likewise counts as a node only (a `style=""` value may be reformatted) |
/// | `Fragment` | **erase** | a container, one per owner; an `{#await}` branch's fragment appears and vanishes with the branch's authored form (`{:then value}` with an empty body is dropped, `{:catch}` folds to the `catch` shorthand) — its NODES still count |
/// | `EmptyStatement` | **erase** | a stray `;` the formatter drops by design |
/// | `TSParenthesizedType` / `TSUnionType` / `TSIntersectionType` / `TSInstantiationExpression` / `ChainExpression` | **erase** | wrapper shells the formatter strips or flattens by design (a redundant type paren, a single-member `\| A`, a nested `A \| (B \| C)`, a redundant `(a?.b)?.c` chain shell); acorn-typescript reads `Base<T>` in a class heritage as an instantiation expression or not by the SPELLING of what follows, so tsv's drop-in parse does too. The shell's operands still count |
/// | a string `Literal` or an `Identifier` reached through `key` | **conflate** as `n:key` | the quote-props rewrite (`{"a": 1}` ↔ `{a: 1}`) flips the node type of a property key; the leaf check conserves the text across it |
/// | a `*Directive`'s `value` / `expression` | **skip the subtree** | shorthand normalization (`style:color={color}` → `style:color`, `let:x={x}` → `let:x`) drops the expression node; the directive and its name still count |
/// | an `AwaitBlock`'s `value` / `error` | **skip the subtree** | the binding rides the branch's authored form (above) |
/// | `comments` / `leadingComments` / `trailingComments` / `comment` | **skip the subtree** | comments have their own gates (ledger, census), and the nestled-JSDoc merge legitimately prints two parsed blocks as one; the singular `comment` is a Svelte `<style>`'s `content.comment`, a COPY of the template comment preceding the tag (which the fragment already counts once) |
/// | `extra` / `loc` | **skip the subtree** | metadata / positions |
///
/// Walks generically on `type` plus the key a subtree was reached through, so a wire shape with
/// none of the erased kinds (CSS) is simply counted whole. A `Comment` node in a Svelte fragment
/// (a template `<!-- -->`) IS counted — it is a child node, never merged.
pub(crate) fn node_census(wire: &Value) -> BTreeMap<CensusKey<'_>, usize> {
    let mut ms: BTreeMap<CensusKey<'_>, usize> = BTreeMap::new();
    collect_conserved_nodes(wire, None, false, &mut ms);
    ms
}

/// One entry of the node census — a conserved NODE, or a WORD of template text, the two things
/// the table above counts.
///
/// Borrowed from the wire rather than formatted into an owned `String` per entry: the census runs
/// over both wires of every file in a standing gate, so a `format!` per node would be pure
/// allocation. The `n:` / `w:` spellings the table and the reports use are this type's
/// [`Display`](std::fmt::Display), paid only on the entries that actually appear in a delta.
///
/// The variant order is the report order (nodes before words), matching the prefixes it renders.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum CensusKey<'a> {
    /// A conserved node, keyed by its wire `type` — or by `key`, the one position where two
    /// spellings of the same thing conflate (a quoted vs bare property key).
    Node(&'a str),
    /// A whitespace-split word of a fragment text — what a content drop loses, where the text
    /// NODE count is not conserved (see the table above).
    Word(&'a str),
}

impl std::fmt::Display for CensusKey<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Node(ty) => write!(f, "n:{ty}"),
            Self::Word(word) => write!(f, "w:{word}"),
        }
    }
}

/// Walk `v`, counting one entry per conserved node (and per word of a fragment text). `key` is the
/// object key the value was reached through (`None` at the root; an array hands its own key
/// down), which is how a text node's role — fragment child vs attribute value — and a property
/// key's position are told apart; `raw_island` is set inside a `<script>` / `<style>` element
/// the parser kept as an element, whose text is code the formatter may reformat.
fn collect_conserved_nodes<'a>(
    v: &'a Value,
    key: Option<&str>,
    raw_island: bool,
    out: &mut BTreeMap<CensusKey<'a>, usize>,
) {
    match v {
        Value::Object(map) => {
            let ty = map.get("type").and_then(Value::as_str);
            match ty {
                None
                | Some(
                    "EmptyStatement"
                    | "TSParenthesizedType"
                    | "TSUnionType"
                    | "TSIntersectionType"
                    | "TSInstantiationExpression"
                    | "ChainExpression"
                    | "Fragment",
                ) => {}
                Some("Text") => {
                    let data = map.get("data").and_then(Value::as_str).unwrap_or("");
                    if data.trim().is_empty() {
                        // The formatter's own separator.
                    } else if key == Some("nodes") && !raw_island {
                        for word in data.split_whitespace() {
                            count(out, CensusKey::Word(word));
                        }
                    } else {
                        count(out, CensusKey::Node("Text"));
                    }
                }
                Some("Identifier") if key == Some("key") => count(out, CensusKey::Node("key")),
                Some("Literal")
                    if key == Some("key") && map.get("value").is_some_and(Value::is_string) =>
                {
                    count(out, CensusKey::Node("key"));
                }
                Some(ty) => count(out, CensusKey::Node(ty)),
            }
            let is_directive = ty.is_some_and(|t| t.ends_with("Directive"));
            let is_await = ty == Some("AwaitBlock");
            let raw_island = raw_island
                || (ty == Some("RegularElement")
                    && matches!(
                        map.get("name").and_then(Value::as_str),
                        Some("script" | "style")
                    ));
            for (k, child) in map {
                let skip = matches!(
                    k.as_str(),
                    "extra"
                        | "loc"
                        | "comments"
                        | "leadingComments"
                        | "trailingComments"
                        | "comment"
                ) || (is_directive && matches!(k.as_str(), "value" | "expression"))
                    || (is_await && matches!(k.as_str(), "value" | "error"));
                if skip {
                    continue;
                }
                collect_conserved_nodes(child, Some(k), raw_island, out);
            }
        }
        Value::Array(arr) => {
            for child in arr {
                collect_conserved_nodes(child, key, raw_island, out);
            }
        }
        _ => {}
    }
}

/// Add one to `key`'s tally — the census's only mutation, named so the walk above reads as the
/// classification it is.
fn count<'a>(out: &mut BTreeMap<CensusKey<'a>, usize>, key: CensusKey<'a>) {
    *out.entry(key).or_insert(0) += 1;
}

/// `None` when every conserved node (and fragment word) survives `input` → `output`; otherwise a
/// compact description — nodes lost from the input, nodes gained in the output. The gate-fatal
/// signal every round-trip consumer files as a node-loss finding: a formatter never changes a
/// document's node population, so there is no sanctioned reading of a difference here.
pub(crate) fn node_conservation_diff(input: &Value, output: &Value) -> Option<String> {
    let before = node_census(input);
    let after = node_census(output);
    if before == after {
        return None;
    }
    Some(format!(
        "node population not conserved — {}",
        multiset_delta(&before, &after)
    ))
}

/// The `lost [...] gained [...]` rendering of two multisets' difference, shared by the two
/// conservation diffs so their reports read alike.
///
/// Generic over the key so each census keeps the key it walks with — owned for the leaf check,
/// borrowed for the node census ([`CensusKey`]) — and the spelling is paid here, on the entries
/// that actually moved, rather than on every node of both trees.
fn multiset_delta<K: Ord + std::fmt::Display>(
    before: &BTreeMap<K, usize>,
    after: &BTreeMap<K, usize>,
) -> String {
    let mut lost: Vec<String> = Vec::new();
    for (k, &n_before) in before {
        let n_after = after.get(k).copied().unwrap_or(0);
        if n_before > n_after {
            lost.push(format!("{k} (-{})", n_before - n_after));
        }
    }
    let mut gained: Vec<String> = Vec::new();
    for (k, &n_after) in after {
        let n_before = before.get(k).copied().unwrap_or(0);
        if n_after > n_before {
            gained.push(format!("{k} (+{})", n_after - n_before));
        }
    }
    format!("lost [{}] gained [{}]", lost.join(", "), gained.join(", "))
}

/// The verdict of comparing a document's parse against the parse of its formatted output —
/// the one reparse question every round-trip consumer asks, in one priority order.
pub(crate) enum ReparseCompare {
    /// Same population, same skeleton, same conserved leaves.
    Equal,
    /// A conserved node (or a word of template text) was dropped, duplicated or re-typed
    /// ([`node_conservation_diff`]) — always a bug, and the precise subset of a structural
    /// divergence. Carries the population delta.
    NodeLoss(String),
    /// The skeleton changed with the population intact ([`structurally_equivalent`]) — the soft,
    /// render-model-noisy bucket every consumer reports without gating. Carries the AST diff
    /// when the caller asked for it.
    Divergent(Option<String>),
    /// Equal skeleton, but a decode-invariant leaf value changed
    /// ([`leaf_conservation_diff`]). Carries the leaf delta.
    LeafCorruption(String),
}

/// Compare an input wire against its formatted output's wire. Node loss is asked FIRST: it is
/// a subset of structural divergence, so asked second it would be filed into the soft bucket
/// and never gate — which is exactly how a dropped element shipped. Then the skeleton (a shape
/// change is a divergence, which the skeleton owns), then — on an equal shape only — the
/// leaves. `render` toggles Svelte-5 render-time whitespace normalization before the skeleton
/// compare; `verbose` keeps the readable AST diff on a divergence.
pub(crate) fn compare_reparsed(
    wire_in: Value,
    wire_out: Value,
    render: bool,
    verbose: bool,
) -> ReparseCompare {
    if let Some(detail) = node_conservation_diff(&wire_in, &wire_out) {
        return ReparseCompare::NodeLoss(detail);
    }
    // Computed before the move-consuming structural compare.
    let leaf_diff = leaf_conservation_diff(&wire_in, &wire_out);
    let (equal, diff) = structurally_equivalent(wire_in, wire_out, render, verbose);
    if !equal {
        return ReparseCompare::Divergent(diff);
    }
    if let Some(detail) = leaf_diff {
        return ReparseCompare::LeafCorruption(detail);
    }
    ReparseCompare::Equal
}

/// The outcome of the shared format fixed-point check ([`f1_check`]) on one input — every
/// step's failure a distinct variant.
///
/// The panic-free property core the [`fuzz`](crate::cli::commands) and
/// [`blank_audit`](crate::cli::commands) commands share: parse → format → reparse →
/// node conservation → structural-skeleton compare → leaf conservation → idempotency fixed
/// point. It does **not**
/// catch panics — the caller wraps the call in
/// [`catch_unwind`](std::panic::catch_unwind) (fuzz's `attempt`, blank_audit's inject loop),
/// so a panic is the caller's own finding and this stays a pure, panic-free classifier.
///
/// `Rejected` (tsv cleanly refused) and `Ok` (every invariant held) are **not** findings; the
/// rest each name one broken invariant.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum F1Outcome {
    /// tsv's parser cleanly rejected the input — expected, not a finding.
    Rejected,
    /// Parsed, formatted idempotently, reparsed structurally equal, leaves conserved.
    Ok,
    /// Parsed, but `format` errored (should be impossible — `format` re-parses internally).
    FormatError,
    /// `format`'s output does not reparse (tsv rejects its own output).
    Unreparseable,
    /// Output reparses but a conserved node — an element, a block, a statement, a word of
    /// template text — was dropped, duplicated or re-typed. The precise, always-a-bug subset
    /// of `StructuralDivergence`, filed ahead of it so it gates.
    NodeLoss,
    /// Output reparses with an **equal skeleton** but a decode-invariant leaf value changed
    /// (a mis-decoded string, a miscanonicalized number, a mangled comment) — the
    /// skeleton-blind class.
    LeafValueCorruption,
    /// Output reparses but the document structure changed (delimiter/structure corruption).
    StructuralDivergence,
    /// `format(format(x)) != format(x)` — a non-idempotent fixed point.
    NonIdempotent,
}

impl F1Outcome {
    /// A **reliable** finding — always a real bug: `format_error` / `unreparseable` mean tsv
    /// can't round-trip its own output, `node_loss` is a dropped or duplicated node,
    /// `non_idempotent` breaks the F1 fixed point, and `leaf_value_corruption` is a
    /// still-parses value change invisible to the structural skeleton. `structural_divergence`
    /// is the soft bucket; `rejected` and `ok` aren't findings.
    pub(crate) fn is_hard(self) -> bool {
        matches!(
            self,
            Self::FormatError
                | Self::Unreparseable
                | Self::NodeLoss
                | Self::LeafValueCorruption
                | Self::NonIdempotent
        )
    }

    /// The outcome's report and `--json` label.
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Rejected => "rejected",
            Self::Ok => "ok",
            Self::FormatError => "format_error",
            Self::Unreparseable => "unreparseable",
            Self::NodeLoss => "node_loss",
            Self::LeafValueCorruption => "leaf_value_corruption",
            Self::StructuralDivergence => "structural_divergence",
            Self::NonIdempotent => "non_idempotent",
        }
    }
}

/// Run the shared format-fixed-point invariants on one already-valid-UTF-8 input. See
/// [`F1Outcome`] for the variants.
///
/// `render` toggles Svelte-5 render-time whitespace normalization before the structural
/// compare (matching `roundtrip_audit`'s default). Panic-free by contract — the caller
/// catches panics; this only ever *returns* an outcome.
///
/// Hoisted here from `fuzz::check` so a second consumer ([`blank_audit`](crate::cli::commands))
/// can drive the no-panic guard, the F1 idempotency fixed point and the reparse compare
/// without copying the sequence.
pub(crate) fn f1_check(src: &str, parser: ParserType, render: bool) -> F1Outcome {
    // 1. Parse. A clean rejection is the common, expected case for garbage / a broken splice.
    let Some(wire_in) = tsv_parse_to_value(src, parser) else {
        return F1Outcome::Rejected;
    };
    // 2. Format (parses internally — an error here means parse/format disagree).
    let Ok(f1) = format_source(src, parser) else {
        return F1Outcome::FormatError;
    };
    // 3. Reparse the output.
    let Some(wire_out) = tsv_parse_to_value(&f1, parser) else {
        return F1Outcome::Unreparseable;
    };
    // 4. Same document? Population, then shape, then leaves.
    match compare_reparsed(wire_in, wire_out, render, false) {
        ReparseCompare::NodeLoss(_) => return F1Outcome::NodeLoss,
        ReparseCompare::Divergent(_) => return F1Outcome::StructuralDivergence,
        ReparseCompare::LeafCorruption(_) => return F1Outcome::LeafValueCorruption,
        ReparseCompare::Equal => {}
    }
    // 5. Idempotent fixed point.
    match format_source(&f1, parser) {
        Ok(f2) if f2 == f1 => F1Outcome::Ok,
        Ok(_) => F1Outcome::NonIdempotent,
        Err(_) => F1Outcome::FormatError,
    }
}

#[cfg(test)]
mod node_census_tests {
    use super::*;
    use serde_json::json;

    fn fragment(nodes: Vec<Value>) -> Value {
        json!({"type": "Fragment", "nodes": nodes})
    }

    fn element(name: &str, text: &str) -> Value {
        json!({"type": "RegularElement", "name": name, "attributes": [],
               "fragment": fragment(vec![json!({"type": "Text", "data": text})])})
    }

    #[test]
    fn dropped_element_is_a_loss() {
        let a = fragment(vec![element("b", "x"), element("i", "y")]);
        let b = fragment(vec![element("i", "y")]);
        let detail = node_conservation_diff(&a, &b).expect("a dropped element must not conserve");
        assert!(detail.contains("n:RegularElement (-1)"), "{detail}");
        assert!(detail.contains("w:x (-1)"), "{detail}");
    }

    #[test]
    fn adjacent_texts_merging_conserves() {
        // `}<style>…</style>!` hoists the style out and leaves `}` and `!` one text.
        let a = fragment(vec![
            json!({"type": "Text", "data": "}"}),
            json!({"type": "Text", "data": "!"}),
        ]);
        let b = fragment(vec![json!({"type": "Text", "data": "}\n!"})]);
        assert!(node_conservation_diff(&a, &b).is_none());
    }

    #[test]
    fn whitespace_text_appearing_or_vanishing_conserves() {
        let a = fragment(vec![element("b", "x"), element("i", "y")]);
        let b = fragment(vec![
            json!({"type": "Text", "data": "\n\t"}),
            element("b", "x"),
            json!({"type": "Text", "data": " "}),
            element("i", "y"),
            json!({"type": "Text", "data": "\n"}),
        ]);
        assert!(node_conservation_diff(&a, &b).is_none());
    }

    #[test]
    fn reflowed_text_conserves_its_words() {
        let a = fragment(vec![json!({"type": "Text", "data": "a  b\n\tc"})]);
        let b = fragment(vec![json!({"type": "Text", "data": "a b c"})]);
        assert!(node_conservation_diff(&a, &b).is_none());
        let c = fragment(vec![json!({"type": "Text", "data": "a b"})]);
        assert!(node_conservation_diff(&a, &c).is_some());
    }

    #[test]
    fn attribute_value_text_counts_as_a_node_but_not_by_word() {
        let attr = |v: &str| json!({"type": "Attribute", "name": "style", "value": [{"type": "Text", "data": v}]});
        assert!(node_conservation_diff(&attr("color:red"), &attr("color: red")).is_none());
        assert!(
            node_conservation_diff(
                &attr("color:red"),
                &json!({"type": "Attribute", "name": "style", "value": true})
            )
            .is_some()
        );
    }

    #[test]
    fn sanctioned_drops_are_erased() {
        let body = |stmts: Vec<Value>| json!({"type": "Program", "body": stmts});
        let expr = json!({"type": "ExpressionStatement", "expression": {"type": "Identifier", "name": "a"}});
        let a = body(vec![expr.clone(), json!({"type": "EmptyStatement"})]);
        let b = body(vec![expr]);
        assert!(node_conservation_diff(&a, &b).is_none());
        let ty = |t: Value| json!({"type": "TSTypeAliasDeclaration", "typeAnnotation": t});
        let inner = json!({"type": "TSStringKeyword"});
        let a = ty(json!({"type": "TSParenthesizedType", "typeAnnotation": inner}));
        let b = ty(inner.clone());
        assert!(node_conservation_diff(&a, &b).is_none());
        // `type A = | string` → `type A = string`: the single-member union shell.
        let a = ty(json!({"type": "TSUnionType", "types": [inner]}));
        assert!(node_conservation_diff(&a, &b).is_none());
        // …but a dropped MEMBER is not a shell.
        let a = ty(json!({"type": "TSUnionType", "types": [inner, {"type": "TSNumberKeyword"}]}));
        assert!(node_conservation_diff(&a, &b).is_some());
    }

    #[test]
    fn quote_props_key_flip_conserves_in_key_position_only() {
        let prop = |k: Value| json!({"type": "Property", "key": k, "value": {"type": "Literal", "value": 1}});
        let quoted = prop(json!({"type": "Literal", "value": "a"}));
        let bare = prop(json!({"type": "Identifier", "name": "a"}));
        assert!(node_conservation_diff(&quoted, &bare).is_none());
        // A value-position flip is not the quote-props rewrite.
        let v = |x: Value| json!({"type": "ExpressionStatement", "expression": x});
        assert!(
            node_conservation_diff(
                &v(json!({"type": "Literal", "value": "a"})),
                &v(json!({"type": "Identifier", "name": "a"}))
            )
            .is_some()
        );
    }

    #[test]
    fn directive_shorthand_and_raw_island_words_are_not_graded() {
        let long = json!({"type": "StyleDirective", "name": "color",
            "value": [{"type": "ExpressionTag", "expression": {"type": "Identifier", "name": "color"}}]});
        let short = json!({"type": "StyleDirective", "name": "color", "value": true});
        assert!(node_conservation_diff(&long, &short).is_none());
        let script = |code: &str| {
            json!({"type": "RegularElement", "name": "script", "attributes": [],
            "fragment": fragment(vec![json!({"type": "Text", "data": code})])})
        };
        assert!(node_conservation_diff(&script("a=1;"), &script("a = 1;")).is_none());
        assert!(node_conservation_diff(&script("a=1;"), &json!({"type": "RegularElement", "name": "script", "attributes": [], "fragment": fragment(vec![])})).is_some());
    }

    #[test]
    fn comment_arrays_are_not_the_census_business() {
        let prog =
            |comments: Vec<Value>| json!({"type": "Program", "body": [], "comments": comments});
        let a = prog(vec![
            json!({"type": "Block", "value": "a"}),
            json!({"type": "Block", "value": "b"}),
        ]);
        let b = prog(vec![json!({"type": "Block", "value": "a*//*b"})]);
        assert!(node_conservation_diff(&a, &b).is_none());
    }

    #[test]
    fn style_content_comment_copy_is_not_double_counted() {
        let comment = json!({"type": "Comment", "data": " c "});
        let root = |css_comment: Option<Value>| {
            json!({"type": "Root",
            "fragment": fragment(vec![comment.clone()]),
            "css": {"type": "StyleSheet", "children": [], "content": {"styles": "", "comment": css_comment}}})
        };
        assert!(node_conservation_diff(&root(Some(comment.clone())), &root(None)).is_none());
    }

    #[test]
    fn template_comment_node_is_counted() {
        let a = fragment(vec![
            json!({"type": "Comment", "data": " c "}),
            element("b", "x"),
        ]);
        let b = fragment(vec![element("b", "x")]);
        assert!(
            node_conservation_diff(&a, &b)
                .unwrap()
                .contains("n:Comment (-1)")
        );
    }
}

#[cfg(test)]
mod leaf_tests {
    use super::*;
    use serde_json::json;

    fn str_literal(value: &str, raw: &str) -> Value {
        json!({"type": "Literal", "value": value, "raw": raw})
    }

    /// A re-quote changes `raw` but not the decoded `value` — the conserved leaf survives.
    #[test]
    fn requote_conserves_the_decoded_value() {
        let before = str_literal("a", "\"a\"");
        let after = str_literal("a", "'a'");
        assert_eq!(leaf_conservation_diff(&before, &after), None);
    }

    /// A changed decoded value is the corruption class the skeleton is blind to.
    #[test]
    fn a_changed_value_is_a_finding() {
        let before = str_literal("a", "'a'");
        let after = str_literal("b", "'b'");
        assert!(leaf_conservation_diff(&before, &after).is_some());
    }

    /// A number reformatted (`1_000` → `1000`) keeps its numeric value; a miscanonicalized
    /// one (a *different* value) is caught.
    #[test]
    fn number_value_conserved_but_miscanonicalization_caught() {
        let a = json!({"type": "Literal", "value": 1000, "raw": "1_000"});
        let b = json!({"type": "Literal", "value": 1000, "raw": "1000"});
        assert_eq!(leaf_conservation_diff(&a, &b), None);
        let c = json!({"type": "Literal", "value": 100, "raw": "100"});
        assert!(leaf_conservation_diff(&a, &c).is_some());
    }

    /// Regex body + flags are opaque and conserved; identifier names too.
    #[test]
    fn regex_and_identifier_leaves_are_conserved() {
        let re = |p: &str, f: &str| json!({"type": "Literal", "regex": {"pattern": p, "flags": f}, "raw": "/x/"});
        assert_eq!(
            leaf_conservation_diff(&re("foo", "g"), &re("foo", "g")),
            None
        );
        // A pure flag REORDER conserves (tsv/prettier canonicalize order) …
        assert_eq!(
            leaf_conservation_diff(&re("foo", "mgi"), &re("foo", "gim")),
            None
        );
        // … but an added/removed flag or a changed pattern is a real change.
        assert!(leaf_conservation_diff(&re("foo", "g"), &re("foo", "gi")).is_some());
        assert!(leaf_conservation_diff(&re("foo", "g"), &re("bar", "g")).is_some());
        let id = |n: &str| json!({"type": "Identifier", "name": n});
        assert_eq!(leaf_conservation_diff(&id("x"), &id("x")), None);
        assert!(leaf_conservation_diff(&id("x"), &id("y")).is_some());
    }

    /// The quote-props false positive that the corpus probe surfaced: a property key
    /// legitimately flips between a string Literal (`{"a": 1}`) and a bare Identifier
    /// (`{a: 1}`) under quote normalization. Same text, different node — the shape change is
    /// the skeleton's to judge, so the leaf check must **conserve** (string value and
    /// identifier name share the `s:` tag). A genuine text change is still caught.
    #[test]
    fn quote_props_key_flip_is_conserved() {
        let str_key = json!({"type": "Literal", "value": "a", "raw": "\"a\""});
        let id_key = json!({"type": "Identifier", "name": "a"});
        assert_eq!(leaf_conservation_diff(&str_key, &id_key), None);
        // But a key whose text changed is a real finding.
        let id_other = json!({"type": "Identifier", "name": "b"});
        assert!(leaf_conservation_diff(&str_key, &id_other).is_some());
        // A string value and a NUMBER never cancel (distinct tags).
        let num = json!({"type": "Literal", "value": 1, "raw": "1"});
        let str_one = json!({"type": "Literal", "value": "1", "raw": "\"1\""});
        assert!(leaf_conservation_diff(&num, &str_one).is_some());
    }

    /// `value.cooked` (not `raw`) is the conserved template chunk.
    #[test]
    fn template_element_conserves_cooked_not_raw() {
        let te = |cooked: &str, raw: &str| json!({"type": "TemplateElement", "value": {"cooked": cooked, "raw": raw}});
        assert_eq!(
            leaf_conservation_diff(&te("a", "a"), &te("a", "a\\n")),
            None
        );
        assert!(leaf_conservation_diff(&te("a", "a"), &te("b", "a")).is_some());
    }

    /// A wire shape with none of the recognized nodes (CSS-like) yields an empty multiset, so
    /// the check is a graceful no-op — never a false finding.
    #[test]
    fn unrecognized_nodes_yield_no_leaves() {
        let css_like = json!({
            "type": "Declaration",
            "property": "color",
            "value": "red",
            "children": [{"type": "Rule", "prelude": ".x"}]
        });
        assert!(leaf_value_multiset(&css_like).is_empty());
        // And two differently-"formatted" CSS-like trees compare conserved (nothing to lose).
        let other = json!({"type": "Declaration", "property": "color", "value": "#f00"});
        assert_eq!(leaf_conservation_diff(&css_like, &other), None);
    }

    /// The same value appearing twice is a multiset of two — a drop of one copy is caught
    /// even though the other survives.
    #[test]
    fn multiset_counts_duplicate_values() {
        let two = json!([str_literal("a", "'a'"), str_literal("a", "'a'")]);
        let one = json!([str_literal("a", "'a'")]);
        assert_eq!(leaf_value_multiset(&two).get("s:\"a\""), Some(&2));
        assert!(leaf_conservation_diff(&two, &one).is_some());
    }

    /// A nested `Identifier` inside a `Literal`'s recursion path is not double-counted, and
    /// child nodes are reached generically.
    #[test]
    fn nested_nodes_counted_once_each() {
        let tree = json!({
            "type": "Program",
            "body": [
                {"type": "Identifier", "name": "x"},
                {"type": "Literal", "value": "s", "raw": "'s'"}
            ]
        });
        let ms = leaf_value_multiset(&tree);
        assert_eq!(ms.get("s:\"x\""), Some(&1));
        assert_eq!(ms.get("s:\"s\""), Some(&1));
        assert_eq!(ms.len(), 2);
    }
}

#[cfg(test)]
mod coord_tests {
    use super::*;

    /// The wire→byte map, graded against a direct walk on every prefix of strings covering
    /// each width class: ASCII (1 byte / 1 unit), 2- and 3-byte BMP (n bytes / 1 unit), and
    /// astral (4 bytes / **2** units — the arm an "offset == char index" reading gets wrong).
    ///
    /// This is the only thing that can fail on a bad map: the corpus is ~all ASCII, where
    /// every arm is the identity, so a broken translation formats byte-identically.
    #[test]
    fn utf16_to_byte_matches_a_direct_walk() {
        for src in [
            "",
            "abc",
            "é",
            "aéb",
            "日本語",
            "a😀b",
            "😀😀",
            "const é = 1; // 日本\nx😀y",
        ] {
            let map = Utf16ToByte::new(src);

            // Every char boundary must round-trip: the char's UTF-16 offset maps back to
            // exactly its byte offset.
            let mut units = 0usize;
            for (byte, ch) in src.char_indices() {
                assert_eq!(
                    map.byte(units),
                    Some(byte),
                    "src {src:?}: utf16 offset {units} should be byte {byte}"
                );
                units += ch.len_utf16();
            }
            // The end offset maps to the source length, and one past it is out of range.
            assert_eq!(map.byte(units), Some(src.len()), "src {src:?}: end offset");
            assert_eq!(map.byte(units + 1), None, "src {src:?}: past the end");

            // Every produced offset is a char boundary — the property that keeps slicing
            // from panicking.
            for u in 0..=units {
                let b = map.byte(u).expect("in range");
                assert!(src.is_char_boundary(b), "src {src:?}: byte {b} mid-char");
            }
        }
    }

    /// The ASCII fast path must be indistinguishable from the table, not merely close.
    #[test]
    fn utf16_to_byte_ascii_fast_path_matches_the_table() {
        let src = "const a = 1;\n\tb();";
        let fast = Utf16ToByte::new(src);
        assert!(fast.table.is_none(), "ASCII source should skip the table");
        for u in 0..=src.len() + 1 {
            let table_answer = if u <= src.len() { Some(u) } else { None };
            assert_eq!(fast.byte(u), table_answer, "offset {u}");
        }
    }
}

// The ledger-driven property layer is only reachable through the `comment_check`
// feature (it arms `tsv_lang::comment_ledger`), so production and default
// `tsv_debug` builds compile it out entirely — the same gate the audits that
// consume it (`comment_audit`, `gap_audit`, `blank_audit`) sit behind.
#[cfg(feature = "comment_check")]
pub(crate) use ledger::*;

#[cfg(feature = "comment_check")]
mod ledger {
    use tsv_cli::cli::format_source::format_source;
    use tsv_cli::cli::input::ParserType;
    use tsv_lang::comment_ledger::{self, CommentFinding};

    /// What one ledger-armed format did.
    pub(crate) enum Formatted {
        /// The parser or printer panicked — a finding in its own right (a comment in a gap
        /// must never crash the formatter).
        Panicked,
        /// The source did not parse, so the injection is not a legal comment here. The
        /// overwhelmingly common case, and **not** a finding: it means the offset names no gap.
        Rejected,
        /// Formatted.
        Ok {
            /// The ledger's findings — normally empty.
            findings: Vec<CommentFinding>,
            /// PROTOTYPE (measurement only): the render-time swallow reports drained from
            /// the same format. Empty unless `swallow::set_swallow_check(true)` is armed.
            swallows: Vec<tsv_lang::doc::swallow::SwallowReport>,
            /// The source text of every comment the document registered — the `verify_example`
            /// content oracle. Populated only by [`ledger_format_with_comments`] (empty under
            /// the hot-path [`ledger_format`], which never reads it), so the per-injection loop
            /// pays nothing to clone comment texts it doesn't use.
            comments: Vec<String>,
            /// The formatted text, already built by `format_source` — free to carry.
            output: String,
        },
    }

    /// Format `src` with the ledger armed and drain it — **without** collecting comment texts
    /// (the per-injection hot path, which reads only `findings`). See
    /// [`ledger_format_with_comments`] for the verify path.
    pub(crate) fn ledger_format(src: &str, parser: ParserType) -> Formatted {
        ledger_format_inner(src, parser, false)
    }

    /// [`ledger_format`], but also reads back the registered comment texts into
    /// [`Formatted::Ok::comments`]. Used by `gap_audit`'s self-verify, which decides a
    /// finding by the multiset of comment *contents* in the input vs the output. Reads the
    /// texts (via [`comment_ledger::parsed_comment_texts`]) **before** the drain discards them.
    pub(crate) fn ledger_format_with_comments(src: &str, parser: ParserType) -> Formatted {
        ledger_format_inner(src, parser, true)
    }

    /// The shared body. Drains on every path, including the failing ones: the ledger is
    /// thread-local and keyed on source identity, so a straggler left by a rejected parse could
    /// otherwise be attributed to the next injection.
    fn ledger_format_inner(src: &str, parser: ParserType, collect_comments: bool) -> Formatted {
        let _ = comment_ledger::take_comment_ledger();
        let _ = tsv_lang::doc::swallow::take_swallow_reports();
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| format_source(src, parser)));
        match result {
            Err(_) => {
                let _ = comment_ledger::take_comment_ledger();
                let _ = tsv_lang::doc::swallow::take_swallow_reports();
                Formatted::Panicked
            }
            Ok(Err(_)) => {
                let _ = comment_ledger::take_comment_ledger();
                let _ = tsv_lang::doc::swallow::take_swallow_reports();
                Formatted::Rejected
            }
            Ok(Ok(output)) => {
                // Read the texts before the drain — `take_comment_ledger` discards them.
                let comments = if collect_comments {
                    comment_ledger::parsed_comment_texts()
                } else {
                    Vec::new()
                };
                let ledger = comment_ledger::take_comment_ledger();
                let swallows = tsv_lang::doc::swallow::take_swallow_reports();
                Formatted::Ok {
                    findings: ledger.findings,
                    swallows,
                    comments,
                    output,
                }
            }
        }
    }

    /// The pristine-format outcome for a seed file: whether it is injectable, and if so the byte
    /// spans of the comments it already holds plus the formatted output.
    ///
    /// The audit checks a file is clean *as authored* before injecting. `Clean` also carries the
    /// existing comment spans so `injection_sites` can skip a site that falls strictly *inside*
    /// one — injecting there mutilates the author's comment (a `line` payload terminates it
    /// early) rather than probing a gap, which reads as a false drop.
    pub(crate) enum Pristine {
        /// Rejected, panicked, or already dirty — not injected into. `dirty` distinguishes the
        /// already-had-findings case (reported) from the doesn't-parse case (silently skipped).
        Skip { dirty: bool },
        /// Clean; carries the byte spans of the comments the seed already holds (empty when it
        /// has none) and the formatted output the check already paid for — `format_source(src)`,
        /// free to carry (see [`pristine_format`]).
        Clean {
            comment_spans: Vec<tsv_lang::Span>,
            /// The pristine format's own output. `blank_audit` grades its fast path against it
            /// (the pristine output) and `ignore_audit` gates on `output == src` (the strict
            /// fixed-point check) — both formerly re-formatted the seed to recompute exactly
            /// this string, one whole format per file.
            output: String,
        },
    }

    /// Format `src` once to check it is clean AND capture its registered comment spans (plus the
    /// formatted output itself, so a consumer needing it doesn't format the seed a second time).
    ///
    /// Kept separate from [`ledger_format`] because it reads the spans **before** the drain (via
    /// [`comment_ledger::parsed_comment_spans`], which the drain discards). Only the once-per-file
    /// pristine check needs them; the per-injection hot path only ever drains and must not pay to
    /// collect them.
    pub(crate) fn pristine_format(src: &str, parser: ParserType) -> Pristine {
        let _ = comment_ledger::take_comment_ledger();
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| format_source(src, parser)));
        match result {
            // A seed that panics or doesn't parse is not injectable — nothing to report, just skip.
            Err(_) | Ok(Err(_)) => {
                let _ = comment_ledger::take_comment_ledger();
                Pristine::Skip { dirty: false }
            }
            Ok(Ok(output)) => {
                // Pass `src` itself, so `document_key(src)` matches the host document by pointer
                // identity and the spans are strictly host-absolute (a nested `<style>` island
                // registers under its own key and is excluded — see `parsed_comment_spans`).
                let comment_spans = comment_ledger::parsed_comment_spans(src);
                let ledger = comment_ledger::take_comment_ledger();
                if ledger.findings.is_empty() {
                    Pristine::Clean {
                        comment_spans,
                        output,
                    }
                } else {
                    Pristine::Skip { dirty: true }
                }
            }
        }
    }

    /// Whether ONE of a shape's examples survives an **observational** re-check, independent of
    /// the ledger that reported it.
    ///
    /// The decision is the multiset of comment *contents* in the injected input vs the format's
    /// output (see `gap_audit::verify_example`), which supersedes the earlier
    /// `parsed - dropped + double` count comparison — the count had two named blind spots
    /// (a balancing drop+dup nets zero, and a *mangled* rebuild is count-invariant), both of
    /// which the content multiset closes.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub(crate) enum Verdict {
        /// Re-formatting really does lose, mangle, or duplicate a comment — the content
        /// multiset of the output differs from the input's.
        Confirmed,
        /// The claim did not reproduce — but WHY matters, and the causes are not one thing:
        /// most are instrument gaps or stale examples, while [`UnverifiedCause::OutputUnparseable`]
        /// and [`UnverifiedCause::OutputPanicked`] are a **real bug class** no other gate sees.
        Unconfirmed(UnverifiedCause),
    }

    /// Which of `verify_example`'s early-exit paths declined to confirm — the split that stops
    /// one word ("UNCONFIRMED") from conflating an instrument artifact with a live corruption
    /// class. Ordered by where the path sits in the verify sequence.
    #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
    pub(crate) enum UnverifiedCause {
        /// The example's seed file could not be re-read.
        SeedUnreadable,
        /// The recorded payload label resolves to no payload (report/record drift).
        PayloadUnknown,
        /// The recorded injection offset is out of range or mid-`char`.
        OffsetInvalid,
        /// Re-splicing the payload no longer parses — the injection is rejected on the re-run.
        InjectionRejected,
        /// Re-splicing the payload panics the formatter on the re-run.
        InjectionPanicked,
        /// The re-run formats cleanly with NO ledger findings — the example no longer fires.
        NoLongerFires,
        /// ⚠ The formatter's **own output does not reparse** after the injection. A real bug —
        /// an injected comment produced output tsv itself rejects — and invisible to
        /// `roundtrip_audit`, whose gate only formats files AS AUTHORED. Triage first.
        OutputUnparseable,
        /// ⚠ Re-formatting the formatter's own output PANICS. The output-side sibling of
        /// [`Self::OutputUnparseable`], and equally a real bug class.
        OutputPanicked,
        /// The output holds the same comment *contents* as its input, yet the ledger filed a
        /// finding. Something printed the comment without recording the emit (an instrument
        /// gap) — real that the ledger's account is off, but not the content loss it is filed
        /// as. The one cause that was the label's original documented meaning.
        ContentConserved,
    }

    impl UnverifiedCause {
        pub(crate) const ALL: [Self; 9] = [
            Self::SeedUnreadable,
            Self::PayloadUnknown,
            Self::OffsetInvalid,
            Self::InjectionRejected,
            Self::InjectionPanicked,
            Self::NoLongerFires,
            Self::OutputUnparseable,
            Self::OutputPanicked,
            Self::ContentConserved,
        ];
        pub(crate) const COUNT: usize = Self::ALL.len();

        /// The tally-array slot — [`Self::ALL`]'s order, pinned by a test below.
        pub(crate) fn index(self) -> usize {
            self as usize
        }

        /// The report/JSON label. The two output-side causes are UPPERCASE deliberately —
        /// they are the real-bug half of the split and must not skim as one more artifact.
        pub(crate) fn label(self) -> &'static str {
            match self {
                Self::SeedUnreadable => "seed-unreadable",
                Self::PayloadUnknown => "payload-unknown",
                Self::OffsetInvalid => "offset-invalid",
                Self::InjectionRejected => "injection-rejected",
                Self::InjectionPanicked => "injection-panicked",
                Self::NoLongerFires => "no-longer-fires",
                Self::OutputUnparseable => "OUTPUT-UNPARSEABLE",
                Self::OutputPanicked => "OUTPUT-PANICKED",
                Self::ContentConserved => "content-conserved",
            }
        }

        /// Whether this cause is itself a **real bug** rather than an instrument/staleness
        /// artifact — the output-side pair, where the formatter's own output fails to survive
        /// a re-format. These get their own loud accounting on every report path.
        pub(crate) fn is_output_bug(self) -> bool {
            matches!(self, Self::OutputUnparseable | Self::OutputPanicked)
        }
    }

    /// A shape's self-verification tally across its kept examples — the ratio that separates
    /// "uniformly an instrument gap" from "a mixed real drop", plus the per-cause split of the
    /// unconfirmed side and the anchor-probe tallies over the confirmed side.
    #[derive(Clone, Copy, Debug, Default)]
    pub(crate) struct VerifyOutcome {
        /// Examples whose ledger claim was reproduced against the output.
        pub(crate) confirmed: usize,
        /// Examples verified — up to `VERIFY_EXAMPLES`, never zero for a recorded shape.
        pub(crate) total: usize,
        /// Per-[`UnverifiedCause`] tallies of the unconfirmed examples, indexed by
        /// [`UnverifiedCause::index`]. Sums to `total - confirmed`.
        pub(crate) unconfirmed_causes: [usize; UnverifiedCause::COUNT],
        /// Confirmed examples the anchor probe re-ran (a leading space ahead of the payload) —
        /// see `gap_audit::anchor_probe`. Excludes inconclusive probes (padded splice rejected).
        pub(crate) anchor_probed: usize,
        /// Of [`Self::anchor_probed`], how many the leading space RESCUED (no findings on the
        /// padded splice) — the fixed-offset-anchor signature. A hint, never a verdict.
        pub(crate) anchor_rescued: usize,
    }

    impl VerifyOutcome {
        pub(crate) fn summary(self) -> VerifySummary {
            match self.confirmed {
                // A recorded shape always has ≥1 example, so `total == 0` is unreachable; treat
                // it as clean rather than risk a divide-by-nothing reading.
                _ if self.total == 0 => VerifySummary::Clean,
                0 => VerifySummary::Unconfirmed,
                c if c == self.total => VerifySummary::Clean,
                _ => VerifySummary::Partial,
            }
        }

        /// How many unconfirmed examples carry the given cause.
        pub(crate) fn cause_count(self, cause: UnverifiedCause) -> usize {
            self.unconfirmed_causes[cause.index()]
        }

        /// Unconfirmed examples on a **real-bug** cause (the output-side pair) — nonzero means
        /// this shape's injection makes the formatter emit output it then rejects or crashes on.
        pub(crate) fn output_bug_examples(self) -> usize {
            UnverifiedCause::ALL
                .into_iter()
                .filter(|c| c.is_output_bug())
                .map(|c| self.cause_count(c))
                .sum()
        }

        /// The nonzero unconfirmed causes as `(label, count)` pairs, [`UnverifiedCause::ALL`]-
        /// ordered — the report envelope's per-shape cause list (empty when fully confirmed).
        pub(crate) fn nonzero_causes(self) -> Vec<(&'static str, usize)> {
            nonzero_unverified_causes(&self.unconfirmed_causes)
        }

        /// The nonzero causes as `N label` fragments, [`UnverifiedCause::ALL`]-ordered.
        fn cause_breakdown(self) -> String {
            unverified_cause_breakdown(&self.unconfirmed_causes)
        }

        /// The anchor-probe suffix — empty unless a probe RESCUED an example (the interesting
        /// signal; "probed, still fires" is silent since it merely fails to add information).
        fn anchor_label(self) -> String {
            if self.anchor_rescued > 0 {
                format!(
                    "  ⚑ ANCHOR? ({}/{} rescued by a leading space)",
                    self.anchor_rescued, self.anchor_probed
                )
            } else {
                String::new()
            }
        }

        /// The report suffix — the anchor hint alone when every example confirmed, else the
        /// `confirmed/total` ratio behind an `UNCONFIRMED` / `PARTIAL` label with the per-cause
        /// breakdown of the unconfirmed side.
        pub(crate) fn report_label(self) -> String {
            let anchor = self.anchor_label();
            match self.summary() {
                VerifySummary::Clean => anchor,
                VerifySummary::Unconfirmed => format!(
                    "  ⚠ UNCONFIRMED (0/{}: {}){anchor}",
                    self.total,
                    self.cause_breakdown()
                ),
                VerifySummary::Partial => format!(
                    "  ⚠ PARTIAL ({}/{} confirmed; unconfirmed: {}){anchor}",
                    self.confirmed,
                    self.total,
                    self.cause_breakdown()
                ),
            }
        }
    }

    /// The nonzero entries of a per-cause tally (indexed by [`UnverifiedCause::index`]) as
    /// `(label, count)` pairs, [`UnverifiedCause::ALL`]-ordered — the one filter both cause
    /// renderings and the report envelope's per-shape list go through.
    pub(crate) fn nonzero_unverified_causes(
        totals: &[usize; UnverifiedCause::COUNT],
    ) -> Vec<(&'static str, usize)> {
        UnverifiedCause::ALL
            .into_iter()
            .filter_map(|c| {
                let n = totals[c.index()];
                (n > 0).then_some((c.label(), n))
            })
            .collect()
    }

    /// Render a per-cause tally as `N label` fragments joined with `, ` — the per-shape
    /// [`VerifyOutcome::report_label`] and the run-level verify summaries share it.
    pub(crate) fn unverified_cause_breakdown(totals: &[usize; UnverifiedCause::COUNT]) -> String {
        nonzero_unverified_causes(totals)
            .into_iter()
            .map(|(label, n)| format!("{n} {label}"))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// The three-way per-shape verdict once every kept example has been re-checked.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub(crate) enum VerifySummary {
        /// Every kept example reproduced — the finding is what it says it is.
        Clean,
        /// Some examples reproduced, some didn't — a mixed real drop.
        Partial,
        /// No kept example reproduced — uniformly an instrument gap (likely mangles, not drops).
        Unconfirmed,
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn out(confirmed: usize, total: usize) -> VerifyOutcome {
            VerifyOutcome {
                confirmed,
                total,
                ..VerifyOutcome::default()
            }
        }

        /// The verify ratio's three-way split. Arithmetic, so no corpus run grades it.
        #[test]
        fn verify_outcome_splits_clean_partial_unconfirmed() {
            assert_eq!(out(5, 5).summary(), VerifySummary::Clean);
            assert_eq!(out(1, 1).summary(), VerifySummary::Clean);
            assert_eq!(out(0, 5).summary(), VerifySummary::Unconfirmed);
            assert_eq!(out(2, 5).summary(), VerifySummary::Partial);
        }

        /// [`UnverifiedCause::index`] must agree with [`UnverifiedCause::ALL`]'s order — the
        /// tally array is indexed by one and iterated by the other, and a drift silently
        /// mislabels every cause count.
        #[test]
        fn unverified_cause_index_matches_all_order() {
            for (i, cause) in UnverifiedCause::ALL.into_iter().enumerate() {
                assert_eq!(cause.index(), i, "{cause:?} out of place");
            }
            assert_eq!(UnverifiedCause::COUNT, UnverifiedCause::ALL.len());
        }

        /// The report label carries the per-cause breakdown, and the output-side real-bug pair
        /// is counted by [`VerifyOutcome::output_bug_examples`].
        #[test]
        fn report_label_names_causes_and_output_bugs_are_counted() {
            assert_eq!(out(5, 5).report_label(), "", "clean flags nothing");

            let mut unconfirmed = out(0, 5);
            unconfirmed.unconfirmed_causes[UnverifiedCause::ContentConserved.index()] = 4;
            unconfirmed.unconfirmed_causes[UnverifiedCause::OutputUnparseable.index()] = 1;
            assert_eq!(
                unconfirmed.report_label(),
                "  ⚠ UNCONFIRMED (0/5: 1 OUTPUT-UNPARSEABLE, 4 content-conserved)"
            );
            assert_eq!(unconfirmed.output_bug_examples(), 1);

            let mut partial = out(2, 5);
            partial.unconfirmed_causes[UnverifiedCause::NoLongerFires.index()] = 3;
            assert_eq!(
                partial.report_label(),
                "  ⚠ PARTIAL (2/5 confirmed; unconfirmed: 3 no-longer-fires)"
            );
            assert_eq!(partial.output_bug_examples(), 0);
        }

        /// The anchor hint appends to any summary when a probe rescued an example, and stays
        /// silent otherwise — "probed, still fires" adds no information.
        #[test]
        fn anchor_label_appends_only_when_rescued() {
            let mut clean = out(3, 3);
            clean.anchor_probed = 3;
            assert_eq!(clean.report_label(), "", "probed-but-not-rescued is silent");
            clean.anchor_rescued = 2;
            assert_eq!(
                clean.report_label(),
                "  ⚑ ANCHOR? (2/3 rescued by a leading space)"
            );

            let mut partial = out(1, 2);
            partial.unconfirmed_causes[UnverifiedCause::ContentConserved.index()] = 1;
            partial.anchor_probed = 1;
            partial.anchor_rescued = 1;
            assert_eq!(
                partial.report_label(),
                "  ⚠ PARTIAL (1/2 confirmed; unconfirmed: 1 content-conserved)  \
                 ⚑ ANCHOR? (1/1 rescued by a leading space)"
            );
        }
    }
}

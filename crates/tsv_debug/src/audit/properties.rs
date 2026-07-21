//! Per-input properties an audit checks — the panic-safe primitives that turn a
//! source string into a verdict.
//!
//! This is the shared home for the **input → property** layer that every audit
//! in the [`audit`](crate::audit) substrate builds on:
//!
//! - **reparse** — [`tsv_parse_to_value`] (parse to the wire `Value`),
//!   [`structurally_equivalent`] (the structural-skeleton compare), and
//!   [`leaf_conservation_diff`] / [`leaf_value_multiset`] (the complementary
//!   decode-invariant leaf-value check that the skeleton, erasing every scalar,
//!   is blind to) — the round-trip primitives the `roundtrip_audit` / `fuzz`
//!   commands share.
//! - **ledger** (behind the `comment_check` feature) — [`ledger_format`] /
//!   [`ledger_format_with_comments`] / [`pristine_format`] drive `format_source`
//!   with the print-once comment ledger armed, and the [`Verdict`] /
//!   [`VerifyOutcome`] / [`VerifySummary`] verdict types turn a ledger claim
//!   into a falsifiable, self-verified outcome. `gap_audit` and `blank_audit` are
//!   the consumers.
//!
//! The shared property set is still growing: [`f1_check`] now lives here — the
//! core that (wrapped in `catch_unwind` by its callers) drives the no-panic
//! guard, the F1 idempotency fixed point, the reparse-skeleton compare, and the
//! leaf-conservation check — and `fuzz` consumes it (as does `blank_audit`). Still
//! pending: `roundtrip_audit`'s phase-1 reparse gate has not yet migrated onto the
//! substrate.

use std::collections::BTreeMap;

use serde_json::Value;

use tsv_cli::cli::format_source::format_source;
use tsv_cli::cli::input::ParserType;

use crate::diff::{DiffOptions, diff_to_string};
use crate::render_normalize::{normalize_pair, structural_skeleton};

/// Parse `source` with tsv's own parser and convert to the wire-JSON `Value`
/// (the same shape the canonical ASTs use). `None` on a tsv parse error.
///
/// The parse-to-wire primitive shared across the audit substrate — the
/// `roundtrip_audit` / `fuzz` round-trips and the gap audit's Svelte region walk
/// ([`sites::code_regions`](crate::audit::sites)) all reduce a source string to
/// this `Value`.
pub(crate) fn tsv_parse_to_value(source: &str, parser: ParserType) -> Option<Value> {
    let arena = bumpalo::Bump::new();
    let mut interner = tsv_lang::Interner::new();
    match parser {
        ParserType::TypeScript => {
            let ast = tsv_ts::parse(source, &arena, &mut interner).ok()?;
            Some(tsv_ts::convert_ast_json(&ast, source, &interner))
        }
        ParserType::Svelte => {
            let ast = tsv_svelte::parse(source, &arena, &mut interner).ok()?;
            Some(tsv_svelte::convert_ast_json(&ast, source, &interner))
        }
        ParserType::Css => {
            let ast = tsv_css::parse(source, &arena).ok()?;
            Some(tsv_css::convert_ast_json(&ast, source))
        }
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
    let mut lost: Vec<String> = Vec::new();
    for (k, &n_before) in &before {
        let n_after = after.get(k).copied().unwrap_or(0);
        if n_before > n_after {
            lost.push(format!("{k} (-{})", n_before - n_after));
        }
    }
    let mut gained: Vec<String> = Vec::new();
    for (k, &n_after) in &after {
        let n_before = before.get(k).copied().unwrap_or(0);
        if n_after > n_before {
            gained.push(format!("{k} (+{})", n_after - n_before));
        }
    }
    Some(format!(
        "leaf-value not conserved — lost [{}] gained [{}]",
        lost.join(", "),
        gained.join(", ")
    ))
}

/// The outcome of the shared format fixed-point check ([`f1_check`]) on one input — every
/// step's failure a distinct variant.
///
/// The panic-free property core the [`fuzz`](crate::cli::commands) and
/// [`blank_audit`](crate::cli::commands) commands share: parse → format → reparse →
/// structural-skeleton compare → leaf conservation → idempotency fixed point. It does **not**
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
    /// Output reparses with an **equal skeleton** but a decode-invariant leaf value changed
    /// (a mis-decoded string, a miscanonicalized number, a mangled comment) — the
    /// skeleton-blind class.
    LeafValueCorruption,
    /// Output reparses but the document structure changed (delimiter/structure corruption).
    StructuralDivergence,
    /// `format(format(x)) != format(x)` — a non-idempotent fixed point.
    NonIdempotent,
}

/// Run the shared format-fixed-point invariants on one already-valid-UTF-8 input. See
/// [`F1Outcome`] for the variants.
///
/// `render` toggles Svelte-5 render-time whitespace normalization before the structural
/// compare (matching `roundtrip_audit`'s default). Panic-free by contract — the caller
/// catches panics; this only ever *returns* an outcome.
///
/// Hoisted here from `fuzz::check` so a second consumer ([`blank_audit`](crate::cli::commands))
/// can drive the no-panic guard, the F1 idempotency fixed point, the reparse-skeleton compare,
/// and the leaf-conservation check without copying the six-step sequence — exactly the
/// migration this module's docs anticipate.
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
    // 4. Same document (structure)? Compute the leaf diff first, before the move-consuming
    //    structural compare.
    let leaf_changed = leaf_conservation_diff(&wire_in, &wire_out).is_some();
    let (equal, _) = structurally_equivalent(wire_in, wire_out, render, false);
    if !equal {
        return F1Outcome::StructuralDivergence;
    }
    // 5. Same shape, but a decode-invariant leaf value changed — the skeleton-blind class.
    if leaf_changed {
        return F1Outcome::LeafValueCorruption;
    }
    // 6. Idempotent fixed point.
    match format_source(&f1, parser) {
        Ok(f2) if f2 == f1 => F1Outcome::Ok,
        Ok(_) => F1Outcome::NonIdempotent,
        Err(_) => F1Outcome::FormatError,
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
    /// spans of the comments it already holds.
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
        /// has none).
        Clean { comment_spans: Vec<tsv_lang::Span> },
    }

    /// Format `src` once to check it is clean AND capture its registered comment spans.
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
            Ok(Ok(_output)) => {
                // Pass `src` itself, so `document_key(src)` matches the host document by pointer
                // identity and the spans are strictly host-absolute (a nested `<style>` island
                // registers under its own key and is excluded — see `parsed_comment_spans`).
                let comment_spans = comment_ledger::parsed_comment_spans(src);
                let ledger = comment_ledger::take_comment_ledger();
                if ledger.findings.is_empty() {
                    Pristine::Clean { comment_spans }
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
        /// The output holds the same comment *contents* as its input, yet the ledger filed a
        /// finding. Something printed the comment without recording the emit (an instrument
        /// gap) — real that the ledger's account is off, but not the content loss it is filed
        /// as.
        Unconfirmed,
    }

    /// A shape's self-verification tally across its kept examples — the ratio that separates
    /// "uniformly an instrument gap" from "a mixed real drop".
    #[derive(Clone, Copy, Debug)]
    pub(crate) struct VerifyOutcome {
        /// Examples whose ledger claim was reproduced against the output.
        pub(crate) confirmed: usize,
        /// Examples verified — up to `VERIFY_EXAMPLES`, never zero for a recorded shape.
        pub(crate) total: usize,
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

        /// The report suffix — empty when every example confirmed (nothing to flag), else the
        /// `confirmed/total` ratio behind an `UNCONFIRMED` / `PARTIAL` label.
        pub(crate) fn report_label(self) -> String {
            let ratio = format!("({}/{} confirmed)", self.confirmed, self.total);
            match self.summary() {
                VerifySummary::Clean => String::new(),
                VerifySummary::Unconfirmed => format!("  ⚠ UNCONFIRMED {ratio}"),
                VerifySummary::Partial => format!("  ⚠ PARTIAL {ratio}"),
            }
        }
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

        /// The verify ratio's three-way split, and the labels that carry it. Arithmetic, so no
        /// corpus run grades it.
        #[test]
        fn verify_outcome_splits_clean_partial_unconfirmed() {
            let out = |confirmed, total| VerifyOutcome { confirmed, total };
            assert_eq!(out(5, 5).summary(), VerifySummary::Clean);
            assert_eq!(out(1, 1).summary(), VerifySummary::Clean);
            assert_eq!(out(0, 5).summary(), VerifySummary::Unconfirmed);
            assert_eq!(out(2, 5).summary(), VerifySummary::Partial);

            assert_eq!(out(5, 5).report_label(), "", "clean flags nothing");
            assert_eq!(out(0, 5).report_label(), "  ⚠ UNCONFIRMED (0/5 confirmed)");
            assert_eq!(out(2, 5).report_label(), "  ⚠ PARTIAL (2/5 confirmed)");
        }
    }
}

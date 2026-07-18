//! Site enumeration and shape keying — where a gap-injection audit may put a
//! comment, and the file-independent name it dedups a finding by.
//!
//! Two halves:
//!
//! - **enumeration** — [`code_regions`] names the byte spans the AST says are JS
//!   or CSS (so an injection lands where the payload IS lexed as a comment), and
//!   [`injection_sites`] turns those into the concrete offsets to inject at,
//!   minus the two classes that are a gap in no real document (a word interior,
//!   an existing comment's interior).
//! - **shape keying** — [`site_shape`] collapses a `(file, offset)` finding to a
//!   compact, file-independent name (`import⟨⟩.`, `IDENT⟨⟩=`), the dedup/ratchet
//!   key of the whole report, and [`snippet`] is its human companion.
//!
//! Pure Rust, no sidecar. See the `gap_audit` command's module docs for why sites
//! are byte offsets and not tokens, and why tsv's own (deliberately permissive)
//! parser cannot answer "is this a comment context" for Svelte markup.

use tsv_cli::cli::input::ParserType;

use crate::audit::properties::tsv_parse_to_value;

/// Whether `c` can sit inside an identifier — the site filter's notion of "in a word".
fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$'
}

/// `(start, end)` of a wire node, when it carries a span — in the wire's own position
/// space, **not** bytes. See [`Utf16ToByte`].
fn span_of(node: &serde_json::Value) -> Option<(usize, usize)> {
    let s = node.get("start")?.as_u64()? as usize;
    let e = node.get("end")?.as_u64()? as usize;
    (e >= s).then_some((s, e))
}

/// Translates the wire AST's positions into byte offsets.
///
/// The wire emits **UTF-16 code-unit** offsets (`tsv_lang::location::ByteToCharMap`), not
/// byte offsets — they coincide on ASCII and diverge the moment a file holds a `é` or an
/// emoji. Slicing `source` with a raw wire offset is then off by the multi-byte count:
/// wrong regions, or a panic on a non-char-boundary. Nothing downstream can catch that —
/// an ASCII-only corpus grades identical either way — so the map is unit-tested against a
/// direct `char_indices` walk instead.
struct Utf16ToByte {
    /// `None` for an all-ASCII source, where the two spaces are identical and the table is
    /// pure overhead (the overwhelmingly common case).
    table: Option<Vec<usize>>,
    len: usize,
}

impl Utf16ToByte {
    fn new(source: &str) -> Self {
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
    fn byte(&self, wire: usize) -> Option<usize> {
        match &self.table {
            None => (wire <= self.len).then_some(wire),
            Some(t) => t.get(wire).copied(),
        }
    }
}

/// Collect the ranges the payload would actually be **lexed as a comment** in.
///
/// For a `.ts` / `.css` file that is the whole file. For `.svelte` it must be asked of the
/// AST, because the markup around the code is *not* a comment context — see the `gap_audit`
/// module docs on why tsv's own acceptance can't be used for this.
///
/// Svelte regions come from **two walks**, because no one AST expresses them all:
///
/// - [`collect_regions`] over the **wire** shape names the two carriers a canonical node's
///   own span already is: a `Script`'s `content` (the `Program` span is exactly the
///   `>`-to-`</script>` region), and an `ExpressionTag`'s brace interior
///   (`{ /* c */ x.y }` is legal). It finds them by recursive walk, so it cannot miss a
///   path an `ExpressionTag` hides in (an attribute value, a `<svelte:element this={…}>`).
/// - [`svelte_only_regions`] over tsv's **internal** AST names the rest, which exist only
///   as tsv's own parse bookkeeping: a block's `opening_tag_span` and a directive's
///   `head_span`. Svelte's AST carries neither, so the wire cannot express them — an
///   `IfBlock`'s span covers the whole block (body included) and its `test` span is the
///   expression alone, so the head is not derivable from either without a scan.
///
/// TODO: `<style>` content is still unnamed, so no comment is probed there. `Style` carries
/// a `content_span` that names it in one line, and the prerequisite is now met — the ledger
/// registers CSS in-block `CssBlockChild::Comment` AST nodes (a declaration-VALUE comment is
/// still never lexed as a `Comment`, so it stays outside the model by construction), so
/// probing `<style>` now exercises a genuinely guarded surface rather than mostly a
/// registration gap. What remains is a **yield / cost** call: naming this region measured
/// +154k sites (+20% gate runtime) over `tests/fixtures` under the old scope, and its finding
/// surface wants re-measuring against the extended ledger before it earns that cost. `<style>`
/// probing stays a follow-up.
pub(crate) fn code_regions(source: &str, parser: ParserType) -> Vec<(usize, usize)> {
    match parser {
        ParserType::TypeScript | ParserType::Css => vec![(0, source.len())],
        ParserType::Svelte => {
            let Some(wire) = tsv_parse_to_value(source, parser) else {
                return Vec::new();
            };
            let mut wire_spans = Vec::new();
            collect_regions(&wire, &mut wire_spans);
            let map = Utf16ToByte::new(source);
            let mut byte_spans: Vec<(usize, usize)> = wire_spans
                .into_iter()
                .filter_map(|(s, e)| Some((map.byte(s)?, map.byte(e)?)))
                .collect();
            byte_spans.extend(svelte_only_regions(source));
            merge_regions(byte_spans)
        }
    }
}

/// The Svelte regions the wire shape cannot express — read off tsv's internal AST.
///
/// Every one is a **head**: the run from a construct's opening delimiter to the code it
/// introduces. Svelte's public AST records only the finished expression, so a head is not
/// derivable from it; tsv's parser already keeps the two spans this needs
/// (`opening_tag_span`, `head_span`) for its own comment lookup.
///
/// **Interiors only** — never the enclosing delimiter's outside. A tag's `}` is where the
/// code region ends; the byte *after* it is markup (harmless, but noise), and for a
/// directive it is the middle of an element tag, where tsv over-accepts a comment Svelte
/// would reject (the `<script lang="ts"/* c */>` class the `gap_audit` module docs name). So
/// a block / tag contributes `span` minus its two delimiters, and a directive contributes
/// `head_span.end ..= span.end - 1`, which stops on the closing `}`.
///
/// What this deliberately does **not** do is filter the positions within a head where a
/// comment is illegal — `{#each list as ⟨⟩item}` and `{#await p then ⟨⟩v}` are Svelte's
/// own hand-read pattern slots, not acorn's, and it rejects a comment in them. No
/// whitelist is needed because **tsv rejects there too**, so `Formatted::Rejected` filters
/// them exactly as it does a word interior. The same covers `{⟨⟩#if` and `{#⟨⟩if`.
fn svelte_only_regions(source: &str) -> Vec<(usize, usize)> {
    use tsv_svelte::ast::internal::{AttributeNode, Fragment, FragmentNode, SpecialElementKind};

    /// A `{…}`-delimited construct: its interior, both delimiters excluded.
    fn interior(span: tsv_lang::Span, out: &mut Vec<(usize, usize)>) {
        let (s, e) = (span.start as usize, span.end as usize);
        if e > s + 1 {
            out.push((s + 1, e - 1));
        }
    }

    /// A span taken as-is (a bare expression already bounded by its delimiters).
    fn span_of(span: tsv_lang::Span, out: &mut Vec<(usize, usize)>) {
        if span.end > span.start {
            out.push((span.start as usize, span.end as usize));
        }
    }

    fn attributes(attrs: &[AttributeNode<'_>], out: &mut Vec<(usize, usize)>) {
        for a in attrs {
            match a {
                // `{...rest}` / `{@attach f()}` — brace-delimited, like an ExpressionTag.
                AttributeNode::SpreadAttribute(x) => interior(x.span, out),
                AttributeNode::AttachTag(x) => interior(x.span, out),
                // A directive's value: `on:click⟨={handler}⟩`. Bounded below by the head
                // (`on:click|once` is not a comment context) and above by the closing `}`.
                AttributeNode::OnDirective(x) => directive_value(x.head_span, x.span, out),
                AttributeNode::BindDirective(x) => directive_value(x.head_span, x.span, out),
                AttributeNode::ClassDirective(x) => directive_value(x.head_span, x.span, out),
                AttributeNode::StyleDirective(x) => directive_value(x.head_span, x.span, out),
                AttributeNode::UseDirective(x) => directive_value(x.head_span, x.span, out),
                AttributeNode::TransitionDirective(x) => directive_value(x.head_span, x.span, out),
                AttributeNode::AnimateDirective(x) => directive_value(x.head_span, x.span, out),
                AttributeNode::LetDirective(x) => directive_value(x.head_span, x.span, out),
                // A plain attribute's expression value is an `ExpressionTag`, which the
                // wire walk already names.
                AttributeNode::Attribute(_) => {}
            }
        }
    }

    /// `head_span.end ..= span.end - 1` — the `={expr}` run, stopping on the `}`. Empty for
    /// a shorthand directive (`bind:value`), which has no value to probe.
    fn directive_value(head: tsv_lang::Span, span: tsv_lang::Span, out: &mut Vec<(usize, usize)>) {
        let (s, e) = (head.end as usize, span.end as usize);
        if e > s + 1 {
            out.push((s, e - 1));
        }
    }

    fn walk(frag: &Fragment<'_>, out: &mut Vec<(usize, usize)>) {
        for node in frag.nodes {
            match node {
                FragmentNode::IfBlock(b) => {
                    interior(b.opening_tag_span, out);
                    walk(&b.consequent, out);
                    if let Some(alt) = &b.alternate {
                        walk(alt, out);
                    }
                }
                FragmentNode::EachBlock(b) => {
                    interior(b.opening_tag_span, out);
                    walk(&b.body, out);
                    if let Some(fallback) = &b.fallback {
                        walk(fallback, out);
                    }
                }
                FragmentNode::AwaitBlock(b) => {
                    interior(b.opening_tag_span, out);
                    for f in [&b.pending, &b.then, &b.catch].into_iter().flatten() {
                        walk(f, out);
                    }
                }
                FragmentNode::KeyBlock(b) => {
                    interior(b.opening_tag_span, out);
                    walk(&b.fragment, out);
                }
                FragmentNode::SnippetBlock(b) => {
                    interior(b.opening_tag_span, out);
                    walk(&b.body, out);
                }
                // `{@html x}` / `{@const a = b}` / `{@render f()}` / `{@debug a}` — the
                // whole tag is one brace-delimited head, expression included.
                FragmentNode::HtmlTag(t) => interior(t.span, out),
                FragmentNode::ConstTag(t) => interior(t.span, out),
                FragmentNode::DeclarationTag(t) => interior(t.span, out),
                FragmentNode::DebugTag(t) => interior(t.span, out),
                FragmentNode::RenderTag(t) => interior(t.span, out),
                FragmentNode::Element(e) => {
                    attributes(e.attributes, out);
                    walk(&e.fragment, out);
                }
                FragmentNode::SpecialElement(e) => {
                    // `<svelte:element this={tag}>` / `<svelte:component this={x}>` hold
                    // their expression **bare**, not wrapped in an `ExpressionTag` — so the
                    // wire walk does not name it and this is its only cover. The expression
                    // span alone is the region: its ends already sit against the two
                    // braces, so the brace-adjacent gaps come along.
                    // Listed exhaustively rather than with a `_` arm, deliberately: a
                    // future variant that carries an expression would otherwise go
                    // unprobed **silently**, which is the exact failure this walk exists to
                    // fix. Let it break the build instead.
                    match &e.kind {
                        SpecialElementKind::SvelteElement { tag } => span_of(tag.span(), out),
                        SpecialElementKind::SvelteComponent { expression } => {
                            span_of(expression.expression.span(), out);
                        }
                        SpecialElementKind::SvelteHead
                        | SpecialElementKind::SvelteWindow
                        | SpecialElementKind::SvelteBody
                        | SpecialElementKind::SvelteDocument
                        | SpecialElementKind::SvelteSelf
                        | SpecialElementKind::SlotElement
                        | SpecialElementKind::SvelteFragment
                        | SpecialElementKind::SvelteBoundary
                        | SpecialElementKind::TitleElement => {}
                    }
                    attributes(e.attributes, out);
                    walk(&e.fragment, out);
                }
                FragmentNode::ExpressionTag(_)
                | FragmentNode::Text(_)
                | FragmentNode::Comment(_) => {}
            }
        }
    }

    let arena = bumpalo::Bump::new();
    // A parse failure is not this function's business to report: the caller already skipped
    // any seed file tsv rejects, and an injected source that stops parsing is a `Rejected`
    // the inject loop drops on the floor.
    let Ok(root) = tsv_svelte::parse(source, &arena) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    walk(&root.fragment, &mut out);
    out
}

/// Walk the wire AST accumulating [`code_regions`]' carriers.
fn collect_regions(node: &serde_json::Value, out: &mut Vec<(usize, usize)>) {
    match node {
        serde_json::Value::Object(obj) => {
            match obj.get("type").and_then(serde_json::Value::as_str) {
                Some("Script") => {
                    if let Some(span) = obj.get("content").and_then(span_of) {
                        out.push(span);
                    }
                }
                // The braces themselves aren't a comment context; their interior is.
                Some("ExpressionTag") => {
                    if let Some((s, e)) = span_of(node)
                        && e > s + 1
                    {
                        out.push((s + 1, e - 1));
                    }
                }
                _ => {}
            }
            for (k, v) in obj {
                if k != "loc" {
                    collect_regions(v, out);
                }
            }
        }
        serde_json::Value::Array(items) => {
            for v in items {
                collect_regions(v, out);
            }
        }
        _ => {}
    }
}

/// Sort and coalesce overlapping/adjacent ranges, so a site is never injected twice.
fn merge_regions(mut regions: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
    regions.sort_unstable();
    let mut merged: Vec<(usize, usize)> = Vec::with_capacity(regions.len());
    for (s, e) in regions {
        match merged.last_mut() {
            Some(last) if s <= last.1 => last.1 = last.1.max(e),
            _ => merged.push((s, e)),
        }
    }
    merged
}

/// The byte offsets to inject at: every `char` boundary within a code region, minus two
/// classes that are a gap in no real document — offsets strictly **inside a word** and
/// offsets strictly **inside an existing comment**.
///
/// The word filter keeps every punctuator- and whitespace-adjacent offset, so `describe`
/// `.` `only` retains **both** dot gaps — the exact positions the class hides in — while the
/// two word interiors go. Splitting `describe` into `desc/* c */ribe` probes a gap that
/// exists in no real document. Worth ~2.2× on real source. It is a **heuristic**, and
/// `--all-bytes` relaxes it (probing word interiors too — a diagnostic for that boundary).
///
/// The comment filter (`comment_spans`, the seed's own parsed comments) is **exact and
/// always on**, `--all-bytes` included. An offset strictly inside an author's comment is not
/// a gap: injecting there mutilates the comment — `/* c1 ⟨⟩*/` with a `line` payload
/// terminates it early, `// after empty i⟨⟩nit` turns `nit` into code — which then reads as a
/// false drop. `--all-bytes` relaxing the word filter is orthogonal: it probes word interiors
/// in *code*, and a comment interior is not code. The word filter used to *mostly* hide this
/// by accident (comment prose is mostly words) but missed the punctuator/whitespace
/// boundaries within a comment; this closes it outright. The comment's own start and end
/// offsets stay — those are the legit gaps immediately before and after it.
pub(crate) fn injection_sites(
    source: &str,
    regions: &[(usize, usize)],
    comment_spans: &[tsv_lang::Span],
    all_bytes: bool,
) -> Vec<usize> {
    // Sorted by start; comments never overlap, so at most one can contain a given offset —
    // the one with the largest start `<= offset`. (`comment_spans` is scoped to the host
    // document key by `parsed_comment_spans`, so every span here is host-absolute over
    // `source` — a nested `<style>` island's own-key spans are excluded by construction, not
    // just by failing to match.)
    let mut comments: Vec<(usize, usize)> = comment_spans
        .iter()
        .map(|s| (s.start as usize, s.end as usize))
        .collect();
    comments.sort_unstable();
    let inside_comment = |offset: usize| -> bool {
        let idx = comments.partition_point(|&(s, _)| s <= offset);
        idx > 0 && {
            let (s, e) = comments[idx - 1];
            s < offset && offset < e
        }
    };

    let mut sites = Vec::new();
    for &(start, end) in regions {
        let mut prev: Option<char> = source[..start].chars().next_back();
        // Inclusive of `end`: the last offset of a region is a gap like any other (the
        // position just before `</script>` is where a trailing comment goes).
        for (i, ch) in source[start..end].char_indices() {
            let offset = start + i;
            let word_interior = prev.is_some_and(is_word) && is_word(ch);
            if (all_bytes || !word_interior) && !inside_comment(offset) {
                sites.push(offset);
            }
            prev = Some(ch);
        }
        let tail_is_word = source[end..].chars().next().is_some_and(is_word);
        let word_interior = prev.is_some_and(is_word) && tail_is_word;
        if (all_bytes || !word_interior) && !inside_comment(end) {
            sites.push(end);
        }
    }
    sites
}

/// Words kept verbatim in a [`site_shape`] rather than abstracted to `IDENT`.
///
/// Heuristic and deliberately generous: a shape is a **report/dedup key**, not a parse. The
/// point is that `import⟨⟩.` and `IDENT⟨⟩.` name different bugs — the meta-property/import-
/// phase header versus every member access in the corpus — so a keyword that heads a
/// concatenated construct must survive. `source` / `defer` / `meta` / `target` are here for
/// exactly that reason despite being contextual keywords.
const SHAPE_KEYWORDS: &[&str] = &[
    "abstract",
    "as",
    "async",
    "await",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "constructor",
    "continue",
    "declare",
    "default",
    "defer",
    "delete",
    "do",
    "else",
    "enum",
    "export",
    "extends",
    "finally",
    "for",
    "from",
    "function",
    "get",
    "global",
    "if",
    "implements",
    "import",
    "in",
    "infer",
    "instanceof",
    "interface",
    "is",
    "keyof",
    "let",
    "meta",
    "module",
    "namespace",
    "new",
    "of",
    "out",
    "private",
    "protected",
    "public",
    "readonly",
    "require",
    "return",
    "satisfies",
    "set",
    "source",
    "static",
    "super",
    "switch",
    "target",
    "this",
    "throw",
    "try",
    "type",
    "typeof",
    "unique",
    "var",
    "void",
    "while",
    "yield",
];

/// The word ending at `end`, or `None` when the char before `end` isn't identifier-ish.
fn word_before(source: &str, end: usize) -> Option<&str> {
    let start = source[..end]
        .char_indices()
        .rev()
        .take_while(|(_, c)| is_word(*c))
        .map(|(i, _)| i)
        .last()?;
    Some(&source[start..end])
}

/// The word starting at `start`, or `None` when the char at `start` isn't identifier-ish.
fn word_after(source: &str, start: usize) -> Option<&str> {
    let len: usize = source[start..]
        .chars()
        .take_while(|c| is_word(*c))
        .map(char::len_utf8)
        .sum();
    (len > 0).then(|| &source[start..start + len])
}

/// Render one side of a shape: a keyword verbatim, any other word as `IDENT`.
fn shape_word(w: &str) -> String {
    if SHAPE_KEYWORDS.contains(&w) {
        w.to_string()
    } else if w.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        "NUM".to_string()
    } else {
        "IDENT".to_string()
    }
}

/// The non-word, non-whitespace run (a punctuator) ending at `end`, capped at 3 chars.
fn punct_before(source: &str, end: usize) -> String {
    let s: String = source[..end]
        .chars()
        .rev()
        .take_while(|c| !is_word(*c) && !c.is_whitespace())
        .take(3)
        .collect();
    s.chars().rev().collect()
}

/// The non-word, non-whitespace run (a punctuator) starting at `start`, capped at 3 chars.
fn punct_after(source: &str, start: usize) -> String {
    source[start..]
        .chars()
        .take_while(|c| !is_word(*c) && !c.is_whitespace())
        .take(3)
        .collect()
}

/// A compact, **file-independent** name for an injection position: the source token on
/// each side, with identifiers abstracted.
///
/// This is the dedup key of the whole report. One bug fires at every site that reaches it —
/// a member-access drop would land thousands of times across the corpus — so raw
/// `(file, offset)` findings are unreadable and, as a ratchet key, would go stale on the
/// next fixture edit. A shape collapses those to one line: `import⟨⟩.`, `IDENT⟨⟩=`,
/// `.⟨⟩IDENT`. Whitespace is elided rather than represented, since a gap's *width* is not
/// what distinguishes the position.
pub(crate) fn site_shape(source: &str, offset: usize) -> String {
    let before = word_before(source, offset).map_or_else(
        || {
            let p = punct_before(source, offset);
            if p.is_empty() { "␣".to_string() } else { p }
        },
        shape_word,
    );
    let after = word_after(source, offset).map_or_else(
        || {
            let p = punct_after(source, offset);
            if p.is_empty() { "␣".to_string() } else { p }
        },
        shape_word,
    );
    format!("{before}⟨⟩{after}")
}

/// A readable source window around an injection point — the eyeball companion to the
/// abstracted [`site_shape`], so a finding can be reproduced by hand.
pub(crate) fn snippet(source: &str, offset: usize) -> String {
    let lo = (0..=offset)
        .rev()
        .find(|i| source.is_char_boundary(*i) && offset - i >= 28)
        .unwrap_or(0);
    let hi = (offset..=source.len())
        .find(|i| source.is_char_boundary(*i) && i - offset >= 28)
        .unwrap_or(source.len());
    let one_line = |s: &str| s.replace('\n', "⏎").replace('\t', "→");
    format!(
        "{}⟨⟩{}",
        one_line(&source[lo..offset]),
        one_line(&source[offset..hi])
    )
}

/// A coarse **node-edge** key for an injection offset: the enclosing AST node and the
/// child-role edge its gap sits in.
///
/// Where [`site_shape`] keys a finding by the raw source tokens on each side — the *fine*
/// ratchet key — this keys it by STRUCTURE: the innermost wire node whose span contains the
/// offset, and which pair of that node's child roles the offset falls between. It is the
/// COARSE emitter rollup that complements the token shape: the ratchet already pins the fine
/// view, so rolling the ~700 token shapes up onto `(node_type, edge)` collapses them into the
/// few dozen emitter clusters — each ≈ one printer function — that a burn-down works through.
///
/// `edge` is `"{left}→{right}"` over child ROLES, where a role is the child's wire field key
/// with any array index collapsed (an element of `"arguments": [...]` is role `arguments`),
/// `^` is the parent's own start (offset before the first child), and `$` is its own end
/// (after the last). So `(CallExpression, callee→arguments)` is a comment in the `f⟨⟩(` gap,
/// `(VariableDeclarator, id→init)` the `=`-gap, `(ImportDeclaration, ^→specifiers)` the
/// `import⟨⟩{` gap.
///
/// Report-only: nothing here feeds the ratchet key or the snapshot.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) struct NodeEdgeKey {
    pub(crate) node_type: String,
    pub(crate) edge: String,
}

impl std::fmt::Display for NodeEdgeKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}, {})", self.node_type, self.edge)
    }
}

/// A direct span-bearing child of a wire node, in byte space, tagged with the field role it
/// sits under (spanless containers seen through — see [`collect_child`]).
struct WireChild<'a> {
    role: &'a str,
    start: usize,
    end: usize,
    node: &'a serde_json::Value,
}

/// The byte-space `[start, end)` of a wire node, or `None` when it carries no span (or a wire
/// offset lands out of range). The wire's own positions are UTF-16, so every span is
/// translated through `map` exactly as [`code_regions`] does.
fn byte_span(node: &serde_json::Value, map: &Utf16ToByte) -> Option<(usize, usize)> {
    let (s, e) = span_of(node)?;
    Some((map.byte(s)?, map.byte(e)?))
}

/// Wire keys that are NOT structural children.
///
/// `loc` is line/column, never a span. The rest are the comment-attachment decoration the wire
/// re-attaches to mirror acorn (`leadingComments` / `trailingComments` per node, plus the
/// language root's detached `comments` list) — arrays of comment NODES, not emitter children.
/// Left in, they become child ROLES, and since a comment attaches only where the seed happens
/// to carry one, the SAME structural gap would label `leadingComments→id` on one corpus and
/// `^→id` on another purely by which canonical example sorts first — splitting one emitter's
/// edge in two. Skipping them is sound: a comment's span lies outside its host node's
/// structural span (and comment interiors are excluded from injection sites), so one never
/// *strictly contains* an offset the descent would enter, and it carries no span-bearing
/// descendant — so this only relabels a mislabeled `leadingComments→X` / `X→trailingComments`
/// back to the structural `^→X` / `X→$`, leaving node selection and descent unchanged.
fn is_non_structural_key(key: &str) -> bool {
    matches!(
        key,
        "loc" | "leadingComments" | "trailingComments" | "comments"
    )
}

/// Collect a wire node's **direct** span-bearing children.
///
/// A field's value is dispatched by [`collect_child`]: a span-bearing object is one child
/// (role = the field key), an array contributes each span-bearing element (role = the field
/// key, index collapsed), and a SPANLESS object is seen THROUGH — its span-bearing
/// descendants become children of this node under the outer field key. That last case is
/// load-bearing for Svelte: `Fragment` carries no span yet holds the template `nodes`, so
/// without it a template offset could reach nothing below the `Root`. (`loc` / `metadata` /
/// `name_loc` are spanless too but hold no span-bearing descendants, so seeing through them
/// yields nothing.) Comment-attachment decoration keys are skipped — see
/// [`is_non_structural_key`].
fn wire_children<'a>(node: &'a serde_json::Value, map: &Utf16ToByte, out: &mut Vec<WireChild<'a>>) {
    let serde_json::Value::Object(obj) = node else {
        return;
    };
    for (key, value) in obj {
        if is_non_structural_key(key) {
            continue;
        }
        collect_child(key, value, map, out);
    }
}

/// Route one field value into the child list under `role`, seeing through spanless objects
/// and arrays. See [`wire_children`].
fn collect_child<'a>(
    role: &'a str,
    value: &'a serde_json::Value,
    map: &Utf16ToByte,
    out: &mut Vec<WireChild<'a>>,
) {
    match value {
        serde_json::Value::Object(inner) => match byte_span(value, map) {
            Some((start, end)) => out.push(WireChild {
                role,
                start,
                end,
                node: value,
            }),
            // A spanless container (Svelte's `Fragment` is the one that matters): see through
            // it, keeping the OUTER role so a template node reads as a child of its element.
            None => {
                for (k, v) in inner {
                    if is_non_structural_key(k) {
                        continue;
                    }
                    collect_child(role, v, map, out);
                }
            }
        },
        serde_json::Value::Array(items) => {
            for item in items {
                collect_child(role, item, map, out);
            }
        }
        _ => {}
    }
}

/// The `{left}→{right}` edge an `offset` falls in among a node's `children`.
///
/// None of `children` strictly contains `offset` — the descent already entered any that did —
/// so each sits wholly at-or-before it (`end <= offset`) or wholly at-or-after it
/// (`start >= offset`). The edge names the nearest child on each side: the largest-`end` child
/// to the left (`^` when none), the smallest-`start` child to the right (`$` when none). A
/// childless leaf is therefore `^→$`.
fn edge_between(children: &[WireChild<'_>], offset: usize) -> String {
    let left = children
        .iter()
        .filter(|c| c.end <= offset)
        .max_by_key(|c| (c.end, c.start))
        .map_or("^", |c| c.role);
    let right = children
        .iter()
        .filter(|c| c.start >= offset)
        .min_by_key(|c| (c.start, c.end))
        .map_or("$", |c| c.role);
    format!("{left}→{right}")
}

/// Key a byte `offset` to the AST node-edge whose gap it sits in — the coarse structural
/// companion to [`site_shape`].
///
/// `wire` is the parse of `source` ([`tsv_parse_to_value`]); `offset` is a **byte** offset
/// (wire positions are UTF-16, translated through [`Utf16ToByte`]). The
/// walk descends into the innermost node strictly containing `offset` — a `.svelte` script
/// offset through the embedded `Program`, a template offset through the spanless `Fragment`
/// into the tag it lands in — then names the child-role edge the offset sits between.
///
/// `None` when `offset` is outside the wire root's span, the root carries no span, or the
/// innermost node has no `type`.
pub(crate) fn node_edge_key(
    wire: &serde_json::Value,
    source: &str,
    offset: usize,
) -> Option<NodeEdgeKey> {
    let map = Utf16ToByte::new(source);
    let (root_start, root_end) = byte_span(wire, &map)?;
    if offset < root_start || offset > root_end {
        return None;
    }
    let mut node = wire;
    loop {
        let mut children = Vec::new();
        wire_children(node, &map, &mut children);
        let inner = children
            .iter()
            .find(|c| c.start < offset && offset < c.end)
            .map(|c| c.node);
        match inner {
            Some(next) => node = next,
            None => {
                let node_type = node.get("type").and_then(serde_json::Value::as_str)?;
                return Some(NodeEdgeKey {
                    node_type: node_type.to_string(),
                    edge: edge_between(&children, offset),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::properties::{Formatted, ledger_format};

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

    /// A region is only useful if it names a spot a comment can actually go: the `Program`
    /// span must start after `>` and end before `</script>`.
    #[test]
    fn code_regions_name_the_script_body_only() {
        let src = "<script lang=\"ts\">\n\tconst a = 1;\n</script>\n";
        let regions = code_regions(src, ParserType::Svelte);
        assert_eq!(regions.len(), 1, "one script body: {regions:?}");
        let (s, e) = regions[0];
        assert_eq!(&src[s..e], "\n\tconst a = 1;\n");
    }

    /// Every region as a source slice, for tests that care about *what* was named rather
    /// than where it sits.
    fn named(src: &str) -> Vec<&str> {
        code_regions(src, ParserType::Svelte)
            .into_iter()
            .map(|(s, e)| &src[s..e])
            .collect()
    }

    /// A block's head is the region the wire shape cannot express: `IfBlock`'s own span
    /// covers the whole block (body included) and its `test` span is the expression alone,
    /// so neither names `#if cond`. The head is where the `{#if ⟨here⟩ a.b}` class lives.
    #[test]
    fn code_regions_name_a_block_head_without_its_body() {
        // One region, and it stops at the head's `}` — the body is markup, not a comment
        // context, and `x` must not appear in it.
        assert_eq!(named("{#if a.b}x{/if}"), ["#if a.b"]);
        // `{:else if}` is a nested IfBlock and gets its own head.
        assert_eq!(named("{#if a}x{:else if b}y{/if}"), ["#if a", ":else if b"]);
    }

    /// Each head kind, plus the tags — one case per construct, because each carries its
    /// span on a different field and a typo in the walk would silently name nothing.
    #[test]
    fn code_regions_name_every_head_kind() {
        assert_eq!(named("{#each xs as x}{/each}"), ["#each xs as x"]);
        assert_eq!(named("{#await p}{/await}"), ["#await p"]);
        assert_eq!(named("{#key k}{/key}"), ["#key k"]);
        assert_eq!(named("{#snippet f(a)}{/snippet}"), ["#snippet f(a)"]);
        assert_eq!(named("{@html x}"), ["@html x"]);
        assert_eq!(named("{@render f()}"), ["@render f()"]);
        assert_eq!(named("{@const a = b}"), ["@const a = b"]);
        assert_eq!(named("{@debug a}"), ["@debug a"]);
    }

    /// A directive's value is named from `head_span.end` to the closing `}` — never the
    /// head itself (`on:click|once` is not a comment context) and never the byte *after*
    /// the `}`, which is the middle of an element tag: tsv over-accepts a comment there
    /// while Svelte rejects it, so naming it would manufacture the junk shapes the module
    /// docs warn about. A shorthand directive has no value and contributes nothing.
    #[test]
    fn code_regions_name_a_directive_value_not_its_head() {
        // The slice stops *before* the `}` because a range's end is exclusive — but a
        // region's end is an injection site (see `injection_sites`), so the closing `}` is
        // still probed. That inclusive end is what reaches `on:click={h/* c */}`; the byte
        // after it — inside the element tag — is what stays out.
        let src = "<div on:click={h}></div>";
        let regions = code_regions(src, ParserType::Svelte);
        assert_eq!(regions.len(), 1, "one directive value: {regions:?}");
        let (s, e) = regions[0];
        assert_eq!(&src[s..e], "={h", "the head `on:click` is not named");
        assert_eq!(
            &src[e..=e],
            "}",
            "the last site is the closing brace, not past it"
        );

        assert_eq!(named("<div on:click|once={h}></div>"), ["={h"]);
        assert!(
            named("<input bind:value />").is_empty(),
            "a shorthand directive has no value to probe"
        );
        // A plain attribute's value is an `ExpressionTag`, which the wire walk names —
        // the interior only, so the braces stay out.
        assert_eq!(named("<div class={c}></div>"), ["c"]);
        // `{...rest}` is brace-delimited like an ExpressionTag.
        assert_eq!(named("<div {...rest}></div>"), ["...rest"]);
    }

    /// `<svelte:element this={tag}>` holds its expression **bare** — Svelte's AST has no
    /// `ExpressionTag` around it — so the wire walk never names it and this is its only
    /// cover. Regression guard for a real drop: the comment survives in `{'a' + 'b'}` and
    /// vanished in `this={'a' + 'b'}`.
    #[test]
    fn code_regions_name_a_bare_special_element_expression() {
        assert_eq!(named("<svelte:element this={tag} />"), ["tag"]);
        assert_eq!(named("<svelte:component this={C} />"), ["C"]);
    }

    /// The walk names a head **whole**, including the slots where Svelte hand-reads a
    /// pattern and rejects a comment (`{#each xs as ⟨here⟩ x}`). That is deliberate: tsv
    /// rejects in exactly those slots too, so `Formatted::Rejected` filters them the same
    /// way it filters a word interior — no whitelist to keep in sync with Svelte's parser.
    #[test]
    fn a_head_region_covers_slots_the_parser_filters() {
        let src = "{#each xs as x}{/each}";
        let (s, e) = code_regions(src, ParserType::Svelte)[0];
        let as_slot = src.find(" x}").expect("the pattern slot") + 1;
        assert!(
            (s..=e).contains(&as_slot),
            "the `as` pattern slot is inside the named head"
        );
        // ...and tsv rejects a comment there, so no site survives to a finding.
        let injected = format!("{}/* c */{}", &src[..as_slot], &src[as_slot..]);
        assert!(
            matches!(
                ledger_format(&injected, ParserType::Svelte),
                Formatted::Rejected
            ),
            "tsv must reject a comment in Svelte's pattern slot, as Svelte does"
        );
    }

    /// The shape is the **ratchet key** — the thing the gate diffs against the snapshot —
    /// so what it abstracts and what it keeps is a contract, not a formatting choice.
    #[test]
    fn site_shape_keeps_keywords_and_abstracts_identifiers() {
        // A keyword must survive verbatim on both sides: `import⟨⟩.` names the
        // meta-property/import-phase header, while `IDENT⟨⟩.` names every member access in
        // the corpus. Collapsing the two would hide one bug inside the other's entry.
        assert_eq!(site_shape("import.meta", 6), "import⟨⟩.");
        assert_eq!(site_shape("import.meta", 7), ".⟨⟩meta");
        assert_eq!(site_shape("new.target", 3), "new⟨⟩.");

        // A non-keyword word abstracts, so one bug is one line however many identifiers
        // reach it.
        assert_eq!(site_shape("foo.bar", 3), "IDENT⟨⟩.");
        assert_eq!(site_shape("foo.bar", 4), ".⟨⟩IDENT");
        assert_eq!(site_shape("x9.y", 2), "IDENT⟨⟩.");

        // Digits are their own class — `1⟨⟩.` (a float's point) is not `IDENT⟨⟩.`.
        assert_eq!(site_shape("1.5", 1), "NUM⟨⟩.");

        // Whitespace is elided rather than represented: a gap's WIDTH doesn't distinguish
        // the position, so `a  =` and `a =` must land on one shape.
        assert_eq!(site_shape("a = 1", 2), "␣⟨⟩=");
        assert_eq!(site_shape("a  = 1", 2), "␣⟨⟩␣");

        // Punctuator runs are kept literally, capped, and read in source order.
        assert_eq!(site_shape("a);", 1), "IDENT⟨⟩);");
        assert_eq!(site_shape("f(x)", 2), "(⟨⟩IDENT");

        // The ends of a file are gaps too and must not panic.
        assert_eq!(site_shape("ab", 0), "␣⟨⟩IDENT");
        assert_eq!(site_shape("ab", 2), "IDENT⟨⟩␣");

        // Non-ASCII must not panic or slice mid-char.
        assert_eq!(site_shape("é.b", 2), "IDENT⟨⟩.");
    }

    /// The word filter must keep both dot gaps of a punctuator-joined header — the exact
    /// positions the whole audit exists to probe — while dropping word interiors.
    #[test]
    fn injection_sites_keep_dot_gaps_and_drop_word_interiors() {
        let src = "a.b";
        let sites = injection_sites(src, &[(0, src.len())], &[], false);
        assert_eq!(sites, vec![0, 1, 2, 3], "every gap around `.` is a site");

        let src = "ab";
        let sites = injection_sites(src, &[(0, src.len())], &[], false);
        assert_eq!(sites, vec![0, 2], "the interior of `ab` is not a gap");
        let all = injection_sites(src, &[(0, src.len())], &[], true);
        assert_eq!(all, vec![0, 1, 2], "--all-bytes keeps the interior");
    }

    /// An offset strictly inside a comment span is never a site — its interior is not a gap.
    /// The comment's own start and end offsets stay (the legit gaps before and after it), and
    /// the exclusion holds even under `--all-bytes`.
    #[test]
    fn injection_sites_drop_offsets_inside_a_comment() {
        // `x /* c */` — the block comment occupies bytes 2..9. Interiors 3..=8 must go; the
        // boundaries 2 (before) and 9 (after) stay, as do the code offsets 0 and 1.
        let src = "x /* c */";
        let comment = [tsv_lang::Span::new(2, 9)];
        let sites = injection_sites(src, &[(0, src.len())], &comment, false);
        assert_eq!(
            sites,
            vec![0, 1, 2, 9],
            "comment interior 3..=8 excluded; start/end and surrounding code kept"
        );

        // `--all-bytes` still probes every word interior in code, but never a comment
        // interior — the two filters are orthogonal.
        let all = injection_sites(src, &[(0, src.len())], &comment, true);
        assert_eq!(
            all,
            vec![0, 1, 2, 9],
            "--all-bytes relaxes the word filter, not the comment filter"
        );

        // With no comment spans the behavior is unchanged. Here `x` and `c` are each
        // surrounded by non-word chars, so no offset is a word interior — every gap is a site.
        let none = injection_sites(src, &[(0, src.len())], &[], false);
        assert_eq!(none, vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    /// Parse `src` and key `offset` — the node-edge harness the tests below share. Both
    /// arms must succeed (the snippets parse; the offsets are in range), so `expect` here
    /// is asserting the test's own premise, not swallowing a real failure.
    fn edge_at(src: &str, parser: ParserType, offset: usize) -> NodeEdgeKey {
        let wire = tsv_parse_to_value(src, parser).expect("snippet parses");
        node_edge_key(&wire, src, offset).expect("offset in range")
    }

    /// A key spelled out, so a test reads as the `(node, edge)` it asserts.
    fn key(node_type: &str, edge: &str) -> NodeEdgeKey {
        NodeEdgeKey {
            node_type: node_type.to_string(),
            edge: edge.to_string(),
        }
    }

    /// The three call-argument edges the whole rollup is named for: the `f⟨⟩(` gap before the
    /// first argument (`callee→arguments`), the gap between two arguments
    /// (`arguments→arguments`), and the gap after the last (`arguments→$`). All three key to
    /// the one `CallExpression` — the coarse emitter — not to the arguments themselves.
    #[test]
    fn node_edge_keys_the_call_argument_edges() {
        let src = "f(a, b)";
        // `f⟨⟩(` — after the callee, before the argument list.
        assert_eq!(
            edge_at(src, ParserType::TypeScript, src.find('(').unwrap()),
            key("CallExpression", "callee→arguments")
        );
        // `a⟨⟩,` — between the two arguments.
        assert_eq!(
            edge_at(src, ParserType::TypeScript, src.find(',').unwrap()),
            key("CallExpression", "arguments→arguments")
        );
        // `b⟨⟩)` — after the last argument, before the node's own end.
        assert_eq!(
            edge_at(src, ParserType::TypeScript, src.rfind(')').unwrap()),
            key("CallExpression", "arguments→$")
        );
    }

    /// `^→{first}` — an offset before every child names the parent's own start on the left.
    /// The `import⟨⟩{` gap sits between `ImportDeclaration`'s start and its first specifier.
    #[test]
    fn node_edge_keys_a_before_first_edge() {
        let src = "import {a} from 'm';";
        assert_eq!(
            edge_at(src, ParserType::TypeScript, src.find('{').unwrap()),
            key("ImportDeclaration", "^→specifiers")
        );
    }

    /// The `=`-gap rollup: a comment between a declarator's `id` and `init` keys to
    /// `(VariableDeclarator, id→init)`, one of the D2 worked examples.
    #[test]
    fn node_edge_keys_the_declarator_assignment_gap() {
        let src = "const x = 1;";
        assert_eq!(
            edge_at(src, ParserType::TypeScript, src.find('=').unwrap()),
            key("VariableDeclarator", "id→init")
        );
    }

    /// Innermost-node selection: the `.` in `g(a.b, c)` sits in the FIRST argument's member
    /// access, so the walk must descend PAST the `CallExpression` into that argument's
    /// `MemberExpression` before naming the edge — not stop at the call.
    #[test]
    fn node_edge_descends_to_the_innermost_node() {
        let src = "g(a.b, c)";
        assert_eq!(
            edge_at(src, ParserType::TypeScript, src.find('.').unwrap()),
            key("MemberExpression", "object→property")
        );
    }

    /// A childless leaf is `^→$`: an offset strictly inside an `Identifier` (which has no
    /// span-bearing children) names the node's own start-to-end, since there is no child on
    /// either side.
    #[test]
    fn node_edge_keys_a_childless_leaf() {
        // Offset 1 is strictly inside `foo` [0,3), so the walk descends into the Identifier
        // and finds it childless.
        assert_eq!(
            edge_at("foo;", ParserType::TypeScript, 1),
            key("Identifier", "^→$")
        );
    }

    /// A CSS edge — `(Rule, prelude→block)` at the `{` — proving the walk covers parseCss too,
    /// and that `Rule`'s spanless `metadata` object is NOT mistaken for a child (it would
    /// otherwise wedge a bogus role between the prelude and the block).
    #[test]
    fn node_edge_keys_a_css_rule_edge() {
        let src = "a { color: red; }";
        assert_eq!(
            edge_at(src, ParserType::Css, src.find('{').unwrap()),
            key("Rule", "prelude→block")
        );
    }

    /// NON-ASCII: the `é` is two bytes but one UTF-16 unit, so every wire span past it is off
    /// by one from its byte position. The `b` of `bar` sits at byte 7; only a correct
    /// `Utf16ToByte` translation keeps `property` starting at byte 7 (not the wire's 6), so
    /// the offset lands on the `object→property` edge. An identity ("offset == char index")
    /// map would instead read the offset as strictly inside `property` and descend into the
    /// leaf, keying `(Identifier, ^→$)` — the exact ASCII-invisible bug this guards.
    #[test]
    fn node_edge_keys_through_a_multibyte_offset() {
        let src = "foo(é.bar)";
        let b = src.find("bar").unwrap(); // byte 7, a char boundary just past the `é`
        assert_eq!(
            edge_at(src, ParserType::TypeScript, b),
            key("MemberExpression", "object→property")
        );
    }

    /// A `.svelte` SCRIPT offset resolves to a JS node: the walk descends `Root` → `Script`
    /// (both span-bearing) → the embedded `Program`, so the `a, b` comma keys to the script's
    /// `CallExpression`, exactly as the standalone `.ts` case does.
    #[test]
    fn node_edge_descends_into_a_svelte_script() {
        let src = "<script>f(a, b)</script>";
        assert_eq!(
            edge_at(src, ParserType::Svelte, src.find(',').unwrap()),
            key("CallExpression", "arguments→arguments")
        );
    }

    /// A `.svelte` TEMPLATE offset resolves through the SPANLESS `Fragment`: the walk sees
    /// through `Root`'s fragment into the element, then through the element's fragment into
    /// the `ExpressionTag`'s expression — so the `.` in `{a.b}` keys to the member access.
    /// Without transparency every template offset would collapse onto the `Root`.
    #[test]
    fn node_edge_descends_through_a_svelte_fragment() {
        let src = "<div>{a.b}</div>";
        assert_eq!(
            edge_at(src, ParserType::Svelte, src.find('.').unwrap()),
            key("MemberExpression", "object→property")
        );
    }

    /// Comment-attachment decoration is NOT a structural child. The wire attaches this
    /// leading comment onto the `FunctionDeclaration` as a `leadingComments` array, so without
    /// the skip the `function⟨⟩ f` gap would key on that comment role
    /// (`leadingComments→id`) — and only because a comment happens to sit there. The skip
    /// keeps the edge STRUCTURAL: `^→id`. (The comment leads a non-first statement so it
    /// attaches to the node rather than landing on the root's detached `comments` list.)
    #[test]
    fn node_edge_skips_comment_attachment_decoration() {
        let src = "<script>let x;\n/* c */\nfunction f() {}</script>";
        // The gap just after the `function` keyword, before the name.
        let offset = src.find("function").unwrap() + "function".len();
        assert_eq!(
            edge_at(src, ParserType::Svelte, offset),
            key("FunctionDeclaration", "^→id")
        );
    }
}

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

use crate::audit::properties::{Utf16ToByte, tsv_parse_to_value};

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

    /// Push a span as-is (a bare expression already bounded by its delimiters). Named
    /// distinctly from the module-level `span_of` (which reads a wire node's span) — this one
    /// takes an internal-AST [`tsv_lang::Span`] and appends it.
    fn push_span(span: tsv_lang::Span, out: &mut Vec<(usize, usize)>) {
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
                        SpecialElementKind::SvelteElement { tag } => push_span(tag.span(), out),
                        SpecialElementKind::SvelteComponent { expression } => {
                            push_span(expression.expression.span(), out);
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

/// The byte spans a **blank-line** injection must never land inside, because there the payload
/// becomes string CONTENT rather than a code gap — the third exclusion class, alongside
/// [`injection_sites`]' word interiors and comment interiors.
///
/// It exists for the blank-line audit specifically. tsv's TS/CSS lexers are permissive: a
/// quoted string is scanned only for its close quote or a `\`, so a **raw newline inside a
/// string is accepted as content**, not a syntax error. A blank line injected there therefore
/// does *not* trip `Formatted::Rejected` (the way a word- or delimiter-splitting injection
/// does) — it silently yields a string holding a blank run, which then reads as a false finding
/// (a `BlankRun` the output scan can't tell from a real one, since it now lives inside a plain
/// string). The gap audit never hits this because a comment injected inside a string simply
/// isn't lexed as a comment (no finding); a blank *is* content, so it must be excluded up
/// front. Combine the result with the seed's comment spans and hand the union to
/// [`injection_sites`] — every span is non-overlapping (a string is neither a comment nor
/// another string; quasis nest but don't overlap), so its single-candidate `inside_*` scan
/// stays correct.
///
/// Two carriers, read off `wire` (the parse of `source`, in the wire's UTF-16 space, translated
/// through [`Utf16ToByte`] exactly as [`code_regions`] does):
///
/// - a **string `Literal`** (a `Literal` whose `value` is a JSON string — never a regex, whose
///   `value` is `{}`, nor a bigint / number, which can hold no newline and are word interiors
///   anyway) contributes its **full** span, quotes included. The span already brackets the
///   content, so excluding *strictly inside* it (the [`injection_sites`] rule) drops every
///   interior offset while keeping the gaps just before the open quote and just after the close
///   quote.
/// - a **`TemplateElement`** (a template quasi) contributes its span **widened by one byte on
///   each side**. Unlike a string, a quasi's span is content-*only* — the backtick / `${` / `}`
///   delimiters sit just outside it — so the bare span brackets nothing to exclude (`` `x` ``'s
///   quasi `x` is `[s, s+1)`, with no offset strictly inside). Widening to `(s-1, e+1)` excludes
///   the content *and* its two delimiter-adjacent boundaries, while leaving the `${ … }`
///   expression interior (a real code gap) and the gaps outside the backticks untouched. Every
///   delimiter is ASCII, so the ±1 stays on a char boundary.
///
/// Scope: TS and Svelte-embedded TS. CSS is deferred — a `.css` seed is skipped and `<style>` is
/// unprobed by [`code_regions`], so CSS string interiors are never a site — which is why a
/// generic `type`-keyed walk suffices (CSS AST nodes carry neither `Literal`(string) nor
/// `TemplateElement`).
pub(crate) fn string_and_template_spans(
    source: &str,
    wire: &serde_json::Value,
) -> Vec<tsv_lang::Span> {
    let map = Utf16ToByte::new(source);
    let mut out = Vec::new();
    collect_string_template(wire, &map, &mut out);
    out
}

/// Walk `wire` accumulating the string-literal / template-quasi exclusion spans (byte space).
/// See [`string_and_template_spans`].
fn collect_string_template(
    node: &serde_json::Value,
    map: &Utf16ToByte,
    out: &mut Vec<tsv_lang::Span>,
) {
    match node {
        serde_json::Value::Object(obj) => {
            match obj.get("type").and_then(serde_json::Value::as_str) {
                // A string literal — its `value` is a JSON string (a regex's is `{}`, a
                // bigint's / null's is not a string). The full span brackets the quotes.
                Some("Literal") if obj.get("value").is_some_and(serde_json::Value::is_string) => {
                    if let Some((s, e)) = map.node_byte_span(node) {
                        out.push(tsv_lang::Span::new(s as u32, e as u32));
                    }
                }
                // A template quasi — content-only span, widened by one ASCII delimiter byte
                // each side so its boundaries are excluded but the `${…}` holes are not.
                Some("TemplateElement") => {
                    if let Some((s, e)) = map.node_byte_span(node) {
                        out.push(tsv_lang::Span::new(
                            s.saturating_sub(1) as u32,
                            (e + 1) as u32,
                        ));
                    }
                }
                _ => {}
            }
            for (k, v) in obj {
                if k != "loc" {
                    collect_string_template(v, map, out);
                }
            }
        }
        serde_json::Value::Array(items) => {
            for v in items {
                collect_string_template(v, map, out);
            }
        }
        _ => {}
    }
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
    // `word_before` / `punct_before` slice `source[..offset]` directly; every caller passes an
    // injection-site or victim-site offset, both provably char boundaries. Assert it (debug
    // builds only) so a future caller with a raw offset trips here rather than panicking
    // mid-slice — the invariant `snippet` states by searching for a boundary instead.
    debug_assert!(
        source.is_char_boundary(offset),
        "site_shape offset {offset} must be a char boundary"
    );
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::properties::{Formatted, ledger_format};

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

    /// Every reserved control-flow/declaration keyword the lexer recognizes must be in
    /// [`SHAPE_KEYWORDS`], or the shape key silently degrades it to `IDENT` — merging a
    /// keyword-headed bug (`return⟨⟩`) into the generic-identifier entry and hiding it under
    /// one ratchet line. That is a *quiet* failure: nothing else notices a keyword fell out
    /// of the table, because both keys are well-formed shapes. The oracle is
    /// `tsv_ts::reserved_words()` (the lexer's own list), so a keyword added to the lexer
    /// without a matching `SHAPE_KEYWORDS` entry fails here. Holds today with zero re-key.
    #[test]
    fn reserved_keywords_are_all_shape_keywords() {
        let missing: Vec<&str> = tsv_ts::reserved_words()
            .iter()
            .copied()
            .filter(|w| !SHAPE_KEYWORDS.contains(w))
            .collect();
        assert!(
            missing.is_empty(),
            "reserved words missing from SHAPE_KEYWORDS (they would degrade to IDENT and merge \
             two distinct bugs onto one ratchet line): {missing:?}"
        );
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

    /// The blank-audit sites for `src` under the string/template exclusion — the harness the
    /// three tests below share. Parses `src`, collects the exclusion spans, and returns the
    /// surviving injection offsets over the whole file.
    fn blank_sites(src: &str) -> Vec<usize> {
        let wire = tsv_parse_to_value(src, ParserType::TypeScript).expect("snippet parses");
        let spans = string_and_template_spans(src, &wire);
        injection_sites(src, &[(0, src.len())], &spans, false)
    }

    /// A string literal's interior is excluded — a blank injected between the quotes is string
    /// CONTENT, not a code gap (tsv's permissive lexer accepts a raw newline there). The full
    /// span brackets the quotes, so the gaps just before `'` and just after `'` survive.
    #[test]
    fn string_and_template_excludes_a_string_interior() {
        // const s = 'ab';  — `'`@10 a@11 b@12 `'`@13 ;@14
        let src = "const s = 'ab';";
        let sites = blank_sites(src);
        // Interior (after the open quote through the close quote) is gone …
        for off in [11, 12, 13] {
            assert!(
                !sites.contains(&off),
                "offset {off} inside the string must be excluded"
            );
        }
        // … the surrounding gaps stay.
        assert!(
            sites.contains(&10),
            "the gap before the open quote survives"
        );
        assert!(
            sites.contains(&14),
            "the gap after the close quote survives"
        );
    }

    /// A template quasi's content AND its two delimiter-adjacent boundaries are excluded, while
    /// the `${ … }` expression interior — a real code gap — is kept. The widen-by-one is what
    /// separates the two: a content-only span alone would exclude nothing.
    #[test]
    fn string_and_template_excludes_quasi_but_keeps_the_hole() {
        // const t = `x${y}z`;  — `@10 x@11 $@12 {@13 y@14 }@15 z@16 `@17 ;@18
        let src = "const t = `x${y}z`;";
        let sites = blank_sites(src);
        // Quasi content + delimiter boundaries: excluded (template text, not a gap).
        for off in [11, 12, 16, 17] {
            assert!(
                !sites.contains(&off),
                "offset {off} in the template text must be excluded"
            );
        }
        // The `${ y }` expression interior is a real code gap — a blank there is valid.
        assert!(
            sites.contains(&14),
            "the `{{` → `y` gap inside the hole survives"
        );
        assert!(
            sites.contains(&15),
            "the `y` → `}}` gap inside the hole survives"
        );
        // Outside the backticks, the gaps stay too.
        assert!(sites.contains(&10), "the gap before the backtick survives");
        assert!(sites.contains(&18), "the gap after the backtick survives");
    }

    /// MULTIBYTE: the wire's spans are UTF-16, so a `é` before the string shifts every wire
    /// offset one unit short of its byte position. Only a correct `Utf16ToByte` translation
    /// excludes the right BYTES; an identity ("offset == char index") reading would exclude the
    /// open-quote gap (byte 11) and miss the interior (byte 13) — the exact ASCII-invisible bug
    /// the arithmetic guard exists to catch (the corpus can't grade it: an all-ASCII file is
    /// byte-identical either way).
    #[test]
    fn string_and_template_translates_multibyte_offsets() {
        // const é = 'x';  — `é` is 2 bytes / 1 UTF-16 unit; `'`@byte11 x@12 `'`@13 ;@14
        let src = "const é = 'x';";
        assert_eq!(&src[11..14], "'x'", "the string is at bytes 11..14");
        let sites = blank_sites(src);
        for off in [12, 13] {
            assert!(
                !sites.contains(&off),
                "byte {off} inside the string must be excluded"
            );
        }
        assert!(
            sites.contains(&11),
            "the gap before the open quote (byte 11) survives"
        );
        assert!(
            sites.contains(&14),
            "the gap after the close quote (byte 14) survives"
        );
    }
}

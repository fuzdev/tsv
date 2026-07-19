//! Structural anchor extraction — the offsets a compile-fuzz operator may splice at.
//!
//! A compile mutant must stay **oracle-compilable** to grade anything (a mutant the
//! oracle rejects only ever tests the over-acceptance probe, and a mutant neither
//! side parses tests nothing at all). Byte-level mutation — what the formatter's
//! [`fuzz`](super::super::fuzz) does — is therefore the wrong tool here: it
//! overwhelmingly produces garbage that dies at the oracle's parser.
//!
//! So the operators splice **whole constructs at structurally valid offsets**, and
//! this module finds those offsets by parsing the seed with tsv's own Svelte parser
//! and walking the internal AST. Every anchor is a byte offset (or span) in the seed
//! source; an operator is a pure text splice at one of them.
//!
//! ## What is exact and what is heuristic
//!
//! Be precise about which is which — a fuzzer's anchors are load-bearing.
//!
//! - **Exact** (read off the AST): every fragment gap, wrappable subtree span,
//!   attribute slot, script insertion point, and the dropped-region classification.
//!   These come from `FragmentNode::span` / `Element::name_span` (an attribute slot is
//!   the byte just past the tag name, so it precedes any existing attribute and the
//!   `>` alike) / `Script::content`'s `Program` span, so they are as correct as the
//!   parse.
//! - **Heuristic** (a source scan, documented as such): the *name* lists —
//!   [`Anchors::declared`] and [`Anchors::template_reads`]. Extracting a binding's
//!   identifier properly means walking `tsv_ts` patterns through destructuring; here
//!   a name only has to be *plausible*, because a wrong pick costs a less-interesting
//!   mutant, never a wrong verdict (the grader reads the compilers, not this list).
//!   So the declared side scans statement text after a `let`/`const`/`var`/
//!   `function`/`class` keyword, and the read side takes identifier-shaped tokens out
//!   of expression spans.
//!
//! ## The exotic-whitespace anchors
//!
//! [`Anchors::script_ws`], [`Anchors::attr_value_edges`], and
//! [`Anchors::css_ident_ends`] serve `Operator::ExoticWhitespace`
//! (`super::operators`), and each is chosen so that the *insertion itself* cannot
//! break well-formedness — the property the whole design rests on (see that
//! operator's docs for the argument).
//!
//! - **`script_ws`** — both edges of every ASCII-whitespace RUN inside a script's
//!   content. A source scan, and deliberately **not** trivia-aware: it makes no
//!   claim that a run is inter-token trivia rather than string / template / comment
//!   / regex interior, because it *nearly* does not have to. Every context that
//!   admits the run's existing whitespace character admits one more character of
//!   the same class, so the mutant stays parseable either way — it is merely a
//!   *content* mutation instead of a *trivia* mutation, and both compilers see the
//!   same bytes. Both edges are recorded so a keyword-adjacent run reaches
//!   `static\u{FEFF} {` **and** `static \u{FEFF}{`.
//!
//!   ⚠️ **The one context where that argument fails**, and the reason the scan is
//!   not fully position-blind: a string literal's **`LineContinuation`**. In
//!   `"a\<LF>b"` the run's first character is the `<LF>` of a `\<LF>` pair, and the
//!   pair is what makes the literal legal — inserting **any** whitespace between
//!   the `\` and the `<LF>` re-reads the `\` as a `NonEscapeCharacter` escape and
//!   leaves a RAW `<LF>` inside the string, which ECMA-262 §12.9.4.1 forbids
//!   (`DoubleStringCharacter :: SourceCharacter but not one of " or \ or
//!   LineTerminator`). So the run's **START** edge is skipped when the preceding
//!   byte is `\` and the run opens with a line terminator; the **END** edge stays,
//!   since a character appended after the terminator is ordinary string content and
//!   the continuation is already complete. The hazard is character-INDEPENDENT —
//!   every code point in the insertion set breaks it identically — which is exactly
//!   why scoping it to `U+2028`/`U+2029` (as an earlier rationale did) was wrong.
//! - **`attr_value_edges`** — the inner edges of a QUOTED attribute value's text,
//!   on **every** attribute-bearing node: a regular element, a `<svelte:*>` special
//!   element, and `<svelte:options>`. Quoting is checked against the byte before the
//!   span, so the value's extent is delimiter-defined and any inserted character is
//!   content by construction.
//! - **`css_ident_ends`** — the end of an ASCII ident run introduced by `:`, `::`,
//!   `.`, or `#` inside `<style>`. A source scan over the CSS content span.

use tsv_svelte::ast::internal::{
    AttributeNode, AttributeValue, Fragment, FragmentNode, Root, ScriptContext, SpecialElementKind,
};

/// A script's splice slot: where a statement can be inserted, and whether the
/// script is TypeScript (the erasure axis wants to know).
pub struct ScriptSlot {
    /// Byte offset just inside the script's content — a valid statement position.
    pub insert_at: u32,
    /// `lang="ts"` — the type-erasure operators only apply here.
    pub is_ts: bool,
    /// Statement-boundary offsets inside the script content (comment injection).
    pub stmt_gaps: Vec<u32>,
}

/// A template-block binding: the name it introduces, and a gap inside its body.
pub struct BlockBinding {
    /// The bound name as written (`{#each xs as name}` → `name`).
    pub name: String,
    /// A splice offset inside the block's body.
    pub body_gap: u32,
}

/// The structural anchors of one seed component.
#[derive(Default)]
pub struct Anchors {
    /// Offsets a fragment node may be spliced at (before each template sibling).
    pub template_gaps: Vec<u32>,
    /// The subset of [`Self::template_gaps`] inside a region the **server** target
    /// drops — an `{:await}` block's `{:catch}` body, and a `<svelte:boundary>`
    /// `pending`/`failed` snippet body. The dropped-region axis: a construct here is
    /// parsed and scope-analyzed but never emitted, which is exactly where the two
    /// known-unreached over-acceptances live.
    pub dropped_gaps: Vec<u32>,
    /// `(start, end)` of template subtrees a block may be wrapped around.
    pub wrappable: Vec<(u32, u32)>,
    /// Offsets just after an element's tag name, where an attribute or directive
    /// may be added.
    pub attr_slots: Vec<u32>,
    /// Names bound by a template block scope, with a gap inside that scope.
    pub block_bindings: Vec<BlockBinding>,
    /// `(snippet name, declared in a dropped region)`.
    pub snippets: Vec<(String, bool)>,
    pub instance: Option<ScriptSlot>,
    pub module: Option<ScriptSlot>,
    /// Identifier-shaped tokens the template reads (heuristic — see module docs).
    pub template_reads: Vec<String>,
    /// Top-level names the instance script binds — declarations AND imports
    /// (heuristic — see module docs).
    pub declared: Vec<String>,
    /// The same for the module script. Kept apart from [`Self::declared`] because a
    /// name may legally exist in both scripts, so a duplicate-binding guard has to
    /// ask about the scope it is actually inserting into.
    pub module_declared: Vec<String>,
    /// The names the module script EXPORTS. A distinct question from what it
    /// *declares*: `export { s }` exports without declaring, and re-exporting a name
    /// is its own parse error — so a guard that only knew declarations still emitted
    /// `export const s = 1; export { s };`.
    pub module_exported: Vec<String>,
    /// Brace interiors of `{expression}` tags — a block comment is legal there.
    pub expr_interiors: Vec<u32>,
    /// Both edges of every ASCII-whitespace run inside a script's content — the
    /// JS token boundaries (see module docs).
    pub script_ws: Vec<u32>,
    /// The inner edges of quoted attribute values' text (see module docs).
    pub attr_value_edges: Vec<u32>,
    /// The end of each `:` / `::` / `.` / `#`-introduced ident run in `<style>`.
    pub css_ident_ends: Vec<u32>,
}

impl Anchors {
    /// Parse `source` and collect its anchors, or `None` when tsv's parser rejects
    /// it (an unparseable seed is simply not usable as one).
    pub fn collect(source: &str) -> Option<Self> {
        let arena = bumpalo::Bump::new();
        let root = tsv_svelte::parse(source, &arena).ok()?;
        let mut anchors = Self::default();
        anchors.walk_fragment(&root.fragment, source, false);
        anchors.instance = script_slot(&root, source, ScriptContext::Default);
        anchors.module = script_slot(&root, source, ScriptContext::Module);
        if let Some(script) = root.instance {
            anchors.declared = declared_names(script.content.span.extract(source));
        }
        if let Some(script) = root.module {
            let text = script.content.span.extract(source);
            anchors.module_declared = declared_names(text);
            anchors.module_exported = exported_names(text);
        }
        for script in [root.instance, root.module].into_iter().flatten() {
            push_whitespace_runs(&mut anchors.script_ws, source, script.content.span);
        }
        if let Some(style) = root.css {
            push_css_ident_ends(&mut anchors.css_ident_ends, source, style.content_span);
        }
        // `<svelte:options>` hangs off the root rather than the fragment, so the
        // node walk never reaches it.
        if let Some(options) = root.options {
            anchors.push_attr_value_edges(options.attributes, source);
        }
        for names in [
            &mut anchors.template_reads,
            &mut anchors.declared,
            &mut anchors.module_declared,
            &mut anchors.module_exported,
        ] {
            names.sort_unstable();
            names.dedup();
        }
        Some(anchors)
    }

    /// Walk one fragment, recording its gaps and recursing. `dropped` marks that
    /// this fragment sits inside a region the server target never emits.
    fn walk_fragment(&mut self, fragment: &Fragment<'_>, source: &str, dropped: bool) {
        for node in fragment.nodes {
            // Whitespace-only text is not a useful splice neighbor (and splicing
            // before it produces the same document as splicing after the previous
            // sibling), so gaps anchor on real nodes only.
            if node.is_whitespace_only_text() {
                continue;
            }
            let start = node.span().start;
            self.template_gaps.push(start);
            if dropped {
                self.dropped_gaps.push(start);
            }
            self.walk_node(node, source, dropped);
        }
    }

    fn walk_node(&mut self, node: &FragmentNode<'_>, source: &str, dropped: bool) {
        match node {
            FragmentNode::Element(element) => {
                self.wrappable.push((element.span.start, element.span.end));
                self.attr_slots.push(element.name_span.end);
                self.push_attr_value_edges(element.attributes, source);
                // TODO: attribute VALUES are not scanned for reads, so a name used only
                // in `<Foo {...r} />` or `attr={x}` is missing from `template_reads`.
                // That list is the donor graft's collision guard, so the gap lets a
                // rare graft re-declare such a name — measured at 1 ungradeable mutant
                // per ~14.5k, the residual after the destructuring/export guards. The
                // fix is walking `element.attributes`' nested value structure.
                self.walk_fragment(&element.fragment, source, dropped);
            }
            FragmentNode::SpecialElement(element) => {
                // A `<svelte:boundary>`'s `pending` / `failed` snippet children are
                // dropped on the server; its default content is not. The snippet
                // blocks below carry that through, so only mark the boundary itself.
                let boundary = matches!(element.kind, SpecialElementKind::SvelteBoundary);
                self.push_attr_value_edges(element.attributes, source);
                self.walk_fragment(&element.fragment, source, dropped || boundary);
            }
            FragmentNode::ExpressionTag(tag) => {
                self.expr_interiors.push(tag.span.start + 1);
                self.push_reads(tag.expression.span().extract(source));
            }
            FragmentNode::IfBlock(block) => {
                self.wrappable.push((block.span.start, block.span.end));
                self.push_reads(block.test.span().extract(source));
                self.walk_fragment(&block.consequent, source, dropped);
                if let Some(alternate) = &block.alternate {
                    self.walk_fragment(alternate, source, dropped);
                }
            }
            FragmentNode::EachBlock(block) => {
                self.wrappable.push((block.span.start, block.span.end));
                self.push_reads(block.expression.span().extract(source));
                if let Some(context) = &block.context
                    && let Some(gap) = first_gap(&block.body)
                    && let Some(name) = plain_identifier(context.span().extract(source))
                {
                    self.block_bindings.push(BlockBinding {
                        name,
                        body_gap: gap,
                    });
                }
                self.walk_fragment(&block.body, source, dropped);
                if let Some(fallback) = &block.fallback {
                    self.walk_fragment(fallback, source, dropped);
                }
            }
            FragmentNode::AwaitBlock(block) => {
                self.wrappable.push((block.span.start, block.span.end));
                self.push_reads(block.expression.span().extract(source));
                if let Some(pending) = &block.pending {
                    self.walk_fragment(pending, source, dropped);
                }
                if let Some(then) = &block.then {
                    self.walk_fragment(then, source, dropped);
                }
                // The `{:catch}` body is the canonical server-dropped region.
                if let Some(catch) = &block.catch {
                    self.walk_fragment(catch, source, true);
                }
            }
            FragmentNode::KeyBlock(block) => {
                self.wrappable.push((block.span.start, block.span.end));
                self.push_reads(block.expression.span().extract(source));
                self.walk_fragment(&block.fragment, source, dropped);
            }
            FragmentNode::SnippetBlock(block) => {
                if let Some(name) = plain_identifier(block.expression.span().extract(source)) {
                    self.snippets.push((name, dropped));
                }
                self.walk_fragment(&block.body, source, dropped);
            }
            FragmentNode::HtmlTag(tag) => self.push_reads(tag.expression.span().extract(source)),
            FragmentNode::RenderTag(tag) => self.push_reads(tag.expression.span().extract(source)),
            FragmentNode::ConstTag(_)
            | FragmentNode::DeclarationTag(_)
            | FragmentNode::DebugTag(_)
            | FragmentNode::Text(_)
            | FragmentNode::Comment(_) => {}
        }
    }

    /// Record the inner edges of each QUOTED plain attribute's text value.
    ///
    /// The quote check is what makes an inserted character content by
    /// construction: an unquoted value's extent is decided by the parser's
    /// whitespace notion, which is the very thing under test, so injecting there
    /// would be reasoning in a circle.
    fn push_attr_value_edges(&mut self, attributes: &[AttributeNode<'_>], source: &str) {
        for attribute in attributes {
            let AttributeNode::Attribute(attribute) = attribute else {
                continue;
            };
            let Some(values) = attribute.value else {
                continue;
            };
            for value in values {
                let AttributeValue::Text(text) = value else {
                    continue;
                };
                let start = text.raw_span.start as usize;
                if !matches!(
                    source.as_bytes().get(start.wrapping_sub(1)),
                    Some(b'"' | b'\'')
                ) {
                    continue;
                }
                self.attr_value_edges.push(text.raw_span.start);
                if text.raw_span.end > text.raw_span.start {
                    self.attr_value_edges.push(text.raw_span.end);
                }
            }
        }
    }

    /// Record the identifier-shaped tokens of an expression's source text.
    fn push_reads(&mut self, text: &str) {
        for name in identifier_tokens(text) {
            self.template_reads.push(name);
        }
    }
}

/// The ASCII members of ECMAScript `WhiteSpace` ∪ `LineTerminator` (ECMA-262
/// §12.2 table 34 + §12.3 table 35): TAB, VT, FF, SP, LF, CR.
///
/// Deliberately **not** `u8::is_ascii_whitespace`, which omits `<VT>` (`U+000B`).
/// `<VT>` is in the operator's insertion set, so a Rust-notion scan would leave a
/// VT-only run unanchored — the host-vs-target whitespace-class mismatch this whole
/// operator exists to hunt, reproduced inside the hunter.
const fn is_js_ascii_whitespace(b: u8) -> bool {
    matches!(b, b'\t' | 0x0B | 0x0C | b' ' | b'\n' | b'\r')
}

/// Record both edges of every ASCII-whitespace run inside `span`.
///
/// Host-absolute offsets. ASCII-only, so every recorded offset is a UTF-8
/// character boundary by construction. See the module docs for why this scan is
/// deliberately not trivia-aware — and for the single position where that is not
/// enough, the string-literal `LineContinuation` guarded below.
fn push_whitespace_runs(out: &mut Vec<u32>, source: &str, span: tsv_lang::Span) {
    let base = span.start as usize;
    let bytes = span.extract(source).as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if !is_js_ascii_whitespace(bytes[i]) {
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len() && is_js_ascii_whitespace(bytes[i]) {
            i += 1;
        }
        // A `\` immediately before a run that OPENS with a line terminator is a
        // `LineContinuation`: splitting the pair strands a raw line terminator in
        // the string literal, which no insertion character survives. The end edge
        // is past the terminator, so the pair is already complete there.
        let continuation = matches!(bytes[start], b'\n' | b'\r')
            && source.as_bytes().get((base + start).wrapping_sub(1)) == Some(&b'\\');
        #[allow(clippy::cast_possible_truncation)]
        {
            if !continuation {
                out.push((base + start) as u32);
            }
            out.push((base + i) as u32);
        }
    }
}

/// Record the end offset of each ASCII ident run introduced by `:` / `::` / `.` /
/// `#` in `span` — the CSS pseudo-class, class, and id NAME positions.
///
/// Appending there is where the CSS side of this bug family lives: a code point a
/// host-language trim would strip is a CSS *ident* code point, so `:global\u{A0}`
/// is genuinely a different pseudo-class. A character that is NOT an ident code
/// point makes the oracle's CSS parser reject the mutant, which is a perfectly
/// good grade too (tsv must then refuse, or it is an over-acceptance).
///
/// ⚠️ The scan is **not trivia-aware**, so "an ident run introduced by `:`/`.`/`#`"
/// over-reports: it also fires inside CSS strings and comments, on a declaration
/// value (`color:red` records after `red`), on a hex color (`#fff`), and in an
/// at-rule prelude. That costs *budget*, never soundness — at each of those
/// positions an appended code point is still either ident content or an oracle
/// rejection, exactly as at a real selector name, so the grade stays honest and a
/// finding there would be a real one rather than a harness artifact. It is stated
/// here rather than fixed so the fn's reach is not read as narrower than it is.
fn push_css_ident_ends(out: &mut Vec<u32>, source: &str, span: tsv_lang::Span) {
    let base = span.start as usize;
    let bytes = span.extract(source).as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            // `::before` as well as `:global` — consume the whole introducer.
            b':' => {
                while i < bytes.len() && bytes[i] == b':' {
                    i += 1;
                }
            }
            b'.' | b'#' => i += 1,
            _ => {
                i += 1;
                continue;
            }
        }
        let start = i;
        while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'-') {
            i += 1;
        }
        if i > start {
            #[allow(clippy::cast_possible_truncation)]
            out.push((base + i) as u32);
        }
    }
}

/// The first splice offset inside `fragment`, or `None` when it is empty (an empty
/// body has no anchor — nothing in this module invents one).
fn first_gap(fragment: &Fragment<'_>) -> Option<u32> {
    fragment.nodes.first().map(|node| node.span().start)
}

/// Build a script's slot: the insertion offset, the `lang="ts"` flag, and the
/// statement-boundary gaps inside it.
fn script_slot(root: &Root<'_>, source: &str, context: ScriptContext) -> Option<ScriptSlot> {
    let script = match context {
        ScriptContext::Default => root.instance?,
        ScriptContext::Module => root.module?,
    };
    let program = &script.content;
    let stmt_gaps: Vec<u32> = program.body.iter().map(|s| s.span().start).collect();
    Some(ScriptSlot {
        // The program span starts just past the opening tag's `>`, so it is always
        // a valid statement position — including for an empty script.
        insert_at: stmt_gaps.first().copied().unwrap_or(program.span.start),
        is_ts: script_is_ts(script.span, source),
        stmt_gaps,
    })
}

/// Whether the script tag carries `lang="ts"`. Read off the opening tag's source
/// text — the attribute list is an AST slice of interned names, and the tag text is
/// both shorter to state and exactly what the oracle's own `lang === 'ts'` test sees.
pub(super) fn script_is_ts(span: tsv_lang::Span, source: &str) -> bool {
    let text = span.extract(source);
    let head = &text[..text.find('>').map_or(text.len(), |i| i + 1)];
    head.contains("lang=\"ts\"") || head.contains("lang='ts'")
}

/// The identifier in a simple binding pattern, or `None` for a destructuring
/// pattern (`{a, b}` / `[a]`) — those are deliberately skipped rather than
/// half-parsed, since a shadow operator needs one exact name.
fn plain_identifier(text: &str) -> Option<String> {
    let trimmed = text.trim();
    is_identifier(trimmed).then(|| trimmed.to_string())
}

/// Whether `s` is a plain JS identifier that is safe to shadow — ASCII-only, not a
/// keyword or literal. Conservative on purpose: a rejected candidate costs one
/// less-interesting mutant, an accepted bad one costs a wasted oracle round trip.
fn is_identifier(s: &str) -> bool {
    !s.is_empty()
        && s.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_' || c == '$')
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
        && !RESERVED.contains(&s)
}

/// Words an identifier scan must not hand back as a binding name.
const RESERVED: &[&str] = &[
    "as",
    "async",
    "await",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "else",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "function",
    "if",
    "import",
    "in",
    "instanceof",
    "let",
    "new",
    "null",
    "of",
    "return",
    "static",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "undefined",
    "var",
    "void",
    "while",
    "with",
    "yield",
];

/// Identifier-shaped tokens of a source slice (heuristic — see module docs). Skips
/// the member-access tail of `a.b` so `b` is not offered as a standalone binding.
fn identifier_tokens(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'$' {
            let start = i;
            while i < bytes.len()
                && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'$')
            {
                i += 1;
            }
            let word = &text[start..i];
            let after_dot = start > 0 && bytes[start - 1] == b'.';
            if !after_dot && is_identifier(word) {
                out.push(word.to_string());
            }
        } else {
            i += 1;
        }
    }
    out
}

/// The top-level names a script BINDS (heuristic — see module docs): the bindings a
/// `let` / `const` / `var` / `function` / `class` introduces, plus every binding an
/// `import` introduces.
///
/// **Destructuring is included, deliberately over-approximated.** This list is the
/// duplicate-binding guard for the mutation operators, and re-declaring a name is a JS
/// parse error — a mutant the ORACLE cannot parse grades nothing and merely spends a
/// round trip. Missing a destructured binding was exactly that bug: `let { value: v }
/// = $props()` bound `v`, the scan reported `value`, and a donor graft happily emitted
/// a second `let v`. So inside a `{…}` / `[…]` pattern every identifier is claimed,
/// property keys and call callees included. Over-claiming costs a skipped mutation;
/// under-claiming costs an ungradeable mutant, so the bias runs this way on purpose.
///
/// One known over-claim: `const C = class { m() {} }` — `class` followed by `{` takes
/// the destructuring branch, so every method name is claimed as a binding. Harmless per
/// the bias above, but it silently suppresses donor grafts that would otherwise collide
/// on one of those names.
pub(super) fn declared_names(script_text: &str) -> Vec<String> {
    const DECLARATORS: &[&str] = &["let", "const", "var", "function", "class"];
    let mut out = Vec::new();
    let spans = word_spans(script_text);
    for (index, &(word, _, end)) in spans.iter().enumerate() {
        if !DECLARATORS.contains(&word) {
            continue;
        }
        let rest = script_text[end..].trim_start();
        let opener = rest.chars().next();
        if matches!(opener, Some('{' | '[')) {
            // A destructuring pattern: claim every identifier inside it.
            let pattern_start = script_text.len() - rest.len();
            let Some(pattern_end) = matching_close(script_text, pattern_start) else {
                continue;
            };
            for &(w, s, _) in &spans {
                if s > pattern_start && s < pattern_end && is_identifier(w) {
                    out.push(w.to_string());
                }
            }
        } else if let Some(&(next, _, _)) = spans.get(index + 1)
            && is_identifier(next)
        {
            out.push(next.to_string());
        }
    }
    // `import a, { b as c } from 'm'` — every identifier up to `from` is a binding.
    let mut in_import = false;
    for &(word, _, _) in &spans {
        match word {
            "import" => in_import = true,
            "from" => in_import = false,
            w if in_import && is_identifier(w) => out.push(w.to_string()),
            _ => {}
        }
    }
    out
}

/// The byte offset just past the bracket matching the one at `open`, or `None` when it
/// is unbalanced. Depth-counting only — it does not track strings or comments, which
/// is adequate for a guard that is allowed to over-claim.
fn matching_close(text: &str, open: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let (opener, closer) = match bytes.get(open)? {
        b'{' => (b'{', b'}'),
        b'[' => (b'[', b']'),
        _ => return None,
    };
    let mut depth = 0usize;
    for (offset, &byte) in bytes.iter().enumerate().skip(open) {
        if byte == opener {
            depth += 1;
        } else if byte == closer {
            depth -= 1;
            if depth == 0 {
                return Some(offset);
            }
        }
    }
    None
}

/// The identifier-shaped words of a source slice with their spans, keywords included.
fn word_spans(text: &str) -> Vec<(&str, usize, usize)> {
    let bytes = text.as_bytes();
    let mut words = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'$' {
            let start = i;
            while i < bytes.len()
                && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'$')
            {
                i += 1;
            }
            words.push((&text[start..i], start, i));
        } else {
            i += 1;
        }
    }
    words
}

/// The names a script EXPORTS (heuristic): every identifier word following an
/// `export` keyword up to the statement's `from` or its end — so it covers both
/// `export const s = 1` and the bare specifier list `export { s, t }`.
pub(super) fn exported_names(script_text: &str) -> Vec<String> {
    const DECLARATORS: &[&str] = &["const", "let", "var", "function", "class", "default"];
    let mut out = Vec::new();
    let words = words_of(script_text);
    let mut remaining = 0usize;
    for word in &words {
        if *word == "export" {
            // A specifier list can hold several names; a declaration exports one. The
            // window is generous on purpose (see the over-approximation note above).
            remaining = 8;
            continue;
        }
        if remaining == 0 || *word == "from" {
            remaining = 0;
            continue;
        }
        remaining -= 1;
        if is_identifier(word) && !DECLARATORS.contains(word) {
            out.push((*word).to_string());
        }
    }
    out
}

/// The identifier-shaped words of a source slice, keywords included.
fn words_of(text: &str) -> Vec<&str> {
    word_spans(text).into_iter().map(|(w, _, _)| w).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_finds_template_and_script_anchors() {
        let source = "<script lang=\"ts\">\n\tlet count = 0;\n</script>\n\n<p>{count}</p>\n";
        let anchors = Anchors::collect(source).expect("parses");
        assert!(!anchors.template_gaps.is_empty());
        assert!(!anchors.attr_slots.is_empty());
        let instance = anchors.instance.expect("instance script");
        assert!(instance.is_ts);
        assert!(anchors.declared.contains(&"count".to_string()));
        assert!(anchors.template_reads.contains(&"count".to_string()));
    }

    #[test]
    fn catch_body_is_a_dropped_region_and_the_then_body_is_not() {
        let source = "{#await p}<i>w</i>{:then v}<b>{v}</b>{:catch e}<u>{e}</u>{/await}\n";
        let anchors = Anchors::collect(source).expect("parses");
        // The `{:catch}` body and everything nested in it, and nothing else: the
        // `<u>` and the `{e}` inside it are dropped; the `{:then}` body's `<b>` and
        // the pending `<i>` are not.
        let catch_at = source.find("{:catch").expect("has a catch");
        assert_eq!(anchors.dropped_gaps.len(), 2);
        for &gap in &anchors.dropped_gaps {
            assert!(
                gap as usize > catch_at,
                "gap {gap} is outside the catch body"
            );
        }
        let first = anchors.dropped_gaps[0] as usize;
        assert!(
            source[first..].starts_with("<u>"),
            "got {:?}",
            &source[first..]
        );
    }

    #[test]
    fn boundary_pending_snippet_is_dropped() {
        let source = "<svelte:boundary>{#snippet pending()}<i>x</i>{/snippet}<p>y</p>\
             </svelte:boundary>\n";
        let anchors = Anchors::collect(source).expect("parses");
        assert_eq!(anchors.snippets, vec![("pending".to_string(), true)]);
    }

    #[test]
    fn each_binding_records_its_name_and_body_gap() {
        let source = "{#each xs as item}<p>{item}</p>{/each}\n";
        let anchors = Anchors::collect(source).expect("parses");
        assert_eq!(anchors.block_bindings.len(), 1);
        assert_eq!(anchors.block_bindings[0].name, "item");
        let at = anchors.block_bindings[0].body_gap as usize;
        assert!(source[at..].starts_with("<p>"));
    }

    #[test]
    fn destructuring_context_yields_no_binding_name() {
        let source = "{#each xs as { v }}<p>{v}</p>{/each}\n";
        let anchors = Anchors::collect(source).expect("parses");
        assert!(anchors.block_bindings.is_empty());
    }

    #[test]
    fn declared_names_claim_destructured_bindings() {
        // The bug this guards: the scan used to report `value` (the property KEY) and
        // miss `v` (the binding), so a donor graft emitted a second `let v` — a mutant
        // the oracle cannot parse.
        let names = declared_names("let { value: v = $bindable(), open = false } = $props();");
        assert!(names.contains(&"v".to_string()), "{names:?}");
        assert!(names.contains(&"open".to_string()), "{names:?}");
        // Array patterns too.
        let names = declared_names("const [first, second] = xs;");
        assert!(names.contains(&"first".to_string()), "{names:?}");
        assert!(names.contains(&"second".to_string()), "{names:?}");
    }

    #[test]
    fn declared_names_claim_import_bindings() {
        let names = declared_names("import a, { b as c } from 'm';");
        assert!(names.contains(&"a".to_string()), "{names:?}");
        assert!(names.contains(&"c".to_string()), "{names:?}");
        assert!(
            !names.contains(&"m".to_string()),
            "the module specifier is not a binding"
        );
    }

    #[test]
    fn exported_names_cover_both_export_forms() {
        // `export { s }` exports WITHOUT declaring, so a declaration-only guard let
        // `export const s = 1; export { s };` through — invalid JS, ungradeable.
        assert!(exported_names("export const s = 1;").contains(&"s".to_string()));
        assert!(exported_names("export { s, t };").contains(&"s".to_string()));
        assert!(exported_names("export { s, t };").contains(&"t".to_string()));
        // A re-export's module specifier is not an exported name.
        assert!(!exported_names("export { s } from 'm';").contains(&"m".to_string()));
    }

    #[test]
    fn identifier_tokens_skip_member_tails_and_keywords() {
        assert_eq!(
            identifier_tokens("a.b + c"),
            vec!["a".to_string(), "c".to_string()]
        );
        assert!(identifier_tokens("typeof x").contains(&"x".to_string()));
        assert!(!identifier_tokens("typeof x").contains(&"typeof".to_string()));
    }
}

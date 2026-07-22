//! `<style>` scoping analysis, combinator matching, and CSS splicing.
//!
//! A top-level rule's selector is a chain of compounds joined by combinators. Each
//! compound (a [`ScopedRelative`]) becomes a kind-tagged predicate list; the whole
//! chain is matched against the [element census](crate::element_census) with a
//! direct port of the oracle's backward matcher (`apply_selector` /
//! `apply_combinator`, `phases/2-analyze/css/css-prune.js`). A compound that a
//! successful chain match reaches gains the `svelte-tsvhash` hash class, and every
//! element the match touches gains it too, **source-spliced** into the style text
//! (author whitespace preserved), matching the oracle byte-for-byte
//! (`phases/3-transform/css/index.js`).
//!
//! Supported: the four combinators (descendant ` `, child `>`, next-sibling `+`,
//! subsequent-sibling `~`) over type / id / class / attribute / universal compounds
//! (plus trailing pseudo); basic `:global` — leading `:global(<compound>)`, trailing
//! `:global(<compound>)` (dropped by truncate), and a bare `:global` combinator
//! (`div :global.x` → `div.x`); a non-`@keyframes` **group at-rule**
//! (`@media`/`@supports`/`@container`/`@layer`/`@scope`/…), which recurses into its
//! block and scopes the inner rules the ordinary way (the oracle's generic `next()`
//! recursion — `phases/3-transform/css/index.js:82-99`; the at-rule prelude is never
//! scoped); and **`@keyframes`** — name-prefixed (`@keyframes foo` → `@keyframes
//! svelte-tsvhash-foo`, a `-global-` prefix stripped and un-scoped instead) with every
//! `animation` / `animation-name` value referencing a collected name rewritten to the
//! prefixed spelling (the oracle's `is_keyframes_node` collect + `Atrule`/`Declaration`
//! transform, `css-analyze.js:52-63` / `css/index.js:82-124`). The transform never
//! descends into a keyframes block, but the oracle's phase-2 PRUNE walk does: it matches
//! each step rule's selectors against every element the ordinary backward way, so a step
//! selector can scope elements (a `from` step matches `<from>`, `.c` matches
//! `class="c"`). A `Percentage` / `Nth` simple selector is skipped within its compound
//! (`css-prune.js:509`), so a percentage-only compound matches ANY element (the
//! fallthrough) while `0%.c` still narrows to `class="c"`. Step matching feeds ONLY the
//! scoped-element set — step selectors are never spliced, pruned, or refused for no
//! match. Everything else refuses:
//! the `||` column combinator, `:global{}` blocks, `:is`/`:where`/`:has`/`:not`,
//! `:root`/`:host`, nesting, namespaced/escaped names, a snippet/render-crossing
//! combinator path, and a compound matching no element (`CssSelectorNoMatch`; the
//! oracle comment-wraps).

use std::collections::HashSet;

use tsv_css::ast::internal::{
    AttributeMatcher, Combinator, ComplexSelector, CssAtrule, CssBlockChild, CssDeclaration,
    CssNode, CssRule, PseudoClassArgs, RelativeSelector, SimpleSelector,
};
use tsv_lang::Span;
use tsv_svelte::ast::internal::{AttributeNode, AttributeValue, Element, SpecialElement, Style};
use tsv_ts::ast::internal::Expression;

use crate::element_census::{
    CensusNode, ElementCensus, PathFrame, get_ancestor_elements, get_possible_element_siblings,
    has_element_parent,
};
use crate::text_class::{is_css_whitespace, is_js_whitespace};
use crate::transform_server::unsupported;
use crate::{CompileError, Refusal};

/// The deterministic scoping class — the fixed `cssHash` the oracle sidecar
/// compiles with, so outputs are byte-comparable across runs.
pub(crate) const SCOPE_HASH_CLASS: &str = "svelte-tsvhash";

/// The `@keyframes` name-prefix (`<hash>-`) — the oracle's `${state.hash}-`, inserted
/// before a keyframes name and before each `animation`-value token that references a
/// collected name (`css/index.js:92`/`:111`). NOT a class (no leading `.`).
const SCOPE_HASH_PREFIX: &str = "svelte-tsvhash-";

/// HTML attributes whose enumerated values are case-insensitive per the HTML
/// spec (the oracle's `case_insensitive_attributes`, `css-prune.js:30-67`).
const HTML_CASE_INSENSITIVE_ATTRIBUTES: &[&str] = &[
    "accept-charset",
    "autocapitalize",
    "autocomplete",
    "behavior",
    "charset",
    "crossorigin",
    "decoding",
    "dir",
    "direction",
    "draggable",
    "enctype",
    "enterkeyhint",
    "fetchpriority",
    "formenctype",
    "formmethod",
    "formtarget",
    "hidden",
    "http-equiv",
    "inputmode",
    "kind",
    "loading",
    "method",
    "preload",
    "referrerpolicy",
    "rel",
    "rev",
    "role",
    "rules",
    "scope",
    "shape",
    "spellcheck",
    "target",
    "translate",
    "type",
    "valign",
    "wrap",
];

/// The pseudo-classes the oracle's matcher treats specially (a filter, a
/// global-like exemption, or a nested selector list). All but `global` refuse in
/// this slice; `global` is handled explicitly (see [`classify_relative`]).
const REFUSED_PSEUDO_CLASSES: &[&str] = &["host", "root", "is", "where", "has", "not"];

/// One simple selector's element filter (the joint-AND leaf test).
enum Predicate {
    /// `*` — matches any regular element.
    Universal,
    /// `div` — tag-name case-insensitive equality (the oracle's `TypeSelector`).
    Type(String),
    /// `.c` — routes through `attribute_matches(el, "class", name, "~=")`.
    Class(String),
    /// `#x` — routes through `attribute_matches(el, "id", name, "=")`.
    Id(String),
    /// `[a]` / `[a=b]` / `[a="b" i]` — the general `attribute_matches` path.
    Attribute {
        name: String,
        matcher: Option<AttributeMatcher>,
        value: Option<String>,
        case_insensitive: bool,
    },
}

/// A source removal (a `:global` wrapper strip): drop `[at, remove_to)`, insert
/// nothing.
#[derive(Clone, Copy)]
struct Removal {
    at: u32,
    remove_to: u32,
}

/// A hash-splice anchor within a compound: insert the modifier at `at`, dropping
/// `[at, remove_to)`. An **append** (the common case) has `at == remove_to`; the
/// bare-`*` **replace** covers the `*` span so the `*` vanishes.
#[derive(Clone, Copy)]
struct Splice {
    at: u32,
    remove_to: u32,
}

/// What kind of leaf a relative selector is (the `:global` classification).
enum RelKind {
    /// A plain compound — the leaf test is `predicates` against the element.
    Normal,
    /// `:global(<compound>)` — the leaf test is the inner compound's `predicates`
    /// (the oracle applies the single inner complex selector BACKWARD).
    PureGlobal,
    /// A bare `:global` (possibly glued, `:global.x`) — the leaf always matches.
    BareGlobal,
}

/// One compound in the selector chain.
struct ScopedRelative {
    kind: RelKind,
    /// The leaf predicate list ([`RelKind::Normal`] compound, or [`RelKind::PureGlobal`]
    /// inner compound). Empty for [`RelKind::BareGlobal`].
    predicates: Vec<Predicate>,
    /// The combinator *before* this compound (`None` for the first).
    combinator: Option<Combinator>,
    /// A global relative (the oracle's `is_global` == `is_outer_global` in this
    /// slice): never scoped (no hash), always satisfied by `every_is_global`.
    global: bool,
    /// The hash-splice anchor ([`RelKind::Normal`] only; `None` for globals).
    anchor: Option<Splice>,
    /// Fixed source removals stripping a `:global(...)` / bare-`:global` wrapper.
    global_strip: Vec<Removal>,
}

/// One top-level compound chain (a `ComplexSelector`).
struct ScopedSelector {
    relatives: Vec<ScopedRelative>,
    /// How many leading relatives participate in matching — the oracle's `truncate`
    /// drops trailing global relatives. Splicing still touches every relative.
    match_len: usize,
    /// Every relative is global (the oracle's `ComplexSelector.is_global`): always
    /// "used" (never pruned), scopes no element.
    fully_global: bool,
    /// The compound's source text — the `CssSelectorNoMatch` refusal message.
    display: String,
}

/// The scoping analysis product: the parsed selector chains, in source order, plus
/// the `@keyframes` source edits.
pub(crate) struct ScopeInfo {
    selectors: Vec<ScopedSelector>,
    /// The `@keyframes` name-prefix / `-global-` strip / `animation`-value inserts,
    /// computed during analysis (the oracle's keyframes transform). Merged into
    /// [`splice_scoped_css`]'s edit vec before its sort — they touch source regions
    /// disjoint from the selector splices (keyframes names, `-global-` prefixes, and
    /// `animation`/`animation-name` value tokens, never a selector anchor).
    keyframes_edits: Vec<Edit>,
    /// The `@keyframes` STEP selectors (`0%`, `from`, `.c`, …), kept **separate** from
    /// [`selectors`](Self::selectors). The oracle's phase-2 prune walk descends into
    /// keyframes blocks and matches each step rule's selectors against every element the
    /// ordinary backward way; the transform never descends, so step matching affects ONLY
    /// the scoped-element set. Kept out of [`selectors`](Self::selectors) so they
    /// contribute NOTHING to `used` / `unused_selectors` (steps are never pruned),
    /// `relative_scoped` (never specificity-bumped), or the edit stream ([`splice_scoped_css`]
    /// pushes `global_strip` removals unconditionally from `selectors` — a step's source
    /// must stay verbatim, since the transform doesn't descend). [`match_scope`] matches
    /// each against the census, accumulating into `scoped_elements` only.
    step_selectors: Vec<ScopedSelector>,
}

/// The matching product — read at emission (`element_scope`), at the post-emission
/// no-match check (`used`), and at splice time (`relative_scoped`).
pub(crate) struct CssScoping {
    info: ScopeInfo,
    /// Element span `(start, end)` keys that gain the hash class (the oracle's
    /// `element.metadata.scoped`, accumulated across all selectors and every
    /// recursion level). `Span` is not `Hash`, so the pair is the key.
    scoped_elements: HashSet<(u32, u32)>,
    /// Per `ComplexSelector`: did it match any element?
    used: Vec<bool>,
    /// Per `ComplexSelector`, per relative: was the relative scoped (gets a hash)?
    relative_scoped: Vec<Vec<bool>>,
}

impl CssScoping {
    /// Whether the element at `span` gained the hash class (a lookup — matching
    /// already ran). The scope set is span-keyed, so a regular element and a
    /// `<svelte:element>` share one lookup.
    fn span_scoped(&self, span: Span) -> bool {
        self.scoped_elements.contains(&(span.start, span.end))
    }

    /// Whether `element` (a regular element) gained the hash class.
    pub(crate) fn element_scoped(&self, element: &Element<'_>) -> bool {
        self.span_scoped(element.span)
    }

    /// Whether `special` (a `<svelte:element>`) gained the hash class. The oracle
    /// scopes it whenever a type/universal selector reaches it (its type match is
    /// unconditional), synthesizing `class="svelte-…"` in its attributes closure.
    pub(crate) fn special_element_scoped(&self, special: &SpecialElement<'_>) -> bool {
        self.span_scoped(special.span)
    }

    /// The compounds that matched no element (pruning candidates). Each yields a
    /// [`Refusal::CssSelectorNoMatch`] — tsv refuses rather than reproduce the
    /// oracle's comment-wrap.
    pub(crate) fn unused_selectors(&self) -> impl Iterator<Item = Refusal> + '_ {
        self.info
            .selectors
            .iter()
            .zip(&self.used)
            .filter(|(_, used)| !**used)
            .map(|(selector, _)| Refusal::CssSelectorNoMatch {
                selector: selector.display.clone(),
            })
    }
}

/// Analyze a `<style>` for the supported shape and, when a census is present, match
/// the selectors against it.
///
/// `sink` is the [`refusal_census`](mod@crate::refusal_census) collect seam: `None` bails at the first
/// unsupported (parse-time) shape (the compile path); `Some` pushes each and
/// continues. In collect mode the returned [`ScopeInfo`] is partial and unused.
///
/// Matching is deferred to [`match_scope`] because it needs the element census
/// (built by the caller). In collect mode there is no census, so [`analyze_style`]
/// only surfaces the parse-time refusals.
pub(crate) fn analyze_style(
    style: &Style<'_>,
    source: &str,
    mut sink: Option<&mut Vec<Refusal>>,
) -> Result<ScopeInfo, CompileError> {
    let mut info = ScopeInfo {
        selectors: Vec::new(),
        keyframes_edits: Vec::new(),
        step_selectors: Vec::new(),
    };
    // Collection is a separate pre-pass: an `animation`/`animation-name` value can
    // reference a keyframes name declared LATER in source (the oracle collects every
    // keyframes prelude in phase 2 before the phase-3 value rewrite). The walk descends
    // everywhere — including into a keyframes block, where a nested keyframes still
    // collects (`css-analyze.js` calls `context.next()` unconditionally).
    let keyframes = collect_keyframes(style.css_stylesheet.nodes, source);
    let content_end = style.content_span.end;
    for node in style.css_stylesheet.nodes {
        match node {
            CssNode::Rule(rule) => {
                analyze_rule(rule, source, &keyframes, content_end, &mut sink, &mut info)?;
            }
            CssNode::Atrule(atrule) => {
                analyze_atrule(
                    atrule,
                    source,
                    &keyframes,
                    content_end,
                    &mut sink,
                    &mut info,
                )?;
            }
        }
    }
    Ok(info)
}

// ── @keyframes collection + transform (the oracle's is_keyframes_node handling) ─

/// Collect every `@keyframes` prelude the oracle's phase-2 analysis pushes into
/// `state.keyframes` (`css-analyze.js:52-63`): the trimmed prelude of every keyframes
/// at-rule at any nesting depth whose prelude does not start with `-global-`. The walk
/// descends into every at-rule block (including a keyframes block, so a nested keyframes
/// collects too), mirroring the analysis walk's unconditional `context.next()`.
/// Duplicates are harmless — the value rewrite is a membership test.
///
/// The name string is the oracle's `node.prelude` exactly, via
/// [`CssAtrule::public_prelude`] — the wire-parity form (block comments elided, JS-whitespace
/// trim), NOT the printer-facing `Raw::content` (comment-preserving, CSS-whitespace trim).
/// The two diverge whenever the prelude carries a comment or JS-only whitespace, and the
/// membership compare here is against a raw-source `animation` value token, so a wrong name
/// silently fails to rewrite the reference while the at-rule name still gets prefixed —
/// renamed keyframes, un-renamed reference.
///
/// ⚠️ The oracle also gates the push on `!is_in_global_block(context.path)`
/// (`css-analyze.js:52`), a conjunct tsv drops here (matching [`keyframes_name_edit`]). It
/// is safe: the only shape it excludes is a keyframes inside a `:global { … }` block, which
/// sits inside a rule — and a rule containing a nested at-rule refuses [`Refusal::CssNestedRule`]
/// in [`analyze_rule`] before any name edit is emitted, so a name collected here is discarded
/// on that over-refusal and never reaches output.
fn collect_keyframes(nodes: &[CssNode<'_>], source: &str) -> Vec<String> {
    let mut names = Vec::new();
    for node in nodes {
        match node {
            CssNode::Rule(rule) => {
                collect_keyframes_children(rule.declarations, source, &mut names);
            }
            CssNode::Atrule(atrule) => collect_keyframes_atrule(atrule, source, &mut names),
        }
    }
    names
}

fn collect_keyframes_children(
    children: &[CssBlockChild<'_>],
    source: &str,
    names: &mut Vec<String>,
) {
    for child in children {
        match child {
            CssBlockChild::Rule(rule) => {
                collect_keyframes_children(rule.declarations, source, names);
            }
            CssBlockChild::Atrule(atrule) => collect_keyframes_atrule(atrule, source, names),
            CssBlockChild::Declaration(_) | CssBlockChild::Comment(_) => {}
        }
    }
}

fn collect_keyframes_atrule(atrule: &CssAtrule<'_>, source: &str, names: &mut Vec<String>) {
    if is_keyframes_atrule(atrule.name) {
        let prelude = atrule.public_prelude(source);
        if !prelude.starts_with("-global-") {
            names.push(prelude.into_owned());
        }
    }
    if let Some(block) = &atrule.block {
        // Descend regardless — the analysis walk enters even a keyframes block, so a nested
        // keyframes collects (it is never PREFIXED, since the transform returns early).
        collect_keyframes_children(block.children, source, names);
    }
}

/// Analyze one top-level-or-nested CSS rule: refuse a nested rule / an empty rule,
/// else build a [`ScopedSelector`] per `ComplexSelector` into `info`. Shared by the
/// top-level walk and the at-rule descent, so a rule inside `@media` scopes exactly
/// like a top-level one. Preserves the sink's collect-vs-bail semantics via
/// [`refuse`] (`None` bails at the first refusal; `Some` pushes and continues).
fn analyze_rule(
    rule: &CssRule<'_>,
    source: &str,
    keyframes: &[String],
    content_end: u32,
    sink: &mut Option<&mut Vec<Refusal>>,
    info: &mut ScopeInfo,
) -> Result<(), CompileError> {
    for child in rule.declarations {
        if matches!(child, CssBlockChild::Rule(_) | CssBlockChild::Atrule(_)) {
            refuse(sink, Refusal::CssNestedRule)?;
            break;
        }
    }
    // An empty rule (no declarations) is comment-wrapped `/* (empty) … */` by the
    // oracle in non-dev mode; tsv declines to reproduce the wrap and refuses.
    if !rule
        .declarations
        .iter()
        .any(|child| matches!(child, CssBlockChild::Declaration(_)))
    {
        refuse(sink, Refusal::CssEmptyRule)?;
        return Ok(());
    }
    // The oracle's `Declaration` visitor runs on every declaration the transform reaches
    // (`css/index.js:100`) — including inside a plain rule — rewriting `animation` /
    // `animation-name` values that reference a collected keyframes name.
    for child in rule.declarations {
        if let CssBlockChild::Declaration(decl) = child {
            scan_animation_declaration(
                decl,
                source,
                keyframes,
                content_end,
                &mut info.keyframes_edits,
            );
        }
    }
    for complex in rule.selector.selectors {
        match build_selector(complex, source, false) {
            Ok(selector) => info.selectors.push(selector),
            Err(reason) => refuse(sink, reason)?,
        }
    }
    Ok(())
}

/// Analyze one at-rule (the oracle's `Atrule` visitor,
/// `phases/3-transform/css/index.js:82-99`). `@keyframes` (name-discriminated — the
/// ONLY at-rule family whose inner "rules" are keyframe stops rather than element
/// selectors) is name-prefixed and its block is LEFT UNTRANSFORMED: the oracle emits the
/// name edit (`@keyframes foo` → `@keyframes svelte-tsvhash-foo`, or strips a `-global-`
/// prefix) and `return`s WITHOUT descending, so its inner step rules are never scoped
/// (spliced), its declarations never rewritten, and nothing inside it fails to compile
/// for an empty rule or a no-match selector. But the oracle's separate phase-2 PRUNE walk
/// DOES descend into the block and match each step rule's selectors against every element
/// — so [`analyze_keyframes_steps`] collects those step selectors (they scope elements
/// without ever touching the CSS output). Every other at-rule
/// recurses generically into its block — inner rules scope like top-level ones, nested
/// at-rules recurse further, and a descriptor block (`@font-face`/`@page`, whose
/// children are descriptors — and, for `@page`, margin at-rules like `@top-center` —
/// never element-selector rules, so a margin at-rule recurses harmlessly) or a
/// statement at-rule (`@import`/`@charset`/`@layer a,b;`, `block: None`) scopes nothing.
/// A descriptor declaration is still `animation`-scanned (the oracle's `Declaration`
/// visitor reaches `@font-face`, and `@media`-nested declarations at any depth). The
/// at-rule PRELUDE is never touched — `@scope (.a) to (.b) { .a {} }` scopes only the
/// inner `.a`.
fn analyze_atrule(
    atrule: &CssAtrule<'_>,
    source: &str,
    keyframes: &[String],
    content_end: u32,
    sink: &mut Option<&mut Vec<Refusal>>,
    info: &mut ScopeInfo,
) -> Result<(), CompileError> {
    // `@keyframes` (name-discriminated case-sensitively): emit the name edit and RETURN —
    // the TRANSFORM does not descend into the block (no scoping splice, no declaration
    // rewrite), mirroring the oracle's `return; // don't transform anything within`. The
    // name-prefix path runs regardless of block presence (`@keyframes foo;`, `block: None`).
    // The oracle's separate PRUNE walk still descends the block: collect its step selectors
    // (they scope elements without ever splicing the block).
    if is_keyframes_atrule(atrule.name) {
        info.keyframes_edits
            .push(keyframes_name_edit(atrule, source));
        analyze_keyframes_steps(atrule, source, sink, info)?;
        return Ok(());
    }
    // A statement at-rule (`block: None`) scopes nothing.
    let Some(block) = &atrule.block else {
        return Ok(());
    };
    for child in block.children {
        match child {
            CssBlockChild::Rule(rule) => {
                analyze_rule(rule, source, keyframes, content_end, sink, info)?;
            }
            CssBlockChild::Atrule(nested) => {
                analyze_atrule(nested, source, keyframes, content_end, sink, info)?;
            }
            // A descriptor declaration (`@font-face`/`@page`) scopes no selector but is
            // still `animation`-scanned; a comment scopes nothing (emitted verbatim).
            CssBlockChild::Declaration(decl) => {
                scan_animation_declaration(
                    decl,
                    source,
                    keyframes,
                    content_end,
                    &mut info.keyframes_edits,
                );
            }
            CssBlockChild::Comment(_) => {}
        }
    }
    Ok(())
}

/// Collect a `@keyframes` block's direct step rules into [`ScopeInfo::step_selectors`].
/// The oracle's phase-2 prune walk (`css-prune.js`) descends into every keyframes block
/// — for a `-global-` keyframes, one nested in `@media`, at any depth — and matches each
/// step rule's selectors against every element with the ordinary backward matcher; a
/// matched element gains the hash class. The transform never descends, so a step is never
/// spliced, never pruned as unused (no `CssSelectorNoMatch`), and an empty step (`from {}`)
/// never hits the empty-rule refusal.
///
/// Each step's prelude `ComplexSelector`s are built through the SAME
/// [`build_selector`] machinery as ordinary rules, with `keyframe_step = true` so a
/// `Percentage`/`Nth` simple selector is skipped within its compound
/// (`css-prune.js:509`): `0%` becomes an empty predicate list that matches any element,
/// `0%.c` narrows to `class="c"`. A build failure on a step (an unsupported pseudo, …)
/// refuses via the sink (safe over-refusal, corpus-absent). A nested rule/at-rule inside
/// a step, or an at-rule directly inside the keyframes block, refuses
/// [`Refusal::CssNestedRule`] (safe over-refusal — the oracle's step preludes are simple
/// compounds). Declaration/comment children of the block or of a step are ignored.
fn analyze_keyframes_steps(
    atrule: &CssAtrule<'_>,
    source: &str,
    sink: &mut Option<&mut Vec<Refusal>>,
    info: &mut ScopeInfo,
) -> Result<(), CompileError> {
    let Some(block) = &atrule.block else {
        return Ok(());
    };
    for child in block.children {
        match child {
            CssBlockChild::Rule(step) => {
                // A nested rule/at-rule INSIDE a step rule refuses (corpus-absent).
                for grandchild in step.declarations {
                    if matches!(
                        grandchild,
                        CssBlockChild::Rule(_) | CssBlockChild::Atrule(_)
                    ) {
                        refuse(sink, Refusal::CssNestedRule)?;
                        break;
                    }
                }
                for complex in step.selector.selectors {
                    match build_selector(complex, source, true) {
                        Ok(selector) => info.step_selectors.push(selector),
                        Err(reason) => refuse(sink, reason)?,
                    }
                }
            }
            // An at-rule directly inside a keyframes block refuses (corpus-absent).
            CssBlockChild::Atrule(_) => refuse(sink, Refusal::CssNestedRule)?,
            CssBlockChild::Declaration(_) | CssBlockChild::Comment(_) => {}
        }
    }
    Ok(())
}

/// The `@keyframes` name edit (`css/index.js:82-93`): the insert point is
/// `atrule.start + 1 + decoded_name.len()` (the `@` plus the decoded at-keyword — ASCII
/// by the keyframes discriminator, so byte length == the oracle's `node.name.length`),
/// advanced over LITERAL SPACE bytes only (not tabs/newlines). A `-global-` prelude
/// REMOVES the 8-byte prefix there; otherwise `svelte-tsvhash-` is INSERTED. The
/// `-global-` test reads [`CssAtrule::public_prelude`] — the oracle's `node.prelude`
/// byte-for-byte (comment-elided, JS-whitespace-trimmed), so a prelude whose `-global-`
/// hides behind a comment (`@keyframes /* c */-global-foo`) still takes the strip branch,
/// as the oracle does.
///
/// ⚠️ The oracle gates the INSERT branch on `!is_in_global_block(path)`
/// (`css/index.js:83`), a conjunct tsv drops (matching [`collect_keyframes_atrule`]). The
/// `-global-` STRIP branch runs unconditionally on both sides, so only the insert is
/// affected — and the sole shape it changes is a keyframes inside a `:global { … }` block,
/// which tsv never reaches: the enclosing rule holds a nested at-rule and refuses
/// [`Refusal::CssNestedRule`] in [`analyze_rule`] before this edit is emitted (a safe
/// over-refusal masking the dropped conjunct; the oracle instead compiles it un-prefixed).
fn keyframes_name_edit(atrule: &CssAtrule<'_>, source: &str) -> Edit {
    let mut start = atrule.span.start + 1 + atrule.name.len() as u32;
    let bytes = source.as_bytes();
    while (start as usize) < bytes.len() && bytes[start as usize] == b' ' {
        start += 1;
    }
    if atrule.public_prelude(source).starts_with("-global-") {
        Edit {
            at: start,
            remove_to: start + 8,
            insert: "",
        }
    } else {
        Edit {
            at: start,
            remove_to: start,
            insert: SCOPE_HASH_PREFIX,
        }
    }
}

/// Rewrite an `animation` / `animation-name` declaration's value tokens that reference a
/// collected keyframes name (the oracle's `Declaration` visitor, `css/index.js:100-124`).
/// The property compare uses the RAW property source text (so an escaped `\61nimation`
/// spelling never matches, as the oracle's raw compare doesn't), ASCII-lowercased then
/// vendor-prefix-stripped. ASCII-lowercase is provably equivalent to JS `.toLowerCase()`
/// here: the only non-ASCII→ASCII single-char lower map is U+212A→`k`, and neither target
/// string contains `k`; `İ` lowers to two code units and can never equal them.
fn scan_animation_declaration(
    decl: &CssDeclaration<'_>,
    source: &str,
    keyframes: &[String],
    content_end: u32,
    edits: &mut Vec<Edit>,
) {
    if keyframes.is_empty() {
        return;
    }
    // Raw property = source from the declaration start (the property identifier start,
    // like the oracle's `node.start`) up to the first JS `\s` — mirroring Svelte's
    // `read_until(/[\s:]/)`, since the slice already ends at the colon.
    let prop_slice = &source[decl.span.start as usize..decl.colon_offset as usize];
    let raw_property = match prop_slice.find(is_js_whitespace) {
        Some(i) => &prop_slice[..i],
        None => prop_slice,
    };
    let lowered = raw_property.to_ascii_lowercase();
    let stripped = remove_css_prefix(&lowered);
    if stripped != "animation" && stripped != "animation-name" {
        return;
    }
    // Scan forward from `decl.start + raw_property.len() + 1` (the `+1` is the `:`).
    let index_start = decl.span.start as usize + raw_property.len() + 1;
    let end = (content_end as usize).min(source.len());
    if index_start >= end {
        return;
    }
    // A name token is the run of chars between boundaries (JS `\s` or `,` `;` `}` — the
    // oracle's `regex_css_name_boundary`). At each boundary the accumulated token — the
    // EMPTY token included, so a collected `''` matches at every boundary — is inserted
    // before if it is a collected keyframes name. `token_start` tracks the token's start
    // byte (never index-minus-length arithmetic), matching the oracle's `index -
    // name.length`. The scan stops at a `;`/`}` boundary.
    let mut token_start = index_start;
    for (rel, ch) in source[index_start..end].char_indices() {
        let index = index_start + rel;
        if is_js_whitespace(ch) || ch == ',' || ch == ';' || ch == '}' {
            let token = &source[token_start..index];
            if keyframes.iter().any(|name| name == token) {
                edits.push(Edit {
                    at: token_start as u32,
                    remove_to: token_start as u32,
                    insert: SCOPE_HASH_PREFIX,
                });
            }
            if ch == ';' || ch == '}' {
                break;
            }
            token_start = index + ch.len_utf8();
        }
    }
}

/// Whether an at-rule name is `@keyframes` — the oracle's `is_keyframes_node`
/// (`remove_css_prefix(node.name) === 'keyframes'`, `phases/css.js:14`).
/// **Case-sensitive** on purpose: `@KEYFRAMES` is NOT keyframes to the oracle, so it
/// recurses as a group at-rule and its `from`/`to` are treated as element selectors
/// (which match nothing → `CssSelectorNoMatch`). `atrule.name` is escape-decoded by
/// tsv_css, matching the oracle's decoded `node.name`.
fn is_keyframes_atrule(name: &str) -> bool {
    remove_css_prefix(name) == "keyframes"
}

/// Strip a leading vendor prefix (`-webkit-`/`-moz-`/`-o-`/`-ms-`) — the oracle's
/// `remove_css_prefix` (`/^-((webkit)|(moz)|(o)|(ms))-/`, `phases/css.js:2-9`).
/// Case-sensitive, like the regex (no `i` flag).
fn remove_css_prefix(name: &str) -> &str {
    for prefix in ["-webkit-", "-moz-", "-o-", "-ms-"] {
        if let Some(rest) = name.strip_prefix(prefix) {
            return rest;
        }
    }
    name
}

/// Match every selector chain against the census, producing the [`CssScoping`]
/// emission reads. Runs the oracle's backward matcher per (ComplexSelector, census
/// element); a dynamic-attribute or non-ASCII case-fold match, or a
/// snippet-crossing combinator, refuses.
pub(crate) fn match_scope(
    info: ScopeInfo,
    census: &ElementCensus<'_>,
    source: &str,
) -> Result<CssScoping, CompileError> {
    let mut scoped_elements: HashSet<(u32, u32)> = HashSet::new();
    let mut used = Vec::with_capacity(info.selectors.len());
    let mut relative_scoped = Vec::with_capacity(info.selectors.len());

    for selector in &info.selectors {
        let mut rel_scoped = vec![false; selector.relatives.len()];
        let mut selector_used = selector.fully_global;
        if !selector.fully_global {
            for census_element in &census.elements {
                if apply_selector(
                    selector,
                    census_element.node,
                    &census_element.path,
                    0,
                    selector.match_len,
                    census,
                    source,
                    &mut scoped_elements,
                    &mut rel_scoped,
                )? {
                    selector_used = true;
                }
            }
        }
        used.push(selector_used);
        relative_scoped.push(rel_scoped);
    }

    // `@keyframes` step selectors: the oracle's prune walk descends into keyframes blocks
    // and matches each step compound against every element. This only expands the
    // scoped-element set — steps are never spliced (the transform doesn't descend), never
    // marked `used`/`relative_scoped`, so they run through a THROWAWAY `rel_scoped` buffer
    // and touch only `scoped_elements`. A percentage-only compound has an empty predicate
    // list and matches every element (the fallthrough); a no-match step scopes nothing and
    // never refuses. A dynamic-attribute / non-ASCII case-fold / snippet-crossing step
    // refuses via `apply_selector` (safe over-refusal, corpus-absent).
    for selector in &info.step_selectors {
        if selector.fully_global {
            continue;
        }
        let mut scratch = vec![false; selector.relatives.len()];
        for census_element in &census.elements {
            apply_selector(
                selector,
                census_element.node,
                &census_element.path,
                0,
                selector.match_len,
                census,
                source,
                &mut scoped_elements,
                &mut scratch,
            )?;
        }
    }

    Ok(CssScoping {
        info,
        scoped_elements,
        used,
        relative_scoped,
    })
}

// ── Selector chain parsing ────────────────────────────────────────────────────

/// Build one `ComplexSelector` into the chain model, or the [`Refusal`] its shape
/// maps to. `keyframe_step` marks a `@keyframes` step selector: a `Percentage`/`Nth`
/// simple selector is then skipped within its compound (the oracle's `css-prune.js:509`
/// `continue`). Ordinary selectors pass `false`, so a top-level `50% {}` keeps refusing.
fn build_selector(
    complex: &ComplexSelector<'_>,
    source: &str,
    keyframe_step: bool,
) -> Result<ScopedSelector, Refusal> {
    if complex.children.is_empty() {
        return Err(Refusal::CssUnsupportedSelector);
    }
    // A leading combinator on the first relative is invalid (the oracle errors it).
    if complex.children[0].combinator.is_some() {
        return Err(Refusal::CssCombinatorSelector);
    }

    let mut relatives = Vec::with_capacity(complex.children.len());
    for relative in complex.children {
        relatives.push(classify_relative(relative, source, keyframe_step)?);
    }

    // `truncate` (css-prune.js:209-232): drop trailing global relatives from the
    // MATCH chain (they still splice). `match_len` is one past the last non-global.
    let match_len = relatives
        .iter()
        .rposition(|r| !r.global)
        .map_or(0, |i| i + 1);
    let fully_global = match_len == 0;

    Ok(ScopedSelector {
        relatives,
        match_len,
        fully_global,
        display: complex.span.extract(source).to_string(),
    })
}

/// Classify one compound: `:global(...)` (pure), bare `:global` (possibly glued),
/// or a plain compound. Refuses unsupported combinators/`:global` shapes. `keyframe_step`
/// is threaded to [`parse_plain_compound`] (skip `Percentage`/`Nth` within a keyframes
/// step compound).
fn classify_relative(
    relative: &RelativeSelector<'_>,
    source: &str,
    keyframe_step: bool,
) -> Result<ScopedRelative, Refusal> {
    let combinator = relative.combinator;
    if combinator == Some(Combinator::Column) {
        return Err(Refusal::CssCombinatorSelector);
    }
    let simples = relative.selectors;
    // An empty compound (consecutive combinators, `> > .a`) has no anchor.
    if simples.is_empty() {
        return Err(Refusal::CssCombinatorSelector);
    }

    // PureGlobal: a lone `:global(<args>)`.
    if simples.len() == 1
        && let SimpleSelector::PseudoClass {
            args: Some(args),
            span,
        } = &simples[0]
        && pseudo_name(span.extract(source)) == "global"
    {
        let inner = parse_global_args(args, source, keyframe_step)?;
        return Ok(ScopedRelative {
            kind: RelKind::PureGlobal,
            predicates: inner,
            combinator,
            global: true,
            anchor: None,
            global_strip: pure_global_strip(*span),
        });
    }

    // BareGlobal: the compound leads with a bare `:global` (no args). `:global`
    // short-circuits the leaf to "matches", so the tail is unscoped but printed.
    if let SimpleSelector::PseudoClass { args: None, span } = &simples[0]
        && pseudo_name(span.extract(source)) == "global"
    {
        for simple in &simples[1..] {
            validate_bare_global_tail(simple, source)?;
        }
        return Ok(ScopedRelative {
            kind: RelKind::BareGlobal,
            predicates: Vec::new(),
            combinator,
            global: true,
            anchor: None,
            global_strip: bare_global_strip(*span, combinator, source),
        });
    }

    // Any other `:global` usage (`.x:global`, `:global(a, b)`, `:global(.x).y`) is
    // outside the supported forms.
    if simples
        .iter()
        .any(|simple| is_global_pseudo(simple, source))
    {
        return Err(Refusal::CssUnsupportedSelector);
    }

    let (predicates, anchor) = parse_plain_compound(simples, source, keyframe_step)?;
    Ok(ScopedRelative {
        kind: RelKind::Normal,
        predicates,
        combinator,
        global: false,
        anchor: Some(anchor),
        global_strip: Vec::new(),
    })
}

/// Parse a plain compound into its predicate list and hash-splice anchor. Refuses
/// combinators (via the caller), the refused pseudos, namespaced/escaped/nesting/nth
/// selectors, and a bare pseudo-only compound (no anchor). `keyframe_step` skips a
/// `Percentage`/`Nth` simple selector (the oracle's `css-prune.js:509` `continue`): the
/// skipped selector contributes no predicate, so a compound of only these has an empty
/// predicate list and matches ANY element (the oracle's fallthrough `return true`).
fn parse_plain_compound(
    simples: &[SimpleSelector<'_>],
    source: &str,
    keyframe_step: bool,
) -> Result<(Vec<Predicate>, Splice), Refusal> {
    let mut predicates = Vec::new();
    for simple in simples {
        match simple {
            // Skipped within a keyframes step compound (the oracle's `continue`). Gated on
            // `keyframe_step` — a top-level `50% {}` falls through to the `_` arm and keeps
            // refusing `CssUnsupportedSelector` (safe over-refusal, byte-unchanged).
            SimpleSelector::Percentage { .. } | SimpleSelector::Nth { .. } if keyframe_step => {}
            SimpleSelector::Universal {
                namespace: None, ..
            } => predicates.push(Predicate::Universal),
            SimpleSelector::Type {
                namespace: None,
                span,
            } => {
                let name = span.extract(source);
                refuse_if_escaped(name)?;
                refuse_if_non_ascii(name)?;
                predicates.push(Predicate::Type(name.to_string()));
            }
            SimpleSelector::Class { span } => {
                let name = &span.extract(source)[1..];
                refuse_if_escaped(name)?;
                predicates.push(Predicate::Class(name.to_string()));
            }
            SimpleSelector::Id { span } => {
                let name = &span.extract(source)[1..];
                refuse_if_escaped(name)?;
                predicates.push(Predicate::Id(name.to_string()));
            }
            SimpleSelector::Attribute {
                namespace: None,
                name_span,
                matcher,
                value,
                flags,
                ..
            } => {
                let name = name_span.extract(source);
                refuse_if_escaped(name)?;
                refuse_if_non_ascii(name)?;
                let name_lower = name.to_ascii_lowercase();
                let case_insensitive = flags_has(*flags, 'i')
                    || (!flags_has(*flags, 's')
                        && HTML_CASE_INSENSITIVE_ATTRIBUTES.contains(&name_lower.as_str()));
                let value = match value {
                    Some(v) => {
                        refuse_if_escaped(v)?;
                        if case_insensitive {
                            refuse_if_non_ascii(v)?;
                        }
                        Some((*v).to_string())
                    }
                    None => None,
                };
                predicates.push(Predicate::Attribute {
                    name: name.to_string(),
                    matcher: *matcher,
                    value,
                    case_insensitive,
                });
            }
            SimpleSelector::PseudoClass { span, .. } => {
                let raw = span.extract(source);
                refuse_if_escaped(raw)?;
                let name = pseudo_name(raw);
                if name == "global" || REFUSED_PSEUDO_CLASSES.contains(&name.as_str()) {
                    return Err(Refusal::CssUnsupportedSelector);
                }
            }
            SimpleSelector::PseudoElement { span, .. } => {
                refuse_if_escaped(span.extract(source))?;
            }
            _ => return Err(Refusal::CssUnsupportedSelector),
        }
    }

    let Some(splice) = compute_splice(simples) else {
        return Err(Refusal::CssUnsupportedSelector);
    };
    Ok((predicates, splice))
}

/// Parse a `:global(<args>)`'s inner compound: exactly one complex selector, one
/// relative selector (no combinator), a plain compound. Yields its leaf predicates.
/// `keyframe_step` is threaded to [`parse_plain_compound`] for consistency (a `:global`
/// inside a keyframes step is corpus-absent, but the machinery stays one mechanism).
fn parse_global_args(
    args: &PseudoClassArgs<'_>,
    source: &str,
    keyframe_step: bool,
) -> Result<Vec<Predicate>, Refusal> {
    let PseudoClassArgs::SelectorList { selectors, .. } = args else {
        return Err(Refusal::CssUnsupportedSelector);
    };
    let [complex] = selectors.selectors else {
        return Err(Refusal::CssUnsupportedSelector);
    };
    let [relative] = complex.children else {
        return Err(Refusal::CssUnsupportedSelector);
    };
    if relative.combinator.is_some() {
        return Err(Refusal::CssUnsupportedSelector);
    }
    // Reuse the plain-compound parser; the inner anchor is unused (the whole
    // `:global(...)` is stripped, not hash-spliced).
    let (predicates, _anchor) = parse_plain_compound(relative.selectors, source, keyframe_step)?;
    Ok(predicates)
}

/// The strip removals for a `:global(<args>)`: drop `:global(` (8 chars) and the
/// closing `)`.
fn pure_global_strip(global_span: Span) -> Vec<Removal> {
    let open_len = ":global(".len() as u32;
    vec![
        Removal {
            at: global_span.start,
            remove_to: global_span.start + open_len,
        },
        Removal {
            at: global_span.end - 1,
            remove_to: global_span.end,
        },
    ]
}

/// The strip removal for a bare `:global`: drop `:global`, plus the preceding
/// whitespace when the combinator is descendant (`div :global.x` → `div.x`,
/// index.js `remove_global_pseudo_class`). The oracle's back-scan is
/// `while (/\s/.test(state.code.original[start - 1])) start--` — a JavaScript
/// regex over CSS text, so the class is [`is_js_whitespace`], not a CSS one.
fn bare_global_strip(
    global_span: Span,
    combinator: Option<Combinator>,
    source: &str,
) -> Vec<Removal> {
    let mut start = global_span.start;
    if combinator == Some(Combinator::Descendant) {
        let before = &source[..global_span.start as usize];
        for (i, c) in before.char_indices().rev() {
            if is_js_whitespace(c) {
                start = i as u32;
            } else {
                break;
            }
        }
    }
    vec![Removal {
        at: start,
        remove_to: global_span.end,
    }]
}

/// Whether `simple` is a `:global` pseudo-class (any form).
fn is_global_pseudo(simple: &SimpleSelector<'_>, source: &str) -> bool {
    matches!(simple, SimpleSelector::PseudoClass { span, .. } if pseudo_name(span.extract(source)) == "global")
}

/// Validate a simple selector that trails a bare `:global` in the same compound
/// (`:global.x` → `.x` is fine; `:global:has(...)` is not).
fn validate_bare_global_tail(simple: &SimpleSelector<'_>, source: &str) -> Result<(), Refusal> {
    match simple {
        SimpleSelector::Universal {
            namespace: None, ..
        } => Ok(()),
        SimpleSelector::Type {
            namespace: None,
            span,
        } => {
            refuse_if_escaped(span.extract(source))?;
            refuse_if_non_ascii(span.extract(source))
        }
        SimpleSelector::Class { span } | SimpleSelector::Id { span } => {
            refuse_if_escaped(span.extract(source))
        }
        SimpleSelector::Attribute {
            namespace: None,
            name_span,
            ..
        } => {
            refuse_if_escaped(name_span.extract(source))?;
            refuse_if_non_ascii(name_span.extract(source))
        }
        SimpleSelector::PseudoElement { span, .. } => refuse_if_escaped(span.extract(source)),
        SimpleSelector::PseudoClass { span, .. } => {
            let raw = span.extract(source);
            refuse_if_escaped(raw)?;
            let name = pseudo_name(raw);
            if name == "global" || REFUSED_PSEUDO_CLASSES.contains(&name.as_str()) {
                Err(Refusal::CssUnsupportedSelector)
            } else {
                Ok(())
            }
        }
        _ => Err(Refusal::CssUnsupportedSelector),
    }
}

/// The splice anchor for a compound: after the LAST non-pseudo simple selector (the
/// oracle's backward walk skipping trailing pseudo). A bare `*` **replaces** its
/// span; every other anchor **appends**. `None` when pseudo-only (no anchor).
fn compute_splice(simples: &[SimpleSelector<'_>]) -> Option<Splice> {
    for simple in simples.iter().rev() {
        match simple {
            SimpleSelector::PseudoClass { .. } | SimpleSelector::PseudoElement { .. } => continue,
            SimpleSelector::Universal {
                namespace: None,
                span,
            } => {
                return Some(Splice {
                    at: span.start,
                    remove_to: span.end,
                });
            }
            other => {
                let end = other.span().end;
                return Some(Splice {
                    at: end,
                    remove_to: end,
                });
            }
        }
    }
    None
}

/// A pseudo-class's name, lowercased (CSS keywords are ASCII case-insensitive).
///
/// The trim is [`is_css_whitespace`], NOT Rust's `str::trim`. Every code point
/// at or above `U+0080` is a CSS ident code point, so a Unicode-whitespace trim
/// eats part of the NAME: `:global\u{00A0}` is the pseudo-class
/// `global\u{00A0}`, which the oracle does not recognize as `:global` at all. It
/// is then just an unknown trailing pseudo-class — the rule is KEPT and its
/// compounds scoped the ordinary way, in BOTH the descendant form
/// (`div :global\u{00A0}.x` → `div.svelte-tsvhash :global\u{00A0}.x:where(…)`)
/// and the compound form (`.x:global\u{00A0}` → `.x.svelte-tsvhash:global\u{00A0}`);
/// oracle-probed, and tsv reaches parity on each. A `str::trim` instead read the
/// name as `:global` and took the global-handling path (strip / no hash) — an
/// oracle-verified MISMATCH.
fn pseudo_name(raw: &str) -> String {
    let stripped = raw.trim_start_matches(':');
    let end = stripped.find('(').unwrap_or(stripped.len());
    stripped[..end]
        .trim_matches(is_css_whitespace)
        .to_ascii_lowercase()
}

fn flags_has(flags: Option<&str>, ch: char) -> bool {
    flags.is_some_and(|f| f.contains(ch))
}

fn refuse_if_escaped(text: &str) -> Result<(), Refusal> {
    if text.contains('\\') {
        return Err(Refusal::CssUnsupportedSelector);
    }
    Ok(())
}

fn refuse_if_non_ascii(text: &str) -> Result<(), Refusal> {
    if !text.is_ascii() {
        return Err(Refusal::CssCaseInsensitiveNonAscii);
    }
    Ok(())
}

fn refuse(sink: &mut Option<&mut Vec<Refusal>>, reason: Refusal) -> Result<(), CompileError> {
    match sink {
        Some(collected) => {
            collected.push(reason);
            Ok(())
        }
        None => Err(unsupported(reason)),
    }
}

// ── Combinator matching (the oracle's apply_selector / apply_combinator) ───────

/// Whether `element` satisfies the whole chain `sel.relatives[from..to]`, matched
/// BACKWARD from the rightmost relative (`css-prune.js:243-279`). Marks a matched
/// relative scoped (unless global) and inserts every touched element's span.
#[allow(clippy::too_many_arguments)]
fn apply_selector<'a>(
    sel: &ScopedSelector,
    element: CensusNode<'a>,
    path: &[PathFrame<'a>],
    from: usize,
    to: usize,
    census: &ElementCensus<'a>,
    source: &str,
    scoped: &mut HashSet<(u32, u32)>,
    rel_scoped: &mut [bool],
) -> Result<bool, CompileError> {
    if from >= to {
        return Ok(false);
    }
    let idx = to - 1;
    let relative = &sel.relatives[idx];
    let matched = relative_might_apply(relative, element, source)?
        && apply_combinator(
            sel, relative, path, from, idx, census, source, scoped, rel_scoped,
        )?;
    if matched {
        if !relative.global {
            rel_scoped[idx] = true;
        }
        let span = element.span();
        scoped.insert((span.start, span.end));
    }
    Ok(matched)
}

/// Resolve the combinator to the left of `relative` (`css-prune.js:291-359`,
/// BACKWARD only). A snippet-crossing walk refuses.
#[allow(clippy::too_many_arguments)]
fn apply_combinator<'a>(
    sel: &ScopedSelector,
    relative: &ScopedRelative,
    path: &[PathFrame<'a>],
    from: usize,
    to: usize,
    census: &ElementCensus<'a>,
    source: &str,
    scoped: &mut HashSet<(u32, u32)>,
    rel_scoped: &mut [bool],
) -> Result<bool, CompileError> {
    let Some(combinator) = relative.combinator else {
        return Ok(true);
    };
    match combinator {
        Combinator::Descendant | Combinator::Child => {
            let is_adjacent = combinator == Combinator::Child;
            let ancestors = get_ancestor_elements(path, is_adjacent)
                .map_err(|()| unsupported(Refusal::CssCombinatorSelector))?;
            let mut parent_matched = false;
            for ancestor in &ancestors {
                if apply_selector(
                    sel,
                    ancestor.node,
                    &path[..ancestor.path_len],
                    from,
                    to,
                    census,
                    source,
                    scoped,
                    rel_scoped,
                )? {
                    parent_matched = true;
                }
            }
            Ok(parent_matched
                || ((!is_adjacent || ancestors.is_empty()) && every_is_global(sel, from, to)))
        }
        Combinator::NextSibling | Combinator::SubsequentSibling => {
            let adjacent = combinator == Combinator::NextSibling;
            let siblings = get_possible_element_siblings(census, path, adjacent, source)
                .map_err(|()| unsupported(Refusal::CssCombinatorSelector))?;
            let mut sibling_matched = false;
            for sibling in &siblings {
                if apply_selector(
                    sel,
                    sibling.node,
                    sibling.path.unwrap_or(&[]),
                    from,
                    to,
                    census,
                    source,
                    scoped,
                    rel_scoped,
                )? {
                    sibling_matched = true;
                }
            }
            Ok(sibling_matched || (!has_element_parent(path) && every_is_global(sel, from, to)))
        }
        // `||` is refused at parse time.
        Combinator::Column => Err(unsupported(Refusal::CssCombinatorSelector)),
    }
}

/// Whether every relative in `sel.relatives[from..to]` is global — the oracle's
/// `every_is_global` (`css-prune.js:368-373`). A global remainder is satisfied by
/// an out-of-component `:global(...)`, so the leaf matches regardless of ancestors.
fn every_is_global(sel: &ScopedSelector, from: usize, to: usize) -> bool {
    sel.relatives[from..to].iter().all(|r| r.global)
}

/// The leaf test — `relative_selector_might_apply_to_node` restricted to this
/// slice's compounds (`css-prune.js:436-675`).
fn relative_might_apply(
    relative: &ScopedRelative,
    element: CensusNode<'_>,
    source: &str,
) -> Result<bool, CompileError> {
    match relative.kind {
        // A bare `:global` short-circuits to "matches" (`css-prune.js:530`).
        RelKind::BareGlobal => Ok(true),
        // A plain compound, or a `:global(<compound>)`'s inner compound.
        RelKind::Normal | RelKind::PureGlobal => {
            let element_name = element.name_span().extract(source);
            for predicate in &relative.predicates {
                if !predicate_matches(predicate, element, element_name, source)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
    }
}

// ── Leaf predicate matching (the oracle's attribute_matches, unchanged) ────────

fn predicate_matches(
    predicate: &Predicate,
    element: CensusNode<'_>,
    element_name: &str,
    source: &str,
) -> Result<bool, CompileError> {
    match predicate {
        Predicate::Universal => Ok(true),
        Predicate::Type(name) => {
            // A `<svelte:element>`'s runtime tag is unknown, so a type selector
            // matches it for ANY name (`css-prune.js:637-647`, `element.type !==
            // 'SvelteElement'`). No name compare — the literal `svelte:element` is
            // never the runtime tag.
            if element.is_dynamic() {
                return Ok(true);
            }
            if !element_name.is_ascii() {
                return Err(unsupported(Refusal::CssCaseInsensitiveNonAscii));
            }
            Ok(element_name.eq_ignore_ascii_case(name))
        }
        Predicate::Class(name) => attribute_matches(
            element,
            "class",
            Some(name),
            Some(AttributeMatcher::Contains),
            false,
            false,
            source,
        ),
        Predicate::Id(name) => attribute_matches(
            element,
            "id",
            Some(name),
            Some(AttributeMatcher::Exact),
            false,
            false,
            source,
        ),
        Predicate::Attribute {
            name,
            matcher,
            value,
            case_insensitive,
        } => {
            if is_attribute_whitelisted(element_name, name) {
                return Ok(true);
            }
            attribute_matches(
                element,
                name,
                value.as_deref(),
                *matcher,
                *case_insensitive,
                true,
                source,
            )
        }
    }
}

/// The oracle's `whitelist_attribute_selector` (`css-prune.js:20-23`): `[open]` on
/// `<details>`/`<dialog>` matches unconditionally.
fn is_attribute_whitelisted(element_name: &str, attr_name: &str) -> bool {
    let element_lower = element_name.to_ascii_lowercase();
    (element_lower == "details" || element_lower == "dialog")
        && attr_name.eq_ignore_ascii_case("open")
}

/// Port of the oracle's `attribute_matches` (`css-prune.js:713-822`). The
/// `get_possible_values` bounded static-eval is not ported: a single plain
/// expression (`{x}`) is `UNKNOWN` → assume match; anything enumerable refuses.
#[allow(clippy::too_many_arguments)]
fn attribute_matches(
    element: CensusNode<'_>,
    name: &str,
    expected_value: Option<&str>,
    operator: Option<AttributeMatcher>,
    case_insensitive: bool,
    attribute_selector: bool,
    source: &str,
) -> Result<bool, CompileError> {
    let name_lower = name.to_ascii_lowercase();
    for node in element.attributes() {
        match node {
            AttributeNode::SpreadAttribute(_) => return Ok(true),
            AttributeNode::BindDirective(bind) if bind.name_span.extract(source) == name => {
                return Ok(true);
            }
            AttributeNode::StyleDirective(_) if name_lower == "style" => return Ok(true),
            AttributeNode::ClassDirective(directive) if name_lower == "class" => {
                if matches!(operator, Some(AttributeMatcher::Contains)) {
                    if Some(directive.name_span.extract(source)) == expected_value {
                        return Ok(true);
                    }
                } else {
                    return Ok(true);
                }
            }
            AttributeNode::Attribute(attr) => {
                let attr_name = attr.name_span.extract(source);
                if attribute_selector && !attr_name.is_ascii() {
                    return Err(unsupported(Refusal::CssCaseInsensitiveNonAscii));
                }
                if !attr_name.eq_ignore_ascii_case(&name_lower) {
                    continue;
                }
                let Some(values) = attr.value else {
                    return Ok(operator.is_none());
                };
                let Some(expected) = expected_value else {
                    return Ok(true);
                };
                if let [AttributeValue::Text(text)] = values {
                    let data = text.data(source);
                    if case_insensitive && !data.is_ascii() {
                        return Err(unsupported(Refusal::CssCaseInsensitiveNonAscii));
                    }
                    let matches = test_attribute(operator, expected, case_insensitive, &data);
                    if !matches && (name_lower == "class" || name_lower == "style") {
                        continue;
                    }
                    return Ok(matches);
                }
                if let [AttributeValue::ExpressionTag(tag)] = values
                    && is_unknown_expression(&tag.expression)
                {
                    return Ok(true);
                }
                return Err(unsupported(Refusal::CssDynamicAttributeMatch));
            }
            _ => {}
        }
    }
    Ok(false)
}

fn is_unknown_expression(expr: &Expression<'_>) -> bool {
    matches!(
        expr,
        Expression::Identifier(_) | Expression::MemberExpression(_) | Expression::CallExpression(_)
    )
}

fn test_attribute(
    operator: Option<AttributeMatcher>,
    expected: &str,
    case_insensitive: bool,
    value: &str,
) -> bool {
    if case_insensitive {
        test_attribute_cs(
            operator,
            &expected.to_ascii_lowercase(),
            &value.to_ascii_lowercase(),
        )
    } else {
        test_attribute_cs(operator, expected, value)
    }
}

fn test_attribute_cs(operator: Option<AttributeMatcher>, expected: &str, value: &str) -> bool {
    match operator {
        None | Some(AttributeMatcher::Exact) => value == expected,
        Some(AttributeMatcher::Contains) => {
            value.split(is_js_whitespace).any(|token| token == expected)
        }
        Some(AttributeMatcher::DashMatch) => {
            format!("{value}-").starts_with(&format!("{expected}-"))
        }
        Some(AttributeMatcher::Prefix) => value.starts_with(expected),
        Some(AttributeMatcher::Suffix) => value.ends_with(expected),
        Some(AttributeMatcher::Substring) => value.contains(expected),
    }
}

// ── Splicing ──────────────────────────────────────────────────────────────────

/// One source edit applied to the `<style>` content: drop `[at, remove_to)`, insert
/// `insert`.
#[derive(Clone, Copy)]
struct Edit {
    at: u32,
    remove_to: u32,
    insert: &'static str,
}

/// The scoped CSS: the author's style text verbatim with the hash class spliced
/// after each scoped compound's anchor, global wrappers stripped, and specificity
/// modifiers (`.svelte-tsvhash` first, `:where(.svelte-tsvhash)` after) applied
/// per `ComplexSelector`.
pub(crate) fn splice_scoped_css(style: &Style<'_>, source: &str, scope: &CssScoping) -> String {
    let mut edits: Vec<Edit> = Vec::new();
    for (selector, rel_scoped) in scope.info.selectors.iter().zip(&scope.relative_scoped) {
        // Specificity bump is per `ComplexSelector`, left-to-right: the first scoped
        // compound gets a plain class (+0-1-0), every later one a zero-specificity
        // `:where(...)` (index.js:283-372).
        let mut bumped = false;
        for (relative, &was_scoped) in selector.relatives.iter().zip(rel_scoped) {
            for removal in &relative.global_strip {
                edits.push(Edit {
                    at: removal.at,
                    remove_to: removal.remove_to,
                    insert: "",
                });
            }
            if was_scoped && let Some(anchor) = relative.anchor {
                let modifier = if bumped {
                    ":where(.svelte-tsvhash)"
                } else {
                    ".svelte-tsvhash"
                };
                bumped = true;
                edits.push(Edit {
                    at: anchor.at,
                    remove_to: anchor.remove_to,
                    insert: modifier,
                });
            }
        }
    }

    // The `@keyframes` name / `-global-` / `animation`-value edits touch source regions
    // disjoint from every selector splice, so they merge into one sorted edit stream.
    edits.extend_from_slice(&scope.info.keyframes_edits);

    let content_start = style.content_span.start;
    let content = style.content_span.extract(source);
    edits.sort_by_key(|e| e.at);
    let mut out = String::with_capacity(content.len() + 24 * edits.len());
    let mut prev = 0usize;
    for edit in &edits {
        let at = (edit.at - content_start) as usize;
        out.push_str(&content[prev..at]);
        out.push_str(edit.insert);
        prev = (edit.remove_to - content_start) as usize;
    }
    out.push_str(&content[prev..]);
    out
}

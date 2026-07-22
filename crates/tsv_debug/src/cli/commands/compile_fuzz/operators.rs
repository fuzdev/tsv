//! The mutation operators — feature-level splices aimed at the compiler's
//! structural weak point.
//!
//! ## Why these operators
//!
//! The refusal taxonomy splits three ways and only one bucket is worth mutating
//! toward. Deliberate fences (`Refusal::is_deliberate_fence`) are permanent product
//! choices — generating them only re-confirms a refusal. Validation refusals mean
//! the oracle rejects too, so refusing is correct. **Unimplemented** is the live
//! target, and within it one sub-group is structural rather than incidental:
//! `GeneratedNameCollision`, `MemberCallAmbiguousRoot`, `DerivedReadShadowed`,
//! `SnippetHoistAmbiguous`, `BlockScopeShadowsDerived`, `StoreScopedSubscription`.
//! Those exist **because tsv's port is name-based where the oracle is
//! scope-sensitive**, so each is inherently a two-name cross-product: declare a name
//! in scope A, shadow or read it in scope B. That is the porting strategy's weak
//! point, not a list of unrelated gaps — so the operator set is built to compose
//! names and scopes, not to enumerate features.
//!
//! Each operator therefore crosses **two** axes:
//!
//! | operator | axes crossed |
//! | --- | --- |
//! | [`Operator::ShadowRead`] | name × shadowing scope (a template read re-bound by a wrapping `{#each}`) |
//! | [`Operator::ShadowDeclared`] | instance-script name × block scope (`{@const}` re-binding it) |
//! | [`Operator::CollideGeneratedName`] | generated-name space × user-name space |
//! | [`Operator::InjectDropped`] | construct × server-dropped region |
//! | [`Operator::ExportDroppedSnippet`] | dropped `{#snippet}` × module-script `export` |
//! | [`Operator::WrapInBlock`] | existing subtree × new enclosing scope (incl. dropping it) |
//! | [`Operator::SpliceDonor`] | one seed's features × another's (the cross-product engine) |
//! | [`Operator::InjectComment`] | comment × the span-minting rewrite it lands in |
//! | [`Operator::DuplicateSubtree`] | generated-name ordering × visit-vs-emission order |
//! | [`Operator::AddDirective`] | spread × co-present directives on one element |
//! | [`Operator::ExoticWhitespace`] | one code point × the four languages that disagree about it |
//!
//! ## Why splices and not byte edits
//!
//! A mutant must stay **oracle-compilable** to grade anything, so every operator
//! inserts a whole well-formed construct at an offset [`Anchors`] proved is
//! structurally valid. The mutant is re-anchored (re-parsed) between operators, so
//! operator *N+1* always sees the real post-*N* document rather than stale offsets.
//!
//! ## Where [`Operator::ExoticWhitespace`] sits against that rule
//!
//! It inserts a single code point, which *looks* like the byte-level mutation the
//! rule forbids. It is not, and the distinction is the rule's own reason rather
//! than its letter: the rule exists so a mutant stays **oracle-compilable**, and
//! the enumerated construct is one way — not the only way — to get there. This
//! operator gets there by a stronger route. Its positions are read off tsv's own
//! parse exactly as every other operator's are, and at each of them the insertion
//! is well-formedness-preserving **by construction**:
//!
//! - inside a script, into an existing whitespace RUN. Every context that admits
//!   the run's whitespace character admits one more of the same class — if the run
//!   is trivia the insert is trivia, and if it is string / template / comment /
//!   regex interior the insert is content. So the scan does not need to know which
//!   it is, with **one** guarded exception: a string literal's `LineContinuation`,
//!   where the run's leading line terminator is bound to the `\` before it and
//!   splitting the pair strands a raw line terminator in the literal. That is a
//!   property of the *position*, not of the character, so the run's start edge is
//!   simply not an anchor there (see [`Anchors::script_ws`]);
//! - inside a QUOTED attribute value or in template text, where the extent is
//!   delimiter- or tag-defined, so any code point is content;
//! - after a CSS ident, where a non-ASCII code point either continues the ident or
//!   makes the oracle's CSS parser reject — and an oracle rejection grades the
//!   refusal contract perfectly well.
//!
//! **Why it is worth an operator.** Three live compiler defects and one panic had
//! exactly one shape: *a scan whose whitespace notion was the HOST language's
//! rather than the TARGET language's*. Rust's `char::is_whitespace` is Unicode
//! `White_Space`, which agrees with neither ECMAScript `WhiteSpace` (which adds
//! `U+FEFF` and drops `U+0085`) nor CSS `white-space` (strictly ASCII, with
//! everything at or above `U+0080` an ident code point). Every one of the four was
//! invisible to the whole gate suite and to a ~2996-file corpus, and each was found
//! by hand. `tsv_svelte_compile::text_class` now states the classes once; this
//! operator is the mechanized search for the scans that still do not read it.
//!
//! **The character sets are position-specific**, because an illegal code point
//! merely burns a round trip in the harness buckets:
//!
//! - a **script token boundary** admits only ECMAScript `WhiteSpace`. `U+0085`
//!   (`<NEL>`) and `U+180E` are *not* ECMAScript whitespace — a script-position
//!   `U+0085` is a parse error — so they appear only in the content families,
//!   where they are the sharpest probes precisely because Rust's `trim` strips
//!   `U+0085` and JS's does not;
//! - `U+2028` / `U+2029` are `LineTerminator`s. They are excluded from the script
//!   family outright rather than weighted down, for two reasons that are genuinely
//!   specific to them: inserting one can flip **ASI** (a semantic change both sides
//!   see identically, so it grades fairly but attributes a finding to the wrong
//!   cause), and one is illegal raw inside a regex body. They stay in the content
//!   families, where they are inert.
//!
//!   ⚠️ A third reason used to be listed here — "illegal raw after a string's
//!   line-continuation backslash" — and it was **misscoped**. That hazard is
//!   character-INDEPENDENT: *every* code point in [`JS_WHITESPACE_CHARS`] breaks a
//!   `\<LF>` pair identically, by turning the `\` into a `NonEscapeCharacter`
//!   escape and stranding the `<LF>`. Excluding two characters could never have
//!   addressed it; the anchor scan skips the position instead
//!   ([`Anchors::script_ws`]). The exclusion decision was right, its stated
//!   reasoning was one-third wrong, and that misscoping is precisely what left the
//!   general case unhandled;
//! - **CSS** wants code points at or above `U+0080` — there they are *ident*
//!   characters, not whitespace, which is the whole of the third bug.
//!
//! The set is aligned with the formatter fuzzer's `INTERESTING_SEQUENCES`
//! (`super::super::fuzz`) — same NBSP / zero-width / BOM / CJK stress family —
//! and extended with the code points on which the three whitespace classes
//! actually disagree, which is what makes it a differential probe rather than a
//! width-math one.

use super::super::fuzz::Rng;
use super::anchors::Anchors;

/// A donor seed: the material [`Operator::SpliceDonor`] grafts across components.
pub struct Donor {
    /// The donor's template text (everything outside its `<script>` / `<style>`).
    pub template: String,
    /// The donor's instance-script body.
    pub script: String,
    /// The names that script binds — the collision guard.
    pub declared: Vec<String>,
    /// The donor's instance script is `lang="ts"`.
    pub is_ts: bool,
}

impl Donor {
    /// Split a component into graftable material: its template (everything outside
    /// `<script>` / `<style>`) and its instance-script body.
    ///
    /// Returns `None` when tsv's parser rejects the component — the same gate the
    /// seed list uses, so a donor is always a real, parseable component.
    pub fn from_source(source: &str) -> Option<Self> {
        let arena = bumpalo::Bump::new();
        let root = tsv_svelte::parse(source, &arena).ok()?;
        // The three top-level regions to cut out of the template. `<svelte:options>`
        // stays in: it is a template-level construct and carries no bindings.
        let mut cuts: Vec<(usize, usize)> = Vec::new();
        for span in [
            root.instance.map(|s| s.span),
            root.module.map(|s| s.span),
            root.css.map(|s| s.span),
        ]
        .into_iter()
        .flatten()
        {
            cuts.push((span.start as usize, span.end as usize));
        }
        cuts.sort_unstable();
        let mut template = String::with_capacity(source.len());
        let mut at = 0usize;
        for (start, end) in cuts {
            if start >= at {
                template.push_str(&source[at..start]);
                at = end;
            }
        }
        template.push_str(&source[at..]);

        let script = root
            .instance
            .map(|s| s.content.span.extract(source).to_string())
            .unwrap_or_default();
        let is_ts = root
            .instance
            .is_some_and(|s| super::anchors::script_is_ts(s.span, source));
        let declared = super::anchors::declared_names(&script);
        Some(Self {
            template: template.trim().to_string(),
            script,
            declared,
            is_ts,
        })
    }
}

/// The mutation operators. The generator draws **uniformly** over [`Operator::ALL`];
/// an operator the current document has no anchor for returns `None`, which SPENDS that
/// mutation turn without changing the document (the next turn draws afresh). So the
/// effective mix is the corpus's anchor availability rather than any weighting here,
/// and a mutant's realized operator count is at most `--max-mutations`. The run's
/// report prints the per-operator counts that actually landed — the honest read of
/// what got exercised.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Operator {
    ShadowRead,
    ShadowDeclared,
    CollideGeneratedName,
    InjectDropped,
    ExportDroppedSnippet,
    WrapInBlock,
    SpliceDonor,
    InjectComment,
    DuplicateSubtree,
    AddDirective,
    ExoticWhitespace,
}

impl Operator {
    pub const ALL: &'static [Self] = &[
        Self::ShadowRead,
        Self::ShadowDeclared,
        Self::CollideGeneratedName,
        Self::InjectDropped,
        Self::ExportDroppedSnippet,
        Self::WrapInBlock,
        Self::SpliceDonor,
        Self::InjectComment,
        Self::DuplicateSubtree,
        Self::AddDirective,
        Self::ExoticWhitespace,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::ShadowRead => "shadow_read",
            Self::ShadowDeclared => "shadow_declared",
            Self::CollideGeneratedName => "collide_generated_name",
            Self::InjectDropped => "inject_dropped",
            Self::ExportDroppedSnippet => "export_dropped_snippet",
            Self::WrapInBlock => "wrap_in_block",
            Self::SpliceDonor => "splice_donor",
            Self::InjectComment => "inject_comment",
            Self::DuplicateSubtree => "duplicate_subtree",
            Self::AddDirective => "add_directive",
            Self::ExoticWhitespace => "exotic_whitespace",
        }
    }
}

/// Names the oracle's server generator mints for itself. Declaring one in user
/// scope is the `GeneratedNameCollision` cross-product.
const GENERATED_NAMES: &[&str] = &[
    "$$payload",
    "$$props",
    "$$slots",
    "$$renderer",
    "$$sanitized_props",
    "$$anchor",
    "$$settled",
    "$0",
    "$1",
];

/// Constructs injected into a server-DROPPED region (a `{:catch}` body, a
/// `<svelte:boundary>` `pending`/`failed` snippet). Each is legal in an ordinary
/// fragment position and self-contained — no free identifiers, so a mutant fails
/// for a structural reason rather than an undefined name.
///
/// `{$$slots.x}` and the `{#snippet}` / `{@render}` pair are here deliberately:
/// they are the ingredients of the two over-acceptances documented as
/// corpus-UNREACHED (`slot_snippet_conflict`, `snippet_invalid_export`).
const DROPPED_PAYLOADS: &[&str] = &[
    "{$$slots.fz}",
    "{$$props.fz}",
    "{#snippet fz_snip()}<i>d</i>{/snippet}",
    "{@render fz_snip()}",
    "<svelte:head><title>fz</title></svelte:head>",
    "<p class:fz_a={true}>d</p>",
    "<p {...{ fz: 1 }}>d</p>",
    "{#each [1] as fz_i}<i>{fz_i}</i>{/each}",
    "{#await Promise.resolve(1)}<i>p</i>{:catch fz_e}<u>{fz_e}</u>{/await}",
    "{@html '<i>x</i>'}",
    "<!-- fz -->",
];

/// Attribute / directive fragments added to an existing element — the spread ×
/// co-present-directive axis (each is valid beside any of the others).
const DIRECTIVES: &[&str] = &[
    " class:fz_a={true}",
    " style:color=\"red\"",
    " {...{ fz: 1 }}",
    " data-fz=\"1\"",
    " class=\"fz\"",
    " style=\"color: red\"",
    " onclick={() => {}}",
];

/// A **12-of-21 SUBSET** of ECMAScript `WhiteSpace` minus the `LineTerminator`s —
/// the only class legal at a JS token boundary that also leaves ASI alone
/// (ECMA-262 §12.2, table 34).
///
/// A subset, not the set, and deliberately: the full class holds all 11 code points
/// of the `Zs` run `U+2000`..=`U+200A`, which are indistinguishable to every scan
/// this operator probes (all `Zs`, all non-ASCII, all ECMAScript whitespace, none
/// Unicode-`White_Space`-divergent). Carrying the 9 interior ones would add no
/// discriminating power and would dilute each draw toward that one redundant
/// family, so the run is represented by its two endpoints `U+2000` and `U+200A`.
/// The omission is exactly `U+2001`..=`U+2009` and nothing else, which the unit
/// test below pins in both directions.
///
/// `\u{0009}` / `\u{0020}` are here for a reason beyond completeness: an ordinary
/// tab or space is the CONTROL. A finding that reproduces with one of these is a
/// plain whitespace-handling bug, not a character-class bug, and reading the two
/// apart is the first triage step.
///
/// ⚠️ `U+0085` (`<NEL>`) and `U+180E` are deliberately ABSENT: `U+0085` carries the
/// Unicode `White_Space` property but is not ECMAScript `WhiteSpace`, and `U+180E`
/// is neither, so a script-position insert of either is a parse error rather than
/// a probe. They live in [`CONTENT_CHARS`], where that mismatch is the point.
const JS_WHITESPACE_CHARS: &[&str] = &[
    "\u{0009}", // <TAB> — the ASCII control
    "\u{0020}", // <SP> — the ASCII control
    "\u{000B}", // <VT>, which `u8::is_ascii_whitespace` does not count
    "\u{000C}", // <FF>
    "\u{00A0}", // <NBSP>
    "\u{1680}", "\u{2000}", "\u{200A}", "\u{202F}", "\u{205F}", "\u{3000}",
    "\u{FEFF}", // <ZWNBSP> — ECMAScript whitespace, NOT Unicode `White_Space`
];

/// Code points for the CONTENT positions (attribute values, template text), where
/// every code point is legal and the question is only which scans mis-classify it.
///
/// This is the wider set on purpose. It carries the two ECMAScript-illegal
/// separators [`JS_WHITESPACE_CHARS`] must exclude — `U+0085`, which Rust's `trim`
/// strips and JavaScript's `.trim()` keeps, and `U+180E`, which neither counts but
/// many hand-rolled scans do — plus the two `LineTerminator`s, inert here, plus the
/// zero-width and wide characters the formatter fuzzer's `INTERESTING_SEQUENCES`
/// stresses span math with.
const CONTENT_CHARS: &[&str] = &[
    "\u{00A0}", "\u{0085}", // <NEL>: Unicode `White_Space`, but NOT ECMAScript whitespace
    "\u{180E}", // neither class counts it; hand-rolled scans often do
    "\u{2028}", // <LS>
    "\u{2029}", // <PS>
    "\u{202F}", "\u{3000}", "\u{200B}", // zero-width space
    "\u{FEFF}", "\u{4E2D}", // a wide CJK character — width math, not class
];

/// Code points appended to a CSS ident. Every one of these is at or above
/// `U+0080`, which is where CSS and the host language disagree hardest: a scan
/// that trims one of these off a selector name silently RENAMES it, which is how
/// `:global\u{00A0}` came to scope an element the oracle prunes.
const CSS_IDENT_CHARS: &[&str] = &[
    "\u{00A0}", "\u{0085}", "\u{180E}", "\u{2000}", "\u{202F}", "\u{205F}", "\u{3000}", "\u{FEFF}",
];

/// Comment payloads, one per ownership path (matching the gap audit's reasoning:
/// a line comment is never owned, a glued block comment always is, a JSDoc cast
/// binds its parenthesized operand, and an HTML comment is an AST node).
const COMMENTS: &[(&str, CommentKind)] = &[
    ("/* fz */", CommentKind::Code),
    ("// fz\n", CommentKind::CodeLine),
    ("/** @type {any} */ ", CommentKind::Code),
    ("<!-- fz -->", CommentKind::Template),
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum CommentKind {
    /// Legal anywhere a JS token is legal.
    Code,
    /// Needs a following newline, so only a statement gap will do.
    CodeLine,
    /// Only legal in a template fragment position.
    Template,
}

/// Apply `op` to `source`, returning the mutated text — or `None` when the seed
/// carries no anchor the operator needs (the caller simply draws again).
pub fn apply(
    op: Operator,
    source: &str,
    anchors: &Anchors,
    donors: &[Donor],
    rng: &mut Rng,
) -> Option<String> {
    match op {
        Operator::ShadowRead => shadow_read(source, anchors, rng),
        Operator::ShadowDeclared => shadow_declared(source, anchors, rng),
        Operator::CollideGeneratedName => collide_generated_name(source, anchors, rng),
        Operator::InjectDropped => inject_dropped(source, anchors, rng),
        Operator::ExportDroppedSnippet => export_dropped_snippet(source, anchors, rng),
        Operator::WrapInBlock => wrap_in_block(source, anchors, rng),
        Operator::SpliceDonor => splice_donor(source, anchors, donors, rng),
        Operator::InjectComment => inject_comment(source, anchors, rng),
        Operator::DuplicateSubtree => duplicate_subtree(source, anchors, rng),
        Operator::AddDirective => add_directive(source, anchors, rng),
        Operator::ExoticWhitespace => exotic_whitespace(source, anchors, rng),
    }
}

/// Splice `insert` into `source` at byte `at`.
fn splice(source: &str, at: u32, insert: &str) -> String {
    let at = at as usize;
    let mut out = String::with_capacity(source.len() + insert.len());
    out.push_str(&source[..at]);
    out.push_str(insert);
    out.push_str(&source[at..]);
    out
}

/// Pick one element of `items`, or `None` when empty.
fn pick<'a, T>(rng: &mut Rng, items: &'a [T]) -> Option<&'a T> {
    if items.is_empty() {
        None
    } else {
        Some(&items[rng.below(items.len())])
    }
}

/// Wrap a template subtree in `{#each [0] as n}…{/each}` where `n` is a name the
/// template already reads — so the read inside the wrap now resolves to the each
/// binding, not the outer declaration. The `DerivedReadShadowed` /
/// `BlockScopeShadowsDerived` cross-product, built rather than hoped for.
fn shadow_read(source: &str, anchors: &Anchors, rng: &mut Rng) -> Option<String> {
    let name = pick(rng, &anchors.template_reads)?;
    let &(start, end) = pick(rng, &anchors.wrappable)?;
    let inner = &source[start as usize..end as usize];
    let wrapped = format!("{{#each [0] as {name}}}{inner}{{/each}}");
    Some(replace_range(source, start, end, &wrapped))
}

/// Re-bind an instance-script name inside a block body with `{@const}`. Anchored on
/// a block *binding's* body gap, which is a block body by construction — `{@const}`
/// is illegal directly inside an element, so an element-interior gap will not do.
fn shadow_declared(source: &str, anchors: &Anchors, rng: &mut Rng) -> Option<String> {
    let name = pick(rng, &anchors.declared)?.clone();
    let binding = pick(rng, &anchors.block_bindings)?;
    // Re-binding the block's OWN binding name is a duplicate declaration in one
    // scope — a validation rejection the oracle and tsv both reach trivially, so it
    // would spend a round trip on nothing. The interesting cross is an OUTER name
    // re-bound by an inner scope.
    if binding.name == name {
        return None;
    }
    const_tag_at(source, binding.body_gap, &name)
}

/// Insert `{@const name = 1}` at `at`, unless a `{@const name` is already sitting
/// there — a second one in the same block body is a duplicate binding, which the
/// oracle rejects outright, so the mutant grades nothing and merely spends a round
/// trip. The guard is LOCAL (it looks only at the insertion point), which is enough
/// because that is exactly where a repeated application of these operators stacks
/// them.
fn const_tag_at(source: &str, at: u32, name: &str) -> Option<String> {
    if source[at as usize..].starts_with(&format!("{{@const {name} ")) {
        return None;
    }
    Some(splice(source, at, &format!("{{@const {name} = 1}}")))
}

/// Declare a name the generator mints for itself — in the instance script, or as a
/// block-scoped `{@const}` (the two sides of the collision differ: one is a module
/// binding the generator must not shadow, the other a block binding it must not
/// capture).
fn collide_generated_name(source: &str, anchors: &Anchors, rng: &mut Rng) -> Option<String> {
    let name = pick(rng, GENERATED_NAMES)?;
    if rng.below(2) == 0
        && let Some(binding) = pick(rng, &anchors.block_bindings)
    {
        return const_tag_at(source, binding.body_gap, name);
    }
    let slot = anchors.instance.as_ref()?;
    // A second `let $$slots = 1;` in one scope is a DUPLICATE BINDING — invalid JS,
    // so the oracle's parser rejects the mutant before it can answer the
    // generated-name question this operator exists to ask.
    if anchors.declared.iter().any(|n| n == name) {
        return None;
    }
    Some(splice(
        source,
        slot.insert_at,
        &format!("let {name} = 1;\n"),
    ))
}

/// Put a construct inside a region the server target never emits.
fn inject_dropped(source: &str, anchors: &Anchors, rng: &mut Rng) -> Option<String> {
    let &at = pick(rng, &anchors.dropped_gaps)?;
    let payload = pick(rng, DROPPED_PAYLOADS)?;
    Some(splice(source, at, payload))
}

/// Export a name a **dropped** `{#snippet}` also defines, from the module script —
/// the `snippet_invalid_export` shape. Falls back to any snippet when none is
/// dropped, and mints a module script when the component has none.
fn export_dropped_snippet(source: &str, anchors: &Anchors, rng: &mut Rng) -> Option<String> {
    let dropped: Vec<&(String, bool)> = anchors.snippets.iter().filter(|(_, d)| *d).collect();
    let name = if dropped.is_empty() {
        pick(rng, &anchors.snippets)?.0.clone()
    } else {
        pick(rng, &dropped)?.0.clone()
    };
    // A module script that already BINDS or already EXPORTS this name would make the
    // insert a duplicate declaration / duplicate export — invalid JS either way, so
    // the oracle stops at its parser and never reaches the snippet-export rule this
    // operator exists to probe. Both questions are asked: `export { s }` exports
    // without declaring, so a declaration-only guard still emitted it twice.
    if anchors.module_declared.contains(&name) || anchors.module_exported.contains(&name) {
        return None;
    }
    // Two export forms, because they reach different oracle rules: a fresh
    // `export const` merely shadows the snippet name, while a bare specifier
    // export names the SNIPPET itself — the form the oracle answers with
    // `snippet_invalid_export`. Emitting only the first never reaches that rule
    // (verified live: `export const p = 1` compiles, `export {p}` does not).
    let statement = if rng.below(2) == 0 {
        format!("export const {name} = 1;\n")
    } else {
        format!("export {{ {name} }};\n")
    };
    match &anchors.module {
        Some(slot) => Some(splice(source, slot.insert_at, &statement)),
        None => Some(format!("<script module>\n\t{statement}</script>\n{source}")),
    }
}

/// Wrap a subtree in a new enclosing scope. Two of the five wraps move the subtree
/// into a server-DROPPED region, which is how an ordinary already-composed fixture
/// becomes a dropped-region cross-product.
fn wrap_in_block(source: &str, anchors: &Anchors, rng: &mut Rng) -> Option<String> {
    let &(start, end) = pick(rng, &anchors.wrappable)?;
    let inner = &source[start as usize..end as usize];
    let test = anchors
        .template_reads
        .first()
        .cloned()
        .unwrap_or_else(|| "true".to_string());
    let wrapped = match rng.below(5) {
        0 => format!("{{#if {test}}}{inner}{{/if}}"),
        1 => format!("{{#key {test}}}{inner}{{/key}}"),
        // Dropped: a `{:catch}` body is never emitted on the server.
        2 => format!("{{#await Promise.resolve(1)}}<i>p</i>{{:catch fz_e}}{inner}{{/await}}"),
        // Dropped: a boundary's `pending` snippet is never emitted on the server.
        3 => format!(
            "<svelte:boundary>{{#snippet pending()}}{inner}{{/snippet}}<i>b</i></svelte:boundary>"
        ),
        _ => format!("<svelte:boundary>{inner}</svelte:boundary>"),
    };
    Some(replace_range(source, start, end, &wrapped))
}

/// Graft another seed's template **and** its instance-script body into this one —
/// the cross-product engine, and the reason the seed corpus is
/// `tests/fixtures_compile` (420 fixtures, many already 2–3-way feature crosses by
/// name). Mutating *within* an already-composed seed reaches interaction bugs that
/// layering onto a single-feature seed does not.
///
/// Guarded on two counts so the graft usually still compiles rather than dying at
/// the oracle on a trivial error: a donor whose declared names collide with the
/// host's is skipped (a duplicate declaration is a validation rejection, not an
/// interesting one), and a TypeScript donor is only grafted into a TypeScript host.
fn splice_donor(
    source: &str,
    anchors: &Anchors,
    donors: &[Donor],
    rng: &mut Rng,
) -> Option<String> {
    let &at = pick(rng, &anchors.template_gaps)?;
    let donor = pick(rng, donors)?;
    if donor.template.trim().is_empty() {
        return None;
    }
    let host_is_ts = anchors.instance.as_ref().is_some_and(|s| s.is_ts);
    if donor.is_ts && !host_is_ts {
        return None;
    }
    // A graft that re-declares one of the host's bindings is invalid JS, and a mutant
    // the ORACLE cannot parse grades nothing. Both of the host's script scopes are
    // asked, plus its template reads — `declared_names` over-claims inside a
    // destructuring pattern precisely so this guard does not under-claim.
    if donor.declared.iter().any(|n| {
        anchors.declared.contains(n)
            || anchors.module_declared.contains(n)
            || anchors.template_reads.contains(n)
    }) {
        return None;
    }
    let with_template = splice(source, at, &donor.template);
    if donor.script.trim().is_empty() {
        return Some(with_template);
    }
    // Re-anchor: the template splice moved every offset after `at`.
    let reanchored = Anchors::collect(&with_template)?;
    match &reanchored.instance {
        Some(slot) => Some(splice(
            &with_template,
            slot.insert_at,
            &format!("{}\n", donor.script.trim()),
        )),
        None => {
            let lang = if donor.is_ts { " lang=\"ts\"" } else { "" };
            Some(format!(
                "<script{lang}>\n{}\n</script>\n{with_template}",
                donor.script.trim()
            ))
        }
    }
}

/// Inject a comment where a rewrite may mint a synthetic span around it. Comments
/// are the arc's recurring hazard class (a comment can be dropped, double-printed,
/// or re-bound by a rewrite that gives the surrounding node a fresh span), and the
/// payloads cover the distinct ownership paths.
fn inject_comment(source: &str, anchors: &Anchors, rng: &mut Rng) -> Option<String> {
    let &(text, kind) = pick(rng, COMMENTS)?;
    let at = match kind {
        CommentKind::Template => *pick(rng, &anchors.template_gaps)?,
        CommentKind::CodeLine => {
            let slot = anchors.instance.as_ref().or(anchors.module.as_ref())?;
            *pick(rng, &slot.stmt_gaps)?
        }
        CommentKind::Code => {
            // Either a statement gap or an expression-tag brace interior — the
            // latter is where an expression rewrite is most likely to re-span.
            let interiors = &anchors.expr_interiors;
            if !interiors.is_empty() && rng.below(2) == 0 {
                *pick(rng, interiors)?
            } else {
                let slot = anchors.instance.as_ref().or(anchors.module.as_ref())?;
                *pick(rng, &slot.stmt_gaps)?
            }
        }
    };
    Some(splice(source, at, text))
}

/// Duplicate a subtree elsewhere in the template — the ordering axis. A duplicated
/// `{#snippet}` collides by name; a duplicated element re-runs the same
/// generated-name allocation in a different visit order.
fn duplicate_subtree(source: &str, anchors: &Anchors, rng: &mut Rng) -> Option<String> {
    let &(start, end) = pick(rng, &anchors.wrappable)?;
    let subtree = source[start as usize..end as usize].to_string();
    let &at = pick(rng, &anchors.template_gaps)?;
    Some(splice(source, at, &subtree))
}

/// Add an attribute or directive to an existing element — the spread ×
/// co-present-directive axis, which the fused `$.attributes(…)` path makes a real
/// three-way interaction (object, `classes`, `styles`).
fn add_directive(source: &str, anchors: &Anchors, rng: &mut Rng) -> Option<String> {
    let &at = pick(rng, &anchors.attr_slots)?;
    let directive = pick(rng, DIRECTIVES)?;
    Some(splice(source, at, directive))
}

/// Insert one exotic code point at a position whose language decides which code
/// points are legal there. The whitespace-class cross-product: one character ×
/// the four languages (JS, CSS, HTML attribute, Svelte template text) that
/// disagree about whether it is whitespace, an ident character, or content.
///
/// Draws a POSITION FAMILY first and the character from that family's set, rather
/// than the reverse — a character drawn first would have to be rejected at most
/// positions, which biases the mix toward whichever family happens to accept the
/// widest set. A family the document has no anchor for yields `None`, spending the
/// turn exactly as every other operator does.
fn exotic_whitespace(source: &str, anchors: &Anchors, rng: &mut Rng) -> Option<String> {
    let (at, text) = match rng.below(4) {
        // A JS token boundary — the static-block fence's home, and the type
        // annotation's (`let x:\u{A0}T`, where the erased region begins).
        0 => (
            *pick(rng, &anchors.script_ws)?,
            *pick(rng, JS_WHITESPACE_CHARS)?,
        ),
        // A quoted attribute value's edge — where the oracle's NARROW ASCII
        // collapse and its WIDE JS trim disagree, which was seven live mismatches.
        1 => (
            *pick(rng, &anchors.attr_value_edges)?,
            *pick(rng, CONTENT_CHARS)?,
        ),
        // A CSS selector name's tail.
        2 => (
            *pick(rng, &anchors.css_ident_ends)?,
            *pick(rng, CSS_IDENT_CHARS)?,
        ),
        // Template text at a node boundary — Svelte's own whitespace collapse,
        // which is ASCII-only (`Text::is_ascii_ws_only`).
        _ => (
            *pick(rng, &anchors.template_gaps)?,
            *pick(rng, CONTENT_CHARS)?,
        ),
    };
    Some(splice(source, at, text))
}

/// Replace `source[start..end]` with `text`.
fn replace_range(source: &str, start: u32, end: u32, text: &str) -> String {
    let (start, end) = (start as usize, end as usize);
    let mut out = String::with_capacity(source.len() + text.len());
    out.push_str(&source[..start]);
    out.push_str(text);
    out.push_str(&source[end..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEED: &str = "<script>\n\tlet count = 0;\n</script>\n\n<p>{count}</p>\n";

    fn anchors_of(source: &str) -> Anchors {
        Anchors::collect(source).expect("seed parses")
    }

    #[test]
    fn shadow_read_wraps_a_subtree_and_rebinds_the_name() {
        let mut rng = Rng::new(7);
        let out = shadow_read(SEED, &anchors_of(SEED), &mut rng).expect("has anchors");
        assert!(out.contains("{#each [0] as count}"), "{out}");
        assert!(Anchors::collect(&out).is_some(), "mutant reparses: {out}");
    }

    #[test]
    fn collide_generated_name_declares_a_generator_name() {
        let mut rng = Rng::new(1);
        let out = collide_generated_name(SEED, &anchors_of(SEED), &mut rng).expect("has anchors");
        assert!(
            GENERATED_NAMES
                .iter()
                .any(|n| out.contains(&format!("let {n} = 1;"))),
            "{out}"
        );
    }

    #[test]
    fn inject_dropped_needs_a_dropped_region() {
        let mut rng = Rng::new(3);
        assert!(inject_dropped(SEED, &anchors_of(SEED), &mut rng).is_none());
        let with_catch = "{#await p}<i>w</i>{:catch e}<u>{e}</u>{/await}\n";
        let out = inject_dropped(with_catch, &anchors_of(with_catch), &mut rng).expect("dropped");
        assert!(out.len() > with_catch.len());
        assert!(Anchors::collect(&out).is_some(), "mutant reparses: {out}");
    }

    #[test]
    fn export_dropped_snippet_mints_a_module_script_when_absent() {
        let source = "<svelte:boundary>{#snippet pending()}<i>x</i>{/snippet}<p>y</p>\
             </svelte:boundary>\n";
        let mut rng = Rng::new(5);
        let out = export_dropped_snippet(source, &anchors_of(source), &mut rng).expect("snippet");
        assert!(out.contains("<script module>"), "{out}");
        assert!(out.contains("export const pending = 1;"), "{out}");
    }

    #[test]
    fn wrap_in_block_reaches_the_dropped_wraps() {
        let anchors = anchors_of(SEED);
        let mut seen_catch = false;
        let mut seen_pending = false;
        for seed in 0..40u64 {
            let mut rng = Rng::new(seed);
            let Some(out) = wrap_in_block(SEED, &anchors, &mut rng) else {
                continue;
            };
            seen_catch |= out.contains("{:catch fz_e}");
            seen_pending |= out.contains("{#snippet pending()}");
            assert!(Anchors::collect(&out).is_some(), "mutant reparses: {out}");
        }
        assert!(seen_catch && seen_pending, "both dropped wraps reachable");
    }

    #[test]
    fn splice_donor_skips_a_colliding_donor() {
        let donors = vec![Donor {
            template: "<b>d</b>".to_string(),
            script: "let count = 1;".to_string(),
            declared: vec!["count".to_string()],
            is_ts: false,
        }];
        let mut rng = Rng::new(2);
        assert!(splice_donor(SEED, &anchors_of(SEED), &donors, &mut rng).is_none());
    }

    #[test]
    fn splice_donor_grafts_template_and_script() {
        let donors = vec![Donor {
            template: "<b>{other}</b>".to_string(),
            script: "let other = 2;".to_string(),
            declared: vec!["other".to_string()],
            is_ts: false,
        }];
        let mut rng = Rng::new(2);
        let out = splice_donor(SEED, &anchors_of(SEED), &donors, &mut rng).expect("grafts");
        assert!(out.contains("<b>{other}</b>"), "{out}");
        assert!(out.contains("let other = 2;"), "{out}");
        assert!(Anchors::collect(&out).is_some(), "mutant reparses: {out}");
    }

    #[test]
    fn add_directive_lands_after_the_tag_name() {
        let mut rng = Rng::new(11);
        let out = add_directive(SEED, &anchors_of(SEED), &mut rng).expect("has an element");
        assert!(out.contains("<p "), "{out}");
        assert!(Anchors::collect(&out).is_some(), "mutant reparses: {out}");
    }

    /// The whole design rests on every script-position character being ECMAScript
    /// `WhiteSpace`: one that is not is a parse error, so it grades nothing and
    /// merely burns an oracle round trip. `U+0085` and `U+180E` are the two that
    /// look like they belong and do not.
    #[test]
    fn script_chars_are_all_ecmascript_whitespace() {
        for text in JS_WHITESPACE_CHARS {
            let c = text.chars().next().expect("one char");
            assert_eq!(text.chars().count(), 1, "{c:?} must be a single code point");
            // `eval`-free restatement of ECMA-262 §12.2 table 34, minus the
            // LineTerminators (deliberately excluded — see the module docs).
            let is_js_ws = matches!(
                c,
                '\u{0009}'
                    | '\u{000B}'
                    | '\u{000C}'
                    | '\u{0020}'
                    | '\u{00A0}'
                    | '\u{1680}'
                    | '\u{2000}'
                    ..='\u{200A}' | '\u{202F}' | '\u{205F}' | '\u{3000}' | '\u{FEFF}'
            );
            assert!(is_js_ws, "{c:?} is not ECMAScript WhiteSpace");
        }
        for absent in ['\u{0085}', '\u{180E}', '\u{2028}', '\u{2029}'] {
            let s = absent.to_string();
            assert!(
                !JS_WHITESPACE_CHARS.contains(&s.as_str()),
                "{absent:?} must not reach a script token boundary"
            );
        }
    }

    /// The doc claims a 12-of-21 subset whose only omission is the `Zs` interior
    /// `U+2001`..=`U+2009`. A ⊆ check alone would let the set silently shrink and
    /// leave the doc asserting more than the code does, so pin both directions.
    #[test]
    fn script_chars_are_the_documented_subset() {
        const FULL: &[char] = &[
            '\u{0009}', '\u{000B}', '\u{000C}', '\u{0020}', '\u{00A0}', '\u{1680}', '\u{2000}',
            '\u{2001}', '\u{2002}', '\u{2003}', '\u{2004}', '\u{2005}', '\u{2006}', '\u{2007}',
            '\u{2008}', '\u{2009}', '\u{200A}', '\u{202F}', '\u{205F}', '\u{3000}', '\u{FEFF}',
        ];
        assert_eq!(
            FULL.len(),
            21,
            "ECMAScript WhiteSpace minus LineTerminators"
        );
        assert_eq!(JS_WHITESPACE_CHARS.len(), 12, "the documented subset size");

        let present: Vec<char> = JS_WHITESPACE_CHARS
            .iter()
            .map(|s| s.chars().next().expect("one char"))
            .collect();
        let omitted: Vec<char> = FULL
            .iter()
            .copied()
            .filter(|c| !present.contains(c))
            .collect();
        let expected: Vec<char> = ('\u{2001}'..='\u{2009}').collect();
        assert_eq!(omitted, expected, "the omission is exactly the Zs interior");
    }

    #[test]
    fn exotic_whitespace_reaches_every_position_family() {
        const SOURCE: &str = "<script>\n\tlet count = 0;\n</script>\n\n\
             <p class=\"box\">{count}</p>\n\n<style>\n\t:global(.box) {\n\t\tcolor: red;\n\t}\n</style>\n";
        let anchors = anchors_of(SOURCE);
        assert!(!anchors.script_ws.is_empty(), "script whitespace runs");
        assert!(!anchors.attr_value_edges.is_empty(), "attribute edges");
        assert!(!anchors.css_ident_ends.is_empty(), "css ident ends");

        // Every draw must still be a document tsv's parser accepts — the
        // well-formedness-by-construction claim, exercised rather than asserted.
        let mut seen = std::collections::BTreeSet::new();
        for seed in 0..200u64 {
            let mut rng = Rng::new(seed);
            let Some(out) = exotic_whitespace(SOURCE, &anchors, &mut rng) else {
                continue;
            };
            assert_ne!(out, SOURCE, "an insert always changes the document");
            assert!(Anchors::collect(&out).is_some(), "mutant reparses: {out:?}");
            for c in out.chars().filter(|c| !c.is_ascii()) {
                seen.insert(c);
            }
        }
        assert!(
            seen.len() >= 4,
            "several distinct code points reached: {seen:?}"
        );
    }

    /// A string literal's `LineContinuation` is the one script position where the
    /// by-construction argument fails: `"a\<LF>b"` is legal only while the `\` and
    /// the `<LF>` stay adjacent, so inserting ANY whitespace at the run's start
    /// re-reads the `\` as a `NonEscapeCharacter` escape and strands a raw
    /// `<LF>` in the literal (ECMA-262 §12.9.4.1) — an oracle `js_parse_error`,
    /// which tsv's permissive frontend then compiles into a false OVER-ACCEPTANCE.
    /// The start edge must not be anchored; the end edge must still be.
    #[test]
    fn line_continuation_start_edge_is_not_anchored() {
        const SOURCE: &str = "<script>\n\tlet s = \"a\\\n b\";\n</script>\n\n<p>{s}</p>\n";
        let backslash = SOURCE.find('\\').expect("the continuation backslash");
        let run_start = backslash + 1;
        assert_eq!(SOURCE.as_bytes()[run_start], b'\n', "the run opens with LF");

        let anchors = anchors_of(SOURCE);
        #[allow(clippy::cast_possible_truncation)]
        let run_start = run_start as u32;
        assert!(
            !anchors.script_ws.contains(&run_start),
            "the continuation's start edge must not be an anchor: {:?}",
            anchors.script_ws
        );
        // The end edge is past the terminator — the pair is already complete, so
        // an appended character is ordinary string content.
        #[allow(clippy::cast_possible_truncation)]
        let run_end = (backslash + 3) as u32; // `\` + LF + ' '
        assert!(
            anchors.script_ws.contains(&run_end),
            "the run's end edge stays anchored: {:?}",
            anchors.script_ws
        );

        // An ordinary run in the same script keeps BOTH edges — the guard is
        // scoped to the continuation, not a blanket start-edge drop.
        let plain = anchors_of("<script>\n\tlet s = 1;\n</script>\n\n<p>{s}</p>\n");
        assert!(plain.script_ws.len() > 4, "{:?}", plain.script_ws);
    }

    /// `<svelte:*>` special elements and `<svelte:options>` carry attributes too,
    /// and neither is reached by the regular-element arm of the node walk.
    #[test]
    fn special_element_attribute_values_are_anchored() {
        let dynamic = anchors_of("<svelte:element this=\"div\" class=\"x\">y</svelte:element>\n");
        assert!(
            !dynamic.attr_value_edges.is_empty(),
            "a <svelte:element>'s quoted attribute values anchor"
        );
        let options = anchors_of("<svelte:options namespace=\"svg\" />\n<p>x</p>\n");
        assert!(
            !options.attr_value_edges.is_empty(),
            "a <svelte:options> attribute value anchors"
        );
    }

    /// `u8::is_ascii_whitespace` omits `<VT>`, which IS in the insertion set — the
    /// scan must use the JS ASCII class or a VT-only run goes unanchored.
    #[test]
    fn vertical_tab_runs_are_anchored() {
        let anchors =
            anchors_of("<script>\n\tlet a = 1;\u{000B}let b = 2;\n</script>\n<p>{a}{b}</p>\n");
        let vt = "<script>\n\tlet a = 1;".len();
        #[allow(clippy::cast_possible_truncation)]
        let vt = vt as u32;
        assert!(
            anchors.script_ws.contains(&vt),
            "a VT-only run anchors: {:?}",
            anchors.script_ws
        );
    }

    /// The attribute anchor's quote guard: an UNQUOTED value's extent is decided
    /// by the parser's own whitespace notion, which is the thing under test.
    #[test]
    fn unquoted_attribute_values_are_not_anchored() {
        let quoted = anchors_of("<p class=\"box\">x</p>\n");
        assert!(!quoted.attr_value_edges.is_empty());
        let unquoted = anchors_of("<p class=box>x</p>\n");
        assert!(unquoted.attr_value_edges.is_empty());
    }

    #[test]
    fn css_ident_ends_land_after_the_name() {
        const SOURCE: &str = "<p class=\"a\">x</p>\n<style>\n\t:global(.a)::before {\n\t\tcolor: red;\n\t}\n</style>\n";
        let anchors = anchors_of(SOURCE);
        let names: Vec<&str> = anchors
            .css_ident_ends
            .iter()
            .map(|&end| {
                let end = end as usize;
                let start = SOURCE[..end]
                    .rfind([':', '.', '#'])
                    .expect("an introducer precedes the name");
                &SOURCE[start + 1..end]
            })
            .collect();
        assert!(names.contains(&"global"), "{names:?}");
        assert!(names.contains(&"a"), "{names:?}");
        assert!(names.contains(&"before"), "{names:?}");
    }

    #[test]
    fn every_operator_is_labeled_uniquely() {
        let mut labels: Vec<&str> = Operator::ALL.iter().map(|o| o.label()).collect();
        labels.sort_unstable();
        let count = labels.len();
        labels.dedup();
        assert_eq!(labels.len(), count);
    }
}

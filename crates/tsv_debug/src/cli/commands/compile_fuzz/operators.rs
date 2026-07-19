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
//!
//! ## Why splices and not byte edits
//!
//! A mutant must stay **oracle-compilable** to grade anything, so every operator
//! inserts a whole well-formed construct at an offset [`Anchors`] proved is
//! structurally valid. The mutant is re-anchored (re-parsed) between operators, so
//! operator *N+1* always sees the real post-*N* document rather than stale offsets.

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

    #[test]
    fn every_operator_is_labeled_uniquely() {
        let mut labels: Vec<&str> = Operator::ALL.iter().map(|o| o.label()).collect();
        labels.sort_unstable();
        let count = labels.len();
        labels.dedup();
        assert_eq!(labels.len(), count);
    }
}

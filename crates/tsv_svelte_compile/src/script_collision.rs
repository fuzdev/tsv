//! The whole-component rune/store collision pre-pass: a rune keyword whose
//! `$`-stripped stem is also a binding in scope reads as a STORE subscription to
//! the oracle, not as the rune.
//!
//! Oracle phase 2, run before the binding table is built. Target-independent —
//! the collision changes what the oracle's *analysis* decides, not what any
//! transform emits, so a client transform inherits this unchanged.
//!
//! Two deliberate imprecisions carry their own arguments below: the static-block
//! **lexical fence** ([`script_contains_static_block`]) and the whole-document
//! **source scan** for a `$stem` reference ([`source_references_identifier`]).
//! Both over-refuse on purpose; the direction matters, because a missed binding
//! is a MISMATCH while an extra refusal is merely a gap.

use tsv_ts::ast::internal::{LiteralValue, Statement};

use crate::analyze::{
    RUNE_BASES, RuneInit, classify_rune_init, pattern_binding_names,
    pattern_binds_unnameable_identifier,
};
use crate::script_decls::{ScriptDeclaration, VarScope, each_script_declaration};
use crate::text_class::is_js_whitespace;
use crate::transform_server::unsupported;
use crate::{CompileError, Refusal};

/// What a script declaration of a rune STEM (`state`, `props`, …) initializes it
/// to — the only thing the oracle's store reclassification asks about. See
/// [`refuse_rune_store_collision`].
///
/// The question is about the oracle's `binding.initial`, NOT about the source
/// text of the declarator — the two come apart for a `var` that hoisted through
/// a porous scope, whose binding carries no initializer at all.
enum StemInit {
    /// The binding's `initial` is `$props()` exactly (the oracle's
    /// `get_rune(binding.initial) === '$props'`).
    PropsRune,
    /// The binding's `initial` is some OTHER rune call (`$state(0)`,
    /// `$derived(e)`, `$state.snapshot(x)`, `$props.id()`) —
    /// `get_rune(binding.initial) !== null`.
    OtherRune,
    /// An `import { … } from 'svelte/store'` local.
    SvelteStoreImport,
    /// `get_rune(binding.initial) === null`: another import, a function/class
    /// declaration, a declarator with no init or a non-rune init, **or** a `var`
    /// whose initializer the hoist dropped.
    Plain,
}

/// Refuse a rune keyword whose `$`-stripped stem is ALSO a binding **in scope at
/// the instance script** — `import { state } from './store'` beside a `$state`
/// reference.
///
/// The oracle's `analyze_component` (`phases/2-analyze/index.js`, the "create
/// synthetic bindings for store subscriptions" loop) walks every unresolved
/// `$`-prefixed reference and, for a rune name `$stem`, reclassifies it as a
/// STORE subscription — `store_sub` binding, emitting `$.store_get(…)` — as soon
/// as `instance.scope.get(stem)` is non-null and that binding's own initializer
/// is not itself a rune-creating call. It then DELETES the reference from
/// `module.scope.references`, which is what the runes-mode inference reads a few
/// lines later, so the collision can flip the whole component out of runes mode.
///
/// ⚠️ **`instance.scope.get` walks UP the scope chain**
/// (`phases/scope.js:748` — `this.declarations.get(name) ?? this.parent?.get(name)
/// ?? null`), and the instance scope's parent is the MODULE scope
/// (`2-analyze/index.js:337` — `js(root.instance, scope_root, true, module.scope)`).
/// So a `<script module>` binding of the stem reclassifies an instance-script
/// `$stem` too, and both bodies are searched here, instance first.
///
/// It walks up only. **Downward** — a function parameter named `state`, a
/// block-scoped `let state`, a name bound in a nested FUNCTION body — is a CHILD
/// scope `instance.scope.get` never sees, so none of those collide and all keep
/// compiling. Two nested forms DO reach script scope, and they differ:
///
/// - a **`var`** anywhere below the top level except inside a function: it is
///   function-scoped, so a `var state` in any block, for-head, switch, or
///   try/catch of the script lands in `instance.scope`. But it arrives
///   **stripped of its initializer** — `scope.js:673-681` re-declares it on the
///   parent with the 4-argument `initial` left at its `null` default — so the
///   rune exemption below can never apply to one, whatever it was written with;
/// - a statement in a **class STATIC BLOCK**, which is not a scope at all:
///   `phases/scope.js` has no `StaticBlock` visitor, so a `var` there declares
///   directly in the enclosing scope and **keeps** its initializer. ECMAScript
///   says a static block is its own VariableEnvironment; the oracle is the
///   parity target, so the oracle wins. A class METHOD body is a genuine
///   boundary on both sides — the oracle's `FunctionExpression` visitor gives it
///   a scope; a class PROPERTY INITIALIZER is **not** (there is no
///   `PropertyDefinition` visitor either), so it evaluates in the enclosing scope
///   and a class expression there is as reachable as one anywhere else.
///
/// The two are handled differently, and the asymmetry is deliberate. The `var`
/// hoist is modelled EXACTLY, by [`each_script_declaration`]'s one exhaustive
/// statement enumeration, with `ScriptDeclaration::Declarator::initial_dropped`
/// carrying the initializer distinction — those shapes are ordinary real code and
/// the precision is earned. The static block is instead FENCED lexically
/// ([`script_contains_static_block`]): reaching every class body a script can
/// hold means enumerating every expression position of every statement, which is
/// the surface that shipped holes twice, and a static block in a Svelte component
/// is vanishingly rare (zero of the ~4900 `.svelte` files under the compile-corpus roots contain one),
/// so the precision would buy nothing and cost correctness.
///
/// tsv is a runes-only compiler and models neither the reclassification nor mode
/// inference: it would compile `$state` as the rune and silently emit the wrong
/// code (`const x = void 0` where the oracle emits a store read). Refuse instead.
///
/// The oracle's EXEMPTION covers the majority of real Svelte 5 code and is
/// modelled here: `let state = $state(0)` / `const props = $props()` keep
/// compiling, because `get_rune(binding.initial)` is non-null there. Three
/// corners of the oracle's clause come with it — a stem OTHER than `props`
/// initialized by `$props()` (`let state = $props()`) IS reclassified ("rune-line
/// names received as props are valid too"), `$derived` beside an
/// `import { derived } from 'svelte/store'` is NOT, and a rune-initialized `var`
/// that hoisted through a porous scope IS, because the initializer the exemption
/// reads was dropped on the way up (above).
///
/// It is an over-approximation in one direction: `Plain` is also what an
/// unreadable binding shape yields (an escaped identifier, a pattern the shared
/// walk declines), so a document can refuse where the oracle would have exempted.
/// A refusal is the safe side; a missed binding is a MISMATCH.
///
/// The reference test is a boundary-checked source scan rather than an AST walk:
/// tsv recognizes a rune at half a dozen scattered sites (declarator inits, the
/// statement-position `$effect`/`$inspect` drops, class fields, the rune guard's
/// sanctioned set, the template `$state.snapshot`), and a check that must be
/// wired into each of them can miss one — which is a MISMATCH. One whole-document
/// scan cannot. Its cost is over-refusing a document that merely MENTIONS
/// `$state` while also binding `state` — in a comment, in template text, in a
/// string, or as a **member/property NAME** (`obj.$state`, `{ $state: 1 }`),
/// which is not a rune reference at all. Every one is a clean refusal rather
/// than a wrong compile, which is why the scan is deliberately unbounded.
pub(crate) fn refuse_rune_store_collision<'arena>(
    instance_body: &'arena [Statement<'arena>],
    module_body: &'arena [Statement<'arena>],
    source: &str,
) -> Result<(), CompileError> {
    let static_block = script_contains_static_block(instance_body, source)
        || script_contains_static_block(module_body, source);
    for stem in RUNE_BASES {
        let name = format!("${stem}");
        if !source_references_identifier(source, &name) {
            continue;
        }
        // The static-block fence: a class body is opaque to the declaration walk,
        // so with one present this check cannot rule out a script-scope binding of
        // the stem and refuses unconditionally.
        if static_block {
            return Err(unsupported(Refusal::RuneNameBoundAsStore { name }));
        }
        // `instance.scope.get(stem)`: the instance scope's own declarations, then
        // its parent's — the module scope.
        let Some(init) = stem_declaration(instance_body, stem, source)
            .or_else(|| stem_declaration(module_body, stem, source))
        else {
            continue;
        };
        let reclassified = match init {
            // `get_rune(init) === null` — the plain case.
            StemInit::Plain => true,
            StemInit::SvelteStoreImport => *stem != "derived",
            StemInit::PropsRune => *stem != "props",
            StemInit::OtherRune => false,
        };
        if reclassified {
            return Err(unsupported(Refusal::RuneNameBoundAsStore { name }));
        }
    }
    Ok(())
}

/// Whether a class **static block** occurs anywhere in `stmts`' source range.
///
/// The fence that makes the whole class-body family safe without traversing it.
/// A static block is the ONLY construct below a script's top level, other than the
/// `var` hoists [`crate::script_decls`] models exactly, that can declare a name
/// at script scope: statements appear only in function bodies (a genuine scope
/// boundary on both sides) and in static blocks (no scope at all in the oracle —
/// `phases/scope.js` has no `StaticBlock` visitor, so `class C { static { var
/// state = 5 } }` declares `state` in the ENCLOSING scope; ECMAScript disagrees,
/// but the oracle is the parity target). With those two cases covered, the
/// declaration walk can stop at every class body and every expression position.
///
/// Deliberately **lexical, not an AST walk** — that is the point. Reaching every
/// class body a statement can hold means visiting every expression position of
/// every statement, and a hand-enumerated version of that surface has twice
/// shipped with holes (a class expression in a for-head, a `super_class`, a
/// property initializer, a computed key, a parameter default…), each hole a
/// silent MISMATCH. A scan over the bytes has no positions to enumerate.
///
/// **Under-reporting is what this scan must not do**, and the whitespace class is
/// the whole of that argument. A static block is written `static`, then trivia,
/// then `{`; its `static` token always lies inside a statement's span; and the
/// trivia is JS `WhiteSpace`/`LineTerminator` or a comment. So the scan is
/// complete exactly as far as [`is_js_whitespace`] is the JS class — which it is
/// by construction, unlike Rust's `char::is_whitespace` (that one omits
/// `U+FEFF`, and a `static\u{FEFF}{ var state = 5 }` block written with it was
/// invisible here). A `/` after the trivia run may open a comment, so it counts
/// as "cannot tell" rather than decoding comment syntax.
///
/// It happily OVER-reports — `static` in a comment or a string, a `/` that turns
/// out to be a division, a `U+0085` (`<NEL>`, Unicode whitespace but NOT JS
/// whitespace, so the scan sees a boundary the JS lexer would reject anyway) —
/// and over-reporting only costs an extra refusal.
fn script_contains_static_block(stmts: &[Statement<'_>], source: &str) -> bool {
    let (Some(first), Some(last)) = (stmts.first(), stmts.last()) else {
        return false;
    };
    let range = first.span().start as usize..last.span().end as usize;
    let Some(text) = source.get(range) else {
        // An unexpected span shape is not a reason to go blind.
        return true;
    };
    let mut offset = 0;
    while let Some(found) = text[offset..].find("static") {
        let start = offset + found;
        let end = start + "static".len();
        let preceded_by_ident = text[..start]
            .chars()
            .next_back()
            .is_some_and(is_identifier_part);
        // `static` is ASCII, so `start + 1` is a char boundary.
        offset = start + 1;
        if preceded_by_ident {
            continue;
        }
        // Trivia between `static` and its `{` is JS whitespace and comments —
        // `is_js_whitespace`, NOT Rust's `char::is_whitespace`, which omits
        // `U+FEFF` and would miss a static block written with one. A `/` may open
        // a comment, so treat it as "cannot tell" rather than decoding comment
        // syntax here — the safe direction.
        if text[end..]
            .trim_start_matches(is_js_whitespace)
            .starts_with(['{', '/'])
        {
            return true;
        }
    }
    false
}

/// How `stmts` declare `stem` at script scope, or `None` when they don't (one
/// level of the oracle's `instance.scope.get(stem)` chain). A later declaration
/// wins, mirroring the scope's last-writer-wins map.
///
/// Routed through [`each_script_declaration`] — the ONE exhaustive statement
/// enumeration — so a `var` hoisted out of a nested block or for-head is seen and
/// a new `Statement` variant fails compilation rather than silently escaping the
/// guard.
fn stem_declaration<'arena>(
    stmts: &'arena [Statement<'arena>],
    stem: &str,
    source: &str,
) -> Option<StemInit> {
    let mut found = None;
    let walk = each_script_declaration::<()>(stmts, VarScope::WithHoistedVars, &mut |decl| {
        match decl {
            ScriptDeclaration::Declarator {
                declarator,
                initial_dropped,
            } => {
                let mut names = Vec::new();
                // A pattern the shared walk can't enumerate is not a reason to
                // refuse on its own — the binding table refuses those shapes on
                // their own path — but it IS a name this check cannot rule out,
                // so it counts as declaring the stem (a safe over-refusal). Same
                // for an escaped binding identifier, which
                // `pattern_binding_names` skips outright.
                let unnameable = pattern_binding_names(&declarator.id, source, &mut names).is_err()
                    || pattern_binds_unnameable_identifier(&declarator.id);
                if !unnameable && !names.iter().any(|n| n == stem) {
                    return Ok(());
                }
                // A `var` that hoisted through a porous scope arrives with NO
                // initializer (`ScriptDeclaration::Declarator::initial_dropped`),
                // so the oracle's `get_rune(binding.initial)` sees `null` and the
                // rune EXEMPTION does not apply however the declarator's own init
                // reads.
                found = Some(match declarator.init.as_ref() {
                    Some(init) if !initial_dropped => match classify_rune_init(init, source) {
                        Some(RuneInit::Props) => StemInit::PropsRune,
                        Some(_) => StemInit::OtherRune,
                        None => StemInit::Plain,
                    },
                    _ => StemInit::Plain,
                });
            }
            ScriptDeclaration::Function(id) | ScriptDeclaration::Class(id) => {
                if identifier_name(id, source) == stem {
                    found = Some(StemInit::Plain);
                }
            }
            ScriptDeclaration::Import { local, declaration } => {
                if identifier_name(local, source) == stem {
                    let from_store = matches!(
                        &declaration.source.value,
                        LiteralValue::String(s) if s.resolve(declaration.source.span, source) == "svelte/store"
                    );
                    found = Some(if from_store {
                        StemInit::SvelteStoreImport
                    } else {
                        StemInit::Plain
                    });
                }
            }
        }
        Ok(())
    });
    // The callback never fails.
    debug_assert!(walk.is_ok());
    found
}

/// An identifier's name, escaped forms included (`state` → `state`).
///
/// Unlike [`crate::script_decls::plain_identifier_name`] this never returns
/// `None`: a binding whose name this check cannot read is a binding it would
/// MISS, and a missed binding is an under-refusal.
fn identifier_name(id: &tsv_ts::ast::internal::Identifier<'_>, source: &str) -> String {
    id.name(source).to_string()
}

/// Whether `name` (a `$`-prefixed rune keyword) occurs in `source` as a whole
/// identifier — bounded on both sides by a character that is not an ECMAScript
/// `IdentifierPart`. Deliberately blind to comments, strings, and template text:
/// see [`refuse_rune_store_collision`] for why over-detection there is the safe
/// direction.
///
/// The boundary test decodes CHARACTERS, not bytes. A byte-level "every byte
/// `>= 0x80` continues an identifier" shortcut reads the lead byte of a non-ASCII
/// **whitespace** character — NBSP (U+00A0, `0xC2 0xA0`) is ECMAScript
/// whitespace — as identifier text, so `$state (1)` written with an NBSP would
/// not match and a genuine reference would be MISSED. That is an under-refusal,
/// the direction this whole check exists to avoid.
fn source_references_identifier(source: &str, name: &str) -> bool {
    // The `start + 1` resume below is a char boundary because every caller passes
    // a `$`-prefixed rune keyword, so byte 0 of a match is the ASCII `$`.
    debug_assert!(name.starts_with('$'));
    let mut offset = 0;
    while let Some(found) = source[offset..].find(name) {
        let start = offset + found;
        let end = start + name.len();
        let before_ok = !source[..start]
            .chars()
            .next_back()
            .is_some_and(is_identifier_part);
        let after_ok = !source[end..].chars().next().is_some_and(is_identifier_part);
        if before_ok && after_ok {
            return true;
        }
        offset = start + 1;
    }
    false
}

/// ECMAScript `IdentifierPart`, for the boundary test above.
///
/// `XID_Continue` plus `$` (`_` and ZWNJ/ZWJ are already in `XID_Continue`).
/// ECMAScript actually uses the slightly wider `ID_Continue`; the handful of code
/// points in `ID_Continue \ XID_Continue` therefore read as a BOUNDARY here,
/// which makes an adjacent `$state\u{309B}` match as a whole identifier — an
/// over-refusal, the safe direction.
fn is_identifier_part(ch: char) -> bool {
    unicode_ident::is_xid_continue(ch) || ch == '$'
}

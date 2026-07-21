//! The top-level binding table, the module-script analysis, and the runes-mode
//! import rules.
//!
//! Oracle phase 2 analysis, target-independent: this classifies what the script
//! *declares* and what the evaluator may fold, not what any transform emits. The
//! server's rewrite of those same declarators lives in
//! [`crate::script_rewrite`]; keeping the two apart is what lets a second
//! transform reuse the analysis without inheriting the server's codegen.
//!
//! Routes through [`crate::script_decls::each_script_declaration`] — the one
//! exhaustive statement enumeration — so a new AST variant fails compilation
//! rather than silently escaping the table.

use tsv_lang::SharedInterner;
use tsv_ts::ast::internal::{
    Expression, ImportDeclaration, ImportSpecifier, LiteralValue, ModuleExportName, Statement,
    VariableDeclarator,
};

use crate::analyze::{
    Binding, BindingKind, Bindings, Initial, NameSet, RuneInit, classify_rune_init,
    pattern_binding_names,
};
use crate::rune_guard::{WalkCtx, refuse_dollar_import_locals, walk_statement_guarded};
use crate::script_decls::{
    ScriptDeclaration, VarScope, each_script_declaration, identifier_binding_name,
    plain_identifier_name,
};
use crate::transform_server::unsupported;
use crate::{CompileError, Refusal, erase};

/// Erase and validate a plain module `<script module>` / `<script
/// context="module">`, returning its type-free statement list (imports +
/// declarations + non-default exports, source order) for module-scope emission.
///
/// v1 supports **plain** module scripts only. TypeScript erases under the
/// document `lang="ts"` flag exactly as the instance script does. Then, per
/// statement:
///
/// - `export default` refuses [`Refusal::ModuleDefaultExport`] — the oracle
///   errors `module_illegal_default_export`;
/// - an invalid runes-mode import (`svelte/internal*`,
///   `beforeUpdate`/`afterUpdate`) refuses via [`refuse_runes_invalid_import`];
/// - the statement is guard-walked **without** a store exemption, so a
///   module-scope rune, a `$name` store read (the oracle's
///   `store_invalid_subscription`), or a top-level `await` refuses — v1 defers
///   the oracle's module `$state`→`v` / `$derived`→`$.derived(…)` rewrites (the
///   corpus is rune-free, so this is a lossless over-refusal).
///
/// A supported module body emits **verbatim** (post-erase): the oracle's
/// module-body reassignment/needs_context effects flow through the shared
/// whole-component analysis ([`crate::needs_context::analyze_component`]) and the
/// binding table ([`analyze_script`]), not through any module-only rewrite.
pub(crate) fn analyze_module_script<'arena>(
    root: &tsv_svelte::ast::internal::Root<'arena>,
    source: &str,
    arena: &'arena bumpalo::Bump,
    ts_document: bool,
) -> Result<(&'arena [Statement<'arena>], Vec<tsv_lang::Span>), CompileError> {
    let Some(script) = root.module else {
        return Ok((&[], Vec::new()));
    };
    let erased = erase::erase_statements(arena, source, script.content.body)?;
    // The same document-wide TypeScript gate the instance body pays: without the
    // flag, a `: T` / `as T` / `x!` in the module is a plain-JS parse error in the
    // oracle, so a permissive accept here would be an over-acceptance.
    if erased.typescript && !ts_document {
        return Err(unsupported(Refusal::TypeScriptWithoutLangTs));
    }
    // The module's erased-region windows ride out alongside its body: a carried
    // module-script comment intersecting one refuses, exactly as the instance
    // path does (see `collect_module_script_comments`).
    let body = erased.body;
    let regions = erased.regions;
    // Scratch collection sinks — the guard walk's reassignment/shadow collection is
    // redundant here (the whole-component `analyze_component` covers module scope),
    // so only its REFUSAL is wanted. Derived reads are impossible in a module (no
    // module `$derived` survives the guard), so an empty derived set avoids a false
    // `DerivedBindingRead` on a name that merely coincides with an instance derived.
    let mut updated = NameSet::default();
    let mut nested = NameSet::default();
    let derived = NameSet::default();
    for stmt in body {
        if matches!(stmt, Statement::ExportDefaultDeclaration(_)) {
            return Err(unsupported(Refusal::ModuleDefaultExport));
        }
        if let Statement::ImportDeclaration(import) = stmt {
            refuse_runes_invalid_import(import, source, &root.interner)?;
        }
        let mut ctx = WalkCtx::new(
            source,
            &mut updated,
            &mut nested,
            &derived,
            std::rc::Rc::clone(&root.interner),
        );
        walk_statement_guarded(stmt, &mut ctx, 0)?;
    }
    Ok((body, regions))
}

/// Mirror the oracle's runes-mode import rules (its analyze-phase
/// `ImportDeclaration` visitor): a `$`-prefixed import LOCAL is refused via
/// [`refuse_dollar_import_locals`], which DECODES an escaped local through the
/// `interner` (so `import { $x } from …` written escaped refuses like plain `$x`),
/// any `svelte/internal*` source is forbidden (private runtime code), and
/// `beforeUpdate`/`afterUpdate` cannot be imported from `svelte`. A string-literal
/// imported name is skipped exactly as the oracle skips it (its check matches
/// `Identifier` names only); an escaped IMPORTED name from `svelte` still refuses
/// conservatively — that check reads the raw span, so it can't compare the oracle's
/// DECODED name and over-refuses instead.
pub(crate) fn refuse_runes_invalid_import(
    import: &ImportDeclaration<'_>,
    source: &str,
    interner: &SharedInterner,
) -> Result<(), CompileError> {
    // Checked here rather than in the guard walk because the transform hoists
    // imports out of the statement stream before `walk_statement` runs. The rule
    // — including the type-only-import caveat — lives at
    // `refuse_dollar_import_locals`.
    refuse_dollar_import_locals(import.specifiers, source, interner)?;
    let LiteralValue::String(cooked) = &import.source.value else {
        return Ok(());
    };
    let specifier = cooked.resolve(import.source.span, source);
    if specifier.starts_with("svelte/internal") {
        return Err(unsupported(Refusal::SvelteInternalImport));
    }
    if specifier == "svelte" {
        for spec in import.specifiers {
            let ImportSpecifier::Named(named) = spec else {
                continue;
            };
            let ModuleExportName::Identifier(imported) = &named.imported else {
                continue;
            };
            match plain_identifier_name(imported, source) {
                Some(name) if name == "beforeUpdate" || name == "afterUpdate" => {
                    return Err(unsupported(Refusal::RunesInvalidImport { name }));
                }
                Some(_) => {}
                None => {
                    return Err(unsupported(Refusal::RunesInvalidImport {
                        name: "escaped identifier".to_string(),
                    }));
                }
            }
        }
    }
    Ok(())
}

/// Analysis pass: populate the top-level binding table and the derived-name
/// set from the script's top-level declarations.
pub(crate) fn analyze_script<'arena>(
    stmts: &'arena [Statement<'arena>],
    source: &str,
    bindings: &mut Bindings<'arena>,
    derived_names: &mut NameSet,
) -> Result<(), CompileError> {
    // The oracle's duplicate-`$props()` flag is per-SCRIPT state — its analyze phase
    // seeds a fresh `has_props_rune: false` for the module and the instance analysis
    // alike (`phases/2-analyze/index.js:313,725,793`) — so the flag is scoped to one
    // `analyze_script` call and the export recursion inside it, not to the component.
    let mut seen_props = false;
    analyze_script_in(stmts, source, bindings, derived_names, &mut seen_props)
}

fn analyze_script_in<'arena>(
    stmts: &'arena [Statement<'arena>],
    source: &str,
    bindings: &mut Bindings<'arena>,
    derived_names: &mut NameSet,
    seen_props: &mut bool,
) -> Result<(), CompileError> {
    each_script_declaration(stmts, VarScope::TopLevelOnly, &mut |decl| {
        let (id, initial) = match decl {
            // `VarScope::TopLevelOnly` never reports a hoisted declarator, so
            // `initial_dropped` is always false here.
            ScriptDeclaration::Declarator { declarator, .. } => {
                return analyze_declarator(declarator, source, bindings, derived_names, seen_props);
            }
            ScriptDeclaration::Function(id) => (id, Initial::Function),
            ScriptDeclaration::Class(id) | ScriptDeclaration::Import { local: id, .. } => {
                (id, Initial::None)
            }
        };
        if let Some(name) = plain_identifier_name(id, source) {
            bindings.insert(
                name,
                Binding {
                    kind: BindingKind::Normal,
                    initial,
                    updated: false,
                },
            );
        }
        Ok(())
    })
}

/// Register every leaf of a destructured rune pattern into the binding table
/// with the RUNE'S COMPUTED INITIAL — the same `initial`+`kind` the
/// non-destructured arm of that rune assigns. One helper for all four
/// destructured runes, so the destructure and non-destructure arms of a rune can
/// never disagree on how a leaf folds.
///
/// The fold rule is the oracle's `phases/scope.js:1204-1213`: it declares EVERY
/// destructuring leaf with `initial = declarator.init` (the whole
/// `$state(...)`/`$derived(...)` call), then evaluates that init through the
/// rune's argument. So every leaf folds to the value the *non-destructured*
/// binding of that rune would — a `let {a} = $state(d)` with `d`→5 folds `{a}` to
/// `5` (the CONTAINER value, ignoring the `.a` projection: the fold uses
/// `eval(arg)`, not the runtime `(5).a`). Passing the rune's computed initial to
/// every leaf reproduces that byte-for-byte; a foldable scalar arg is
/// corpus-absent, while an object/array arg stays UNKNOWN (no fold) — the
/// corpus-common case, unchanged.
///
/// `kind` is `Derived` for `$derived`/`$derived.by` (each leaf added to
/// `derived_names` so a `{name}` read lowers to `name()`) and `Normal` for
/// `$state`/`$state.raw`/`$state.snapshot`. The transform (`destructure`)
/// reconstructs the same leaf set from the pattern nodes, so the two stay in
/// step. An ESCAPED binding leaf can't be named from its source span
/// ([`pattern_binding_names`] skips it): registering the leaf is what gives it the
/// fold initial (and, for a derived leaf, makes its read a call), and a missed
/// leaf would MISMATCH, so a pattern binding one refuses (`unnameable`) — a safe
/// over-refusal on a shape absent from the gating Svelte corpus.
fn register_destructured_leaves<'arena>(
    id: &Expression<'arena>,
    source: &str,
    bindings: &mut Bindings<'arena>,
    derived_names: &mut NameSet,
    kind: BindingKind,
    initial: Initial<'arena>,
    unnameable: Refusal,
) -> Result<(), CompileError> {
    if crate::analyze::pattern_binds_unnameable_identifier(id) {
        return Err(unsupported(unnameable));
    }
    let is_derived = kind == BindingKind::Derived;
    let mut names = Vec::new();
    pattern_binding_names(id, source, &mut names)?;
    for name in names {
        if is_derived {
            derived_names.insert(name.clone());
        }
        bindings.insert(
            name,
            Binding {
                kind,
                initial,
                updated: false,
            },
        );
    }
    Ok(())
}

/// Classify one top-level declarator into the binding table.
fn analyze_declarator<'arena>(
    declarator: &'arena VariableDeclarator<'arena>,
    source: &str,
    bindings: &mut Bindings<'arena>,
    derived_names: &mut NameSet,
    seen_props: &mut bool,
) -> Result<(), CompileError> {
    let rune = declarator
        .init
        .as_ref()
        .and_then(|init| classify_rune_init(init, source));

    match rune {
        Some(RuneInit::Props) => {
            // The oracle rejects a second `$props()` (`props_duplicate`) from its
            // analyze-phase `CallExpression` visitor
            // (`phases/2-analyze/visitors/CallExpression.js:68-73`), BEFORE the
            // placement check — so the duplicate wins over `props_invalid_placement`
            // when both apply. Only a top-level declarator init is inspected here;
            // a `$props()` in any other position already refuses on its own path,
            // so this sees every shape tsv would otherwise accept.
            if *seen_props {
                return Err(unsupported(Refusal::DuplicateProps));
            }
            *seen_props = true;
            let mut names = Vec::new();
            pattern_binding_names(&declarator.id, source, &mut names)?;
            for name in names {
                bindings.insert(
                    name,
                    Binding {
                        kind: BindingKind::Prop,
                        initial: Initial::None,
                        updated: false,
                    },
                );
            }
            Ok(())
        }
        Some(RuneInit::PropsId) => {
            // `const id = $props.id()` binds a plain identifier only (the oracle's
            // `props_id_invalid_placement` rejects a destructure). The binding
            // evaluates through the `$props.id()` call — the evaluator maps that
            // keypath to a STRING sentinel, so a `{id}` read never folds (matching
            // the oracle's `$.escape(id)`).
            let name = identifier_binding_name(&declarator.id, source)
                .ok_or_else(|| unsupported(Refusal::PropsIdBindingPattern))?;
            bindings.insert(
                name,
                Binding {
                    kind: BindingKind::Normal,
                    initial: declarator
                        .init
                        .as_ref()
                        .map_or(Initial::None, Initial::Expr),
                    updated: false,
                },
            );
            Ok(())
        }
        Some(RuneInit::State(arg)) => {
            // `$state(arg)` / `$state.raw(arg)`: the leaf folds through the
            // argument (`Undefined` for the argless `$state()` — the oracle's
            // argless value is `void 0`). A destructured target lowers 1→N via
            // `create_state_declarators` (the transform's `expand_destructured_state`);
            // every leaf takes this same initial.
            let initial = arg.map_or(Initial::Undefined, Initial::Expr);
            match identifier_binding_name(&declarator.id, source) {
                Some(name) => bindings.insert(
                    name,
                    Binding {
                        kind: BindingKind::Normal,
                        initial,
                        updated: false,
                    },
                ),
                None => register_destructured_leaves(
                    &declarator.id,
                    source,
                    bindings,
                    derived_names,
                    BindingKind::Normal,
                    initial,
                    Refusal::DestructuringState,
                )?,
            }
            Ok(())
        }
        Some(RuneInit::StateSnapshot(_)) => {
            // `const s = $state.snapshot(x)` unwraps to `const s = x` for EMISSION,
            // but the binding stays UNKNOWN to the evaluator — the unwrap is the
            // emission form, not the evaluation form. The oracle evaluates a rune
            // declarator through its argument for `$state` / `$state.raw` /
            // `$derived` only; every other rune, `$state.snapshot` included, falls
            // to the `default` arm and yields UNKNOWN, so a `{s}` read never folds
            // (`$.escape(s)`). That holds however the argument itself evaluates —
            // a plain `let` argument does not fold either. Every destructured leaf
            // inherits that `Initial::None` (a snapshot leaf never folds), so a
            // destructured target lowers exactly like `$state` MINUS the fold.
            match identifier_binding_name(&declarator.id, source) {
                Some(name) => bindings.insert(
                    name,
                    Binding {
                        kind: BindingKind::Normal,
                        initial: Initial::None,
                        updated: false,
                    },
                ),
                None => register_destructured_leaves(
                    &declarator.id,
                    source,
                    bindings,
                    derived_names,
                    BindingKind::Normal,
                    Initial::None,
                    Refusal::DestructuringStateSnapshot,
                )?,
            }
            Ok(())
        }
        Some(RuneInit::Derived(expr)) => {
            // `$derived(expr)`: a leaf folds through the argument. The oracle's
            // `scope.js` declares every destructured-derived leaf with the whole
            // `$derived(...)` call as its initial and evaluates it through `expr`,
            // exactly like an identifier target — so a `let {a} = $derived(d)` with
            // `d`→5 folds `{a}` to `5` (probe-confirmed). The transform
            // (`expand_destructured_derived`) still lowers each leaf to its own
            // `$.derived(() => path)`; the initial only governs the fold.
            let initial = Initial::Expr(expr);
            match identifier_binding_name(&declarator.id, source) {
                Some(name) => {
                    derived_names.insert(name.clone());
                    bindings.insert(
                        name,
                        Binding {
                            kind: BindingKind::Derived,
                            initial,
                            updated: false,
                        },
                    );
                }
                None => register_destructured_leaves(
                    &declarator.id,
                    source,
                    bindings,
                    derived_names,
                    BindingKind::Derived,
                    initial,
                    Refusal::DestructuringDerived,
                )?,
            }
            Ok(())
        }
        Some(RuneInit::DerivedBy(f)) => {
            // `$derived.by(fn)`: the oracle evaluates through an expression-bodied
            // arrow's body (a block-bodied arrow is UNKNOWN). A destructured leaf
            // folds through that same body.
            use tsv_ts::ast::internal::ArrowFunctionBody;
            let initial = match f {
                Expression::ArrowFunctionExpression(arrow) => match &arrow.body {
                    ArrowFunctionBody::Expression(body) => Initial::Expr(body),
                    ArrowFunctionBody::BlockStatement(_) => Initial::None,
                },
                _ => Initial::None,
            };
            match identifier_binding_name(&declarator.id, source) {
                Some(name) => {
                    derived_names.insert(name.clone());
                    bindings.insert(
                        name,
                        Binding {
                            kind: BindingKind::Derived,
                            initial,
                            updated: false,
                        },
                    );
                }
                None => register_destructured_leaves(
                    &declarator.id,
                    source,
                    bindings,
                    derived_names,
                    BindingKind::Derived,
                    initial,
                    Refusal::DestructuringDerivedBy,
                )?,
            }
            Ok(())
        }
        None => {
            // Plain declarator: an Identifier id gets its init as the
            // evaluation initial; destructured ids are Opaque (the oracle's
            // per-binding initial for those isn't modeled).
            if let Some(name) = identifier_binding_name(&declarator.id, source) {
                bindings.insert(
                    name,
                    Binding {
                        kind: BindingKind::Normal,
                        initial: declarator
                            .init
                            .as_ref()
                            .map_or(Initial::None, Initial::Expr),
                        updated: false,
                    },
                );
            } else {
                let mut names = Vec::new();
                pattern_binding_names(&declarator.id, source, &mut names)?;
                for name in names {
                    bindings.insert(
                        name,
                        Binding {
                            kind: BindingKind::Opaque,
                            initial: Initial::None,
                            updated: false,
                        },
                    );
                }
            }
            Ok(())
        }
    }
}

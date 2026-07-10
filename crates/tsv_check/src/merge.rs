//! The single-threaded global merge — tsgo's `initializeChecker` merge sequence,
//! ported for the merge-path family (TS2397 / TS2664 / TS2649 / TS2671 and the
//! cross-declaration-space TS2300 / TS2451 / TS2567).
//!
//! Each file's bind produces a program-independent [`FileMerge`] product; this
//! phase folds them into one global scope by tsgo's rules. The phase order is
//! lifted verbatim from `initializeChecker` (checker.go:1296), **not** rediscovered:
//!
//! 1. regular locals of each **script** (non-external) file merge into the globals
//!    table (`mergeGlobalSymbol`), preceded by the per-file `globalThis` check;
//! 2. **global** (`declare global`) augmentations merge their exports into globals;
//! 3. the `undefined` redeclaration check (`addUndefinedToGlobalsOrErrorOnRedeclaration`);
//! 4. global **ambient-module** declarations (deferred — they may need global
//!    symbols resolved; tsgo regression #2953) merge last among the globals;
//! 5. non-global **module augmentations** (`declare module "X"`) resolve + merge.
//!
//! Iteration is **deterministic** (file order, then declaration order) — never a
//! hash-map's iteration order (the grimoire-recorded tsgo determinism landmine).
//!
//! **Single-file P1 scope.** With no lib bound (S5) the globals table starts
//! empty, so `mergeGlobalSymbol` on a single file's locals never finds a prior
//! symbol to conflict with — the cross-space TS2300/2451/2567 path
//! ([`report_merge_symbol_error`]) is exercised only by a **multi-file** program
//! (two scripts sharing global scope, or two `declare global` blocks). It is a
//! genuine, unit-tested port here so it is correct the moment a second file (or a
//! lib) lands. Module resolution is likewise trivial single-file: an augmentation
//! resolves iff an ambient module of that name exists in the same file, which for
//! an external-module file is never (every string-literal module in one is itself
//! an augmentation) — so a single-file augmentation is **always** "not found"
//! (TS2664). The resolves-to-a-non-module errors (TS2649 / TS2671) need a
//! multi-file resolution target and are structurally unreachable at single-file
//! P1; their machinery is noted at the site, not emitted.
//!
//! `mergeSymbol`'s member/export **recursion** (merging a `declare global`
//! interface into a lib interface of the same name) is deferred with lib (S5) —
//! P1's globals hold no members to merge into.
//
// tsgo: internal/checker/checker.go initializeChecker (:1296, the phase order),
//       mergeGlobalSymbol (:1386), mergeModuleAugmentation (:1397),
//       addUndefinedToGlobalsOrErrorOnRedeclaration (:1452), mergeSymbol (:14072),
//       reportMergeSymbolError (:14127), addDuplicateDeclarationError (:14158),
//       lookupOrIssueError (:14196), getExcludedSymbolFlags (:14213)

use crate::binder::symbols::SymbolFlags;
use crate::diag::{Category, Diagnostic};
use crate::hash::FxHashMap;
use crate::ids::FileId;
use tsv_lang::Span;

/// tsgo's `InternalSymbolNameDefault`-style reserved global identifiers the merge
/// checks by name.
const NAME_GLOBAL_THIS: &str = "globalThis";
const NAME_UNDEFINED: &str = "undefined";

/// The `Module` composite (tsgo `SymbolFlagsModule`): a namespace/ambient module.
const MODULE_FLAGS: SymbolFlags =
    SymbolFlags(SymbolFlags::VALUE_MODULE.0 | SymbolFlags::NAMESPACE_MODULE.0);

/// tsgo `ast.IsAmbientModuleSymbolName` — a quoted module name (`"X"`), the key of
/// a `declare module "X"` symbol.
fn is_ambient_module_symbol_name(name: &str) -> bool {
    name.starts_with('"') && name.ends_with('"')
}

/// The merge-relevant product of binding one file — program-independent (a C15
/// requirement), fully resolved to owned strings so cross-file names reconcile by
/// value with no shared interner.
pub struct FileMerge {
    /// The file these declarations belong to.
    pub file: FileId,
    /// Whether the file is an external module (its top-level members reach the
    /// module's exports, **not** global scope — so `source_locals` is empty).
    pub is_external: bool,
    /// The source-file locals, in declaration order — the symbols a **script**
    /// contributes to global scope (empty for an external module).
    pub source_locals: Vec<MergeSymbol>,
    /// Each `declare global {}` augmentation's exports (its members merge into
    /// globals), in source order.
    pub global_augmentations: Vec<Vec<MergeSymbol>>,
    /// Non-global `declare module "X"` augmentations, in source order (deduped by
    /// name in [`merge_program`], matching tsgo's first-declaration-only merge).
    pub module_augmentations: Vec<ModuleAug>,
}

/// One symbol exposed to the merge: its accumulated flags, resolved name, and its
/// declarations (each pointing a diagnostic at a name span).
pub struct MergeSymbol {
    /// The resolved symbol-table key (identifier text).
    pub name: String,
    /// The accumulated classification flags.
    pub flags: SymbolFlags,
    /// The declarations that formed this symbol, in declaration order.
    pub decls: Vec<MergeDecl>,
}

/// One declaration of a [`MergeSymbol`], carrying its owning file so a cross-file
/// conflict can point at declarations in either file.
#[derive(Clone)]
pub struct MergeDecl {
    /// The file the declaration lives in.
    pub file: FileId,
    /// The span a diagnostic points at (the declaration name).
    pub error_span: Span,
    /// tsgo `IsTypeDeclaration` (class / interface / enum / type-alias /
    /// type-parameter) — the `undefined` check skips these.
    pub is_type_decl: bool,
}

/// A non-global `declare module "X"` augmentation: the unquoted module name (the
/// `{0}` argument) and the string-literal span a TS2664 points at.
pub struct ModuleAug {
    /// The file the augmentation lives in.
    pub file: FileId,
    /// The unquoted module name (`"M"` → `M`).
    pub name: String,
    /// The string-literal span (points at the opening quote).
    pub name_span: Span,
}

/// A live global-scope symbol accumulated across files.
struct GlobalEntry {
    name: String,
    flags: SymbolFlags,
    decls: Vec<MergeDecl>,
}

/// The merge phase's diagnostic sink.
///
/// tsgo's `lookupOrIssueError` keys its dedup on the **full** `CompareDiagnostics`
/// (related-info length is a sort key), so a primary that has already accreted
/// related info is never found again — every conflicting merge issues a *fresh*
/// primary carrying its own leading TS6203, and the caller's final
/// `compact_and_merge_related_infos` unions the related infos across the duplicate
/// primaries at each node (the one-primary-per-node, all-6203, uncapped result).
/// So the merge just pushes fresh primaries; there is no issued-index map here.
struct MergeOut {
    diags: Vec<Diagnostic>,
}

impl MergeOut {
    fn new() -> MergeOut {
        MergeOut { diags: Vec::new() }
    }

    /// Push a diagnostic.
    fn push(&mut self, diag: Diagnostic) {
        self.diags.push(diag);
    }
}

/// Run the global merge across a program's per-file bind products, returning the
/// merge diagnostics (unsorted — the caller concatenates and canonically sorts).
#[must_use]
pub fn merge_program(files: &[FileMerge]) -> Vec<Diagnostic> {
    let mut out = MergeOut::new();
    let mut globals: FxHashMap<String, GlobalEntry> = FxHashMap::default();

    // Ambient-module-name Module symbols (`declare module "X"` in a script) are
    // deferred from phase 1 to the post-global-type phase — they may need other
    // global symbols/types resolved first (tsgo regression #2953).
    let mut deferred_ambient: Vec<&MergeSymbol> = Vec::new();

    // --- Phase 1: script locals + the globalThis check (file order) ---
    for file in files {
        if file.is_external {
            continue;
        }
        // The globalThis check runs over the file's own locals, before merging.
        if let Some(sym) = file.source_locals.iter().find(|s| s.name == NAME_GLOBAL_THIS) {
            for decl in &sym.decls {
                out.push(conflict_2397(decl, NAME_GLOBAL_THIS));
            }
        }
        for sym in &file.source_locals {
            if sym.flags.intersects(MODULE_FLAGS) && is_ambient_module_symbol_name(&sym.name) {
                deferred_ambient.push(sym);
            } else {
                merge_global_symbol(&mut globals, sym, &mut out);
            }
        }
    }

    // --- Phase 2: global (`declare global`) augmentations ---
    for file in files {
        for aug in &file.global_augmentations {
            for sym in aug {
                merge_global_symbol(&mut globals, sym, &mut out);
            }
        }
    }

    // --- Phase 3: the `undefined` redeclaration check ---
    // tsgo seeds `c.globals["undefined"]` with the builtin `undefinedSymbol`; with
    // no lib (S5) globals["undefined"] is present iff a file declared it, so a
    // present entry is exactly the redeclaration case.
    if let Some(entry) = globals.get(NAME_UNDEFINED) {
        for decl in &entry.decls {
            if !decl.is_type_decl {
                out.push(conflict_2397(decl, NAME_UNDEFINED));
            }
        }
    }

    // --- Phase 4: global ambient-module declarations (deferred) ---
    // tsgo merges these past global-type creation (regression #2953). A script's
    // `declare module "X"` merges into globals here; a conflict needs another
    // globals symbol of the same quoted name (multi-file or lib), so at single-file
    // scope it merges into empty globals with no diagnostic.
    for sym in deferred_ambient {
        merge_global_symbol(&mut globals, sym, &mut out);
    }

    // --- Phase 5: non-global module augmentations (`declare module "X"`) ---
    for file in files {
        // Dedup by name within the file (tsgo merges only a symbol's first
        // declaration; same-name augmentations share one symbol).
        let mut seen: Vec<&str> = Vec::new();
        for aug in &file.module_augmentations {
            if seen.contains(&aug.name.as_str()) {
                continue;
            }
            seen.push(&aug.name);
            merge_module_augmentation(aug, &mut out);
        }
    }

    out.diags
}

/// tsgo `mergeGlobalSymbol` — merge one symbol into the globals table, reporting a
/// cross-declaration-space conflict when the flags exclude each other.
fn merge_global_symbol(
    globals: &mut FxHashMap<String, GlobalEntry>,
    source: &MergeSymbol,
    out: &mut MergeOut,
) {
    match globals.get_mut(&source.name) {
        Some(target) => merge_symbol(target, source, out),
        None => {
            globals.insert(
                source.name.clone(),
                GlobalEntry {
                    name: source.name.clone(),
                    flags: source.flags,
                    decls: source.decls.clone(),
                },
            );
        }
    }
}

/// tsgo `mergeSymbol` (the merge/conflict decision). No member/export recursion at
/// P1 (globals hold no members — see the module header).
fn merge_symbol(target: &mut GlobalEntry, source: &MergeSymbol, out: &mut MergeOut) {
    if !target.flags.intersects(excluded_symbol_flags(source.flags)) {
        // No conflict: accumulate flags + declarations.
        target.flags.insert(source.flags);
        target.decls.extend(source.decls.iter().cloned());
    } else if target.flags.intersects(SymbolFlags::NAMESPACE_MODULE) {
        // A value merging into a non-instantiated namespace: "cannot augment module
        // with value exports" (TS2649) — but NOT when the target is the built-in
        // `globalThis` (tsgo `mergeSymbol`'s `target != globalThisSymbol` guard):
        // the phase-1 TS2397 already reports that conflict, and a second TS2649
        // would not make sense.
        if target.name != NAME_GLOBAL_THIS
            && let Some(decl) = source.decls.first()
        {
            out.push(augment_error(decl.file, decl.error_span, 2649, &target.name));
        }
    } else {
        report_merge_symbol_error(target, source, out);
    }
}

/// tsgo `reportMergeSymbolError` — the same three-way message selection as the
/// bind-time cascade, emitting on **every** declaration of both symbols with
/// related info, deduped through [`MergeOut::lookup_or_issue`].
fn report_merge_symbol_error(target: &GlobalEntry, source: &MergeSymbol, out: &mut MergeOut) {
    let is_either_enum =
        target.flags.intersects(SymbolFlags::ENUM) || source.flags.intersects(SymbolFlags::ENUM);
    let is_either_block = target.flags.intersects(SymbolFlags::BLOCK_SCOPED_VARIABLE)
        || source.flags.intersects(SymbolFlags::BLOCK_SCOPED_VARIABLE);
    let code = if is_either_enum {
        2567
    } else if is_either_block {
        2451
    } else {
        2300
    };
    let symbol_name = source.name.clone();
    add_dup_errors(&source.decls, code, &symbol_name, &target.decls, out);
    add_dup_errors(&target.decls, code, &symbol_name, &source.decls, out);
}

/// tsgo `addDuplicateDeclarationErrorsForSymbols` — one call to
/// [`add_duplicate_declaration_error`] per declaration node of `decls`, each
/// carrying related info pointing at the *other* symbol's declarations.
fn add_dup_errors(
    decls: &[MergeDecl],
    code: u32,
    symbol_name: &str,
    related_nodes: &[MergeDecl],
    out: &mut MergeOut,
) {
    for decl in decls {
        add_duplicate_declaration_error(decl, code, symbol_name, related_nodes, out);
    }
}

/// tsgo `addDuplicateDeclarationError`: issue a **fresh** primary at `decl` and
/// attach its related info — leading (TS6203) for the first related node, follow-on
/// (TS6204) after, capped at 5 *within this primary* and deduped by target node.
///
/// Every conflicting merge issues a fresh primary (tsgo's `lookupOrIssueError`
/// never re-finds a primary that has accreted related info — related-length is a
/// `CompareDiagnostics` sort key), so the cross-merge union of related info across
/// duplicate primaries at one node is left to the caller's final
/// `compact_and_merge_related_infos`. That union is uncapped and all-TS6203 (each
/// primary's related loop starts empty, so each leads with a TS6203).
fn add_duplicate_declaration_error(
    decl: &MergeDecl,
    code: u32,
    symbol_name: &str,
    related_nodes: &[MergeDecl],
    out: &mut MergeOut,
) {
    let needs_name = code != 2567;
    let args = if needs_name { vec![symbol_name.to_string()] } else { Vec::new() };
    let mut primary = Diagnostic {
        file: Some(decl.file),
        span: decl.error_span,
        code,
        category: Category::Error,
        message: message_for(code, Some(symbol_name)),
        args,
        chain: Vec::new(),
        related: Vec::new(),
    };
    for related in related_nodes {
        if related.file == decl.file && related.error_span == decl.error_span {
            continue;
        }
        if primary.related.len() >= 5
            || primary
                .related
                .iter()
                .any(|r| r.file == Some(related.file) && r.span == related.error_span)
        {
            continue;
        }
        let related_diag = if primary.related.is_empty() {
            related_info(related, 6203, Some(symbol_name))
        } else {
            related_info(related, 6204, None)
        };
        primary.related.push(related_diag);
    }
    out.push(primary);
}

/// tsgo `mergeModuleAugmentation` (the non-global arm) at single-file scope: the
/// augmentation's module name never resolves (no sibling module), so it is always
/// "not found" (TS2664). The resolves-to-a-non-module errors (TS2649 / TS2671)
/// need a multi-file resolution target and are unreachable here.
fn merge_module_augmentation(aug: &ModuleAug, out: &mut MergeOut) {
    out.push(augment_error(aug.file, aug.name_span, 2664, &aug.name));
}

/// Build a TS2397 ("declaration name conflicts with built-in global identifier").
fn conflict_2397(decl: &MergeDecl, name: &str) -> Diagnostic {
    Diagnostic {
        file: Some(decl.file),
        span: decl.error_span,
        code: 2397,
        category: Category::Error,
        message: message_for(2397, Some(name)),
        args: vec![name.to_string()],
        chain: Vec::new(),
        related: Vec::new(),
    }
}

/// Build a module-augmentation error (TS2664 / TS2649 / TS2671), all `{0}` = the
/// module name.
fn augment_error(file: FileId, span: Span, code: u32, name: &str) -> Diagnostic {
    Diagnostic {
        file: Some(file),
        span,
        code,
        category: Category::Error,
        message: message_for(code, Some(name)),
        args: vec![name.to_string()],
        chain: Vec::new(),
        related: Vec::new(),
    }
}

/// Build a related-info node (TS6203 / TS6204) pointing at `decl`'s name.
fn related_info(decl: &MergeDecl, code: u32, name: Option<&str>) -> Diagnostic {
    Diagnostic {
        file: Some(decl.file),
        span: decl.error_span,
        code,
        // 6203/6204 are `Message` category (unobservable in code+span grading;
        // faithful to tsgo's diagnosticMessages).
        category: Category::Message,
        message: message_for(code, name),
        args: name.map(|n| vec![n.to_string()]).unwrap_or_default(),
        chain: Vec::new(),
        related: Vec::new(),
    }
}

/// tsgo `getExcludedSymbolFlags` — the union of the `*Excludes` masks for every
/// flag set on `flags` (with the `ReplaceableByMethod` special case).
fn excluded_symbol_flags(flags: SymbolFlags) -> SymbolFlags {
    let mut result = SymbolFlags::NONE;
    let add = |result: &mut SymbolFlags, present: SymbolFlags, mask: SymbolFlags| {
        if flags.intersects(present) {
            *result = result.union(mask);
        }
    };
    add(&mut result, SymbolFlags::BLOCK_SCOPED_VARIABLE, SymbolFlags::BLOCK_SCOPED_VARIABLE_EXCLUDES);
    add(
        &mut result,
        SymbolFlags::FUNCTION_SCOPED_VARIABLE,
        SymbolFlags::FUNCTION_SCOPED_VARIABLE_EXCLUDES,
    );
    add(&mut result, SymbolFlags::PROPERTY, SymbolFlags::PROPERTY_EXCLUDES);
    add(&mut result, SymbolFlags::ENUM_MEMBER, SymbolFlags::ENUM_MEMBER_EXCLUDES);
    add(&mut result, SymbolFlags::FUNCTION, SymbolFlags::FUNCTION_EXCLUDES);
    add(&mut result, SymbolFlags::CLASS, SymbolFlags::CLASS_EXCLUDES);
    add(&mut result, SymbolFlags::INTERFACE, SymbolFlags::INTERFACE_EXCLUDES);
    add(&mut result, SymbolFlags::REGULAR_ENUM, SymbolFlags::REGULAR_ENUM_EXCLUDES);
    add(&mut result, SymbolFlags::CONST_ENUM, SymbolFlags::CONST_ENUM_EXCLUDES);
    add(&mut result, SymbolFlags::VALUE_MODULE, SymbolFlags::VALUE_MODULE_EXCLUDES);
    add(&mut result, SymbolFlags::METHOD, SymbolFlags::METHOD_EXCLUDES);
    add(&mut result, SymbolFlags::GET_ACCESSOR, SymbolFlags::GET_ACCESSOR_EXCLUDES);
    add(&mut result, SymbolFlags::SET_ACCESSOR, SymbolFlags::SET_ACCESSOR_EXCLUDES);
    add(&mut result, SymbolFlags::TYPE_PARAMETER, SymbolFlags::TYPE_PARAMETER_EXCLUDES);
    add(&mut result, SymbolFlags::TYPE_ALIAS, SymbolFlags::TYPE_ALIAS_EXCLUDES);
    add(&mut result, SymbolFlags::ALIAS, SymbolFlags::ALIAS_EXCLUDES);
    // NamespaceModule contributes no excludes (it merges with anything).
    if flags.intersects(SymbolFlags::REPLACEABLE_BY_METHOD) {
        result = SymbolFlags(result.0 & !SymbolFlags::METHOD.0);
    }
    result
}

/// The `.errors.txt` message text for a merge-path / related-info code.
fn message_for(code: u32, name: Option<&str>) -> String {
    match code {
        2300 => format!("Duplicate identifier '{}'.", name.unwrap_or("")),
        2397 => format!(
            "Declaration name conflicts with built-in global identifier '{}'.",
            name.unwrap_or("")
        ),
        2451 => format!("Cannot redeclare block-scoped variable '{}'.", name.unwrap_or("")),
        2567 => {
            "Enum declarations can only merge with namespace or other enum declarations.".to_string()
        }
        2649 => format!(
            "Cannot augment module '{}' with value exports because it resolves to a non-module entity.",
            name.unwrap_or("")
        ),
        2664 => {
            format!("Invalid module name in augmentation, module '{}' cannot be found.", name.unwrap_or(""))
        }
        2671 => format!(
            "Cannot augment module '{}' because it resolves to a non-module entity.",
            name.unwrap_or("")
        ),
        6203 => format!("'{}' was also declared here.", name.unwrap_or("")),
        6204 => "and here.".to_string(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::diag::sort_and_deduplicate;

    fn decl(file: u32, start: u32, name: &str, is_type_decl: bool) -> MergeDecl {
        MergeDecl {
            file: FileId(file),
            error_span: Span::new(start, start + name.len() as u32),
            is_type_decl,
        }
    }

    fn script(file: u32, locals: Vec<MergeSymbol>) -> FileMerge {
        FileMerge {
            file: FileId(file),
            is_external: false,
            source_locals: locals,
            global_augmentations: Vec::new(),
            module_augmentations: Vec::new(),
        }
    }

    /// Two scripts sharing global scope, each declaring `let x`, conflict across
    /// files (TS2451) — the merge-path analog of the bind-time cascade.
    #[test]
    fn cross_file_block_scoped_conflict_is_2451() {
        let a = script(
            0,
            vec![MergeSymbol {
                name: "x".to_string(),
                flags: SymbolFlags::BLOCK_SCOPED_VARIABLE,
                decls: vec![decl(0, 4, "x", false)],
            }],
        );
        let b = script(
            1,
            vec![MergeSymbol {
                name: "x".to_string(),
                flags: SymbolFlags::BLOCK_SCOPED_VARIABLE,
                decls: vec![decl(1, 4, "x", false)],
            }],
        );
        let diags = merge_program(&[a, b]);
        let codes: Vec<u32> = diags.iter().map(|d| d.code).collect();
        // One TS2451 on each declaration; each carries a TS6203 related info.
        assert_eq!(codes, vec![2451, 2451]);
        assert!(diags.iter().all(|d| d.related.len() == 1 && d.related[0].code == 6203));
        // Emitted on both files (raw order is source-then-target — the canonical
        // sort in `check_program` reorders to path order).
        let mut files: Vec<u32> = diags.iter().filter_map(|d| d.file.map(|f| f.0)).collect();
        files.sort_unstable();
        assert_eq!(files, vec![0, 1]);
    }

    /// A globals conflict where neither side is block-scoped nor enum is TS2300.
    #[test]
    fn cross_file_duplicate_identifier_is_2300() {
        let mk = |file: u32| {
            script(
                file,
                vec![MergeSymbol {
                    name: "C".to_string(),
                    flags: SymbolFlags::CLASS,
                    decls: vec![decl(file, 6, "C", true)],
                }],
            )
        };
        let diags = merge_program(&[mk(0), mk(1)]);
        assert_eq!(diags.iter().map(|d| d.code).collect::<Vec<_>>(), vec![2300, 2300]);
    }

    /// A regular enum and a const enum in separate files can't merge (TS2567).
    #[test]
    fn cross_file_enum_merge_is_2567() {
        let a = script(
            0,
            vec![MergeSymbol {
                name: "E".to_string(),
                flags: SymbolFlags::REGULAR_ENUM,
                decls: vec![decl(0, 5, "E", true)],
            }],
        );
        let b = script(
            1,
            vec![MergeSymbol {
                name: "E".to_string(),
                flags: SymbolFlags::CONST_ENUM,
                decls: vec![decl(1, 11, "E", true)],
            }],
        );
        let diags = merge_program(&[a, b]);
        assert_eq!(diags.iter().map(|d| d.code).collect::<Vec<_>>(), vec![2567, 2567]);
        // 2567 carries no `{0}` argument.
        assert!(diags.iter().all(|d| d.args.is_empty()));
    }

    /// A name conflicting across many files: the merge pushes a fresh primary per
    /// conflicting merge (so the raw pool has duplicates at the first file's node),
    /// and the caller's `sort_and_deduplicate` unions them into one primary per
    /// node. The first file's node accretes a related entry per *other* file — all
    /// **TS6203** (each fresh primary leads with a TS6203), uncapped by the
    /// per-primary cap of 5.
    #[test]
    fn cross_merge_related_union_is_all_6203_uncapped() {
        // Seven files each declaring `let x`. File 0 (globals[x]) is the recurring
        // merge target, so its node accretes six related entries after the union.
        let paths: Vec<String> = (0..7).map(|f| format!("f{f}.ts")).collect();
        let files: Vec<FileMerge> = (0..7)
            .map(|f| {
                script(
                    f,
                    vec![MergeSymbol {
                        name: "x".to_string(),
                        flags: SymbolFlags::BLOCK_SCOPED_VARIABLE,
                        decls: vec![decl(f, 4, "x", false)],
                    }],
                )
            })
            .collect();
        let mut diags = merge_program(&files);
        // Raw pool: every conflicting merge pushes a fresh primary (six merges,
        // each emitting a source-side and a target-side primary = twelve).
        assert_eq!(diags.len(), 12);
        // After the caller's canonical sort + related-info union.
        sort_and_deduplicate(&mut diags, &paths);
        assert_eq!(diags.len(), 7); // one primary per file's node
        let head = &diags[0]; // f0.ts, the recurring target
        assert_eq!(head.file, Some(FileId(0)));
        assert_eq!(head.related.len(), 6); // one per *other* file — uncapped
        assert!(
            head.related.iter().all(|r| r.code == 6203),
            "every unioned related entry leads with TS6203"
        );
    }

    /// The review's prescribed case: a name conflicting across three files whose
    /// declarations sit in distinct files, asserting the related **codes** are all
    /// TS6203 after the union (never a TS6204 on the accreting node).
    #[test]
    fn three_way_cross_file_conflict_related_codes_all_6203() {
        let paths = vec!["a.ts".to_string(), "b.ts".to_string(), "c.ts".to_string()];
        let mk = |f: u32| {
            script(
                f,
                vec![MergeSymbol {
                    name: "C".to_string(),
                    flags: SymbolFlags::CLASS,
                    decls: vec![decl(f, 6, "C", true)],
                }],
            )
        };
        let mut diags = merge_program(&[mk(0), mk(1), mk(2)]);
        sort_and_deduplicate(&mut diags, &paths);
        assert_eq!(diags.len(), 3);
        // All primaries are TS2300; a.ts (the recurring target) carries two related
        // entries, both TS6203 (the union of two fresh single-related primaries).
        assert!(diags.iter().all(|d| d.code == 2300));
        let a = diags.iter().find(|d| d.file == Some(FileId(0))).expect("a.ts primary");
        assert_eq!(a.related.len(), 2);
        assert!(a.related.iter().all(|r| r.code == 6203));
    }

    /// A single script declaring `var globalThis` triggers TS2397 per declaration.
    #[test]
    fn global_this_collision_is_2397() {
        let f = script(
            0,
            vec![MergeSymbol {
                name: "globalThis".to_string(),
                flags: SymbolFlags::FUNCTION_SCOPED_VARIABLE,
                decls: vec![decl(0, 4, "globalThis", false)],
            }],
        );
        let diags = merge_program(&[f]);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, 2397);
        assert_eq!(diags[0].args, vec!["globalThis".to_string()]);
    }

    /// The `undefined` check skips type declarations (class/interface) and fires
    /// only on the value (namespace/var) declaration.
    #[test]
    fn undefined_redeclaration_skips_type_declarations() {
        let f = script(
            0,
            vec![MergeSymbol {
                name: "undefined".to_string(),
                flags: SymbolFlags::CLASS.union(SymbolFlags::VALUE_MODULE),
                decls: vec![
                    decl(0, 6, "undefined", true),  // class
                    decl(0, 40, "undefined", false), // namespace
                ],
            }],
        );
        let diags = merge_program(&[f]);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, 2397);
        assert_eq!(diags[0].span.start, 40);
    }

    /// A module augmentation single-file is always "not found" (TS2664), deduped
    /// by name.
    #[test]
    fn module_augmentation_not_found_is_2664_deduped() {
        let f = FileMerge {
            file: FileId(0),
            is_external: true,
            source_locals: Vec::new(),
            global_augmentations: Vec::new(),
            module_augmentations: vec![
                ModuleAug { file: FileId(0), name: "M".to_string(), name_span: Span::new(22, 25) },
                ModuleAug { file: FileId(0), name: "M".to_string(), name_span: Span::new(50, 53) },
            ],
        };
        let diags = merge_program(&[f]);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, 2664);
        assert_eq!(diags[0].span.start, 22);
        assert_eq!(diags[0].args, vec!["M".to_string()]);
    }

    /// An external module's locals never reach global scope (no globalThis/undefined
    /// check, no global merge).
    #[test]
    fn external_module_locals_do_not_reach_globals() {
        let f = FileMerge {
            file: FileId(0),
            is_external: true,
            source_locals: vec![MergeSymbol {
                name: "globalThis".to_string(),
                flags: SymbolFlags::FUNCTION_SCOPED_VARIABLE,
                decls: vec![decl(0, 4, "globalThis", false)],
            }],
            global_augmentations: Vec::new(),
            module_augmentations: Vec::new(),
        };
        assert!(merge_program(&[f]).is_empty());
    }
}

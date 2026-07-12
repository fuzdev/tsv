//! The symbol bind — tsgo's binder ported for the duplicate/conflict family.
//!
//! A container-threaded walk that declares a symbol for every binding-introducing
//! node into the right table (locals / members / exports), running the
//! `declareSymbolEx` conflict cascade at each — the source of TS2300 (duplicate
//! identifier), TS2451 (block-scoped redeclare), TS2567 (enum-merge), and TS2528
//! (multiple default exports). Statement lists bind **functions-first**
//! (`bindEachStatementFunctionsFirst`), so a hoisted function's symbol is the
//! table's first entry — the reason `let x; var x; function x(){}` reports TS2300
//! (function is first) where `let x; { var x; }` reports TS2451 (the `let` is
//! first).
//!
//! Deliberate simplifications from the full binder, each sound for the family:
//! - the exported-member **dual local+export split** is collapsed to a single
//!   export symbol. tsgo gives an exported module member two symbols
//!   (`declareModuleMember`, binder.go:387-414): an export symbol with the full
//!   flags, and a **local** symbol declared into the container's `locals` with
//!   only `ExportValue` as its *includes* but the **full `symbolExcludes`** mask
//!   (binder.go:409-411). That local half exists **precisely to conflict** — when
//!   an exported member follows a same-name **non-exported** local, the export's
//!   local half (full excludes) collides with the prior plain local in the
//!   `locals` table and issues a duplicate-identifier error. Collapsing to
//!   export-only drops that one collision, but it is sound for the **P1 family**
//!   because the local↔export mixing it would catch surfaces instead as the
//!   check-time **TS2395** ("individual declarations … must be all exported or all
//!   local"), which is out of the bind/merge family; and the functions-first pass
//!   defuses the common function-overload cases before they reach the locals
//!   table. It is sound for the **S4 merge** for a separate reason: the global
//!   merge folds a *script's* `file.Locals` (no dual split — scripts declare
//!   straight into locals with full flags) and `declare global` / augmentation
//!   *exports* (the export halves, full flags), and **never** an external module's
//!   locals (they don't reach global scope), so the merge never reads a
//!   dual-split local half at all.
//! - module instantiation state (`getModuleInstanceState`) is approximated:
//!   specifier-only named exports are treated as non-instantiated rather than
//!   resolving each alias target, and const-enum-only propagation is folded into
//!   the instantiated verdict (a const enum makes a namespace `ValueModule`).
//! - the JS-only `declareSymbolEx` branches (`isReplaceableByMethod`
//!   constructor-vs-prototype discard, and the assignment-merge escape that lets
//!   `SymbolFlagsAssignment` declarations coexist with variables) are deliberately
//!   unported: the tsc conformance corpus this grades is TS-only, so those
//!   JS-expando paths are unreachable. Revisit if a `.js`-flavored suite enters scope.
//!
//! The same-table cascade lands here; the **cross-declaration-space merge** (a
//! script's `file.Locals` folded into global scope, `declare global` and
//! `declare module "X"` augmentations) runs in [`crate::merge`] over the
//! [`FileMerge`] product this bind returns.
//!
//! Split for locality across three files: this module (`mod.rs`) keeps the
//! `SymbolBinder` struct, its lifecycle (`new`/`bind_program`/`finish`), the
//! table/symbol/atom primitives both descendants share, and the member-key
//! resolver; `walk.rs` holds the bind-descent methods (`visit_statement`/
//! `visit_expression` and everything they call into); `declare.rs` holds the
//! `declareSymbolEx` cascade and the container routing. No behavior distinction
//! between the three.

mod declare;
mod walk;

use super::atoms::{Atom, Atoms};
use super::symbols::{Symbol, SymbolFlags, SymbolId, TableId};
use super::{FileFacts, NodeKind, addr_of};
use crate::diag::Diagnostic;
use crate::hash::FxHashMap;
use crate::ids::{FileId, NodeId};
use crate::merge::{FileMerge, MergeDecl, MergeSymbol, ModuleAug};
use string_interner::DefaultStringInterner;
use tsv_lang::Span;
use tsv_ts::ast::Program;
use tsv_ts::ast::internal::{Expression, Identifier, Literal, LiteralValue, ModuleExportName};

/// The container kinds that route member declarations (a subset of tsgo's node
/// kinds, enough to dispatch `declareSymbolAndAddToSymbolTable`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum ContainerKind {
    SourceFile,
    Module,
    Class,
    Enum,
    Interface,
    /// A function-like scope, a type-alias, or any other `HasLocals` container
    /// whose members route to `locals`.
    Locals,
}

/// A live scope in the container stack.
#[derive(Clone, Copy)]
struct Scope {
    kind: ContainerKind,
    /// The container's symbol (owns `members`/`exports`); `None` for a script
    /// source file and for a plain block scope.
    symbol: Option<SymbolId>,
    /// This scope's `locals` table; `None` for a class/enum/interface/members
    /// container (they route through the symbol's tables).
    locals: Option<TableId>,
    /// The source file is an external module (routes exported members).
    is_external_module: bool,
    /// Ambient implicit-export context (a `.d.ts` file / ambient module with no
    /// export declarations); routes non-`export`ed members to `exports`.
    is_export_context: bool,
}

/// A declaration's routing inputs for the cascade.
struct DeclInput {
    /// The final table key (default-forced / mangled by the caller).
    name: Atom,
    /// The display name for the `{0}` message argument.
    display: Atom,
    /// The span a diagnostic points at (the declaration name).
    error_span: Span,
    /// This declaration is a default export (`export default`, forcing the
    /// `"default"` name).
    is_default_export: bool,
    /// This declaration is an `export default <expression>` (tsgo's
    /// `ExportAssignment` with `IsExportEquals == false`) — the other TS2528 case.
    is_export_assignment_default: bool,
    /// This declaration carries an `export` modifier (threaded from the wrapper) —
    /// routes it to the container's `exports` table.
    exported: bool,
    /// The declaration node's best-effort dense id (via the SoA address map).
    node: NodeId,
}

/// The symbol bind for one file.
pub(super) struct SymbolBinder<'a> {
    source: &'a str,
    interner: &'a DefaultStringInterner,
    address_map: &'a FxHashMap<(usize, NodeKind), NodeId>,
    file: FileId,
    is_external: bool,

    atoms: Atoms,
    symbols: Vec<Symbol>,
    tables: Vec<FxHashMap<Atom, SymbolId>>,
    diagnostics: Vec<Diagnostic>,

    container: Scope,
    block_scope: Scope,

    /// The source-file `locals` table — a **script**'s globals-eligible symbols.
    source_file_locals: TableId,
    /// `declare global {}` augmentation symbols (their exports merge into globals).
    global_aug_symbols: Vec<SymbolId>,
    /// Non-global `declare module "X"` augmentations: `(unquoted-name atom, span)`.
    module_augs: Vec<(Atom, Span)>,
}

impl<'a> SymbolBinder<'a> {
    /// Build a binder for one file, seeding the source-file scope.
    pub(super) fn new(
        source: &'a str,
        interner: &'a DefaultStringInterner,
        address_map: &'a FxHashMap<(usize, NodeKind), NodeId>,
        file: FileId,
        facts: FileFacts,
    ) -> SymbolBinder<'a> {
        let is_external_module = matches!(facts.module_ness, super::ModuleNess::Module);
        let mut binder = SymbolBinder {
            source,
            interner,
            address_map,
            file,
            is_external: is_external_module,
            atoms: Atoms::new(),
            symbols: Vec::new(),
            tables: Vec::new(),
            diagnostics: Vec::new(),
            container: Scope {
                kind: ContainerKind::SourceFile,
                symbol: None,
                locals: None,
                is_external_module,
                is_export_context: false,
            },
            block_scope: Scope {
                kind: ContainerKind::SourceFile,
                symbol: None,
                locals: None,
                is_external_module,
                is_export_context: false,
            },
            // Provisional; overwritten with the real source-file locals below.
            source_file_locals: TableId(0),
            global_aug_symbols: Vec::new(),
            module_augs: Vec::new(),
        };
        let locals = binder.new_table();
        binder.source_file_locals = locals;
        let symbol = if is_external_module {
            // The file's own module symbol owns the `exports` table.
            let name = binder.atoms.intern("\"module\"");
            let sid = binder.new_symbol(SymbolFlags::VALUE_MODULE, name);
            let exports = binder.new_table();
            binder.symbols[sid.index()].exports = Some(exports);
            Some(sid)
        } else {
            None
        };
        binder.container.symbol = symbol;
        binder.container.locals = Some(locals);
        binder.block_scope = binder.container;
        binder
    }

    /// Bind the program body, then return.
    pub(super) fn bind_program(&mut self, program: &Program<'a>) {
        self.bind_statement_list(program.body, true);
    }

    /// Finish, returning the collected bind diagnostics and the merge product.
    pub(super) fn finish(self) -> (Vec<Diagnostic>, FileMerge) {
        // A script's source-file locals reach global scope; an external module's
        // do not (its members live in the module's exports).
        let source_locals = if self.is_external {
            Vec::new()
        } else {
            self.resolve_table(self.source_file_locals)
        };
        let global_augmentations = self
            .global_aug_symbols
            .iter()
            .map(|&sid| match self.symbols[sid.index()].exports {
                Some(t) => self.resolve_table(t),
                None => Vec::new(),
            })
            .collect();
        let module_augmentations = self
            .module_augs
            .iter()
            .map(|&(name, span)| ModuleAug {
                file: self.file,
                name: self.atoms.resolve(name).to_string(),
                name_span: span,
            })
            .collect();
        let merge = FileMerge {
            file: self.file,
            is_external: self.is_external,
            source_locals,
            global_augmentations,
            module_augmentations,
        };
        (self.diagnostics, merge)
    }

    /// Resolve a symbol table into merge symbols, in **declaration order** (first
    /// declaration's span start) — deterministic iteration, never the hash-map's.
    fn resolve_table(&self, table: TableId) -> Vec<MergeSymbol> {
        let mut symbols: Vec<MergeSymbol> = self.tables[table.index()]
            .values()
            .map(|&sid| {
                let sym = &self.symbols[sid.index()];
                let decls = sym
                    .decls
                    .iter()
                    .map(|d| MergeDecl {
                        file: self.file,
                        error_span: d.error_span,
                        is_type_decl: d.is_type_decl,
                    })
                    .collect();
                MergeSymbol {
                    name: self.atoms.resolve(sym.name).to_string(),
                    flags: sym.flags,
                    decls,
                }
            })
            .collect();
        symbols.sort_by_key(|s| s.decls.first().map_or(u32::MAX, |d| d.error_span.start));
        symbols
    }

    // --- table / symbol pool -------------------------------------------------

    fn new_table(&mut self) -> TableId {
        let id = TableId(self.tables.len() as u32);
        self.tables.push(FxHashMap::default());
        id
    }

    fn new_symbol(&mut self, flags: SymbolFlags, name: Atom) -> SymbolId {
        let id = SymbolId(self.symbols.len() as u32);
        self.symbols.push(Symbol::new(flags, name));
        id
    }

    /// The `exports` table of `symbol`, created on first use.
    fn exports_of(&mut self, symbol: SymbolId) -> TableId {
        if let Some(t) = self.symbols[symbol.index()].exports {
            return t;
        }
        let t = self.new_table();
        self.symbols[symbol.index()].exports = Some(t);
        t
    }

    /// The `members` table of `symbol`, created on first use.
    fn members_of(&mut self, symbol: SymbolId) -> TableId {
        if let Some(t) = self.symbols[symbol.index()].members {
            return t;
        }
        let t = self.new_table();
        self.symbols[symbol.index()].members = Some(t);
        t
    }

    /// The [`NodeId`] of `node` (of kind `kind`), or [`NodeId::FIRST`] on a miss.
    /// Lenient by design — the result feeds `Decl.node`, which is dead in the
    /// single-file pipeline (statement-level inner structs the SoA walk keys on
    /// the enclosing `&Statement` address fall back to the root id here).
    fn node_id_of<T>(&self, node: &T, kind: NodeKind) -> NodeId {
        self.address_map
            .get(&(addr_of(node), kind))
            .copied()
            .unwrap_or(NodeId::FIRST)
    }

    // --- name resolution -----------------------------------------------------

    fn ident_atom(&mut self, id: &Identifier<'_>) -> Atom {
        let name = id.name(self.source, self.interner);
        self.atoms.intern(name)
    }

    fn string_atom(&mut self, lit: &Literal<'_>) -> Atom {
        match &lit.value {
            LiteralValue::String(cooked) => {
                let s = cooked.resolve(lit.span, self.source);
                self.atoms.intern(s)
            }
            // Non-string literals (numbers etc.) key on their source text.
            _ => {
                let s = lit.span.extract(self.source);
                self.atoms.intern(s)
            }
        }
    }

    fn module_export_name_atom(&mut self, name: &ModuleExportName<'_>) -> (Atom, Span) {
        match name {
            ModuleExportName::Identifier(id) => (self.ident_atom(id), id.name_span()),
            ModuleExportName::Literal(lit) => (self.string_atom(lit), lit.span),
        }
    }

    // --- member keys ---------------------------------------------------------

    fn resolve_member_key(
        &mut self,
        key: &Expression<'a>,
        computed: bool,
        class_symbol: Option<SymbolId>,
    ) -> Option<KeyInfo> {
        if computed {
            // A computed key names a member only for a string/numeric literal.
            return match key {
                Expression::Literal(lit)
                    if matches!(lit.value, LiteralValue::String(_) | LiteralValue::Number(_)) =>
                {
                    // The grouping key stays the decoded/canonical value (so `[0]` and
                    // `['0']` collide). The diagnostic points at the whole `[ … ]` name
                    // node — bracket-inclusive, matching tsgo (`getNameOfDeclaration` ->
                    // the ComputedPropertyName) and the check-pass span, so a key that
                    // conflicts at both phases collapses in the sort/dedup. The display
                    // is that raw bracket-inclusive source (tsgo's `symbolToString`).
                    let key_atom = self.string_atom(lit);
                    let source = self.source;
                    let start = crate::span_scan::bracket_start(source, lit.span.start);
                    let end = crate::span_scan::bracket_end(source, lit.span.end);
                    let display = self.atoms.intern(&source[start as usize..end as usize]);
                    Some(KeyInfo {
                        key: key_atom,
                        display,
                        span: Span::new(start, end),
                    })
                }
                _ => None,
            };
        }
        match key {
            Expression::Identifier(id) => {
                let a = self.ident_atom(id);
                Some(KeyInfo {
                    key: a,
                    display: a,
                    span: id.name_span(),
                })
            }
            Expression::Literal(lit) => {
                let a = self.string_atom(lit);
                Some(KeyInfo {
                    key: a,
                    display: a,
                    span: lit.span,
                })
            }
            Expression::PrivateIdentifier(pid) => {
                let raw = pid.name(self.source, self.interner);
                // The display carries the leading `#`, matching the `#name` span and
                // the check-side form (`duplicate_members.rs`'s `member_key`) — a
                // duplicate reported by BOTH the bind cascade and the check pass shares
                // a code+span but must share this message arg too, or sort/dedup can't
                // collapse the pair (a latent span-multiset extra). tsgo prints
                // `Duplicate identifier '#foo'.` — the `#` is included.
                // TODO: member-key display-string derivation is duplicated across the
                // bind (`sym/`) and check (`duplicate_members.rs`) sides with no
                // shared helper (span derivation is centralized in `span_scan.rs`;
                // display is not) — a shared display helper would prevent this class of
                // mismatch.
                let display = self.atoms.intern(&format!("#{raw}"));
                // Mangle with the class symbol id so same-name privates in one
                // class collide (tsgo GetSymbolNameForPrivateIdentifier). The mangled
                // key keeps the bare `raw` — only `display` gains the `#`.
                let mangled = format!("\u{FE}#{}@{}", class_symbol.map_or(0, |s| s.0), raw);
                let key = self.atoms.intern(&mangled);
                // The diagnostic points at the whole `#name` node (tsgo's
                // `getNameOfDeclaration` -> the PrivateIdentifier), so the squiggle
                // covers the `#` — and `display` now matches that span.
                Some(KeyInfo {
                    key,
                    display,
                    span: pid.span,
                })
            }
            _ => None,
        }
    }
}

/// A resolved member key.
struct KeyInfo {
    key: Atom,
    display: Atom,
    span: Span,
}

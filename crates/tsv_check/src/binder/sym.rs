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
//
// tsgo: internal/binder/binder.go declareSymbolEx (the cascade),
//       declareSymbolAndAddToSymbolTable / declareModuleMember / declareClassMember
//       (the routing), bindEachStatementFunctionsFirst (functions-first),
//       bindClassLikeDeclaration (the static-`prototype` clash, :971)

// The routing methods `.expect()`/`.unwrap()` on `Scope::symbol`/`Scope::locals`
// that the scope *kind* guarantees is `Some` — a class/enum/interface/module
// scope always carries its container symbol, a locals scope always carries its
// table. These are structural invariants of the container stack; a violation is a
// binder bug (contained by the harness's per-test `catch_unwind`), not a
// recoverable data error, so the panic points are the honest expression.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use super::atoms::{Atom, Atoms};
use super::symbols::{Decl, Symbol, SymbolFlags, SymbolId, TableId};
use super::{FileFacts, addr_of};
use crate::diag::{Category, Diagnostic};
use crate::hash::FxHashMap;
use crate::ids::{FileId, NodeId};
use crate::merge::{FileMerge, MergeDecl, MergeSymbol, ModuleAug};
use string_interner::DefaultStringInterner;
use tsv_lang::Span;
use tsv_ts::ast::Program;
use tsv_ts::ast::internal::{
    ClassBody, ClassMember, ExportDefaultValue, ExportSpecifier, Expression, ForInOfLeft, ForInit,
    Identifier, ImportSpecifier, Literal, LiteralValue, MethodKind, ModuleExportName,
    ObjectPatternProperty, Statement, TSEnumMemberId, TSInterfaceBody, TSModuleDeclarationBody,
    TSModuleName, TSTypeElement, TSTypeParameterDeclaration,
};

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

/// Modifiers threaded from an `export` wrapper into the wrapped declaration.
#[derive(Clone, Copy, Default)]
struct DeclMods {
    exported: bool,
    default: bool,
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
    address_map: &'a FxHashMap<usize, NodeId>,
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
        address_map: &'a FxHashMap<usize, NodeId>,
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

    fn node_id_of<T>(&self, node: &T) -> NodeId {
        self.address_map
            .get(&addr_of(node))
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

    // --- the cascade ---------------------------------------------------------

    /// tsgo `declareSymbolEx` — declare `decl` into `table`, running the conflict
    /// cascade, and return the symbol the declaration attached to (a fresh orphan
    /// on conflict, so the table's original symbol keeps accumulating priors).
    fn declare_symbol(
        &mut self,
        table: TableId,
        parent: Option<SymbolId>,
        decl: DeclInput,
        includes: SymbolFlags,
        excludes: SymbolFlags,
    ) -> SymbolId {
        let existing = self.tables[table.index()].get(&decl.name).copied();
        let symbol = match existing {
            None => {
                let sid = self.new_symbol(SymbolFlags::NONE, decl.name);
                self.tables[table.index()].insert(decl.name, sid);
                sid
            }
            Some(sid) => {
                let flags = self.symbols[sid.index()].flags;
                if flags.intersects(excludes) {
                    self.report_conflict(sid, &decl, includes);
                    // Accessor bump: mark the (kept) table symbol a full accessor
                    // so a get/non-accessor/set run all conflict.
                    let sflags = self.symbols[sid.index()].flags;
                    if sflags.intersects(SymbolFlags::ACCESSOR)
                        && (sflags.0 & SymbolFlags::ACCESSOR.0)
                            != (includes.0 & SymbolFlags::ACCESSOR.0)
                    {
                        self.symbols[sid.index()]
                            .flags
                            .insert(SymbolFlags::ACCESSOR);
                    }
                    // A fresh orphan (NOT inserted into the table): this
                    // declaration does not merge into the original, so the
                    // original's declaration list — the priors the cascade points
                    // at — stays fixed.
                    self.new_symbol(SymbolFlags::NONE, decl.name)
                } else {
                    sid
                }
            }
        };
        self.add_declaration(symbol, &decl, includes);
        if self.symbols[symbol.index()].parent.is_none() {
            self.symbols[symbol.index()].parent = parent;
        }
        symbol
    }

    fn add_declaration(&mut self, symbol: SymbolId, decl: &DeclInput, includes: SymbolFlags) {
        let is_type_decl = is_type_declaration(includes);
        let s = &mut self.symbols[symbol.index()];
        s.flags.insert(includes);
        s.decls.push(Decl {
            node: decl.node,
            error_span: decl.error_span,
            display: decl.display,
            is_type_decl,
        });
    }

    /// Emit the duplicate/conflict diagnostics for `decl` against `existing`.
    fn report_conflict(&mut self, existing: SymbolId, decl: &DeclInput, includes: SymbolFlags) {
        let sym_flags = self.symbols[existing.index()].flags;
        let mut code: u32 = if sym_flags.intersects(SymbolFlags::BLOCK_SCOPED_VARIABLE) {
            2451
        } else {
            2300
        };
        let mut needs_name = true;
        if sym_flags.intersects(SymbolFlags::ENUM) || includes.intersects(SymbolFlags::ENUM) {
            code = 2567;
            needs_name = false;
        }
        let mut multiple_default = false;
        if !self.symbols[existing.index()].decls.is_empty()
            && (decl.is_default_export || decl.is_export_assignment_default)
        {
            code = 2528;
            needs_name = false;
            multiple_default = true;
        }

        let new_span = decl.error_span;
        let new_name = if needs_name {
            Some(self.atoms.resolve(decl.display).to_string())
        } else {
            None
        };
        let mut new_diag = self.make_diag(new_span, code, new_name.as_deref());

        let priors: Vec<Decl> = self.symbols[existing.index()].decls.to_vec();
        for (index, pdecl) in priors.iter().enumerate() {
            let pname = if needs_name {
                Some(self.atoms.resolve(pdecl.display).to_string())
            } else {
                None
            };
            let mut d = self.make_diag(pdecl.error_span, code, pname.as_deref());
            if multiple_default {
                let rcode = if index == 0 { 2753 } else { 6204 };
                let r_new = self.make_related(new_span, rcode);
                d.related.push(r_new);
                let r_first = self.make_related(pdecl.error_span, 2752);
                new_diag.related.push(r_first);
            }
            self.diagnostics.push(d);
        }
        self.diagnostics.push(new_diag);
    }

    fn make_diag(&self, span: Span, code: u32, name: Option<&str>) -> Diagnostic {
        let message = message_for(code, name);
        let args = name.map(|n| vec![n.to_string()]).unwrap_or_default();
        Diagnostic {
            file: Some(self.file),
            span,
            code,
            category: Category::Error,
            message,
            args,
            chain: Vec::new(),
            related: Vec::new(),
        }
    }

    fn make_related(&self, span: Span, code: u32) -> Diagnostic {
        Diagnostic {
            file: Some(self.file),
            span,
            code,
            // The two `export default` related codes are `Error` category in tsgo's
            // diagnosticMessages; `and here.` (6204) and the `Did you mean` hint
            // (1369) are `Message`. (Category is unobservable in code+span grading;
            // this stays faithful to the oracle.)
            category: match code {
                2752 | 2753 => Category::Error,
                _ => Category::Message,
            },
            message: message_for(code, None),
            args: Vec::new(),
            chain: Vec::new(),
            related: Vec::new(),
        }
    }

    // --- routing -------------------------------------------------------------

    /// tsgo `declareSymbolAndAddToSymbolTable` — route by the current container.
    fn declare_in_container(
        &mut self,
        decl: DeclInput,
        includes: SymbolFlags,
        excludes: SymbolFlags,
    ) -> SymbolId {
        match self.container.kind {
            ContainerKind::Module => self.declare_module_member(decl, includes, excludes),
            ContainerKind::SourceFile => self.declare_source_file_member(decl, includes, excludes),
            ContainerKind::Class => self.declare_class_member(decl, includes, excludes, false),
            ContainerKind::Enum => {
                let sym = self.container.symbol.expect("enum has a symbol");
                let table = self.exports_of(sym);
                self.declare_symbol(table, Some(sym), decl, includes, excludes)
            }
            ContainerKind::Interface => {
                let sym = self
                    .container
                    .symbol
                    .expect("members container has a symbol");
                let table = self.members_of(sym);
                self.declare_symbol(table, Some(sym), decl, includes, excludes)
            }
            ContainerKind::Locals => {
                let table = self.container.locals.expect("locals container has a table");
                self.declare_symbol(table, None, decl, includes, excludes)
            }
        }
    }

    /// tsgo `bindBlockScopedDeclaration` — route by the current block scope.
    fn declare_block_scoped(
        &mut self,
        decl: DeclInput,
        includes: SymbolFlags,
        excludes: SymbolFlags,
    ) -> SymbolId {
        match self.block_scope.kind {
            ContainerKind::Module => self.declare_module_member(decl, includes, excludes),
            ContainerKind::SourceFile => {
                if self.block_scope.is_external_module {
                    self.declare_module_member(decl, includes, excludes)
                } else {
                    let table = self.block_scope.locals.expect("source file has locals");
                    self.declare_symbol(table, None, decl, includes, excludes)
                }
            }
            _ => {
                let table = self.block_scope.locals.expect("block scope has locals");
                self.declare_symbol(table, None, decl, includes, excludes)
            }
        }
    }

    /// tsgo `declareSourceFileMember`.
    fn declare_source_file_member(
        &mut self,
        decl: DeclInput,
        includes: SymbolFlags,
        excludes: SymbolFlags,
    ) -> SymbolId {
        if self.container.is_external_module {
            self.declare_module_member(decl, includes, excludes)
        } else {
            let table = self.container.locals.expect("source file has locals");
            self.declare_symbol(table, None, decl, includes, excludes)
        }
    }

    /// tsgo `declareModuleMember` — the exported-member routing (dual split
    /// collapsed to the export symbol; see the module doc). Aliases route through
    /// [`Self::declare_alias`] instead, so this handles only value/type members.
    fn declare_module_member(
        &mut self,
        mut decl: DeclInput,
        includes: SymbolFlags,
        excludes: SymbolFlags,
    ) -> SymbolId {
        let to_exports =
            decl.exported || decl.is_default_export || self.container.is_export_context;
        if to_exports {
            let sym = self
                .container
                .symbol
                .expect("module member exports needs a container symbol");
            if decl.is_default_export {
                // A default export forces the `"default"` table key.
                decl.name = self.atoms.default_export();
            }
            let table = self.exports_of(sym);
            self.declare_symbol(table, Some(sym), decl, includes, excludes)
        } else {
            let table = self.container.locals.expect("module member locals");
            self.declare_symbol(table, None, decl, includes, excludes)
        }
    }

    /// tsgo `declareClassMember` — static members to `exports`, else `members`.
    fn declare_class_member(
        &mut self,
        decl: DeclInput,
        includes: SymbolFlags,
        excludes: SymbolFlags,
        is_static: bool,
    ) -> SymbolId {
        let sym = self.container.symbol.expect("class has a symbol");
        let table = if is_static {
            self.exports_of(sym)
        } else {
            self.members_of(sym)
        };
        self.declare_symbol(table, Some(sym), decl, includes, excludes)
    }

    // --- statement lists (functions-first) -----------------------------------

    fn bind_statement_list(&mut self, stmts: &[Statement<'a>], functions_first: bool) {
        if functions_first {
            for stmt in stmts {
                if is_function_statement(stmt) {
                    self.declare_hoisted_function(stmt);
                }
            }
        }
        for stmt in stmts {
            let skip = functions_first && is_function_statement(stmt);
            self.visit_statement(stmt, DeclMods::default(), skip);
        }
    }

    /// Sub-step A: declare a hoisted function's symbol only (no body descent),
    /// unwrapping any `export`/`export default` wrapper for its modifiers.
    fn declare_hoisted_function(&mut self, stmt: &Statement<'a>) {
        match stmt {
            Statement::FunctionDeclaration(f) => {
                if let Some(id) = &f.id {
                    self.bind_function_name(id, f.span, DeclMods::default());
                }
            }
            Statement::TSDeclareFunction(f) => {
                self.bind_function_name(&f.id, f.span, DeclMods::default());
            }
            Statement::ExportNamedDeclaration(e) => {
                if let Some(inner) = e.declaration {
                    self.declare_hoisted_function_inner(
                        inner,
                        DeclMods {
                            exported: true,
                            default: false,
                        },
                    );
                }
            }
            Statement::ExportDefaultDeclaration(e) => {
                let mods = DeclMods {
                    exported: true,
                    default: true,
                };
                match &e.declaration {
                    ExportDefaultValue::FunctionDeclaration(f) => {
                        self.bind_default_function(f.id.as_ref(), e.span, mods);
                    }
                    ExportDefaultValue::TSDeclareFunction(f) => {
                        self.bind_default_function(Some(&f.id), e.span, mods);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn declare_hoisted_function_inner(&mut self, inner: &Statement<'a>, mods: DeclMods) {
        match inner {
            Statement::FunctionDeclaration(f) => {
                if let Some(id) = &f.id {
                    self.bind_function_name(id, f.span, mods);
                }
            }
            Statement::TSDeclareFunction(f) => self.bind_function_name(&f.id, f.span, mods),
            _ => {}
        }
    }

    // --- statements ----------------------------------------------------------

    fn visit_statement(&mut self, stmt: &Statement<'a>, mods: DeclMods, skip_symbol: bool) {
        match stmt {
            Statement::VariableDeclaration(decl) => {
                let (includes, excludes, block_scoped) = var_flags(decl.kind);
                for d in decl.declarations {
                    self.bind_binding(&d.id, includes, excludes, block_scoped, mods, decl.span);
                    if let Some(init) = &d.init {
                        self.visit_expression(init);
                    }
                }
            }
            Statement::FunctionDeclaration(f) => {
                if !skip_symbol && let Some(id) = &f.id {
                    self.bind_function_name(id, f.span, mods);
                }
                self.with_function_scope(f.type_parameters.as_ref(), |b| {
                    b.bind_params(f.params);
                    b.bind_statement_list(f.body.body, true);
                });
            }
            Statement::TSDeclareFunction(f) => {
                if !skip_symbol {
                    self.bind_function_name(&f.id, f.span, mods);
                }
                self.with_function_scope(f.type_parameters.as_ref(), |b| {
                    b.bind_params(f.params);
                });
            }
            Statement::ClassDeclaration(c) => {
                let sym = if skip_symbol {
                    None
                } else {
                    c.id.as_ref().map(|id| {
                        let d = self.decl_from_ident(id, c.span, mods);
                        self.declare_block_scoped(
                            d,
                            SymbolFlags::CLASS,
                            SymbolFlags::CLASS_EXCLUDES,
                        )
                    })
                };
                self.bind_class_body(&c.body, sym, c.type_parameters.as_ref());
            }
            Statement::TSInterfaceDeclaration(i) => {
                let d = self.decl_from_ident(&i.id, i.span, mods);
                let sym = self.declare_block_scoped(
                    d,
                    SymbolFlags::INTERFACE,
                    SymbolFlags::INTERFACE_EXCLUDES,
                );
                self.bind_interface_body(&i.body, sym, i.type_parameters.as_ref());
            }
            Statement::TSEnumDeclaration(e) => {
                let (inc, exc) = if e.r#const {
                    (SymbolFlags::CONST_ENUM, SymbolFlags::CONST_ENUM_EXCLUDES)
                } else {
                    (
                        SymbolFlags::REGULAR_ENUM,
                        SymbolFlags::REGULAR_ENUM_EXCLUDES,
                    )
                };
                let d = self.decl_from_ident(&e.id, e.span, mods);
                let sym = self.declare_block_scoped(d, inc, exc);
                self.bind_enum_members(e.members, sym);
            }
            Statement::TSModuleDeclaration(m) => self.bind_module(m, mods),
            Statement::TSTypeAliasDeclaration(t) => {
                // tsgo's `declareSymbolEx` adds a TS1369 "Did you mean
                // 'export type { T }'?" related info when a conflicting declaration
                // is `export type T;` — a type alias with a *missing* `= type`
                // (binder.go:260). That shape is deliberately unported: tsv's parser
                // rejects `export type T;` ("Expected '='"), so the declaration never
                // reaches this cascade. The sole corpus baseline exercising the hint
                // (`exportDeclaration_missingBraces.ts`) is therefore a tsv
                // parse-rejection, not a gradeable bind.
                let d = self.decl_from_ident(&t.id, t.span, mods);
                self.declare_block_scoped(
                    d,
                    SymbolFlags::TYPE_ALIAS,
                    SymbolFlags::TYPE_ALIAS_EXCLUDES,
                );
                self.bind_type_params_in_new_locals(t.type_parameters.as_ref());
            }
            Statement::ImportDeclaration(imp) => {
                for spec in imp.specifiers {
                    self.bind_import_specifier(spec);
                }
            }
            Statement::TSImportEqualsDeclaration(ie) => {
                let d = self.decl_from_ident(
                    &ie.id,
                    ie.span,
                    DeclMods {
                        exported: ie.is_export,
                        default: false,
                    },
                );
                // An `import =` with an external reference or a plain entity name
                // is an alias either way for the family (locals unless exported).
                let _ = &ie.module_reference;
                self.declare_alias(d, ie.is_export);
            }
            Statement::ExportNamedDeclaration(e) => {
                if let Some(inner) = e.declaration {
                    self.visit_statement(
                        inner,
                        DeclMods {
                            exported: true,
                            default: false,
                        },
                        skip_symbol,
                    );
                } else {
                    for spec in e.specifiers {
                        self.bind_export_specifier(spec);
                    }
                }
            }
            Statement::ExportDefaultDeclaration(e) => self.bind_export_default(e, skip_symbol),
            // Control flow: descend for nested bindings + block scopes.
            Statement::BlockStatement(b) => {
                self.with_block_scope(|bd| bd.bind_statement_list(b.body, true));
            }
            Statement::IfStatement(s) => {
                self.visit_expression(&s.test);
                self.visit_statement(s.consequent, DeclMods::default(), false);
                if let Some(alt) = s.alternate {
                    self.visit_statement(alt, DeclMods::default(), false);
                }
            }
            Statement::ForStatement(s) => self.with_block_scope(|bd| {
                if let Some(init) = &s.init {
                    match init {
                        ForInit::VariableDeclaration(decl) => bd.bind_var_declaration(decl),
                        ForInit::Expression(e) => bd.visit_expression(e),
                    }
                }
                if let Some(t) = &s.test {
                    bd.visit_expression(t);
                }
                if let Some(u) = &s.update {
                    bd.visit_expression(u);
                }
                bd.visit_statement(s.body, DeclMods::default(), false);
            }),
            Statement::ForInStatement(s) => self.with_block_scope(|bd| {
                bd.bind_for_left(&s.left);
                bd.visit_expression(&s.right);
                bd.visit_statement(s.body, DeclMods::default(), false);
            }),
            Statement::ForOfStatement(s) => self.with_block_scope(|bd| {
                bd.bind_for_left(&s.left);
                bd.visit_expression(&s.right);
                bd.visit_statement(s.body, DeclMods::default(), false);
            }),
            Statement::WhileStatement(s) => {
                self.visit_expression(&s.test);
                self.visit_statement(s.body, DeclMods::default(), false);
            }
            Statement::DoWhileStatement(s) => {
                self.visit_statement(s.body, DeclMods::default(), false);
                self.visit_expression(&s.test);
            }
            Statement::SwitchStatement(s) => {
                self.visit_expression(&s.discriminant);
                self.with_block_scope(|bd| {
                    for case in s.cases {
                        if let Some(t) = &case.test {
                            bd.visit_expression(t);
                        }
                        bd.bind_statement_list(case.consequent, false);
                    }
                });
            }
            Statement::TryStatement(s) => {
                self.with_block_scope(|bd| bd.bind_statement_list(s.block.body, true));
                if let Some(h) = &s.handler {
                    // The catch clause is a block scope holding the (block-scoped)
                    // parameter; its body is a *separate* nested block scope, so a
                    // `const e` shadowing `catch(e)` is a check-time TS2492, not a
                    // binder conflict (tsgo `bindVariableDeclarationOrBindingElement`
                    // -> `IsBlockOrCatchScoped`).
                    self.with_block_scope(|bd| {
                        if let Some(param) = &h.param {
                            bd.bind_binding(
                                param,
                                SymbolFlags::BLOCK_SCOPED_VARIABLE,
                                SymbolFlags::BLOCK_SCOPED_VARIABLE_EXCLUDES,
                                true,
                                DeclMods::default(),
                                h.span,
                            );
                        }
                        bd.with_block_scope(|body| body.bind_statement_list(h.body.body, true));
                    });
                }
                if let Some(f) = &s.finalizer {
                    self.with_block_scope(|bd| bd.bind_statement_list(f.body, true));
                }
            }
            Statement::LabeledStatement(s) => {
                self.visit_statement(s.body, DeclMods::default(), false);
            }
            Statement::ReturnStatement(s) => {
                if let Some(a) = &s.argument {
                    self.visit_expression(a);
                }
            }
            Statement::ThrowStatement(s) => self.visit_expression(&s.argument),
            Statement::ExpressionStatement(s) => self.visit_expression(&s.expression),
            Statement::TSExportAssignment(ea) => {
                // `export = x` — tsgo `bindExportAssignment` with `IsExportEquals`:
                // declared into `exports` under the `"export="` name with ALL
                // excludes (self-merge-only), so a second `export =` conflicts.
                if let Some(sym) = self.container.symbol {
                    let name = self.atoms.export_equals();
                    // The name node is the expression when it is a bare identifier
                    // (tsgo `getNonAssignedNameOfDeclaration`), else the whole node.
                    let error_span = match &ea.expression {
                        Expression::Identifier(id) => id.name_span(),
                        _ => ea.span,
                    };
                    let d = DeclInput {
                        name,
                        display: name,
                        error_span,
                        is_default_export: false,
                        is_export_assignment_default: false,
                        exported: true,
                        node: self.node_id_of(ea),
                    };
                    let table = self.exports_of(sym);
                    self.declare_symbol(
                        table,
                        Some(sym),
                        d,
                        SymbolFlags::PROPERTY,
                        SymbolFlags::ALL,
                    );
                }
                self.visit_expression(&ea.expression);
            }
            Statement::ExportAllDeclaration(_)
            | Statement::TSNamespaceExportDeclaration(_)
            | Statement::BreakStatement(_)
            | Statement::ContinueStatement(_)
            | Statement::EmptyStatement(_)
            | Statement::DebuggerStatement(_) => {}
        }
    }

    fn bind_var_declaration(&mut self, decl: &tsv_ts::ast::internal::VariableDeclaration<'a>) {
        let (includes, excludes, block_scoped) = var_flags(decl.kind);
        for d in decl.declarations {
            self.bind_binding(
                &d.id,
                includes,
                excludes,
                block_scoped,
                DeclMods::default(),
                decl.span,
            );
            if let Some(init) = &d.init {
                self.visit_expression(init);
            }
        }
    }

    fn bind_for_left(&mut self, left: &ForInOfLeft<'a>) {
        match left {
            ForInOfLeft::VariableDeclaration(decl) => self.bind_var_declaration(decl),
            ForInOfLeft::Pattern(_) => {}
        }
    }

    // --- export default ------------------------------------------------------

    fn bind_export_default(
        &mut self,
        e: &tsv_ts::ast::internal::ExportDefaultDeclaration<'a>,
        skip_symbol: bool,
    ) {
        let mods = DeclMods {
            exported: true,
            default: true,
        };
        match &e.declaration {
            ExportDefaultValue::Expression(expr) => {
                // tsgo `bindExportAssignment` (non-`export =`): excludes = ALL. An
                // entity-name expression (`export default foo`) is an **alias**
                // (`ExpressionIsAlias`) whose diagnostic points at the name; any
                // other expression (`export default 0`) is a `Property` pointing at
                // the whole `export default` node.
                if let Some(sym) = self.container.symbol {
                    let name = self.atoms.default_export();
                    let is_alias = matches!(
                        expr,
                        Expression::Identifier(_) | Expression::MemberExpression(_)
                    );
                    let flags = if is_alias {
                        SymbolFlags::ALIAS
                    } else {
                        SymbolFlags::PROPERTY
                    };
                    // The name node is the expression only when it is a bare
                    // identifier (tsgo `getNonAssignedNameOfDeclaration`); otherwise
                    // the whole `export default` node.
                    let error_span = match expr {
                        Expression::Identifier(id) => id.name_span(),
                        _ => e.span,
                    };
                    let d = DeclInput {
                        name,
                        display: name,
                        error_span,
                        is_default_export: false,
                        is_export_assignment_default: true,
                        exported: false,
                        node: self.node_id_of(e),
                    };
                    let table = self.exports_of(sym);
                    self.declare_symbol(table, Some(sym), d, flags, SymbolFlags::ALL);
                }
                self.visit_expression(expr);
            }
            ExportDefaultValue::FunctionDeclaration(f) => {
                if !skip_symbol {
                    self.bind_default_function(f.id.as_ref(), e.span, mods);
                }
                self.with_function_scope(f.type_parameters.as_ref(), |b| {
                    b.bind_params(f.params);
                    b.bind_statement_list(f.body.body, true);
                });
            }
            ExportDefaultValue::TSDeclareFunction(f) => {
                if !skip_symbol {
                    self.bind_default_function(Some(&f.id), e.span, mods);
                }
                self.with_function_scope(f.type_parameters.as_ref(), |b| b.bind_params(f.params));
            }
            ExportDefaultValue::ClassDeclaration(c) => {
                let d = self.default_decl(c.id.as_ref(), e.span);
                let sym = self.container.symbol.map(|cs| {
                    let table = self.exports_of(cs);
                    self.declare_symbol(
                        table,
                        Some(cs),
                        d,
                        SymbolFlags::CLASS,
                        SymbolFlags::CLASS_EXCLUDES,
                    )
                });
                self.bind_class_body(&c.body, sym, c.type_parameters.as_ref());
            }
            ExportDefaultValue::TSInterfaceDeclaration(i) => {
                let d = self.default_decl(Some(&i.id), e.span);
                if let Some(cs) = self.container.symbol {
                    let table = self.exports_of(cs);
                    self.declare_symbol(
                        table,
                        Some(cs),
                        d,
                        SymbolFlags::INTERFACE,
                        SymbolFlags::INTERFACE_EXCLUDES,
                    );
                }
                self.bind_interface_body_symbol_less(&i.body, i.type_parameters.as_ref());
            }
        }
    }

    fn default_decl(&mut self, id: Option<&Identifier<'a>>, node_span: Span) -> DeclInput {
        let display = match id {
            Some(i) => {
                let name = i.name(self.source, self.interner);
                self.atoms.intern(name)
            }
            None => self.atoms.default_export(),
        };
        DeclInput {
            name: self.atoms.default_export(),
            display,
            error_span: id.map_or(node_span, Identifier::name_span),
            is_default_export: true,
            is_export_assignment_default: false,
            exported: false,
            node: NodeId::FIRST,
        }
    }

    fn bind_default_function(
        &mut self,
        id: Option<&Identifier<'a>>,
        node_span: Span,
        _mods: DeclMods,
    ) {
        if let Some(cs) = self.container.symbol {
            let d = self.default_decl(id, node_span);
            let table = self.exports_of(cs);
            self.declare_symbol(
                table,
                Some(cs),
                d,
                SymbolFlags::FUNCTION,
                SymbolFlags::FUNCTION_EXCLUDES,
            );
        }
    }

    // --- function names + scopes --------------------------------------------

    fn bind_function_name(&mut self, id: &Identifier<'a>, node_span: Span, mods: DeclMods) {
        let d = self.decl_from_ident(id, node_span, mods);
        self.declare_block_scoped(d, SymbolFlags::FUNCTION, SymbolFlags::FUNCTION_EXCLUDES);
    }

    fn with_function_scope(
        &mut self,
        type_params: Option<&TSTypeParameterDeclaration<'a>>,
        f: impl FnOnce(&mut Self),
    ) {
        let saved = (self.container, self.block_scope);
        let locals = self.new_table();
        let scope = Scope {
            kind: ContainerKind::Locals,
            symbol: None,
            locals: Some(locals),
            is_external_module: false,
            is_export_context: false,
        };
        self.container = scope;
        self.block_scope = scope;
        self.bind_type_params(type_params);
        f(self);
        self.container = saved.0;
        self.block_scope = saved.1;
    }

    fn with_block_scope(&mut self, f: impl FnOnce(&mut Self)) {
        let saved = self.block_scope;
        let locals = self.new_table();
        self.block_scope = Scope {
            kind: ContainerKind::Locals,
            symbol: None,
            locals: Some(locals),
            is_external_module: false,
            is_export_context: false,
        };
        f(self);
        self.block_scope = saved;
    }

    // --- params + bindings ---------------------------------------------------

    fn bind_params(&mut self, params: &[Expression<'a>]) {
        for param in params {
            self.bind_param(param);
        }
    }

    fn bind_param(&mut self, param: &Expression<'a>) {
        match param {
            Expression::TSParameterProperty(pp) => {
                // The inner parameter binds as a parameter; a property-parameter
                // also declares a class member (handled where the constructor's
                // owning class scope is live — the constructor scope's parent).
                self.bind_param(pp.parameter);
            }
            _ => self.bind_binding(
                param,
                SymbolFlags::FUNCTION_SCOPED_VARIABLE,
                SymbolFlags::PARAMETER_EXCLUDES,
                false,
                DeclMods::default(),
                param_span(param),
            ),
        }
    }

    /// Bind a binding target: an identifier leaf routes through the given flags;
    /// object/array patterns recurse; assignment patterns and rest unwrap.
    fn bind_binding(
        &mut self,
        target: &Expression<'a>,
        includes: SymbolFlags,
        excludes: SymbolFlags,
        block_scoped: bool,
        mods: DeclMods,
        node_span: Span,
    ) {
        match target {
            Expression::Identifier(id) => {
                let d = self.decl_from_ident(id, node_span, mods);
                if block_scoped {
                    self.declare_block_scoped(d, includes, excludes);
                } else {
                    self.declare_in_container(d, includes, excludes);
                }
            }
            Expression::ObjectPattern(p) => {
                for prop in p.properties {
                    match prop {
                        ObjectPatternProperty::Property(pr) => {
                            self.bind_binding(
                                &pr.value,
                                includes,
                                excludes,
                                block_scoped,
                                mods,
                                pr.span,
                            );
                        }
                        ObjectPatternProperty::RestElement(r) => {
                            self.bind_binding(
                                r.argument,
                                includes,
                                excludes,
                                block_scoped,
                                mods,
                                r.span,
                            );
                        }
                    }
                }
            }
            Expression::ArrayPattern(p) => {
                for el in p.elements.iter().flatten() {
                    self.bind_binding(el, includes, excludes, block_scoped, mods, el_span(el));
                }
            }
            Expression::AssignmentPattern(a) => {
                self.bind_binding(a.left, includes, excludes, block_scoped, mods, node_span);
                self.visit_expression(a.right);
            }
            Expression::RestElement(r) => {
                self.bind_binding(r.argument, includes, excludes, block_scoped, mods, r.span);
            }
            _ => {}
        }
    }

    fn decl_from_ident(
        &mut self,
        id: &Identifier<'a>,
        _node_span: Span,
        mods: DeclMods,
    ) -> DeclInput {
        let name = self.ident_atom(id);
        DeclInput {
            name,
            display: name,
            error_span: id.name_span(),
            is_default_export: mods.default,
            is_export_assignment_default: false,
            exported: mods.exported,
            node: self.node_id_of(id),
        }
    }

    // --- classes -------------------------------------------------------------

    fn bind_class_body(
        &mut self,
        body: &ClassBody<'a>,
        class_symbol: Option<SymbolId>,
        type_params: Option<&TSTypeParameterDeclaration<'a>>,
    ) {
        let Some(class_symbol) = class_symbol else {
            // Anonymous / skipped class: still descend member values for nested
            // bindings, but no member tables to conflict in.
            self.descend_class_values(body);
            return;
        };
        // The static-`prototype` clash (checker.go:971): a pre-seeded export.
        let proto = self.atoms.intern("prototype");
        let exports = self.exports_of(class_symbol);
        if let Some(existing) = self.tables[exports.index()].get(&proto).copied()
            && let Some(pdecl) = self.symbols[existing.index()].decls.first().copied()
        {
            let name = self.atoms.resolve(pdecl.display).to_string();
            let diag = self.make_diag(pdecl.error_span, 2300, Some(&name));
            self.diagnostics.push(diag);
        }
        let proto_sym = self.new_symbol(SymbolFlags::PROPERTY.union(SymbolFlags::PROTOTYPE), proto);
        self.symbols[proto_sym.index()].parent = Some(class_symbol);
        self.tables[exports.index()].insert(proto, proto_sym);

        let saved = (self.container, self.block_scope);
        let scope = Scope {
            kind: ContainerKind::Class,
            symbol: Some(class_symbol),
            locals: None,
            is_external_module: false,
            is_export_context: false,
        };
        self.container = scope;
        self.block_scope = scope;
        self.bind_type_params(type_params);
        for member in body.body {
            self.bind_class_member(member, class_symbol);
        }
        self.container = saved.0;
        self.block_scope = saved.1;
    }

    fn bind_class_member(&mut self, member: &ClassMember<'a>, class_symbol: SymbolId) {
        match member {
            ClassMember::MethodDefinition(m) => {
                let is_static = m.is_static;
                let (inc, exc) = match m.kind {
                    MethodKind::Constructor => (SymbolFlags::CONSTRUCTOR, SymbolFlags::NONE),
                    MethodKind::Get => (
                        SymbolFlags::GET_ACCESSOR,
                        SymbolFlags::GET_ACCESSOR_EXCLUDES,
                    ),
                    MethodKind::Set => (
                        SymbolFlags::SET_ACCESSOR,
                        SymbolFlags::SET_ACCESSOR_EXCLUDES,
                    ),
                    MethodKind::Method => {
                        let opt = if m.optional {
                            SymbolFlags::OPTIONAL
                        } else {
                            SymbolFlags::NONE
                        };
                        (SymbolFlags::METHOD.union(opt), SymbolFlags::METHOD_EXCLUDES)
                    }
                };
                if let MethodKind::Constructor = m.kind {
                    let d = DeclInput {
                        name: self.atoms.intern("__constructor"),
                        display: self.atoms.intern("__constructor"),
                        error_span: m.span,
                        is_default_export: false,
                        is_export_assignment_default: false,
                        exported: false,
                        node: NodeId::FIRST,
                    };
                    self.declare_class_member(d, inc, exc, is_static);
                    // Bind constructor params (incl. parameter properties -> class members).
                    self.with_function_scope(m.value.type_parameters.as_ref(), |b| {
                        b.bind_constructor_params(m.value.params, class_symbol);
                        b.bind_statement_list(method_body(&m.value), true);
                    });
                } else if let Some(key) =
                    self.resolve_member_key(&m.key, m.computed, Some(class_symbol))
                {
                    let d = DeclInput {
                        name: key.key,
                        display: key.display,
                        error_span: key.span,
                        is_default_export: false,
                        is_export_assignment_default: false,
                        exported: false,
                        node: NodeId::FIRST,
                    };
                    self.declare_class_member(d, inc, exc, is_static);
                    self.with_function_scope(m.value.type_parameters.as_ref(), |b| {
                        b.bind_params(m.value.params);
                        b.bind_statement_list(method_body(&m.value), true);
                    });
                } else {
                    // Dynamic computed key: anonymous member, no conflict; still
                    // descend the value for nested bindings.
                    self.with_function_scope(m.value.type_parameters.as_ref(), |b| {
                        b.bind_params(m.value.params);
                        b.bind_statement_list(method_body(&m.value), true);
                    });
                }
            }
            ClassMember::PropertyDefinition(p) => {
                let (inc, exc) = if p.accessor {
                    (SymbolFlags::ACCESSOR, SymbolFlags::ACCESSOR_EXCLUDES)
                } else {
                    let opt = if p.modifier == tsv_ts::ast::internal::PropertyModifier::Optional {
                        SymbolFlags::OPTIONAL
                    } else {
                        SymbolFlags::NONE
                    };
                    (
                        SymbolFlags::PROPERTY.union(opt),
                        SymbolFlags::PROPERTY_EXCLUDES,
                    )
                };
                if let Some(key) = self.resolve_member_key(&p.key, p.computed, Some(class_symbol)) {
                    let d = DeclInput {
                        name: key.key,
                        display: key.display,
                        error_span: key.span,
                        is_default_export: false,
                        is_export_assignment_default: false,
                        exported: false,
                        node: NodeId::FIRST,
                    };
                    self.declare_class_member(d, inc, exc, p.is_static);
                }
                if let Some(v) = &p.value {
                    self.visit_expression(v);
                }
            }
            ClassMember::StaticBlock(s) => {
                self.with_block_scope(|b| b.bind_statement_list(s.body, true));
            }
            ClassMember::IndexSignature(_) => {}
        }
    }

    fn bind_constructor_params(&mut self, params: &[Expression<'a>], class_symbol: SymbolId) {
        for param in params {
            match param {
                Expression::TSParameterProperty(pp) => {
                    // Bind as a parameter (in the constructor scope)...
                    self.bind_param(pp.parameter);
                    // ...and as a class instance member (tsgo bindParameter).
                    if let Expression::Identifier(id) = ident_of_param(pp.parameter) {
                        let opt = if id.optional {
                            SymbolFlags::OPTIONAL
                        } else {
                            SymbolFlags::NONE
                        };
                        let d = self.decl_from_ident(id, pp.span, DeclMods::default());
                        let table = self.members_of(class_symbol);
                        self.declare_symbol(
                            table,
                            Some(class_symbol),
                            d,
                            SymbolFlags::PROPERTY.union(opt),
                            SymbolFlags::PROPERTY_EXCLUDES,
                        );
                    }
                }
                _ => self.bind_param(param),
            }
        }
    }

    fn descend_class_values(&mut self, body: &ClassBody<'a>) {
        for member in body.body {
            match member {
                ClassMember::MethodDefinition(m) => {
                    self.with_function_scope(m.value.type_parameters.as_ref(), |b| {
                        b.bind_params(m.value.params);
                        b.bind_statement_list(method_body(&m.value), true);
                    });
                }
                ClassMember::PropertyDefinition(p) => {
                    if let Some(v) = &p.value {
                        self.visit_expression(v);
                    }
                }
                ClassMember::StaticBlock(s) => {
                    self.with_block_scope(|b| b.bind_statement_list(s.body, true));
                }
                ClassMember::IndexSignature(_) => {}
            }
        }
    }

    // --- interfaces / enums / modules ---------------------------------------

    fn bind_interface_body(
        &mut self,
        body: &TSInterfaceBody<'a>,
        interface_symbol: SymbolId,
        type_params: Option<&TSTypeParameterDeclaration<'a>>,
    ) {
        let saved = (self.container, self.block_scope);
        let scope = Scope {
            kind: ContainerKind::Interface,
            symbol: Some(interface_symbol),
            locals: None,
            is_external_module: false,
            is_export_context: false,
        };
        self.container = scope;
        self.block_scope = scope;
        self.bind_type_params(type_params);
        for member in body.body {
            self.bind_type_element(member);
        }
        self.container = saved.0;
        self.block_scope = saved.1;
    }

    fn bind_interface_body_symbol_less(
        &self,
        _body: &TSInterfaceBody<'a>,
        _type_params: Option<&TSTypeParameterDeclaration<'a>>,
    ) {
        // `export default interface` with no container symbol: nothing to bind.
    }

    fn bind_type_element(&mut self, element: &TSTypeElement<'a>) {
        let (key_expr, computed, span, inc, exc) = match element {
            TSTypeElement::PropertySignature(p) => (
                &p.key,
                p.computed,
                p.span,
                SymbolFlags::PROPERTY,
                SymbolFlags::PROPERTY_EXCLUDES,
            ),
            TSTypeElement::MethodSignature(m) => (
                &m.key,
                m.computed,
                m.span,
                SymbolFlags::METHOD,
                SymbolFlags::METHOD_EXCLUDES,
            ),
            // Call/construct/index signatures are anonymous (Signature, no conflict).
            TSTypeElement::CallSignature(_)
            | TSTypeElement::ConstructSignature(_)
            | TSTypeElement::IndexSignature(_) => return,
        };
        if let Some(key) = self.resolve_member_key(key_expr, computed, None) {
            let d = DeclInput {
                name: key.key,
                display: key.display,
                error_span: key.span,
                is_default_export: false,
                is_export_assignment_default: false,
                exported: false,
                node: NodeId::FIRST,
            };
            let _ = span;
            self.declare_in_container(d, inc, exc);
        }
    }

    fn bind_enum_members(
        &mut self,
        members: &[tsv_ts::ast::internal::TSEnumMember<'a>],
        enum_symbol: SymbolId,
    ) {
        let saved = (self.container, self.block_scope);
        let scope = Scope {
            kind: ContainerKind::Enum,
            symbol: Some(enum_symbol),
            locals: None,
            is_external_module: false,
            is_export_context: false,
        };
        self.container = scope;
        self.block_scope = scope;
        for member in members {
            let (key, span) = match &member.id {
                TSEnumMemberId::Identifier(id) => (self.ident_atom(id), id.name_span()),
                TSEnumMemberId::String(lit) => (self.string_atom(lit), lit.span),
            };
            let d = DeclInput {
                name: key,
                display: key,
                error_span: span,
                is_default_export: false,
                is_export_assignment_default: false,
                exported: false,
                node: NodeId::FIRST,
            };
            self.declare_in_container(
                d,
                SymbolFlags::ENUM_MEMBER,
                SymbolFlags::ENUM_MEMBER_EXCLUDES,
            );
            if let Some(init) = &member.initializer {
                self.visit_expression(init);
            }
        }
        self.container = saved.0;
        self.block_scope = saved.1;
    }

    fn bind_module(&mut self, m: &tsv_ts::ast::internal::TSModuleDeclaration<'a>, mods: DeclMods) {
        // The module's own symbol (name = identifier, or `"name"` for ambient).
        let (name, display, span) = match &m.id {
            TSModuleName::Identifier(id) => {
                let a = self.ident_atom(id);
                (a, a, id.name_span())
            }
            TSModuleName::Literal(lit) => {
                let raw = lit.span.extract(self.source);
                let key = self.atoms.intern(raw);
                (key, key, lit.span)
            }
        };
        let d = DeclInput {
            name,
            display,
            error_span: span,
            is_default_export: mods.default,
            is_export_assignment_default: false,
            exported: mods.exported,
            node: self.node_id_of(m),
        };
        // Instantiation state (tsgo `GetModuleInstanceState`): a namespace of only
        // types binds as the inert `NamespaceModule`, so it never conflicts with a
        // `var`/`let`/`type` of the same name; one with value content is `ValueModule`.
        let (inc, exc) = if module_instantiated(m) {
            (
                SymbolFlags::VALUE_MODULE,
                SymbolFlags::VALUE_MODULE_EXCLUDES,
            )
        } else {
            (
                SymbolFlags::NAMESPACE_MODULE,
                SymbolFlags::NAMESPACE_MODULE_EXCLUDES,
            )
        };
        let sym = self.declare_block_scoped(d, inc, exc);

        // Record cross-declaration-space augmentations for the merge phase — only
        // top-level ones (container still the source file). `declare global {}` is
        // a global-scope augmentation (its exports merge into globals);
        // `declare module "X"` in an external module is a module augmentation
        // (tsgo `IsModuleAugmentationExternal`, the `KindSourceFile` arm).
        if self.container.kind == ContainerKind::SourceFile {
            if m.global {
                self.global_aug_symbols.push(sym);
            } else if self.is_external
                && let TSModuleName::Literal(lit) = &m.id
            {
                let unquoted = self.string_atom(lit);
                self.module_augs.push((unquoted, lit.span));
            }
        }

        let saved = (self.container, self.block_scope);
        let locals = self.new_table();
        let scope = Scope {
            kind: ContainerKind::Module,
            symbol: Some(sym),
            locals: Some(locals),
            is_external_module: false,
            is_export_context: m.declare,
        };
        self.container = scope;
        self.block_scope = scope;
        self.exports_of(sym);
        match &m.body {
            Some(TSModuleDeclarationBody::TSModuleBlock(block)) => {
                self.bind_statement_list(block.body, true);
            }
            Some(TSModuleDeclarationBody::TSModuleDeclaration(nested)) => {
                self.bind_module(nested, DeclMods::default());
            }
            None => {}
        }
        self.container = saved.0;
        self.block_scope = saved.1;
    }

    // --- imports / exports (aliases) -----------------------------------------

    fn bind_import_specifier(&mut self, spec: &ImportSpecifier<'a>) {
        let id = match spec {
            ImportSpecifier::Default(d) => &d.local,
            ImportSpecifier::Named(n) => &n.local,
            ImportSpecifier::Namespace(n) => &n.local,
        };
        let d = self.decl_from_ident(id, id.span, DeclMods::default());
        self.declare_alias(d, false);
    }

    fn bind_export_specifier(&mut self, spec: &ExportSpecifier<'a>) {
        // An export specifier's *exported* name is the table key in `exports`.
        let (name, span) = self.module_export_name_atom(&spec.exported);
        let is_default = matches!(&spec.exported, ModuleExportName::Identifier(id)
            if id.name(self.source, self.interner) == "default");
        let d = DeclInput {
            name,
            display: name,
            error_span: span,
            is_default_export: is_default,
            is_export_assignment_default: false,
            exported: false,
            node: NodeId::FIRST,
        };
        self.declare_alias(d, true);
    }

    /// Declare an alias symbol (import/export specifier, `import =`). Exported
    /// aliases route to `exports`, others to `locals`.
    fn declare_alias(&mut self, decl: DeclInput, to_exports: bool) {
        match self.container.kind {
            ContainerKind::Module | ContainerKind::SourceFile
                if self.container.symbol.is_some() =>
            {
                if to_exports {
                    let sym = self.container.symbol.unwrap();
                    let mut d = decl;
                    if d.is_default_export {
                        d.name = self.atoms.default_export();
                    }
                    let table = self.exports_of(sym);
                    self.declare_symbol(
                        table,
                        Some(sym),
                        d,
                        SymbolFlags::ALIAS,
                        SymbolFlags::ALIAS_EXCLUDES,
                    );
                } else {
                    let table = self.container.locals.expect("locals for alias");
                    self.declare_symbol(
                        table,
                        None,
                        decl,
                        SymbolFlags::ALIAS,
                        SymbolFlags::ALIAS_EXCLUDES,
                    );
                }
            }
            _ => {
                if let Some(table) = self.container.locals {
                    self.declare_symbol(
                        table,
                        None,
                        decl,
                        SymbolFlags::ALIAS,
                        SymbolFlags::ALIAS_EXCLUDES,
                    );
                }
            }
        }
    }

    // --- type parameters -----------------------------------------------------

    fn bind_type_params(&mut self, type_params: Option<&TSTypeParameterDeclaration<'a>>) {
        if let Some(tp) = type_params {
            for p in tp.params {
                let d = self.decl_from_ident(&p.name, p.span, DeclMods::default());
                self.declare_in_container(
                    d,
                    SymbolFlags::TYPE_PARAMETER,
                    SymbolFlags::TYPE_PARAMETER_EXCLUDES,
                );
            }
        }
    }

    fn bind_type_params_in_new_locals(
        &mut self,
        type_params: Option<&TSTypeParameterDeclaration<'a>>,
    ) {
        if type_params.is_none() {
            return;
        }
        self.with_function_scope(type_params, |_| {});
    }

    // --- expressions (nested scopes) -----------------------------------------

    fn visit_expression(&mut self, expr: &Expression<'a>) {
        use Expression as E;
        match expr {
            E::FunctionExpression(f) => {
                self.with_function_scope(f.type_parameters.as_ref(), |b| {
                    b.bind_params(f.params);
                    b.bind_statement_list(f.body.body, true);
                });
            }
            E::ArrowFunctionExpression(a) => {
                self.with_function_scope(a.type_parameters.as_ref(), |b| {
                    b.bind_params(a.params);
                    match &a.body {
                        tsv_ts::ast::internal::ArrowFunctionBody::Expression(e) => {
                            b.visit_expression(e);
                        }
                        tsv_ts::ast::internal::ArrowFunctionBody::BlockStatement(block) => {
                            b.bind_statement_list(block.body, true);
                        }
                    }
                });
            }
            E::ClassExpression(c) => {
                let sym = c.id.as_ref().map(|_| {
                    let name = self.atoms.intern("__class");
                    self.new_symbol(SymbolFlags::CLASS, name)
                });
                self.bind_class_body(&c.body, sym, c.type_parameters.as_ref());
            }
            E::ParenthesizedExpression(p) => self.visit_expression(p.expression),
            E::UnaryExpression(u) => self.visit_expression(u.argument),
            E::UpdateExpression(u) => self.visit_expression(u.argument),
            E::AwaitExpression(a) => self.visit_expression(a.argument),
            E::YieldExpression(y) => {
                if let Some(a) = y.argument {
                    self.visit_expression(a);
                }
            }
            E::BinaryExpression(b) => {
                self.visit_expression(b.left);
                self.visit_expression(b.right);
            }
            E::AssignmentExpression(a) => {
                self.visit_expression(a.left);
                self.visit_expression(a.right);
            }
            E::ConditionalExpression(c) => {
                self.visit_expression(c.test);
                self.visit_expression(c.consequent);
                self.visit_expression(c.alternate);
            }
            E::SequenceExpression(s) => {
                for e in s.expressions {
                    self.visit_expression(e);
                }
            }
            E::CallExpression(c) => {
                self.visit_expression(c.callee);
                for a in c.arguments {
                    self.visit_expression(a);
                }
            }
            E::NewExpression(n) => {
                self.visit_expression(n.callee);
                for a in n.arguments {
                    self.visit_expression(a);
                }
            }
            E::MemberExpression(m) => {
                self.visit_expression(m.object);
                self.visit_expression(m.property);
            }
            E::TSNonNullExpression(t) => self.visit_expression(t.expression),
            E::TSAsExpression(t) => self.visit_expression(t.expression),
            E::TSSatisfiesExpression(t) => self.visit_expression(t.expression),
            E::TSInstantiationExpression(t) => self.visit_expression(t.expression),
            E::SpreadElement(s) => self.visit_expression(s.argument),
            E::ArrayExpression(a) => {
                for e in a.elements.iter().flatten() {
                    self.visit_expression(e);
                }
            }
            E::ObjectExpression(o) => {
                for p in o.properties {
                    if let tsv_ts::ast::internal::ObjectProperty::Property(pr) = p {
                        self.visit_expression(&pr.value);
                    }
                }
            }
            E::TemplateLiteral(t) => {
                for e in t.expressions {
                    self.visit_expression(e);
                }
            }
            E::TaggedTemplateExpression(t) => {
                self.visit_expression(t.tag);
                for e in t.quasi.expressions {
                    self.visit_expression(e);
                }
            }
            _ => {}
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
                    let a = self.string_atom(lit);
                    Some(KeyInfo {
                        key: a,
                        display: a,
                        span: lit.span,
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
                let display = self.atoms.intern(raw);
                // Mangle with the class symbol id so same-name privates in one
                // class collide (tsgo GetSymbolNameForPrivateIdentifier).
                let mangled = format!("\u{FE}#{}@{}", class_symbol.map_or(0, |s| s.0), raw);
                let key = self.atoms.intern(&mangled);
                // The diagnostic points at the whole `#name` node (tsgo's
                // `getNameOfDeclaration` -> the PrivateIdentifier), so the squiggle
                // covers the `#`.
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

/// A [`SymbolFlags`] triple for a variable declaration kind: `(includes,
/// excludes, block_scoped)`. `block_scoped` selects `bindBlockScopedDeclaration`
/// (block-scope routing) over `declareSymbolAndAddToSymbolTable` (container).
fn var_flags(
    kind: tsv_ts::ast::internal::VariableDeclarationKind,
) -> (SymbolFlags, SymbolFlags, bool) {
    use tsv_ts::ast::internal::VariableDeclarationKind as K;
    match kind {
        // `var` is function-scoped (routes through the container).
        K::Var => (
            SymbolFlags::FUNCTION_SCOPED_VARIABLE,
            SymbolFlags::FUNCTION_SCOPED_VARIABLE_EXCLUDES,
            false,
        ),
        // `let` / `const` / `using` / `await using` are block-scoped.
        K::Let | K::Const | K::Using | K::AwaitUsing => (
            SymbolFlags::BLOCK_SCOPED_VARIABLE,
            SymbolFlags::BLOCK_SCOPED_VARIABLE_EXCLUDES,
            true,
        ),
    }
}

/// Whether a declaration's `includes` flags mark it a *type* declaration (tsgo
/// `IsTypeDeclaration`: class / interface / enum / type-alias / type-parameter) —
/// each of those flag families corresponds one-to-one to a type-declaration node
/// kind. The merge's `undefined`-redeclaration check (TS2397) skips these.
fn is_type_declaration(includes: SymbolFlags) -> bool {
    includes.intersects(SymbolFlags(
        SymbolFlags::CLASS.0
            | SymbolFlags::INTERFACE.0
            | SymbolFlags::ENUM.0
            | SymbolFlags::TYPE_ALIAS.0
            | SymbolFlags::TYPE_PARAMETER.0,
    ))
}

/// Whether a statement is a function declaration (possibly `export`-wrapped) —
/// the set tsgo's `bindEachStatementFunctionsFirst` binds first.
fn is_function_statement(stmt: &Statement<'_>) -> bool {
    match stmt {
        Statement::FunctionDeclaration(_) | Statement::TSDeclareFunction(_) => true,
        Statement::ExportNamedDeclaration(e) => e.declaration.is_some_and(|inner| {
            matches!(
                inner,
                Statement::FunctionDeclaration(_) | Statement::TSDeclareFunction(_)
            )
        }),
        Statement::ExportDefaultDeclaration(e) => matches!(
            e.declaration,
            ExportDefaultValue::FunctionDeclaration(_) | ExportDefaultValue::TSDeclareFunction(_)
        ),
        _ => false,
    }
}

/// The span a bare parameter expression points a diagnostic at.
fn param_span(param: &Expression<'_>) -> Span {
    match param {
        Expression::Identifier(id) => id.name_span(),
        _ => param.span(),
    }
}

/// The span an array-pattern element points a diagnostic at.
fn el_span(el: &Expression<'_>) -> Span {
    match el {
        Expression::Identifier(id) => id.name_span(),
        _ => el.span(),
    }
}

/// The binding identifier of a parameter, unwrapping a default (`AssignmentPattern`).
fn ident_of_param<'b, 'a>(param: &'b Expression<'a>) -> &'b Expression<'a> {
    match param {
        Expression::AssignmentPattern(a) => a.left,
        other => other,
    }
}

/// A method's body statements (a `FunctionExpression`'s block body).
fn method_body<'a>(f: &tsv_ts::ast::internal::FunctionExpression<'a>) -> &'a [Statement<'a>] {
    f.body.body
}

/// Whether a namespace/module is instantiated (a `ValueModule`) — a faithful-
/// enough port of tsgo's `getModuleInstanceState`. A module is *non*-instantiated
/// (an inert `NamespaceModule`) only when its whole body is types: interfaces,
/// type aliases, non-exported imports, uninstantiated nested namespaces, and
/// specifier-only named exports (approximated as non-instantiated). Any value
/// content — a var/function/class/enum, an `export =`/`export default`, an
/// expression — makes it instantiated.
fn module_instantiated(m: &tsv_ts::ast::internal::TSModuleDeclaration<'_>) -> bool {
    match &m.body {
        None => true,
        Some(TSModuleDeclarationBody::TSModuleDeclaration(nested)) => module_instantiated(nested),
        Some(TSModuleDeclarationBody::TSModuleBlock(block)) => {
            !block.body.iter().all(statement_is_non_instantiated)
        }
    }
}

/// Whether a module-body statement contributes no value (tsgo
/// `getModuleInstanceStateWorker`).
fn statement_is_non_instantiated(stmt: &Statement<'_>) -> bool {
    match stmt {
        Statement::TSInterfaceDeclaration(_) | Statement::TSTypeAliasDeclaration(_) => true,
        Statement::ImportDeclaration(_) => true,
        Statement::TSImportEqualsDeclaration(ie) => !ie.is_export,
        Statement::TSModuleDeclaration(nested) => !module_instantiated(nested),
        // `export interface`/`export type` wrap a non-instantiated declaration;
        // specifier-only named exports are approximated non-instantiated.
        Statement::ExportNamedDeclaration(e) => match e.declaration {
            Some(inner) => statement_is_non_instantiated(inner),
            None => true,
        },
        _ => false,
    }
}

/// The `.errors.txt` message text for a family / related-info code.
fn message_for(code: u32, name: Option<&str>) -> String {
    match code {
        2300 => format!("Duplicate identifier '{}'.", name.unwrap_or("")),
        2451 => format!(
            "Cannot redeclare block-scoped variable '{}'.",
            name.unwrap_or("")
        ),
        2567 => "Enum declarations can only merge with namespace or other enum declarations."
            .to_string(),
        2528 => "A module cannot have multiple default exports.".to_string(),
        2752 => "The first export default is here.".to_string(),
        2753 => "Another export default is here.".to_string(),
        6204 => "and here.".to_string(),
        _ => String::new(),
    }
}

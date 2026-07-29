//! The declare/conflict cascade — tsgo's `declareSymbolEx` port: declare a
//! symbol into the right table (locals/members/exports) via the container-kind
//! routing (`declareSymbolAndAddToSymbolTable`/`declareModuleMember`/
//! `declareClassMember`/`bindBlockScopedDeclaration`), running the conflict
//! cascade (TS2300/2451/2567/2528) at each, plus alias declarations
//! (`declare_alias`, for import/export specifiers and `import =`).
//
// tsgo: internal/binder/binder.go declareSymbolEx (the cascade),
//       declareSymbolAndAddToSymbolTable / declareModuleMember / declareClassMember
//       (the routing)

// The routing methods `.expect()`/`.unwrap()` on `Scope::symbol`/`Scope::locals`
// that the scope *kind* guarantees is `Some` — a class/enum/interface/module
// scope always carries its container symbol, a locals scope always carries its
// table. These are structural invariants of the container stack; a violation is a
// binder bug (contained by the harness's per-test `catch_unwind`), not a
// recoverable data error, so the panic points are the honest expression.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use super::super::symbols::{Decl, SymbolFlags, SymbolId, TableId};
use crate::diag::{Category, Diagnostic};
use tsv_lang::Span;

use super::{ContainerKind, DeclInput, SymbolBinder};

impl<'a> SymbolBinder<'a> {
    // --- the cascade ---------------------------------------------------------

    /// tsgo `declareSymbolEx` — declare `decl` into `table`, running the conflict
    /// cascade, and return the symbol the declaration attached to (a fresh orphan
    /// on conflict, so the table's original symbol keeps accumulating priors).
    pub(super) fn declare_symbol(
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

    pub(super) fn make_diag(&self, span: Span, code: u32, name: Option<&str>) -> Diagnostic {
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
    pub(super) fn declare_in_container(
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
    pub(super) fn declare_block_scoped(
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
    pub(super) fn declare_class_member(
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

    // --- imports / exports (aliases) -----------------------------------------

    /// Declare an alias symbol (import/export specifier, `import =`). Exported
    /// aliases route to `exports`, others to `locals`.
    pub(super) fn declare_alias(&mut self, decl: DeclInput, to_exports: bool) {
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

//! The fused lower+bind walk (skeleton).
//!
//! One pre-order walk per file assigns dense pre-order [`NodeId`]s to the node
//! kinds the checker addresses — for now statements, the declarations nested in
//! them, and declared-name identifiers — records each node's parent, kind, and
//! span in struct-of-arrays side columns, computes the pre-order `subtree_end`
//! interval (so "is X a descendant of Y" is an O(1) id-range test), and maps
//! each node's arena address to its id. This is tsc's own architecture made
//! eager: tsc's binder is a single walk that stamps parents and lazily mints
//! per-node ids into flat link arrays; we assign the ids eagerly in the same
//! walk (unobservable, and it makes every column dense from the start). Symbol
//! tables and bind diagnostics are later slices; this walk returns none.
//!
//! **Borrow-only discipline (load-bearing).** Every visitor takes `&'arena`
//! references and NEVER clones an AST node. The address map keys on
//! `std::ptr::from_ref(node) as usize` (a safe reference-to-integer cast — the
//! crate keeps `unsafe_code = "forbid"`), and arena addresses are stable for the
//! program's lifetime, which is exactly the checking scope. Every tsv AST type
//! derives `Clone`, so one accidental `.clone()` in a visitor would mint a
//! differently-addressed copy and silently break the map. Nothing type-level
//! enforces this — the discipline is the contract.
//
// tsgo: internal/binder/binder.go bindSourceFile / bindChildren / bindEachChild
//       (single-walk parent stamping; tsgo stamps in the parser, we stamp here —
//       a recorded deviation with identical results)

use crate::diag::Diagnostic;
use crate::hash::FxHashMap;
use crate::ids::{FileId, NodeId};
use tsv_lang::Span;
use tsv_ts::ast::internal::{ForInOfLeft, ForInit};
use tsv_ts::ast::{Expression, Program, Statement, VariableDeclaration, VariableDeclarator};

/// The pre-order node kinds the skeleton walk assigns. Mirrors the tsv_ts
/// `Statement` variants plus the program root and declared-name identifiers;
/// the set grows as checks address more node kinds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u16)]
pub enum NodeKind {
    /// The source file root.
    Program,
    /// A declared-name identifier (a binding), or a `break`/`continue`/label id.
    Identifier,
    ExpressionStatement,
    VariableDeclaration,
    VariableDeclarator,
    FunctionDeclaration,
    ClassDeclaration,
    TSTypeAliasDeclaration,
    TSInterfaceDeclaration,
    TSDeclareFunction,
    TSEnumDeclaration,
    TSModuleDeclaration,
    ImportDeclaration,
    TSImportEqualsDeclaration,
    ExportNamedDeclaration,
    ExportDefaultDeclaration,
    ExportAllDeclaration,
    TSExportAssignment,
    TSNamespaceExportDeclaration,
    ReturnStatement,
    BlockStatement,
    IfStatement,
    ForStatement,
    ForInStatement,
    ForOfStatement,
    WhileStatement,
    DoWhileStatement,
    SwitchStatement,
    TryStatement,
    ThrowStatement,
    BreakStatement,
    ContinueStatement,
    LabeledStatement,
    EmptyStatement,
    DebuggerStatement,
}

/// Whether a file is an external module (has a top-level `import`/`export`) — the
/// tsgo `externalModuleIndicator`, derived post-parse. Nothing consumes it yet;
/// recorded so the future binder's module-vs-script gating has the fact ready.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ModuleNess {
    /// Has a top-level `import`/`export` (or `export =` / `import =`).
    Module,
    /// No module indicator at the top level.
    Script,
}

/// Per-file facts filled at lower+bind (reached O(1) from any node in the file).
#[derive(Clone, Copy, Debug)]
pub struct FileFacts {
    /// Module-vs-script indicator (see [`ModuleNess`]).
    pub module_ness: ModuleNess,
}

/// The product of binding one file: the pre-order node columns, the
/// address->id map, per-file facts, and (empty for now) bind diagnostics.
pub struct BoundFile {
    /// The file these nodes belong to.
    pub file: FileId,
    /// Total nodes assigned (the program root plus every visited node).
    pub node_count: u32,
    /// Parent id per node (`None` for the root), indexed by `NodeId::index`.
    pub parents: Vec<Option<NodeId>>,
    /// Node kind per node.
    pub kinds: Vec<NodeKind>,
    /// Node span per node.
    pub spans: Vec<Span>,
    /// The last id in each node's pre-order subtree (self for a leaf) — makes
    /// descendant tests an O(1) id-range check.
    pub subtree_end: Vec<NodeId>,
    /// Node arena address -> id (the random-access escape hatch).
    pub address_map: FxHashMap<usize, NodeId>,
    /// Bind diagnostics — empty at this slice.
    pub diagnostics: Vec<Diagnostic>,
    /// Per-file facts.
    pub facts: FileFacts,
}

impl BoundFile {
    /// Whether node `descendant` lies in node `ancestor`'s pre-order subtree —
    /// an O(1) id-interval test enabled by pre-order ids + `subtree_end`.
    #[must_use]
    pub fn is_descendant_of(&self, descendant: NodeId, ancestor: NodeId) -> bool {
        ancestor < descendant && descendant <= self.subtree_end[ancestor.index()]
    }
}

/// Derive a file's [`ModuleNess`] from its top-level statements (import/export
/// presence). A cheap body scan — no bind state needed.
#[must_use]
pub fn module_ness(program: &Program<'_>) -> ModuleNess {
    for stmt in program.body {
        if matches!(
            stmt,
            Statement::ImportDeclaration(_)
                | Statement::TSImportEqualsDeclaration(_)
                | Statement::ExportNamedDeclaration(_)
                | Statement::ExportDefaultDeclaration(_)
                | Statement::ExportAllDeclaration(_)
                | Statement::TSExportAssignment(_)
                | Statement::TSNamespaceExportDeclaration(_)
        ) {
            return ModuleNess::Module;
        }
    }
    ModuleNess::Script
}

/// Bind one file: run the fused lower+bind walk and return its [`BoundFile`].
#[must_use]
pub fn bind_file<'arena>(program: &'arena Program<'arena>, file: FileId) -> BoundFile {
    let mut binder = Binder::default();
    let root = binder.add(NodeKind::Program, program.span, None, addr_of(program));
    for stmt in program.body {
        binder.visit_statement(stmt, root);
    }
    binder.close(root);
    BoundFile {
        file,
        node_count: binder.kinds.len() as u32,
        parents: binder.parents,
        kinds: binder.kinds,
        spans: binder.spans,
        subtree_end: binder.subtree_end,
        address_map: binder.address_map,
        diagnostics: Vec::new(),
        facts: FileFacts { module_ness: module_ness(program) },
    }
}

/// The address key of an arena node: a reference-to-integer cast (safe — no
/// dereference, so `unsafe_code = "forbid"` holds). Stable for the arena's life.
#[inline]
fn addr_of<T>(node: &T) -> usize {
    std::ptr::from_ref(node) as usize
}

/// The mutable SoA columns being filled by the walk.
#[derive(Default)]
struct Binder {
    parents: Vec<Option<NodeId>>,
    kinds: Vec<NodeKind>,
    spans: Vec<Span>,
    subtree_end: Vec<NodeId>,
    address_map: FxHashMap<usize, NodeId>,
}

impl Binder {
    /// Assign the next pre-order id to a node, recording its columns and address.
    fn add(
        &mut self,
        kind: NodeKind,
        span: Span,
        parent: Option<NodeId>,
        address: usize,
    ) -> NodeId {
        let id = NodeId::from_index(self.kinds.len());
        self.parents.push(parent);
        self.kinds.push(kind);
        self.spans.push(span);
        self.subtree_end.push(id); // provisional (a leaf owns only itself)
        self.address_map.insert(address, id);
        id
    }

    /// Close a node after its children are visited: its subtree spans every id
    /// assigned since (the current maximum).
    fn close(&mut self, id: NodeId) {
        let last = NodeId::from_index(self.kinds.len() - 1);
        self.subtree_end[id.index()] = last;
    }

    /// Visit a statement: assign its id, then descend into the declarations and
    /// nested statements the skeleton tracks.
    fn visit_statement(&mut self, stmt: &Statement<'_>, parent: NodeId) {
        let id = self.add(statement_kind(stmt), stmt.span(), Some(parent), addr_of(stmt));
        match stmt {
            Statement::VariableDeclaration(decl) => self.visit_declarators(decl, id),
            Statement::FunctionDeclaration(func) => {
                if let Some(name) = &func.id {
                    self.visit_identifier(name, id);
                }
                self.visit_statements(func.body.body, id);
            }
            Statement::ClassDeclaration(class) => {
                if let Some(name) = &class.id {
                    self.visit_identifier(name, id);
                }
            }
            Statement::BlockStatement(block) => self.visit_statements(block.body, id),
            Statement::IfStatement(if_stmt) => {
                self.visit_statement(if_stmt.consequent, id);
                if let Some(alt) = if_stmt.alternate {
                    self.visit_statement(alt, id);
                }
            }
            Statement::ForStatement(for_stmt) => {
                if let Some(ForInit::VariableDeclaration(decl)) = &for_stmt.init {
                    self.visit_variable_declaration(decl, id);
                }
                self.visit_statement(for_stmt.body, id);
            }
            Statement::ForInStatement(for_in) => {
                self.visit_for_left(&for_in.left, id);
                self.visit_statement(for_in.body, id);
            }
            Statement::ForOfStatement(for_of) => {
                self.visit_for_left(&for_of.left, id);
                self.visit_statement(for_of.body, id);
            }
            Statement::WhileStatement(while_stmt) => self.visit_statement(while_stmt.body, id),
            Statement::DoWhileStatement(do_while) => self.visit_statement(do_while.body, id),
            Statement::SwitchStatement(switch) => {
                for case in switch.cases {
                    self.visit_statements(case.consequent, id);
                }
            }
            Statement::TryStatement(try_stmt) => {
                self.visit_statements(try_stmt.block.body, id);
                if let Some(handler) = &try_stmt.handler {
                    self.visit_statements(handler.body.body, id);
                }
                if let Some(finalizer) = &try_stmt.finalizer {
                    self.visit_statements(finalizer.body, id);
                }
            }
            Statement::LabeledStatement(labeled) => {
                self.visit_identifier(&labeled.label, id);
                self.visit_statement(labeled.body, id);
            }
            // The remaining statement kinds carry no nested statement or
            // declared-name node the skeleton tracks yet (expressions, types,
            // module items, throw/return arguments): assigned an id, no descent.
            Statement::ExpressionStatement(_)
            | Statement::TSTypeAliasDeclaration(_)
            | Statement::TSInterfaceDeclaration(_)
            | Statement::TSDeclareFunction(_)
            | Statement::TSEnumDeclaration(_)
            | Statement::TSModuleDeclaration(_)
            | Statement::ReturnStatement(_)
            | Statement::ExportNamedDeclaration(_)
            | Statement::ExportDefaultDeclaration(_)
            | Statement::ExportAllDeclaration(_)
            | Statement::TSExportAssignment(_)
            | Statement::TSNamespaceExportDeclaration(_)
            | Statement::ImportDeclaration(_)
            | Statement::TSImportEqualsDeclaration(_)
            | Statement::ThrowStatement(_)
            | Statement::BreakStatement(_)
            | Statement::ContinueStatement(_)
            | Statement::EmptyStatement(_)
            | Statement::DebuggerStatement(_) => {}
        }
        self.close(id);
    }

    /// Visit a slice of statements under a parent.
    fn visit_statements(&mut self, stmts: &[Statement<'_>], parent: NodeId) {
        for stmt in stmts {
            self.visit_statement(stmt, parent);
        }
    }

    /// Visit a `VariableDeclaration` node (used by a `for` init clause, which is
    /// not itself a `Statement`).
    fn visit_variable_declaration(&mut self, decl: &VariableDeclaration<'_>, parent: NodeId) {
        let id = self.add(NodeKind::VariableDeclaration, decl.span, Some(parent), addr_of(decl));
        self.visit_declarators(decl, id);
        self.close(id);
    }

    /// Visit the declarators of a `VariableDeclaration` (already-assigned parent).
    fn visit_declarators(&mut self, decl: &VariableDeclaration<'_>, parent: NodeId) {
        for declarator in decl.declarations {
            self.visit_declarator(declarator, parent);
        }
    }

    /// Visit one declarator, recording its declared-name identifier when the
    /// binding is a plain identifier (destructuring patterns are a later slice).
    fn visit_declarator(&mut self, declarator: &VariableDeclarator<'_>, parent: NodeId) {
        let id = self.add(
            NodeKind::VariableDeclarator,
            declarator.span,
            Some(parent),
            addr_of(declarator),
        );
        if let Expression::Identifier(name) = &declarator.id {
            self.visit_identifier(name, id);
        }
        self.close(id);
    }

    /// Visit a `for-in`/`for-of` left-hand side (its declarator id when it is a
    /// variable declaration).
    fn visit_for_left(&mut self, left: &ForInOfLeft<'_>, parent: NodeId) {
        if let ForInOfLeft::VariableDeclaration(decl) = left {
            self.visit_variable_declaration(decl, parent);
        }
    }

    /// Visit a declared-name identifier (a leaf).
    fn visit_identifier(&mut self, ident: &tsv_ts::ast::Identifier<'_>, parent: NodeId) {
        let id = self.add(NodeKind::Identifier, ident.span, Some(parent), addr_of(ident));
        self.close(id);
    }
}

/// The [`NodeKind`] for a statement variant.
fn statement_kind(stmt: &Statement<'_>) -> NodeKind {
    match stmt {
        Statement::ExpressionStatement(_) => NodeKind::ExpressionStatement,
        Statement::VariableDeclaration(_) => NodeKind::VariableDeclaration,
        Statement::TSTypeAliasDeclaration(_) => NodeKind::TSTypeAliasDeclaration,
        Statement::TSInterfaceDeclaration(_) => NodeKind::TSInterfaceDeclaration,
        Statement::TSDeclareFunction(_) => NodeKind::TSDeclareFunction,
        Statement::TSEnumDeclaration(_) => NodeKind::TSEnumDeclaration,
        Statement::TSModuleDeclaration(_) => NodeKind::TSModuleDeclaration,
        Statement::ReturnStatement(_) => NodeKind::ReturnStatement,
        Statement::BlockStatement(_) => NodeKind::BlockStatement,
        Statement::FunctionDeclaration(_) => NodeKind::FunctionDeclaration,
        Statement::ClassDeclaration(_) => NodeKind::ClassDeclaration,
        Statement::ExportNamedDeclaration(_) => NodeKind::ExportNamedDeclaration,
        Statement::ExportDefaultDeclaration(_) => NodeKind::ExportDefaultDeclaration,
        Statement::ExportAllDeclaration(_) => NodeKind::ExportAllDeclaration,
        Statement::TSExportAssignment(_) => NodeKind::TSExportAssignment,
        Statement::TSNamespaceExportDeclaration(_) => NodeKind::TSNamespaceExportDeclaration,
        Statement::ImportDeclaration(_) => NodeKind::ImportDeclaration,
        Statement::TSImportEqualsDeclaration(_) => NodeKind::TSImportEqualsDeclaration,
        Statement::IfStatement(_) => NodeKind::IfStatement,
        Statement::ForStatement(_) => NodeKind::ForStatement,
        Statement::ForInStatement(_) => NodeKind::ForInStatement,
        Statement::ForOfStatement(_) => NodeKind::ForOfStatement,
        Statement::WhileStatement(_) => NodeKind::WhileStatement,
        Statement::DoWhileStatement(_) => NodeKind::DoWhileStatement,
        Statement::SwitchStatement(_) => NodeKind::SwitchStatement,
        Statement::TryStatement(_) => NodeKind::TryStatement,
        Statement::ThrowStatement(_) => NodeKind::ThrowStatement,
        Statement::BreakStatement(_) => NodeKind::BreakStatement,
        Statement::ContinueStatement(_) => NodeKind::ContinueStatement,
        Statement::LabeledStatement(_) => NodeKind::LabeledStatement,
        Statement::EmptyStatement(_) => NodeKind::EmptyStatement,
        Statement::DebuggerStatement(_) => NodeKind::DebuggerStatement,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bumpalo::Bump;

    fn bind(source: &str) -> BoundFile {
        let arena = Bump::new();
        let program = tsv_ts::parse(source, &arena).expect("parse");
        bind_file(&program, FileId::ROOT)
    }

    #[test]
    fn preorder_ids_parents_and_kinds() {
        // Program(1) -> VariableDeclaration(2) -> VariableDeclarator(3) -> Identifier(4)
        let bound = bind("const x = 1;");
        assert_eq!(bound.node_count, 4);
        assert_eq!(bound.kinds[0], NodeKind::Program);
        assert_eq!(bound.kinds[1], NodeKind::VariableDeclaration);
        assert_eq!(bound.kinds[2], NodeKind::VariableDeclarator);
        assert_eq!(bound.kinds[3], NodeKind::Identifier);
        assert_eq!(bound.parents[0], None);
        assert_eq!(bound.parents[1], Some(NodeId::FIRST)); // parent is Program
        assert_eq!(bound.parents[3], Some(NodeId::from_index(2))); // Identifier under declarator
    }

    #[test]
    fn subtree_end_enables_descendant_test() {
        let bound = bind("const x = 1;");
        let root = NodeId::FIRST;
        let ident = NodeId::from_index(3);
        let decl = NodeId::from_index(1);
        // Whole program's subtree ends at the identifier.
        assert_eq!(bound.subtree_end[root.index()], ident);
        assert!(bound.is_descendant_of(ident, root));
        assert!(bound.is_descendant_of(ident, decl));
        assert!(!bound.is_descendant_of(root, ident));
        assert!(!bound.is_descendant_of(decl, ident));
    }

    #[test]
    fn address_map_resolves_a_statement() {
        let arena = Bump::new();
        let program = tsv_ts::parse("let a = 1; let b = 2;", &arena).expect("parse");
        let bound = bind_file(&program, FileId::ROOT);
        // The second top-level statement's address resolves to its id.
        let second = &program.body[1];
        let addr = std::ptr::from_ref(second) as usize;
        let id = bound.address_map.get(&addr).copied().expect("mapped");
        assert_eq!(bound.kinds[id.index()], NodeKind::VariableDeclaration);
    }

    #[test]
    fn nested_statements_are_walked() {
        // Program, function decl, its id, and the nested return statement.
        let bound = bind("function f() { return; }");
        assert!(bound.kinds.contains(&NodeKind::FunctionDeclaration));
        assert!(bound.kinds.contains(&NodeKind::ReturnStatement));
        // The return sits inside the function's pre-order subtree.
        let func = NodeId::from_index(1);
        let ret = bound
            .kinds
            .iter()
            .position(|k| *k == NodeKind::ReturnStatement)
            .map(NodeId::from_index)
            .expect("return present");
        assert!(bound.is_descendant_of(ret, func));
    }

    #[test]
    fn module_ness_detects_exports() {
        assert_eq!(bind("export const x = 1;").facts.module_ness, ModuleNess::Module);
        assert_eq!(bind("const x = 1;").facts.module_ness, ModuleNess::Script);
    }
}

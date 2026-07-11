//! The lower+bind pass: node identity (SoA columns) plus the symbol bind.
//!
//! Two cooperating walks run per file, kept in one module:
//!
//! - **the SoA walk** ([`SoaWalk`]) — one **full pre-order descent** assigning a
//!   dense pre-order [`NodeId`] to every AST node the checker addresses:
//!   statements, expressions (including the pattern-shaped ones at
//!   assignment-target / for-left positions), types, and their sub-nodes
//!   (heritage clauses, type parameters, member signatures, decorators, import/
//!   export specifiers, …). It fills the parent/kind/span/`subtree_end` side
//!   columns, the zero-initialized `node_flags` column, and the address→id map.
//!   Pre-order — each parent precedes its contiguous subtree, so the
//!   `subtree_end` interval test (`is X a descendant of Y`) stays valid. Sibling
//!   order follows the traversal, not always source order (an annotated binding
//!   descends its `: T` before the binding), which the interval test does not rely on. The one deliberate carve-out is the pure list-wrapper nodes
//!   (`ClassBody` / `TSInterfaceBody` / a function/try/catch/finally/static body
//!   `BlockStatement` / the `Program.body` slice / the transparent
//!   `TSTypeAnnotation` `: T` wrapper): their members/inner stay flat children of
//!   the owning node, matching today's shape (documented at the sites).
//! - **the symbol bind** ([`sym::SymbolBinder`]) — a container-threaded walk that
//!   ports tsgo's binder: `declareSymbolEx` conflict cascade (TS2300/2451/2567/
//!   2528), the module-member routing, class/enum/interface member tables, and
//!   the **functions-first** statement-list ordering (`bindEachStatementFunctionsFirst`).
//!   It reaches every binding-introducing position and emits the bind-time
//!   duplicate/conflict family.
//!
//! The two are separate passes rather than one fused walk because functions-first
//! symbol binding reorders symbol *creation* within a statement list, which would
//! break the SoA walk's strict pre-order id intervals. The symbol bind resolves a
//! declaration's [`NodeId`] through the SoA walk's address map. Statement-level
//! inner structs it keys on (`TSExportAssignment`, `ExportDefaultDeclaration`,
//! `TSModuleDeclaration`) resolve against the enclosing `&Statement` address, so
//! those `node_id_of` lookups fall back to the root id — the id is not consumed by
//! the family cascade, which keys on name spans, so the fallback is inert. The
//! **strict** resolver flow consumers use is [`BoundFile::require_node_id`], which
//! hard-fails on a miss (a flow graph must never silently drop an attachment).
//!
//! **Borrow-only discipline (load-bearing).** Every visitor takes `&'arena`
//! references and NEVER clones an AST node. The address map keys on
//! `std::ptr::from_ref(node) as usize` (a safe reference-to-integer cast — the
//! crate keeps `unsafe_code = "forbid"`), and arena addresses are stable for the
//! program's lifetime. Every tsv AST type derives `Clone`, so one accidental
//! `.clone()` in a visitor would mint a differently-addressed copy and silently
//! break the map. Nothing type-level enforces this — the discipline is the contract.
//!
//! **No behavior change (F0 invariant).** The SoA columns feed only the symbol
//! bind (which reads the address map) and this module's tests; the graded
//! diagnostic stream carries no `NodeId` (`Diagnostic` has none; the symbol bind's
//! `Decl.node` is dead), and every graded span derives from an AST `.span` /
//! `.name_span()`, never a `spans[node_id]` lookup. So growing the walk — and thus
//! renumbering the ids — cannot perturb graded output.
//
// tsgo: internal/binder/binder.go bindSourceFile / bindChildren / bindContainer
//       (single-walk parent stamping; tsgo stamps in the parser, we stamp here —
//       a recorded deviation with identical results)

mod atoms;
pub mod flow;
mod sym;
pub mod symbols;

use crate::diag::Diagnostic;
use crate::hash::FxHashMap;
use crate::ids::{FileId, NodeId};
use crate::merge::FileMerge;
use tsv_lang::Span;
use tsv_ts::ast::Program;
use tsv_ts::ast::internal::{
    ArrowFunctionBody, CatchClause, ClassBody, ClassDeclaration, ClassMember, Decorator,
    ExportDefaultValue, ExportSpecifier, Expression, ForInOfLeft, ForInit, FunctionDeclaration,
    FunctionExpression, Identifier, ImportAttribute, ImportAttributeKey, ImportSpecifier,
    ModuleExportName, ObjectPatternProperty, ObjectProperty, Property, RestElement, SpreadElement,
    Statement, SwitchCase, TSDeclareFunction, TSEntityName, TSEnumMember, TSEnumMemberId,
    TSImportType, TSIndexSignature, TSInterfaceDeclaration, TSInterfaceHeritage, TSLiteralType,
    TSMappedTypeParameter, TSModuleDeclaration, TSModuleDeclarationBody, TSModuleName,
    TSModuleReference, TSQualifiedName, TSType, TSTypeAnnotation, TSTypeElement, TSTypeParameter,
    TSTypeParameterDeclaration, TSTypeParameterInstantiation, TSTypeQueryExprName, TemplateElement,
    TemplateLiteral, VariableDeclaration, VariableDeclarator,
};

/// The pre-order node kinds the SoA walk assigns — one variant per tsv_ts AST enum
/// variant the walk ids (the program root, then statements, expressions, types, and
/// their sub-nodes). Several kinds are **reused** across positions: `Identifier`
/// tags every identifier — a binding *or* a reference (labels, member/property
/// names, type-param names, entity-name segments, …); `Literal` tags a value
/// literal and a string/number/bigint literal type; `UnaryExpression` a value unary
/// and a negative-number literal type; `TSIndexSignature` both the class-member and
/// type-element index-signature forms; `FunctionExpression` a value function and a
/// method's `value`. The set is not graded or serialized, so its ordering is free.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u16)]
pub enum NodeKind {
    /// The source file root.
    Program,
    /// An identifier — a binding (declaration name, parameter, catch/type-param
    /// name) or a reference (variable use, label, member/property/entity-name
    /// segment). The scope is every identifier the walk reaches, not only bindings.
    Identifier,
    // --- Statements ---
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
    // --- Expressions ---
    Literal,
    PrivateIdentifier,
    ObjectExpression,
    ArrayExpression,
    UnaryExpression,
    UpdateExpression,
    BinaryExpression,
    CallExpression,
    NewExpression,
    MemberExpression,
    ConditionalExpression,
    ArrowFunctionExpression,
    FunctionExpression,
    ClassExpression,
    SpreadElement,
    TemplateLiteral,
    TaggedTemplateExpression,
    AwaitExpression,
    YieldExpression,
    SequenceExpression,
    RegexLiteral,
    ThisExpression,
    Super,
    AssignmentExpression,
    ObjectPattern,
    ArrayPattern,
    AssignmentPattern,
    RestElement,
    TSTypeAssertion,
    TSAsExpression,
    TSSatisfiesExpression,
    TSInstantiationExpression,
    TSNonNullExpression,
    TSParameterProperty,
    ImportExpression,
    MetaProperty,
    JsdocCast,
    ParenthesizedExpression,
    // --- Types ---
    TSKeywordType,
    TSArrayType,
    TSUnionType,
    TSIntersectionType,
    TSTypeReference,
    TSTypeLiteral,
    TSFunctionType,
    TSConstructorType,
    TSTupleType,
    TSParenthesizedType,
    TSTypePredicate,
    TSConditionalType,
    TSMappedType,
    TSTypeOperator,
    TSImportType,
    TSTypeQuery,
    TSIndexedAccessType,
    TSRestType,
    TSOptionalType,
    TSNamedTupleMember,
    TSInferType,
    TSThisType,
    TSTemplateLiteralType,
    // --- Type elements (interface / type-literal members) ---
    TSPropertySignature,
    TSMethodSignature,
    TSCallSignatureDeclaration,
    TSConstructSignatureDeclaration,
    TSIndexSignature,
    // --- Class members ---
    MethodDefinition,
    PropertyDefinition,
    StaticBlock,
    // --- Entity / property / specifier sub-nodes ---
    TSQualifiedName,
    Property,
    ImportDefaultSpecifier,
    ImportNamedSpecifier,
    ImportNamespaceSpecifier,
    ExportSpecifier,
    TSExternalModuleReference,
    TSModuleBlock,
    // --- Plain structs at distinct addressable positions ---
    Decorator,
    TSInterfaceHeritage,
    TSTypeParameterDeclaration,
    TSTypeParameter,
    TSTypeParameterInstantiation,
    CatchClause,
    SwitchCase,
    TemplateElement,
    ImportAttribute,
    TSEnumMember,
    TSMappedTypeParameter,
}

/// Per-node flag bits in the [`BoundFile::node_flags`] column (one `u8` per
/// [`NodeId`]). F0 mints the column zero-initialized and sets nothing; the flow
/// construction pass (F1) sets [`NODE_FLAGS_UNREACHABLE`] during unreachable
/// tagging, and the ambient/context node-identity bits move here later.
#[allow(clippy::identity_op)] // bit 0 — kept in the `1 << N` idiom for the bits F1 adds
pub const NODE_FLAGS_UNREACHABLE: u8 = 1 << 0;

/// Whether a file is an external module — tsgo's `externalModuleIndicator`,
/// derived post-parse (`getExternalModuleIndicator`). Recorded so the binder's
/// module-vs-script member routing has the fact ready.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ModuleNess {
    /// Has a top-level module indicator (`import`/`export`/`export =`/an
    /// `import =` with an external-module reference/`import.meta`).
    Module,
    /// No module indicator.
    Script,
}

/// Per-file facts filled at lower+bind (reached O(1) from any node in the file).
#[derive(Clone, Copy, Debug)]
pub struct FileFacts {
    /// Module-vs-script indicator (see [`ModuleNess`]).
    pub module_ness: ModuleNess,
}

/// The product of binding one file: the pre-order node columns, the address->id
/// map, per-file facts, and the bind diagnostics.
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
    /// Per-node flag byte (see [`NODE_FLAGS_UNREACHABLE`]), one per [`NodeId`],
    /// zero-initialized. F0 sets nothing; the flow pass (F1) writes it.
    pub node_flags: Vec<u8>,
    /// Node arena address -> id (the random-access escape hatch).
    pub address_map: FxHashMap<usize, NodeId>,
    /// Bind diagnostics — the duplicate/conflict family, in emission order (the
    /// caller sorts + dedups across the whole program).
    pub diagnostics: Vec<Diagnostic>,
    /// Per-file facts.
    pub facts: FileFacts,
    /// The program-independent merge product ([`crate::merge`] folds these across
    /// files into the global scope).
    pub merge: FileMerge,
}

impl BoundFile {
    /// Whether node `descendant` lies in node `ancestor`'s pre-order subtree —
    /// an O(1) id-interval test enabled by pre-order ids + `subtree_end`.
    #[must_use]
    pub fn is_descendant_of(&self, descendant: NodeId, ancestor: NodeId) -> bool {
        ancestor < descendant && descendant <= self.subtree_end[ancestor.index()]
    }

    /// The flag byte for `id` (see [`NODE_FLAGS_UNREACHABLE`]).
    #[must_use]
    pub fn node_flags(&self, id: NodeId) -> u8 {
        self.node_flags[id.index()]
    }

    /// Strict address -> [`NodeId`] resolution for flow consumers. Unlike the
    /// symbol bind's lenient `node_id_of` (whose `Decl.node` result is dead), a
    /// flow graph attaches to the node at `address`, so a **miss is a bug** — the
    /// SoA walk failed to cover a node the flow pass reached, and a silent
    /// `NodeId::FIRST` fallback would splice the graph onto the wrong node. So this
    /// hard-fails rather than returning a sentinel.
    ///
    /// `address` is `std::ptr::from_ref(node) as usize` for the same arena node
    /// reference the walk keyed on.
    ///
    /// **Known collision (pre-P3 fix pending).** Pointer-identity keying cannot
    /// distinguish a node from an offset-0 inline struct-typed child at the same
    /// address. The AST has exactly one such pair: `MethodDefinition` and its
    /// inline `value: FunctionExpression` (value at struct offset 0), so
    /// `require_node_id(addr_of(&method))` returns the *value's* id, not the
    /// method's — a silent wrong node, worse than a miss. Inert today (the flow
    /// walk deliberately anchors methods on `value`; nothing else resolves a method
    /// by address), pinned by `method_address_collides_with_value` below. The fix —
    /// keying the map on `(address, NodeKind)` (no same-kind collisions exist) — is
    /// deferred to a pre-P3 slice (P3's method flow-write + typeof narrowing need
    /// the method node's own id).
    #[must_use]
    pub fn require_node_id(&self, address: usize) -> NodeId {
        match self.address_map.get(&address) {
            Some(&id) => id,
            None => node_id_miss(address),
        }
    }
}

/// The `require_node_id` miss path, isolated so its deliberate panic carries the
/// one `#[allow(clippy::panic)]` the crate's restriction-lint posture requires
/// (panic points need an explicit allow + justification). A miss means the SoA
/// walk did not id a node a flow consumer reached — an internal invariant break
/// that must abort, not a recoverable data error.
#[cold]
#[inline(never)]
#[allow(clippy::panic)]
fn node_id_miss(address: usize) -> ! {
    panic!(
        "require_node_id: address {address:#x} not covered by the SoA walk — a flow \
         consumer reached a node the lowering pass did not id (would corrupt the flow graph)"
    );
}

/// Derive a file's [`ModuleNess`] — a faithful port of tsgo's
/// `getExternalModuleIndicator` / `isAnExternalModuleIndicatorNode`: a top-level
/// statement is an indicator when it carries an `export` modifier, is an
/// `import`/`export`/`export =` declaration, or is an `import =` with an external
/// (`require(...)`) module reference; failing that, an `import.meta` anywhere in
/// the file counts. Notably `export as namespace` (a UMD export) does **not**
/// count, and an `import =` with an entity-name reference (`import x = A.B`) does
/// not.
///
/// `source` and the program's interner are unused here (the indicators are all
/// structural); they are accepted for signature symmetry with the binder.
#[must_use]
pub fn module_ness(program: &Program<'_>) -> ModuleNess {
    for stmt in program.body {
        if is_external_module_indicator(stmt) {
            return ModuleNess::Module;
        }
    }
    if program.body.iter().any(stmt_contains_import_meta) {
        return ModuleNess::Module;
    }
    ModuleNess::Script
}

/// tsgo's `isAnExternalModuleIndicatorNode` over one top-level statement.
fn is_external_module_indicator(stmt: &Statement<'_>) -> bool {
    match stmt {
        // `import ...` / `export ... from` / `export {}` / `export *`.
        Statement::ImportDeclaration(_)
        | Statement::ExportNamedDeclaration(_)
        | Statement::ExportAllDeclaration(_)
        // `export = x` and `export default ...` are both `ExportAssignment` in
        // tsgo, both indicators.
        | Statement::TSExportAssignment(_)
        | Statement::ExportDefaultDeclaration(_) => true,
        // `import x = require('y')` counts only with an external-module reference;
        // `import x = A.B` (an entity name) does not.
        Statement::TSImportEqualsDeclaration(decl) => matches!(
            decl.module_reference,
            TSModuleReference::ExternalModuleReference(_)
        ),
        _ => false,
    }
}

/// Whether a statement subtree contains an `import.meta` meta-property (tsgo's
/// `getImportMetaIfNecessary`). A bounded structural walk over the statement and
/// its nested expressions/blocks — `import.meta` is inert for the bind cascade,
/// so this only refines the recorded [`ModuleNess`] fact.
fn stmt_contains_import_meta(stmt: &Statement<'_>) -> bool {
    use Statement as S;
    match stmt {
        S::ExpressionStatement(s) => expr_contains_import_meta(&s.expression),
        S::VariableDeclaration(d) => d
            .declarations
            .iter()
            .any(|decl| decl.init.as_ref().is_some_and(expr_contains_import_meta)),
        S::ReturnStatement(s) => s.argument.as_ref().is_some_and(expr_contains_import_meta),
        S::ThrowStatement(s) => expr_contains_import_meta(&s.argument),
        S::BlockStatement(b) => b.body.iter().any(stmt_contains_import_meta),
        S::IfStatement(s) => {
            expr_contains_import_meta(&s.test)
                || stmt_contains_import_meta(s.consequent)
                || s.alternate.is_some_and(stmt_contains_import_meta)
        }
        S::ForStatement(s) => {
            s.test.as_ref().is_some_and(expr_contains_import_meta)
                || stmt_contains_import_meta(s.body)
        }
        S::ForInStatement(s) => {
            expr_contains_import_meta(&s.right) || stmt_contains_import_meta(s.body)
        }
        S::ForOfStatement(s) => {
            expr_contains_import_meta(&s.right) || stmt_contains_import_meta(s.body)
        }
        S::WhileStatement(s) => {
            expr_contains_import_meta(&s.test) || stmt_contains_import_meta(s.body)
        }
        S::DoWhileStatement(s) => {
            expr_contains_import_meta(&s.test) || stmt_contains_import_meta(s.body)
        }
        S::SwitchStatement(s) => {
            expr_contains_import_meta(&s.discriminant)
                || s.cases.iter().any(|c| {
                    c.test.as_ref().is_some_and(expr_contains_import_meta)
                        || c.consequent.iter().any(stmt_contains_import_meta)
                })
        }
        S::TryStatement(s) => {
            s.block.body.iter().any(stmt_contains_import_meta)
                || s.handler
                    .as_ref()
                    .is_some_and(|h| h.body.body.iter().any(stmt_contains_import_meta))
                || s.finalizer
                    .as_ref()
                    .is_some_and(|f| f.body.iter().any(stmt_contains_import_meta))
        }
        S::LabeledStatement(s) => stmt_contains_import_meta(s.body),
        _ => false,
    }
}

/// Whether an expression subtree contains an `import.meta` meta-property. Covers
/// the common expression positions; deliberately not exhaustive over every type
/// node (types never carry `import.meta`).
fn expr_contains_import_meta(expr: &Expression<'_>) -> bool {
    use Expression as E;
    match expr {
        // `import.meta` vs `new.target`: the only two meta-properties, told apart
        // by the meta keyword's name length (`import` = 6, `new` = 3).
        E::MetaProperty(m) => m.meta.name_len == 6,
        E::ParenthesizedExpression(p) => expr_contains_import_meta(p.expression),
        E::UnaryExpression(u) => expr_contains_import_meta(u.argument),
        E::UpdateExpression(u) => expr_contains_import_meta(u.argument),
        E::AwaitExpression(a) => expr_contains_import_meta(a.argument),
        E::YieldExpression(y) => y.argument.is_some_and(expr_contains_import_meta),
        E::BinaryExpression(b) => {
            expr_contains_import_meta(b.left) || expr_contains_import_meta(b.right)
        }
        E::AssignmentExpression(a) => {
            expr_contains_import_meta(a.left) || expr_contains_import_meta(a.right)
        }
        E::ConditionalExpression(c) => {
            expr_contains_import_meta(c.test)
                || expr_contains_import_meta(c.consequent)
                || expr_contains_import_meta(c.alternate)
        }
        E::SequenceExpression(s) => s.expressions.iter().any(expr_contains_import_meta),
        E::CallExpression(c) => {
            expr_contains_import_meta(c.callee) || c.arguments.iter().any(expr_contains_import_meta)
        }
        E::NewExpression(n) => {
            expr_contains_import_meta(n.callee) || n.arguments.iter().any(expr_contains_import_meta)
        }
        E::MemberExpression(m) => {
            expr_contains_import_meta(m.object) || expr_contains_import_meta(m.property)
        }
        E::TSNonNullExpression(t) => expr_contains_import_meta(t.expression),
        E::TSAsExpression(t) => expr_contains_import_meta(t.expression),
        E::TSSatisfiesExpression(t) => expr_contains_import_meta(t.expression),
        E::ArrayExpression(a) => a.elements.iter().flatten().any(expr_contains_import_meta),
        _ => false,
    }
}

/// Bind one file: run the SoA walk and the symbol bind, returning the [`BoundFile`].
///
/// `source` is the host document the AST spans index into (the binder resolves
/// declared names by slicing it, matching the parser's span-identity names).
#[must_use]
pub fn bind_file<'arena>(
    program: &'arena Program<'arena>,
    source: &str,
    file: FileId,
) -> BoundFile {
    // Pass 1: the SoA node-identity walk (source pre-order).
    let mut walk = SoaWalk::default();
    let root = walk.add(NodeKind::Program, program.span, None, addr_of(program));
    // The `Program.body` slice is a pure list-wrapper: its statements stay flat
    // children of the root (no separate node), matching today's shape.
    for stmt in program.body {
        walk.visit_statement(stmt, root);
    }
    walk.close(root);

    let facts = FileFacts {
        module_ness: module_ness(program),
    };

    // Pass 2: the symbol bind (functions-first, container-threaded).
    let (diagnostics, merge) = {
        let interner = program.interner.borrow();
        let mut binder = sym::SymbolBinder::new(source, &interner, &walk.address_map, file, facts);
        binder.bind_program(program);
        binder.finish()
    };

    BoundFile {
        file,
        node_count: walk.kinds.len() as u32,
        parents: walk.parents,
        kinds: walk.kinds,
        spans: walk.spans,
        subtree_end: walk.subtree_end,
        node_flags: walk.node_flags,
        address_map: walk.address_map,
        diagnostics,
        facts,
        merge,
    }
}

/// The address key of an arena node: a reference-to-integer cast (safe — no
/// dereference, so `unsafe_code = "forbid"` holds). Stable for the arena's life.
#[inline]
pub(crate) fn addr_of<T>(node: &T) -> usize {
    std::ptr::from_ref(node) as usize
}

/// The mutable SoA columns being filled by pass 1.
#[derive(Default)]
struct SoaWalk {
    parents: Vec<Option<NodeId>>,
    kinds: Vec<NodeKind>,
    spans: Vec<Span>,
    subtree_end: Vec<NodeId>,
    node_flags: Vec<u8>,
    address_map: FxHashMap<usize, NodeId>,
}

impl SoaWalk {
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
        self.node_flags.push(0); // F0 mints the column zeroed; F1 sets it
        self.address_map.insert(address, id);
        id
    }

    /// Close a node after its children are visited: its subtree spans every id
    /// assigned since (the current maximum).
    fn close(&mut self, id: NodeId) {
        let last = NodeId::from_index(self.kinds.len() - 1);
        self.subtree_end[id.index()] = last;
    }

    /// Add a leaf node (no children): one `add` immediately followed by `close`.
    fn leaf(&mut self, kind: NodeKind, span: Span, address: usize, parent: NodeId) {
        let id = self.add(kind, span, Some(parent), address);
        self.close(id);
    }

    // --- statements ----------------------------------------------------------

    fn visit_statements(&mut self, stmts: &[Statement<'_>], parent: NodeId) {
        for stmt in stmts {
            self.visit_statement(stmt, parent);
        }
    }

    /// Visit a statement: assign its id (keyed on the `&Statement` address, the key
    /// the symbol bind and the address-map tests use), descend, then close.
    fn visit_statement(&mut self, stmt: &Statement<'_>, parent: NodeId) {
        let id = self.add(
            statement_kind(stmt),
            stmt.span(),
            Some(parent),
            addr_of(stmt),
        );
        match stmt {
            Statement::ExpressionStatement(s) => self.visit_expression(&s.expression, id),
            Statement::VariableDeclaration(decl) => self.visit_declarators(decl, id),
            Statement::FunctionDeclaration(f) => self.descend_function(f, id),
            Statement::ClassDeclaration(c) => self.descend_class(c, id),
            Statement::TSDeclareFunction(f) => self.descend_declare_function(f, id),
            Statement::TSTypeAliasDeclaration(t) => {
                self.visit_identifier(&t.id, id);
                self.visit_type_params(t.type_parameters.as_ref(), id);
                self.visit_type(&t.type_annotation, id);
            }
            Statement::TSInterfaceDeclaration(i) => self.descend_interface(i, id),
            Statement::TSEnumDeclaration(e) => {
                self.visit_identifier(&e.id, id);
                for member in e.members {
                    self.visit_enum_member(member, id);
                }
            }
            Statement::TSModuleDeclaration(m) => self.descend_module(m, id),
            Statement::ImportDeclaration(imp) => {
                for spec in imp.specifiers {
                    self.visit_import_specifier(spec, id);
                }
                self.leaf(NodeKind::Literal, imp.source.span, addr_of(&imp.source), id);
                if let Some(attrs) = imp.attributes {
                    for a in attrs {
                        self.visit_import_attribute(a, id);
                    }
                }
            }
            Statement::TSImportEqualsDeclaration(ie) => {
                self.visit_identifier(&ie.id, id);
                self.visit_module_reference(&ie.module_reference, id);
            }
            Statement::ExportNamedDeclaration(e) => {
                if let Some(inner) = e.declaration {
                    self.visit_statement(inner, id);
                } else {
                    for spec in e.specifiers {
                        self.visit_export_specifier(spec, id);
                    }
                }
                if let Some(src) = &e.source {
                    self.leaf(NodeKind::Literal, src.span, addr_of(src), id);
                }
                if let Some(attrs) = e.attributes {
                    for a in attrs {
                        self.visit_import_attribute(a, id);
                    }
                }
            }
            Statement::ExportDefaultDeclaration(e) => self.visit_export_default(&e.declaration, id),
            Statement::ExportAllDeclaration(e) => {
                if let Some(exp) = &e.exported {
                    self.visit_module_export_name(exp, id);
                }
                self.leaf(NodeKind::Literal, e.source.span, addr_of(&e.source), id);
                if let Some(attrs) = e.attributes {
                    for a in attrs {
                        self.visit_import_attribute(a, id);
                    }
                }
            }
            Statement::TSExportAssignment(ea) => self.visit_expression(&ea.expression, id),
            Statement::TSNamespaceExportDeclaration(n) => self.visit_identifier(&n.id, id),
            Statement::ReturnStatement(s) => {
                if let Some(a) = &s.argument {
                    self.visit_expression(a, id);
                }
            }
            // A function/try/catch/finally body `BlockStatement` is flattened by
            // its owner (a list-wrapper, per today's shape); a *standalone* block
            // statement is its own node whose body follows here.
            Statement::BlockStatement(block) => self.visit_statements(block.body, id),
            Statement::IfStatement(s) => {
                self.visit_expression(&s.test, id);
                self.visit_statement(s.consequent, id);
                if let Some(alt) = s.alternate {
                    self.visit_statement(alt, id);
                }
            }
            Statement::ForStatement(s) => {
                match &s.init {
                    Some(ForInit::VariableDeclaration(decl)) => {
                        self.visit_variable_declaration(decl, id);
                    }
                    Some(ForInit::Expression(e)) => self.visit_expression(e, id),
                    None => {}
                }
                if let Some(t) = &s.test {
                    self.visit_expression(t, id);
                }
                if let Some(u) = &s.update {
                    self.visit_expression(u, id);
                }
                self.visit_statement(s.body, id);
            }
            Statement::ForInStatement(s) => {
                self.visit_for_left(&s.left, id);
                self.visit_expression(&s.right, id);
                self.visit_statement(s.body, id);
            }
            Statement::ForOfStatement(s) => {
                self.visit_for_left(&s.left, id);
                self.visit_expression(&s.right, id);
                self.visit_statement(s.body, id);
            }
            Statement::WhileStatement(s) => {
                self.visit_expression(&s.test, id);
                self.visit_statement(s.body, id);
            }
            Statement::DoWhileStatement(s) => {
                self.visit_statement(s.body, id);
                self.visit_expression(&s.test, id);
            }
            Statement::SwitchStatement(s) => {
                self.visit_expression(&s.discriminant, id);
                for case in s.cases {
                    self.visit_switch_case(case, id);
                }
            }
            Statement::TryStatement(s) => {
                self.visit_statements(s.block.body, id);
                if let Some(handler) = &s.handler {
                    self.visit_catch_clause(handler, id);
                }
                if let Some(finalizer) = &s.finalizer {
                    self.visit_statements(finalizer.body, id);
                }
            }
            Statement::ThrowStatement(s) => self.visit_expression(&s.argument, id),
            Statement::BreakStatement(s) => {
                if let Some(label) = &s.label {
                    self.visit_identifier(label, id);
                }
            }
            Statement::ContinueStatement(s) => {
                if let Some(label) = &s.label {
                    self.visit_identifier(label, id);
                }
            }
            Statement::LabeledStatement(s) => {
                self.visit_identifier(&s.label, id);
                self.visit_statement(s.body, id);
            }
            Statement::EmptyStatement(_) | Statement::DebuggerStatement(_) => {}
        }
        self.close(id);
    }

    // --- declaration descents (shared between statement + export-default) -----

    fn descend_function(&mut self, f: &FunctionDeclaration<'_>, id: NodeId) {
        if let Some(name) = &f.id {
            self.visit_identifier(name, id);
        }
        self.visit_type_params(f.type_parameters.as_ref(), id);
        self.visit_params(f.params, id);
        self.visit_type_annotation_opt(f.return_type.as_ref(), id);
        self.visit_statements(f.body.body, id);
    }

    fn descend_declare_function(&mut self, f: &TSDeclareFunction<'_>, id: NodeId) {
        self.visit_identifier(&f.id, id);
        self.visit_type_params(f.type_parameters.as_ref(), id);
        self.visit_params(f.params, id);
        self.visit_type_annotation_opt(f.return_type.as_ref(), id);
    }

    fn descend_class(&mut self, c: &ClassDeclaration<'_>, id: NodeId) {
        if let Some(name) = &c.id {
            self.visit_identifier(name, id);
        }
        // The class's own `<T>` — kept in sync with the `ClassExpression` arm in
        // `visit_expression` (guarded by the `require_node_id` coverage test).
        self.visit_type_params(c.type_parameters.as_ref(), id);
        self.visit_class_heritage(
            c.decorators,
            c.super_class,
            c.super_type_parameters.as_ref(),
            c.implements,
            id,
        );
        self.visit_class_body(&c.body, id);
    }

    fn descend_interface(&mut self, i: &TSInterfaceDeclaration<'_>, id: NodeId) {
        self.visit_identifier(&i.id, id);
        self.visit_type_params(i.type_parameters.as_ref(), id);
        self.visit_heritages(i.extends, id);
        // `TSInterfaceBody` is a list-wrapper: its members stay flat under the
        // interface (no separate node), matching today's shape.
        self.visit_type_elements(i.body.body, id);
    }

    fn visit_export_default(&mut self, value: &ExportDefaultValue<'_>, parent: NodeId) {
        match value {
            ExportDefaultValue::Expression(e) => self.visit_expression(e, parent),
            ExportDefaultValue::FunctionDeclaration(f) => {
                let id = self.add(
                    NodeKind::FunctionDeclaration,
                    f.span,
                    Some(parent),
                    addr_of(f),
                );
                self.descend_function(f, id);
                self.close(id);
            }
            ExportDefaultValue::TSDeclareFunction(f) => {
                let id = self.add(
                    NodeKind::TSDeclareFunction,
                    f.span,
                    Some(parent),
                    addr_of(f),
                );
                self.descend_declare_function(f, id);
                self.close(id);
            }
            ExportDefaultValue::ClassDeclaration(c) => {
                let id = self.add(NodeKind::ClassDeclaration, c.span, Some(parent), addr_of(c));
                self.descend_class(c, id);
                self.close(id);
            }
            ExportDefaultValue::TSInterfaceDeclaration(i) => {
                let id = self.add(
                    NodeKind::TSInterfaceDeclaration,
                    i.span,
                    Some(parent),
                    addr_of(i),
                );
                self.descend_interface(i, id);
                self.close(id);
            }
        }
    }

    // --- variable declarations / for headers ---------------------------------

    fn visit_variable_declaration(&mut self, decl: &VariableDeclaration<'_>, parent: NodeId) {
        let id = self.add(
            NodeKind::VariableDeclaration,
            decl.span,
            Some(parent),
            addr_of(decl),
        );
        self.visit_declarators(decl, id);
        self.close(id);
    }

    fn visit_declarators(&mut self, decl: &VariableDeclaration<'_>, parent: NodeId) {
        for declarator in decl.declarations {
            self.visit_declarator(declarator, parent);
        }
    }

    fn visit_declarator(&mut self, declarator: &VariableDeclarator<'_>, parent: NodeId) {
        let id = self.add(
            NodeKind::VariableDeclarator,
            declarator.span,
            Some(parent),
            addr_of(declarator),
        );
        // The binding target — an identifier (with its type annotation) or a
        // destructuring pattern — is an `Expression`, routed through the
        // pattern-aware `visit_expression`.
        self.visit_expression(&declarator.id, id);
        if let Some(init) = &declarator.init {
            self.visit_expression(init, id);
        }
        self.close(id);
    }

    fn visit_for_left(&mut self, left: &ForInOfLeft<'_>, parent: NodeId) {
        match left {
            ForInOfLeft::VariableDeclaration(decl) => self.visit_variable_declaration(decl, parent),
            // A pattern here may be an Object/ArrayPattern — pattern-aware descent.
            ForInOfLeft::Pattern(e) => self.visit_expression(e, parent),
        }
    }

    // --- modules / enums / cases / catch -------------------------------------

    /// Descend a module's name and body (the module's own node is `module_id`).
    fn descend_module(&mut self, m: &TSModuleDeclaration<'_>, module_id: NodeId) {
        match &m.id {
            TSModuleName::Identifier(id) => self.visit_identifier(id, module_id),
            TSModuleName::Literal(lit) => {
                self.leaf(NodeKind::Literal, lit.span, addr_of(lit), module_id);
            }
        }
        match &m.body {
            Some(TSModuleDeclarationBody::TSModuleBlock(block)) => {
                let id = self.add(
                    NodeKind::TSModuleBlock,
                    block.span,
                    Some(module_id),
                    addr_of(block),
                );
                self.visit_statements(block.body, id);
                self.close(id);
            }
            // The dotted-namespace continuation (`namespace A.B {}`) — a nested
            // `TSModuleDeclaration` node (reused kind), recursed.
            Some(TSModuleDeclarationBody::TSModuleDeclaration(nested)) => {
                let id = self.add(
                    NodeKind::TSModuleDeclaration,
                    nested.span,
                    Some(module_id),
                    addr_of(nested),
                );
                self.descend_module(nested, id);
                self.close(id);
            }
            None => {}
        }
    }

    fn visit_enum_member(&mut self, member: &TSEnumMember<'_>, parent: NodeId) {
        let id = self.add(
            NodeKind::TSEnumMember,
            member.span,
            Some(parent),
            addr_of(member),
        );
        match &member.id {
            TSEnumMemberId::Identifier(idn) => self.visit_identifier(idn, id),
            TSEnumMemberId::String(lit) => self.leaf(NodeKind::Literal, lit.span, addr_of(lit), id),
        }
        if let Some(init) = &member.initializer {
            self.visit_expression(init, id);
        }
        self.close(id);
    }

    fn visit_switch_case(&mut self, case: &SwitchCase<'_>, parent: NodeId) {
        let id = self.add(NodeKind::SwitchCase, case.span, Some(parent), addr_of(case));
        if let Some(t) = &case.test {
            self.visit_expression(t, id);
        }
        self.visit_statements(case.consequent, id);
        self.close(id);
    }

    fn visit_catch_clause(&mut self, h: &CatchClause<'_>, parent: NodeId) {
        let id = self.add(NodeKind::CatchClause, h.span, Some(parent), addr_of(h));
        if let Some(param) = &h.param {
            self.visit_expression(param, id);
        }
        // The catch body block is flattened (list-wrapper, today's shape).
        self.visit_statements(h.body.body, id);
        self.close(id);
    }

    // --- classes -------------------------------------------------------------

    /// Descend class heritage: decorators, the `extends` expression + its type
    /// arguments, and each `implements`/`extends` heritage clause.
    fn visit_class_heritage(
        &mut self,
        decorators: Option<&[Decorator<'_>]>,
        super_class: Option<&Expression<'_>>,
        super_type_parameters: Option<&TSTypeParameterInstantiation<'_>>,
        heritages: &[TSInterfaceHeritage<'_>],
        parent: NodeId,
    ) {
        if let Some(decs) = decorators {
            self.visit_decorators(decs, parent);
        }
        if let Some(sc) = super_class {
            self.visit_expression(sc, parent);
        }
        if let Some(tp) = super_type_parameters {
            self.visit_type_args(tp, parent);
        }
        self.visit_heritages(heritages, parent);
    }

    fn visit_heritages(&mut self, heritages: &[TSInterfaceHeritage<'_>], parent: NodeId) {
        for h in heritages {
            let id = self.add(
                NodeKind::TSInterfaceHeritage,
                h.span,
                Some(parent),
                addr_of(h),
            );
            // The heritage target (`extends Base` / `implements Base`) — an entity
            // name — plus its type arguments.
            self.visit_entity_name(&h.expression, id);
            if let Some(ta) = &h.type_arguments {
                self.visit_type_args(ta, id);
            }
            self.close(id);
        }
    }

    /// `ClassBody` is a list-wrapper: its members stay flat under the class (no
    /// separate node), matching today's shape.
    fn visit_class_body(&mut self, body: &ClassBody<'_>, parent: NodeId) {
        for member in body.body {
            self.visit_class_member(member, parent);
        }
    }

    fn visit_class_member(&mut self, member: &ClassMember<'_>, parent: NodeId) {
        match member {
            ClassMember::MethodDefinition(m) => {
                let id = self.add(NodeKind::MethodDefinition, m.span, Some(parent), addr_of(m));
                if let Some(decs) = m.decorators {
                    self.visit_decorators(decs, id);
                }
                self.visit_expression(&m.key, id);
                self.visit_function_expression(&m.value, id);
                self.close(id);
            }
            ClassMember::PropertyDefinition(p) => {
                let id = self.add(
                    NodeKind::PropertyDefinition,
                    p.span,
                    Some(parent),
                    addr_of(p),
                );
                if let Some(decs) = p.decorators {
                    self.visit_decorators(decs, id);
                }
                self.visit_expression(&p.key, id);
                self.visit_type_annotation_opt(p.type_annotation.as_ref(), id);
                if let Some(v) = &p.value {
                    self.visit_expression(v, id);
                }
                self.close(id);
            }
            ClassMember::StaticBlock(s) => {
                let id = self.add(NodeKind::StaticBlock, s.span, Some(parent), addr_of(s));
                self.visit_statements(s.body, id);
                self.close(id);
            }
            ClassMember::IndexSignature(i) => self.visit_index_signature(i, parent),
        }
    }

    fn visit_index_signature(&mut self, i: &TSIndexSignature<'_>, parent: NodeId) {
        let id = self.add(NodeKind::TSIndexSignature, i.span, Some(parent), addr_of(i));
        for p in i.parameters {
            self.visit_identifier(p, id);
        }
        self.visit_type_annotation_opt(i.type_annotation.as_ref(), id);
        self.close(id);
    }

    // --- expressions (full pattern-aware descent) ----------------------------

    fn visit_params(&mut self, params: &[Expression<'_>], parent: NodeId) {
        for param in params {
            self.visit_expression(param, parent);
        }
    }

    /// Visit any expression position, including the pattern-shaped ones
    /// (`Object`/`Array`/`Assignment` pattern, `RestElement`, `TSParameterProperty`)
    /// that occupy parameter, declarator, assignment-target, and for-left slots. A
    /// binding identifier / pattern also carries an optional type annotation and
    /// parameter decorators — `None` outside those positions, so descending them
    /// unconditionally lets this one method serve every expression slot.
    fn visit_expression(&mut self, expr: &Expression<'_>, parent: NodeId) {
        use Expression as E;
        match expr {
            E::Identifier(idn) => self.visit_identifier(idn, parent),
            E::Literal(lit) => self.leaf(NodeKind::Literal, lit.span, addr_of(lit), parent),
            E::PrivateIdentifier(pid) => {
                self.leaf(NodeKind::PrivateIdentifier, pid.span, addr_of(pid), parent);
            }
            E::RegexLiteral(r) => self.leaf(NodeKind::RegexLiteral, r.span, addr_of(r), parent),
            E::ThisExpression(t) => self.leaf(NodeKind::ThisExpression, t.span, addr_of(t), parent),
            E::Super(s) => self.leaf(NodeKind::Super, s.span, addr_of(s), parent),
            E::ObjectExpression(o) => {
                let id = self.add(NodeKind::ObjectExpression, o.span, Some(parent), addr_of(o));
                for prop in o.properties {
                    self.visit_object_property(prop, id);
                }
                self.close(id);
            }
            E::ArrayExpression(a) => {
                let id = self.add(NodeKind::ArrayExpression, a.span, Some(parent), addr_of(a));
                for el in a.elements.iter().flatten() {
                    self.visit_expression(el, id);
                }
                self.close(id);
            }
            E::UnaryExpression(u) => {
                let id = self.add(NodeKind::UnaryExpression, u.span, Some(parent), addr_of(u));
                self.visit_expression(u.argument, id);
                self.close(id);
            }
            E::UpdateExpression(u) => {
                let id = self.add(NodeKind::UpdateExpression, u.span, Some(parent), addr_of(u));
                self.visit_expression(u.argument, id);
                self.close(id);
            }
            E::BinaryExpression(b) => {
                let id = self.add(NodeKind::BinaryExpression, b.span, Some(parent), addr_of(b));
                self.visit_expression(b.left, id);
                self.visit_expression(b.right, id);
                self.close(id);
            }
            E::CallExpression(c) => {
                let id = self.add(NodeKind::CallExpression, c.span, Some(parent), addr_of(c));
                self.visit_expression(c.callee, id);
                if let Some(ta) = &c.type_arguments {
                    self.visit_type_args(ta, id);
                }
                for a in c.arguments {
                    self.visit_expression(a, id);
                }
                self.close(id);
            }
            E::NewExpression(n) => {
                let id = self.add(NodeKind::NewExpression, n.span, Some(parent), addr_of(n));
                self.visit_expression(n.callee, id);
                if let Some(ta) = &n.type_arguments {
                    self.visit_type_args(ta, id);
                }
                for a in n.arguments {
                    self.visit_expression(a, id);
                }
                self.close(id);
            }
            E::MemberExpression(m) => {
                let id = self.add(NodeKind::MemberExpression, m.span, Some(parent), addr_of(m));
                self.visit_expression(m.object, id);
                self.visit_expression(m.property, id);
                self.close(id);
            }
            E::ConditionalExpression(c) => {
                let id = self.add(
                    NodeKind::ConditionalExpression,
                    c.span,
                    Some(parent),
                    addr_of(c),
                );
                self.visit_expression(c.test, id);
                self.visit_expression(c.consequent, id);
                self.visit_expression(c.alternate, id);
                self.close(id);
            }
            E::ArrowFunctionExpression(a) => {
                let id = self.add(
                    NodeKind::ArrowFunctionExpression,
                    a.span,
                    Some(parent),
                    addr_of(a),
                );
                self.visit_type_params(a.type_parameters.as_ref(), id);
                self.visit_params(a.params, id);
                self.visit_type_annotation_opt(a.return_type.as_ref(), id);
                match &a.body {
                    ArrowFunctionBody::Expression(e) => self.visit_expression(e, id),
                    ArrowFunctionBody::BlockStatement(b) => self.visit_statements(b.body, id),
                }
                self.close(id);
            }
            E::FunctionExpression(f) => self.visit_function_expression(f, parent),
            E::ClassExpression(c) => {
                let id = self.add(NodeKind::ClassExpression, c.span, Some(parent), addr_of(c));
                if let Some(name) = &c.id {
                    self.visit_identifier(name, id);
                }
                // Kept in sync with `descend_class` (see the coverage test).
                self.visit_type_params(c.type_parameters.as_ref(), id);
                self.visit_class_heritage(
                    c.decorators,
                    c.super_class,
                    c.super_type_parameters.as_ref(),
                    c.implements,
                    id,
                );
                self.visit_class_body(&c.body, id);
                self.close(id);
            }
            E::SpreadElement(s) => self.visit_spread(s, parent),
            E::TemplateLiteral(t) => self.visit_template_literal(t, parent),
            E::TaggedTemplateExpression(t) => {
                let id = self.add(
                    NodeKind::TaggedTemplateExpression,
                    t.span,
                    Some(parent),
                    addr_of(t),
                );
                self.visit_expression(t.tag, id);
                if let Some(ta) = &t.type_arguments {
                    self.visit_type_args(ta, id);
                }
                self.visit_template_literal(&t.quasi, id);
                self.close(id);
            }
            E::AwaitExpression(a) => {
                let id = self.add(NodeKind::AwaitExpression, a.span, Some(parent), addr_of(a));
                self.visit_expression(a.argument, id);
                self.close(id);
            }
            E::YieldExpression(y) => {
                let id = self.add(NodeKind::YieldExpression, y.span, Some(parent), addr_of(y));
                if let Some(a) = y.argument {
                    self.visit_expression(a, id);
                }
                self.close(id);
            }
            E::SequenceExpression(s) => {
                let id = self.add(
                    NodeKind::SequenceExpression,
                    s.span,
                    Some(parent),
                    addr_of(s),
                );
                for e in s.expressions {
                    self.visit_expression(e, id);
                }
                self.close(id);
            }
            E::AssignmentExpression(a) => {
                let id = self.add(
                    NodeKind::AssignmentExpression,
                    a.span,
                    Some(parent),
                    addr_of(a),
                );
                // `a.left` may be an Object/Array pattern (destructuring assignment)
                // — pattern-aware descent, never swallowed by a wildcard.
                self.visit_expression(a.left, id);
                self.visit_expression(a.right, id);
                self.close(id);
            }
            E::ObjectPattern(op) => {
                let id = self.add(NodeKind::ObjectPattern, op.span, Some(parent), addr_of(op));
                if let Some(decs) = op.decorators {
                    self.visit_decorators(decs, id);
                }
                self.visit_type_annotation_opt(op.type_annotation.as_ref(), id);
                for prop in op.properties {
                    self.visit_object_pattern_property(prop, id);
                }
                self.close(id);
            }
            E::ArrayPattern(ap) => {
                let id = self.add(NodeKind::ArrayPattern, ap.span, Some(parent), addr_of(ap));
                if let Some(decs) = ap.decorators {
                    self.visit_decorators(decs, id);
                }
                self.visit_type_annotation_opt(ap.type_annotation.as_ref(), id);
                for el in ap.elements.iter().flatten() {
                    self.visit_expression(el, id);
                }
                self.close(id);
            }
            E::AssignmentPattern(a) => {
                let id = self.add(
                    NodeKind::AssignmentPattern,
                    a.span,
                    Some(parent),
                    addr_of(a),
                );
                if let Some(decs) = a.decorators {
                    self.visit_decorators(decs, id);
                }
                self.visit_expression(a.left, id);
                self.visit_expression(a.right, id);
                self.close(id);
            }
            E::RestElement(r) => self.visit_rest_element(r, parent),
            E::TSTypeAssertion(t) => {
                let id = self.add(NodeKind::TSTypeAssertion, t.span, Some(parent), addr_of(t));
                self.visit_type(t.type_annotation, id);
                self.visit_expression(t.expression, id);
                self.close(id);
            }
            E::TSAsExpression(t) => {
                let id = self.add(NodeKind::TSAsExpression, t.span, Some(parent), addr_of(t));
                self.visit_expression(t.expression, id);
                self.visit_type(t.type_annotation, id);
                self.close(id);
            }
            E::TSSatisfiesExpression(t) => {
                let id = self.add(
                    NodeKind::TSSatisfiesExpression,
                    t.span,
                    Some(parent),
                    addr_of(t),
                );
                self.visit_expression(t.expression, id);
                self.visit_type(t.type_annotation, id);
                self.close(id);
            }
            E::TSInstantiationExpression(t) => {
                let id = self.add(
                    NodeKind::TSInstantiationExpression,
                    t.span,
                    Some(parent),
                    addr_of(t),
                );
                self.visit_expression(t.expression, id);
                self.visit_type_args(&t.type_arguments, id);
                self.close(id);
            }
            E::TSNonNullExpression(t) => {
                let id = self.add(
                    NodeKind::TSNonNullExpression,
                    t.span,
                    Some(parent),
                    addr_of(t),
                );
                self.visit_expression(t.expression, id);
                self.close(id);
            }
            E::TSParameterProperty(pp) => {
                let id = self.add(
                    NodeKind::TSParameterProperty,
                    pp.span,
                    Some(parent),
                    addr_of(pp),
                );
                self.visit_expression(pp.parameter, id);
                self.close(id);
            }
            E::ImportExpression(i) => {
                let id = self.add(NodeKind::ImportExpression, i.span, Some(parent), addr_of(i));
                self.visit_expression(i.source, id);
                if let Some(o) = i.options {
                    self.visit_expression(o, id);
                }
                self.close(id);
            }
            E::MetaProperty(m) => {
                let id = self.add(NodeKind::MetaProperty, m.span, Some(parent), addr_of(m));
                self.visit_identifier(&m.meta, id);
                self.visit_identifier(&m.property, id);
                self.close(id);
            }
            E::JsdocCast(c) => {
                let id = self.add(NodeKind::JsdocCast, c.span, Some(parent), addr_of(c));
                self.visit_expression(c.inner, id);
                self.close(id);
            }
            E::ParenthesizedExpression(p) => {
                let id = self.add(
                    NodeKind::ParenthesizedExpression,
                    p.span,
                    Some(parent),
                    addr_of(p),
                );
                self.visit_expression(p.expression, id);
                self.close(id);
            }
        }
    }

    fn visit_function_expression(&mut self, f: &FunctionExpression<'_>, parent: NodeId) {
        let id = self.add(
            NodeKind::FunctionExpression,
            f.span,
            Some(parent),
            addr_of(f),
        );
        if let Some(name) = &f.id {
            self.visit_identifier(name, id);
        }
        self.visit_type_params(f.type_parameters.as_ref(), id);
        self.visit_params(f.params, id);
        self.visit_type_annotation_opt(f.return_type.as_ref(), id);
        self.visit_statements(f.body.body, id);
        self.close(id);
    }

    fn visit_object_property(&mut self, prop: &ObjectProperty<'_>, parent: NodeId) {
        match prop {
            ObjectProperty::Property(pr) => self.visit_property(pr, parent),
            ObjectProperty::SpreadElement(s) => self.visit_spread(s, parent),
        }
    }

    fn visit_object_pattern_property(&mut self, prop: &ObjectPatternProperty<'_>, parent: NodeId) {
        match prop {
            ObjectPatternProperty::Property(pr) => self.visit_property(pr, parent),
            ObjectPatternProperty::RestElement(r) => self.visit_rest_element(r, parent),
        }
    }

    fn visit_property(&mut self, pr: &Property<'_>, parent: NodeId) {
        let id = self.add(NodeKind::Property, pr.span, Some(parent), addr_of(pr));
        self.visit_expression(&pr.key, id);
        self.visit_expression(&pr.value, id);
        self.close(id);
    }

    fn visit_spread(&mut self, s: &SpreadElement<'_>, parent: NodeId) {
        let id = self.add(NodeKind::SpreadElement, s.span, Some(parent), addr_of(s));
        self.visit_expression(s.argument, id);
        self.close(id);
    }

    fn visit_rest_element(&mut self, r: &RestElement<'_>, parent: NodeId) {
        let id = self.add(NodeKind::RestElement, r.span, Some(parent), addr_of(r));
        self.visit_type_annotation_opt(r.type_annotation.as_ref(), id);
        self.visit_expression(r.argument, id);
        self.close(id);
    }

    fn visit_template_literal(&mut self, t: &TemplateLiteral<'_>, parent: NodeId) {
        let id = self.add(NodeKind::TemplateLiteral, t.span, Some(parent), addr_of(t));
        for q in t.quasis {
            self.visit_template_element(q, id);
        }
        for e in t.expressions {
            self.visit_expression(e, id);
        }
        self.close(id);
    }

    fn visit_template_element(&mut self, q: &TemplateElement<'_>, parent: NodeId) {
        self.leaf(NodeKind::TemplateElement, q.span, addr_of(q), parent);
    }

    fn visit_decorators(&mut self, decorators: &[Decorator<'_>], parent: NodeId) {
        for d in decorators {
            let id = self.add(NodeKind::Decorator, d.span, Some(parent), addr_of(d));
            self.visit_expression(&d.expression, id);
            self.close(id);
        }
    }

    // --- identifiers ---------------------------------------------------------

    /// Id an identifier, then descend the binding-only extras (parameter
    /// decorators + type annotation) it carries — both `None` for a reference, so
    /// this serves reference and binding positions alike.
    fn visit_identifier(&mut self, ident: &Identifier<'_>, parent: NodeId) {
        let id = self.add(
            NodeKind::Identifier,
            ident.span,
            Some(parent),
            addr_of(ident),
        );
        if let Some(decs) = ident.decorators() {
            self.visit_decorators(decs, id);
        }
        if let Some(ann) = ident.type_annotation() {
            self.visit_type_annotation(ann, id);
        }
        self.close(id);
    }

    // --- imports / exports ----------------------------------------------------

    fn visit_import_specifier(&mut self, spec: &ImportSpecifier<'_>, parent: NodeId) {
        match spec {
            ImportSpecifier::Default(d) => {
                let id = self.add(
                    NodeKind::ImportDefaultSpecifier,
                    d.span,
                    Some(parent),
                    addr_of(d),
                );
                self.visit_identifier(&d.local, id);
                self.close(id);
            }
            ImportSpecifier::Named(n) => {
                let id = self.add(
                    NodeKind::ImportNamedSpecifier,
                    n.span,
                    Some(parent),
                    addr_of(n),
                );
                self.visit_module_export_name(&n.imported, id);
                self.visit_identifier(&n.local, id);
                self.close(id);
            }
            ImportSpecifier::Namespace(n) => {
                let id = self.add(
                    NodeKind::ImportNamespaceSpecifier,
                    n.span,
                    Some(parent),
                    addr_of(n),
                );
                self.visit_identifier(&n.local, id);
                self.close(id);
            }
        }
    }

    fn visit_export_specifier(&mut self, spec: &ExportSpecifier<'_>, parent: NodeId) {
        let id = self.add(
            NodeKind::ExportSpecifier,
            spec.span,
            Some(parent),
            addr_of(spec),
        );
        self.visit_module_export_name(&spec.local, id);
        self.visit_module_export_name(&spec.exported, id);
        self.close(id);
    }

    fn visit_module_export_name(&mut self, name: &ModuleExportName<'_>, parent: NodeId) {
        match name {
            ModuleExportName::Identifier(id) => self.visit_identifier(id, parent),
            ModuleExportName::Literal(lit) => {
                self.leaf(NodeKind::Literal, lit.span, addr_of(lit), parent);
            }
        }
    }

    fn visit_import_attribute(&mut self, attr: &ImportAttribute<'_>, parent: NodeId) {
        let id = self.add(
            NodeKind::ImportAttribute,
            attr.span,
            Some(parent),
            addr_of(attr),
        );
        match &attr.key {
            ImportAttributeKey::Identifier(idn) => self.visit_identifier(idn, id),
            ImportAttributeKey::Literal(lit) => {
                self.leaf(NodeKind::Literal, lit.span, addr_of(lit), id);
            }
        }
        self.leaf(NodeKind::Literal, attr.value.span, addr_of(&attr.value), id);
        self.close(id);
    }

    fn visit_module_reference(&mut self, mr: &TSModuleReference<'_>, parent: NodeId) {
        match mr {
            TSModuleReference::ExternalModuleReference(ext) => {
                let id = self.add(
                    NodeKind::TSExternalModuleReference,
                    ext.span,
                    Some(parent),
                    addr_of(ext),
                );
                self.leaf(
                    NodeKind::Literal,
                    ext.expression.span,
                    addr_of(&ext.expression),
                    id,
                );
                self.close(id);
            }
            TSModuleReference::EntityName(en) => self.visit_entity_name(en, parent),
        }
    }

    // --- types ---------------------------------------------------------------

    /// A `TSTypeAnnotation` (`: T`) is a transparent wrapper — not idd; the walk
    /// descends straight into the inner `TSType`, which is the node.
    fn visit_type_annotation(&mut self, ann: &TSTypeAnnotation<'_>, parent: NodeId) {
        self.visit_type(ann.type_annotation, parent);
    }

    fn visit_type_annotation_opt(&mut self, ann: Option<&TSTypeAnnotation<'_>>, parent: NodeId) {
        if let Some(a) = ann {
            self.visit_type_annotation(a, parent);
        }
    }

    fn visit_type_args(&mut self, args: &TSTypeParameterInstantiation<'_>, parent: NodeId) {
        let id = self.add(
            NodeKind::TSTypeParameterInstantiation,
            args.span,
            Some(parent),
            addr_of(args),
        );
        for t in args.params {
            self.visit_type(t, id);
        }
        self.close(id);
    }

    fn visit_type_params(
        &mut self,
        params: Option<&TSTypeParameterDeclaration<'_>>,
        parent: NodeId,
    ) {
        if let Some(decl) = params {
            let id = self.add(
                NodeKind::TSTypeParameterDeclaration,
                decl.span,
                Some(parent),
                addr_of(decl),
            );
            for p in decl.params {
                self.visit_type_parameter(p, id);
            }
            self.close(id);
        }
    }

    fn visit_type_parameter(&mut self, p: &TSTypeParameter<'_>, parent: NodeId) {
        let id = self.add(NodeKind::TSTypeParameter, p.span, Some(parent), addr_of(p));
        self.visit_identifier(&p.name, id);
        if let Some(c) = p.constraint {
            self.visit_type(c, id);
        }
        if let Some(d) = p.default {
            self.visit_type(d, id);
        }
        self.close(id);
    }

    fn visit_mapped_type_parameter(&mut self, mtp: &TSMappedTypeParameter<'_>, parent: NodeId) {
        // The `name` is a bare `IdentName` (no child identifier node); the mapped
        // type parameter's own span covers the name token.
        let id = self.add(
            NodeKind::TSMappedTypeParameter,
            mtp.span,
            Some(parent),
            addr_of(mtp),
        );
        self.visit_type(mtp.constraint, id);
        self.close(id);
    }

    fn visit_entity_name(&mut self, name: &TSEntityName<'_>, parent: NodeId) {
        match name {
            TSEntityName::Identifier(id) => self.visit_identifier(id, parent),
            TSEntityName::QualifiedName(qn) => self.visit_qualified_name(qn, parent),
        }
    }

    fn visit_qualified_name(&mut self, qn: &TSQualifiedName<'_>, parent: NodeId) {
        let id = self.add(
            NodeKind::TSQualifiedName,
            qn.span,
            Some(parent),
            addr_of(qn),
        );
        self.visit_entity_name(&qn.left, id);
        self.visit_identifier(&qn.right, id);
        self.close(id);
    }

    fn visit_import_type(&mut self, i: &TSImportType<'_>, parent: NodeId) {
        let id = self.add(NodeKind::TSImportType, i.span, Some(parent), addr_of(i));
        self.leaf(NodeKind::Literal, i.argument.span, addr_of(&i.argument), id);
        if let Some(o) = i.options {
            self.visit_expression(o, id);
        }
        if let Some(q) = &i.qualifier {
            self.visit_entity_name(q, id);
        }
        if let Some(ta) = &i.type_arguments {
            self.visit_type_args(ta, id);
        }
        self.close(id);
    }

    fn visit_type_elements(&mut self, members: &[TSTypeElement<'_>], parent: NodeId) {
        for member in members {
            self.visit_type_element(member, parent);
        }
    }

    fn visit_type_element(&mut self, member: &TSTypeElement<'_>, parent: NodeId) {
        match member {
            TSTypeElement::PropertySignature(p) => {
                let id = self.add(
                    NodeKind::TSPropertySignature,
                    p.span,
                    Some(parent),
                    addr_of(p),
                );
                self.visit_expression(&p.key, id);
                self.visit_type_annotation_opt(p.type_annotation.as_ref(), id);
                self.close(id);
            }
            TSTypeElement::MethodSignature(m) => {
                let id = self.add(
                    NodeKind::TSMethodSignature,
                    m.span,
                    Some(parent),
                    addr_of(m),
                );
                self.visit_expression(&m.key, id);
                self.visit_type_params(m.type_parameters.as_ref(), id);
                self.visit_params(m.params, id);
                self.visit_type_annotation_opt(m.return_type.as_ref(), id);
                self.close(id);
            }
            TSTypeElement::CallSignature(c) => {
                let id = self.add(
                    NodeKind::TSCallSignatureDeclaration,
                    c.span,
                    Some(parent),
                    addr_of(c),
                );
                self.visit_type_params(c.type_parameters.as_ref(), id);
                self.visit_params(c.params, id);
                self.visit_type_annotation_opt(c.return_type.as_ref(), id);
                self.close(id);
            }
            TSTypeElement::ConstructSignature(c) => {
                let id = self.add(
                    NodeKind::TSConstructSignatureDeclaration,
                    c.span,
                    Some(parent),
                    addr_of(c),
                );
                self.visit_type_params(c.type_parameters.as_ref(), id);
                self.visit_params(c.params, id);
                self.visit_type_annotation_opt(c.return_type.as_ref(), id);
                self.close(id);
            }
            TSTypeElement::IndexSignature(i) => self.visit_index_signature(i, parent),
        }
    }

    fn visit_type(&mut self, ty: &TSType<'_>, parent: NodeId) {
        match ty {
            TSType::Keyword(kw) => self.leaf(NodeKind::TSKeywordType, kw.span, addr_of(kw), parent),
            TSType::ThisType(t) => self.leaf(NodeKind::TSThisType, t.span, addr_of(t), parent),
            TSType::Literal(lit) => self.visit_literal_type(lit, parent),
            TSType::Array(a) => {
                let id = self.add(NodeKind::TSArrayType, a.span, Some(parent), addr_of(a));
                self.visit_type(a.element_type, id);
                self.close(id);
            }
            TSType::Union(u) => {
                let id = self.add(NodeKind::TSUnionType, u.span, Some(parent), addr_of(u));
                for t in u.types {
                    self.visit_type(t, id);
                }
                self.close(id);
            }
            TSType::Intersection(i) => {
                let id = self.add(
                    NodeKind::TSIntersectionType,
                    i.span,
                    Some(parent),
                    addr_of(i),
                );
                for t in i.types {
                    self.visit_type(t, id);
                }
                self.close(id);
            }
            TSType::TypeReference(r) => {
                let id = self.add(NodeKind::TSTypeReference, r.span, Some(parent), addr_of(r));
                self.visit_entity_name(&r.type_name, id);
                if let Some(ta) = &r.type_arguments {
                    self.visit_type_args(ta, id);
                }
                self.close(id);
            }
            TSType::TypeLiteral(tl) => {
                let id = self.add(NodeKind::TSTypeLiteral, tl.span, Some(parent), addr_of(tl));
                self.visit_type_elements(tl.members, id);
                self.close(id);
            }
            TSType::Function(f) => {
                let id = self.add(NodeKind::TSFunctionType, f.span, Some(parent), addr_of(f));
                self.visit_type_params(f.type_parameters.as_ref(), id);
                self.visit_params(f.params, id);
                self.visit_type_annotation(&f.return_type, id);
                self.close(id);
            }
            TSType::Constructor(c) => {
                let id = self.add(
                    NodeKind::TSConstructorType,
                    c.span,
                    Some(parent),
                    addr_of(c),
                );
                self.visit_type_params(c.type_parameters.as_ref(), id);
                self.visit_params(c.params, id);
                self.visit_type_annotation(&c.return_type, id);
                self.close(id);
            }
            TSType::Tuple(t) => {
                let id = self.add(NodeKind::TSTupleType, t.span, Some(parent), addr_of(t));
                for e in t.element_types {
                    self.visit_type(e, id);
                }
                self.close(id);
            }
            TSType::Parenthesized(p) => {
                let id = self.add(
                    NodeKind::TSParenthesizedType,
                    p.span,
                    Some(parent),
                    addr_of(p),
                );
                self.visit_type(p.type_annotation, id);
                self.close(id);
            }
            TSType::TypePredicate(p) => {
                let id = self.add(NodeKind::TSTypePredicate, p.span, Some(parent), addr_of(p));
                self.visit_identifier(&p.parameter_name, id);
                if let Some(t) = p.type_annotation {
                    self.visit_type(t, id);
                }
                self.close(id);
            }
            TSType::Conditional(c) => {
                let id = self.add(
                    NodeKind::TSConditionalType,
                    c.span,
                    Some(parent),
                    addr_of(c),
                );
                self.visit_type(c.check_type, id);
                self.visit_type(c.extends_type, id);
                self.visit_type(c.true_type, id);
                self.visit_type(c.false_type, id);
                self.close(id);
            }
            TSType::Mapped(m) => {
                let id = self.add(NodeKind::TSMappedType, m.span, Some(parent), addr_of(m));
                self.visit_mapped_type_parameter(&m.type_parameter, id);
                if let Some(nt) = m.name_type {
                    self.visit_type(nt, id);
                }
                if let Some(ta) = m.type_annotation {
                    self.visit_type(ta, id);
                }
                self.close(id);
            }
            TSType::TypeOperator(o) => {
                let id = self.add(NodeKind::TSTypeOperator, o.span, Some(parent), addr_of(o));
                self.visit_type(o.type_annotation, id);
                self.close(id);
            }
            TSType::Import(i) => self.visit_import_type(i, parent),
            TSType::TypeQuery(q) => {
                let id = self.add(NodeKind::TSTypeQuery, q.span, Some(parent), addr_of(q));
                match &q.expr_name {
                    TSTypeQueryExprName::EntityName(en) => self.visit_entity_name(en, id),
                    TSTypeQueryExprName::Import(imp) => self.visit_import_type(imp, id),
                }
                if let Some(ta) = &q.type_arguments {
                    self.visit_type_args(ta, id);
                }
                self.close(id);
            }
            TSType::IndexedAccess(i) => {
                let id = self.add(
                    NodeKind::TSIndexedAccessType,
                    i.span,
                    Some(parent),
                    addr_of(i),
                );
                self.visit_type(i.object_type, id);
                self.visit_type(i.index_type, id);
                self.close(id);
            }
            TSType::Rest(r) => {
                let id = self.add(NodeKind::TSRestType, r.span, Some(parent), addr_of(r));
                self.visit_type(r.type_annotation, id);
                self.close(id);
            }
            TSType::Optional(o) => {
                let id = self.add(NodeKind::TSOptionalType, o.span, Some(parent), addr_of(o));
                self.visit_type(o.type_annotation, id);
                self.close(id);
            }
            TSType::NamedTupleMember(n) => {
                let id = self.add(
                    NodeKind::TSNamedTupleMember,
                    n.span,
                    Some(parent),
                    addr_of(n),
                );
                self.visit_identifier(&n.label, id);
                self.visit_type(n.element_type, id);
                self.close(id);
            }
            TSType::Infer(inf) => {
                let id = self.add(NodeKind::TSInferType, inf.span, Some(parent), addr_of(inf));
                self.visit_type_parameter(&inf.type_parameter, id);
                self.close(id);
            }
        }
    }

    /// The nested `TSLiteralType` dispatcher: a template-literal type is its own
    /// node (`TSTemplateLiteralType`); a string/number/bigint literal type reuses
    /// `Literal`; a negative-number literal type reuses `UnaryExpression`.
    fn visit_literal_type(&mut self, lit: &TSLiteralType<'_>, parent: NodeId) {
        match lit {
            TSLiteralType::TemplateLiteral(t) => {
                let id = self.add(
                    NodeKind::TSTemplateLiteralType,
                    t.span,
                    Some(parent),
                    addr_of(t),
                );
                for q in t.quasis {
                    self.visit_template_element(q, id);
                }
                for ty in t.types {
                    self.visit_type(ty, id);
                }
                self.close(id);
            }
            TSLiteralType::String(l) | TSLiteralType::Number(l) | TSLiteralType::BigInt(l) => {
                self.leaf(NodeKind::Literal, l.span, addr_of(l), parent);
            }
            TSLiteralType::UnaryExpression(u) => {
                let id = self.add(NodeKind::UnaryExpression, u.span, Some(parent), addr_of(u));
                self.visit_expression(u.argument, id);
                self.close(id);
            }
        }
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
        bind_file(&program, source, FileId::ROOT)
    }

    #[test]
    fn preorder_ids_parents_and_kinds() {
        // Program(1) -> VariableDeclaration(2) -> VariableDeclarator(3)
        //   -> Identifier(4)  (the `x`, now with the init idd too)
        //   -> Literal(5)     (the `1`)
        let bound = bind("const x = 1;");
        assert_eq!(bound.node_count, 5);
        assert_eq!(bound.kinds[0], NodeKind::Program);
        assert_eq!(bound.kinds[1], NodeKind::VariableDeclaration);
        assert_eq!(bound.kinds[2], NodeKind::VariableDeclarator);
        assert_eq!(bound.kinds[3], NodeKind::Identifier);
        assert_eq!(bound.kinds[4], NodeKind::Literal);
        assert_eq!(bound.parents[0], None);
        assert_eq!(bound.parents[1], Some(NodeId::FIRST));
        assert_eq!(bound.parents[3], Some(NodeId::from_index(2)));
        assert_eq!(bound.parents[4], Some(NodeId::from_index(2)));
    }

    #[test]
    fn subtree_end_enables_descendant_test() {
        // Program(1) .. Literal(5); the root's subtree ends at the last id (5).
        let bound = bind("const x = 1;");
        let root = NodeId::FIRST;
        let ident = NodeId::from_index(3); // the `x`
        let decl = NodeId::from_index(1); // VariableDeclaration
        assert_eq!(bound.subtree_end[root.index()], NodeId::from_index(4));
        assert!(bound.is_descendant_of(ident, root));
        assert!(bound.is_descendant_of(ident, decl));
        assert!(!bound.is_descendant_of(root, ident));
        assert!(!bound.is_descendant_of(decl, ident));
    }

    #[test]
    fn address_map_resolves_a_statement() {
        let arena = Bump::new();
        let program = tsv_ts::parse("let a = 1; let b = 2;", &arena).expect("parse");
        let bound = bind_file(&program, "let a = 1; let b = 2;", FileId::ROOT);
        let second = &program.body[1];
        let addr = std::ptr::from_ref(second) as usize;
        let id = bound.address_map.get(&addr).copied().expect("mapped");
        assert_eq!(bound.kinds[id.index()], NodeKind::VariableDeclaration);
    }

    #[test]
    fn nested_statements_are_walked() {
        let bound = bind("function f() { return; }");
        assert!(bound.kinds.contains(&NodeKind::FunctionDeclaration));
        assert!(bound.kinds.contains(&NodeKind::ReturnStatement));
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
    fn expressions_and_types_are_idd() {
        // The full descent reaches into initializers and type annotations.
        let bound = bind("const x: number = f(1);");
        assert!(bound.kinds.contains(&NodeKind::TSKeywordType)); // `number`
        assert!(bound.kinds.contains(&NodeKind::CallExpression)); // `f(1)`
        assert!(bound.kinds.contains(&NodeKind::Literal)); // `1`
    }

    #[test]
    fn node_flags_column_is_zeroed_and_sized() {
        let bound = bind("const x = 1; function f<T>(a: T) { return a; }");
        assert_eq!(bound.node_flags.len(), bound.node_count as usize);
        assert!(bound.node_flags.iter().all(|&b| b == 0));
        // The accessor agrees with the column.
        assert_eq!(bound.node_flags(NodeId::FIRST), 0);
    }

    #[test]
    fn require_node_id_resolves_a_known_node() {
        let arena = Bump::new();
        let program = tsv_ts::parse("let a = 1;", &arena).expect("parse");
        let bound = bind_file(&program, "let a = 1;", FileId::ROOT);
        let addr = std::ptr::from_ref(&program.body[0]) as usize;
        let id = bound.require_node_id(addr);
        assert_eq!(bound.kinds[id.index()], NodeKind::VariableDeclaration);
    }

    #[test]
    #[should_panic(expected = "not covered by the SoA walk")]
    fn require_node_id_hard_fails_on_a_miss() {
        // Address 0 is never a real arena node — the strict resolver must abort
        // rather than return a corrupting `NodeId::FIRST` sentinel.
        let bound = bind("const x = 1;");
        let _ = bound.require_node_id(0);
    }

    #[test]
    fn method_address_collides_with_value() {
        // KNOWN F0 collision (pre-P3 fix pending; see `require_node_id`'s doc). A
        // `MethodDefinition` and its inline offset-0 `value: FunctionExpression`
        // share an address, so the walk's second insert wins and
        // `require_node_id(addr_of(&method))` returns the FunctionExpression, not
        // the MethodDefinition. Pinned so the `(address, NodeKind)` fix is a
        // visible ratchet (this assertion flips to `MethodDefinition` when it lands).
        use tsv_ts::ast::internal::ClassMember;
        let arena = Bump::new();
        let src = "class C { m() {} }";
        let program = tsv_ts::parse(src, &arena).expect("parse");
        let bound = bind_file(&program, src, FileId::ROOT);
        let Statement::ClassDeclaration(class) = &program.body[0] else {
            panic!("expected a class declaration");
        };
        let ClassMember::MethodDefinition(method) = &class.body.body[0] else {
            panic!("expected a method definition");
        };
        let id = bound.require_node_id(std::ptr::from_ref(method) as usize);
        assert_eq!(bound.kinds[id.index()], NodeKind::FunctionExpression);
    }

    #[test]
    fn class_type_parameters_are_descended() {
        // Regression guard (F0 review): a class's own `<T>` was dropped — no
        // NodeId — so F1's strict `require_node_id` would panic on a class type
        // parameter. `class C<T> {}` mints Program, ClassDeclaration, Identifier(C),
        // TSTypeParameterDeclaration, TSTypeParameter, Identifier(T) = 6 nodes, and
        // `require_node_id` resolves the type-parameter node.
        let arena = Bump::new();
        let program = tsv_ts::parse("class C<T> {}", &arena).expect("parse");
        let bound = bind_file(&program, "class C<T> {}", FileId::ROOT);
        assert_eq!(bound.node_count, 6);
        let tp = bound
            .kinds
            .iter()
            .position(|k| *k == NodeKind::TSTypeParameter)
            .map(NodeId::from_index)
            .expect("class type parameter is idd");
        // The `<T>` decl is the type-param's parent; the class owns both.
        assert!(bound.is_descendant_of(tp, NodeId::from_index(1)));
        // The class-expression path mirrors the declaration path (kept in sync).
        assert!(
            bind("const C = class<T> {};")
                .kinds
                .contains(&NodeKind::TSTypeParameter)
        );
        // A class type param's constraint + default are reached (both `TSTypeReference`s).
        assert!(
            bind("class C<T extends U = V> {}")
                .kinds
                .contains(&NodeKind::TSTypeReference)
        );
    }

    /// The sorted family diagnostic codes a source produces — via the full
    /// program pipeline, so the canonical sort + dedup applies (a conflict emits
    /// one diagnostic per position after collapsing the repeated prior-decl ones).
    fn diag_codes(source: &str) -> Vec<u32> {
        let arena = Bump::new();
        let result = crate::program::check_program(
            &[crate::program::SourceUnit::new("t.ts", source)],
            &arena,
        );
        result.diagnostics.iter().map(|d| d.code).collect()
    }

    #[test]
    fn cascade_block_scoped_redeclare_is_2451() {
        assert_eq!(diag_codes("let x; let x;"), vec![2451, 2451]);
        assert_eq!(diag_codes("const y = 1; const y = 2;"), vec![2451, 2451]);
    }

    #[test]
    fn cascade_functions_first_picks_2300_over_2451() {
        // The function hoists first, so the table symbol is the function (not
        // block-scoped) -> TS2300 for the whole `x` run.
        assert_eq!(
            diag_codes("let x; var x; function x() {}"),
            vec![2300, 2300, 2300]
        );
        // No same-scope function: `let` is first -> TS2451.
        assert_eq!(
            diag_codes("function f() { let y; { var y; } }"),
            vec![2451, 2451]
        );
    }

    #[test]
    fn cascade_class_and_method_conflicts_are_2300() {
        assert_eq!(diag_codes("class C {} class C {}"), vec![2300, 2300]);
        // A method vs a same-named property conflicts (Method in PropertyExcludes).
        assert_eq!(
            diag_codes("class C { m() {} m: number; }"),
            vec![2300, 2300]
        );
        // Duplicate parameters conflict via ParameterExcludes.
        assert_eq!(diag_codes("function f(a, a) {}"), vec![2300, 2300]);
    }

    #[test]
    fn cascade_enum_merge_is_2567() {
        // A regular enum and a const enum cannot merge -> the enum-merge message.
        assert_eq!(diag_codes("enum E {} const enum E {}"), vec![2567, 2567]);
        // Two regular enums merge cleanly (no diagnostic).
        assert!(diag_codes("enum F {} enum F {}").is_empty());
    }

    #[test]
    fn cascade_multiple_default_exports_is_2528() {
        assert_eq!(
            diag_codes("export default 0; export default 1;"),
            vec![2528, 2528]
        );
    }

    #[test]
    fn uninstantiated_namespace_does_not_conflict_with_var() {
        // A types-only namespace binds as the inert NamespaceModule, so a same-named
        // `var` merges without a diagnostic.
        assert!(diag_codes("namespace N { interface I {} } declare var N: any;").is_empty());
        // A value-content namespace is a ValueModule and conflicts with a `let`
        // (TS2300 — the namespace, first in the table, is not block-scoped).
        assert_eq!(
            diag_codes("namespace M { const v = 1; } let M;"),
            vec![2300, 2300]
        );
    }

    #[test]
    fn signature_duplicate_params_conflict() {
        // A method / call / construct signature is its own function scope, so its
        // duplicate params conflict (TS2300) — the anonymous signature declaration
        // itself never conflicts.
        assert_eq!(diag_codes("interface I { foo(x, x); }"), vec![2300, 2300]);
        assert_eq!(diag_codes("interface I { (x, x); }"), vec![2300, 2300]);
        assert_eq!(diag_codes("interface I { new (x, x); }"), vec![2300, 2300]);
        // A generic method signature still conflicts on the params (the type param
        // binds in the same scope without colliding).
        assert_eq!(
            diag_codes("interface I { foo<T>(x: T, x: T); }"),
            vec![2300, 2300]
        );
        // Distinct param names in one signature and a lone param never conflict.
        assert!(diag_codes("interface I { foo(x, y); bar(z); }").is_empty());
    }

    #[test]
    fn type_annotation_type_literal_members_bind() {
        // A typed binding descends its annotation: a type-literal method signature's
        // duplicate params conflict.
        assert_eq!(diag_codes("var a: { foo(x, x); };"), vec![2300, 2300]);
        // Duplicate *members* of a type literal silent-merge at bind, but the
        // check pass emits them (a check-time TS2300 per declaration).
        assert_eq!(
            diag_codes("var a: { x: number; x: string; };"),
            vec![2300, 2300]
        );
    }

    #[test]
    fn object_literal_duplicate_methods_conflict() {
        // Two same-named object-literal methods conflict (the method exclude is the
        // whole Value mask for an object-literal method).
        assert_eq!(
            diag_codes("var b = { foo() {}, foo() {} };"),
            vec![2300, 2300]
        );
        // Duplicate plain properties silent-merge (Property is not in PropertyExcludes).
        assert!(diag_codes("var b = { x: 1, x: 2 };").is_empty());
        // A getter/setter pair of the same name merges without a diagnostic.
        assert!(diag_codes("var b = { get x() { return 1; }, set x(v) {} };").is_empty());
    }

    #[test]
    fn dotted_namespace_merges_with_explicit_nested() {
        // The dotted form's intermediate segments route to the enclosing namespace's
        // exports, so they merge with the explicit-nested form — and their conflicting
        // members surface (two classes named `P` in the merged inner namespace).
        assert_eq!(
            diag_codes(
                "namespace M.X { export class P {} } \
                 namespace M { export namespace X { export class P {} } }"
            ),
            vec![2300, 2300]
        );
        // A lone dotted namespace introduces no spurious conflict.
        assert!(diag_codes("namespace A.B.C { export const x = 1; }").is_empty());
    }

    #[test]
    fn private_name_mangling_collides_at_hash() {
        // A private method vs a same-named private field is a bind-time conflict
        // (Method in PropertyExcludes); the mangling (class symbol id + name) makes
        // the two `#x` share a table key, and the squiggle covers the `#`. (Two
        // private *fields* would be property-vs-property — a check-time TS2300.)
        let src = "class C { #x() {} #x = 1; }";
        let bound = bind(src);
        let mut diags: Vec<(u32, u32)> = bound
            .diagnostics
            .iter()
            .map(|d| (d.code, d.span.start))
            .collect();
        diags.sort_unstable();
        assert_eq!(
            diags.iter().map(|d| d.0).collect::<Vec<_>>(),
            vec![2300, 2300]
        );
        for (_, start) in &diags {
            assert_eq!(&src[*start as usize..=*start as usize], "#");
        }
    }

    #[test]
    fn param_position_type_literal_method_params_conflict() {
        // A method signature inside a parameter's type annotation is its own
        // function scope, so its duplicate params conflict (the param-position
        // type-annotation hook reaches the type literal).
        assert_eq!(
            diag_codes("function f(p: { foo(x, x); }) {}"),
            vec![2300, 2300]
        );
    }

    #[test]
    fn object_literal_getter_getter_conflicts() {
        // Two same-named object-literal getters conflict (GET_ACCESSOR_EXCLUDES);
        // the accessor bump keeps the run reporting.
        assert_eq!(
            diag_codes("var b = { get x() {}, get x() {} };"),
            vec![2300, 2300]
        );
    }

    #[test]
    fn object_literal_computed_key_method_conflicts() {
        // A computed string-literal key names an object-literal method, so two
        // `['foo']()` methods conflict (the object-literal method exclude is Value).
        assert_eq!(
            diag_codes("var b = { ['foo']() {}, ['foo']() {} };"),
            vec![2300, 2300]
        );
    }

    #[test]
    fn check_pass_duplicate_type_parameters() {
        // The check pass emits TS2300 for a duplicate type parameter (the binder
        // silent-merges same-name type params, so this is check-only). One duplicate
        // → one diagnostic after sort/dedup.
        assert_eq!(diag_codes("function f<T, T>() {}"), vec![2300]);
        assert_eq!(diag_codes("class C<T, U, T> {}"), vec![2300]);
        assert_eq!(diag_codes("interface I<A, A> {}"), vec![2300]);
        // Distinct names never fire.
        assert!(diag_codes("function g<T, U>() {}").is_empty());
        // Declaration-merged interfaces are scoped per-declaration — only the second
        // (its own `C, C`) fires; the two decls never cross-compare (that would be
        // TS2428, deliberately not ported).
        assert_eq!(
            diag_codes("interface J<B> {} interface J<C, C> {}"),
            vec![2300]
        );
    }

    #[test]
    fn check_pass_type_parameters_three_way_dedup() {
        // `<T, T, T>` fires 1 at T₂ + 2 at T₃ raw; the program-wide sort/dedup
        // collapses the T₃ pair → 2 final diagnostics.
        assert_eq!(diag_codes("function f<T, T, T>() {}"), vec![2300, 2300]);
    }

    #[test]
    fn check_pass_non_decimal_numeric_keys_stay_distinct() {
        // `0x0` / `0xff` (and octal / binary / numeric-separator forms) decode to
        // NaN upstream; keyed on their verbatim source they stay distinct, so no
        // false TS2300 (a `NaN`-keyed collision would flag them all).
        assert!(diag_codes("type T = { 0x0: number; 0xff: string };").is_empty());
        assert!(diag_codes("type T = { 0o7: number; 1_0: string };").is_empty());
        // The identical form still collides (both key `"0x1"`).
        assert_eq!(
            diag_codes("type T = { 0x1: number; 0x1: string };"),
            vec![2300, 2300]
        );
    }

    #[test]
    fn dotted_namespace_three_deep_merges_with_explicit_nested() {
        // A 3-deep dotted namespace's intermediate segments route to their
        // enclosing namespace's exports, so `M.X.Y` merges with the explicit 3-deep
        // nested form and the inner `P` conflict surfaces.
        assert_eq!(
            diag_codes(
                "namespace M.X.Y { export class P {} } \
                 namespace M { export namespace X { export namespace Y { export class P {} } } }"
            ),
            vec![2300, 2300]
        );
    }

    #[test]
    fn export_default_identifier_is_alias_no_2528() {
        // `export default <identifier>` binds as an inert alias, so a following
        // default declaration does not conflict (matches tsgo; the redeclare is a
        // check-time TS2323, not a bind-time TS2528).
        assert!(
            diag_codes("const foo = 1; export default foo; export default class Foo {}").is_empty()
        );
    }

    #[test]
    fn module_ness_detects_indicators() {
        assert_eq!(
            bind("export const x = 1;").facts.module_ness,
            ModuleNess::Module
        );
        assert_eq!(
            bind("import x from 'y';").facts.module_ness,
            ModuleNess::Module
        );
        assert_eq!(bind("const x = 1;").facts.module_ness, ModuleNess::Script);
        // `import x = require('y')` counts; `import x = A.B` and `export as
        // namespace N` do not.
        assert_eq!(
            bind("import x = require('y');").facts.module_ness,
            ModuleNess::Module
        );
        assert_eq!(
            bind("import x = A.B;").facts.module_ness,
            ModuleNess::Script
        );
        assert_eq!(
            bind("export as namespace N;").facts.module_ness,
            ModuleNess::Script
        );
        // `import.meta` anywhere counts.
        assert_eq!(
            bind("const u = import.meta.url;").facts.module_ness,
            ModuleNess::Module
        );
    }
}

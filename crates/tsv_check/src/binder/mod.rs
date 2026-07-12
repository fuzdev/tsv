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
//!   columns and the address→id map.
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
mod lower;
mod sym;
pub mod symbols;

use crate::diag::Diagnostic;
use crate::hash::FxHashMap;
use crate::ids::{FileId, NodeId};
use crate::merge::FileMerge;
use tsv_lang::Span;
use tsv_ts::ast::Program;
use tsv_ts::ast::internal::{Expression, Statement, TSModuleReference};

/// The pre-order node kinds the SoA walk assigns — one variant per tsv_ts AST enum
/// variant the walk ids (the program root, then statements, expressions, types, and
/// their sub-nodes). Several kinds are **reused** across positions: `Identifier`
/// tags every identifier — a binding *or* a reference (labels, member/property
/// names, type-param names, entity-name segments, …); `Literal` tags a value
/// literal and a string/number/bigint literal type; `UnaryExpression` a value unary
/// and a negative-number literal type; `TSIndexSignature` both the class-member and
/// type-element index-signature forms; `FunctionExpression` a value function and a
/// method's `value`. The set is not graded or serialized, so its ordering is free.
///
/// `Hash` is derived so a `(usize, NodeKind)` pair can key the address map — the
/// compound key that disambiguates the one offset-0 collision pair
/// (`MethodDefinition` / its inline `value: FunctionExpression`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
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

/// Per-node flag bits in the [`flow::FlowProduct::node_flags`] column (one `u8`
/// per [`NodeId`], minted zeroed by the flow builder — the sole writer today,
/// setting [`NODE_FLAGS_UNREACHABLE`] during unreachable tagging). A bind-side
/// column returns to [`BoundFile`] when a bind-time writer lands (the planned
/// ambient/context node-identity bits).
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
    /// Node `(arena address, kind)` -> id (the random-access escape hatch). The
    /// kind is part of the key so the one offset-0 collision pair
    /// (`MethodDefinition` and its inline `value: FunctionExpression`) stays
    /// distinctly resolvable (see [`BoundFile::require_node_id`]).
    pub address_map: FxHashMap<(usize, NodeKind), NodeId>,
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

    /// Strict `(address, kind)` -> [`NodeId`] resolution for flow consumers.
    /// Unlike the symbol bind's lenient `node_id_of` (whose `Decl.node` result is
    /// dead), a flow graph attaches to the node at `address`, so a **miss is a
    /// bug** — the SoA walk failed to cover a node the flow pass reached, and a
    /// silent `NodeId::FIRST` fallback would splice the graph onto the wrong node.
    /// So this hard-fails rather than returning a sentinel.
    ///
    /// `address` is `std::ptr::from_ref(node) as usize` for the same arena node
    /// reference the walk keyed on, and `kind` is the [`NodeKind`] the walk
    /// assigned it.
    ///
    /// **The offset-0 collision, resolved by compound keying.** Pointer-identity
    /// alone cannot distinguish a node from an offset-0 inline struct-typed child
    /// at the same address. The AST has exactly one such pair: `MethodDefinition`
    /// and its inline `value: FunctionExpression` (value at struct offset 0). The
    /// kind component of the key disambiguates them — no same-kind collisions
    /// exist (verified via `-Zprint-type-sizes`), so `(address, kind)` is a total
    /// key. `require_node_id(addr_of(&method), NodeKind::MethodDefinition)` and
    /// `require_node_id(addr_of(&method.value), NodeKind::FunctionExpression)` now
    /// resolve to their respective distinct ids (pinned by
    /// `method_and_value_resolve_distinctly` below).
    #[must_use]
    pub fn require_node_id(&self, address: usize, kind: NodeKind) -> NodeId {
        match self.address_map.get(&(address, kind)) {
            Some(&id) => id,
            None => node_id_miss(address, kind),
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
fn node_id_miss(address: usize, kind: NodeKind) -> ! {
    panic!(
        "require_node_id: ({address:#x}, {kind:?}) not covered by the SoA walk — a flow \
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
    address_map: FxHashMap<(usize, NodeKind), NodeId>,
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
        // The insert must run in ALL profiles — the flow walk's strict
        // `require_node_id` reads this map at runtime. Each node is added exactly
        // once by the pre-order walk, so a prior entry for this `(address, kind)`
        // key is a genuine same-kind offset-0 collision (the "no same-kind
        // collisions exist" claim in `require_node_id`, made a corpus-checked
        // invariant — `tsc_conformance run` is a debug build). Only that
        // collision *assertion* compiles out of release; the insert side effect
        // is hoisted out of the assert condition so it always happens.
        let prev = self.address_map.insert((address, kind), id);
        debug_assert!(
            prev.is_none(),
            "same-kind address collision at {address:#x} / {kind:?}"
        );
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
}

/// The [`NodeKind`] for a statement variant. Shared with the flow walk and the
/// unreachable-code shim, which resolve statements through the compound-keyed
/// address map (`require_node_id` / a lenient `address_map` lookup).
pub(crate) fn statement_kind(stmt: &Statement<'_>) -> NodeKind {
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

/// The `(arena address, NodeKind)` compound key for an expression variant — the
/// key `SoaWalk::visit_expression` registers it under in the address map.
/// Shared with the flow walk's `expr_id`, so the two mappings cannot drift: a
/// `debug_assert` at the end of `visit_expression` pins the agreement on every
/// lowered expression (corpus-exercised — the conformance gate runs debug
/// builds), and the flow walk's strict resolver hard-fails on any residual
/// mismatch. A `JsdocCast` resolves to its **inner** expression's key: the flow
/// walk treats the cast wrapper as transparent (matching tsgo, where the
/// reparsed cast is not a flow subject), and the SoA walk lowers both the
/// wrapper and the inner, so the inner's key is always present.
pub(crate) fn expression_addr_kind(e: &Expression<'_>) -> (usize, NodeKind) {
    use Expression as E;
    match e {
        E::JsdocCast(c) => expression_addr_kind(c.inner),
        E::Literal(x) => (addr_of(x), NodeKind::Literal),
        E::Identifier(x) => (addr_of(x), NodeKind::Identifier),
        E::PrivateIdentifier(x) => (addr_of(x), NodeKind::PrivateIdentifier),
        E::ObjectExpression(x) => (addr_of(x), NodeKind::ObjectExpression),
        E::ArrayExpression(x) => (addr_of(x), NodeKind::ArrayExpression),
        E::UnaryExpression(x) => (addr_of(x), NodeKind::UnaryExpression),
        E::UpdateExpression(x) => (addr_of(x), NodeKind::UpdateExpression),
        E::BinaryExpression(x) => (addr_of(x), NodeKind::BinaryExpression),
        E::CallExpression(x) => (addr_of(x), NodeKind::CallExpression),
        E::NewExpression(x) => (addr_of(x), NodeKind::NewExpression),
        E::MemberExpression(x) => (addr_of(x), NodeKind::MemberExpression),
        E::ConditionalExpression(x) => (addr_of(x), NodeKind::ConditionalExpression),
        E::ArrowFunctionExpression(x) => (addr_of(x), NodeKind::ArrowFunctionExpression),
        E::FunctionExpression(x) => (addr_of(x), NodeKind::FunctionExpression),
        E::ClassExpression(x) => (addr_of(x), NodeKind::ClassExpression),
        E::SpreadElement(x) => (addr_of(x), NodeKind::SpreadElement),
        E::TemplateLiteral(x) => (addr_of(x), NodeKind::TemplateLiteral),
        E::TaggedTemplateExpression(x) => (addr_of(x), NodeKind::TaggedTemplateExpression),
        E::AwaitExpression(x) => (addr_of(x), NodeKind::AwaitExpression),
        E::YieldExpression(x) => (addr_of(x), NodeKind::YieldExpression),
        E::SequenceExpression(x) => (addr_of(x), NodeKind::SequenceExpression),
        E::RegexLiteral(x) => (addr_of(x), NodeKind::RegexLiteral),
        E::ThisExpression(x) => (addr_of(x), NodeKind::ThisExpression),
        E::Super(x) => (addr_of(x), NodeKind::Super),
        E::AssignmentExpression(x) => (addr_of(x), NodeKind::AssignmentExpression),
        E::ObjectPattern(x) => (addr_of(x), NodeKind::ObjectPattern),
        E::ArrayPattern(x) => (addr_of(x), NodeKind::ArrayPattern),
        E::AssignmentPattern(x) => (addr_of(x), NodeKind::AssignmentPattern),
        E::RestElement(x) => (addr_of(x), NodeKind::RestElement),
        E::TSTypeAssertion(x) => (addr_of(x), NodeKind::TSTypeAssertion),
        E::TSAsExpression(x) => (addr_of(x), NodeKind::TSAsExpression),
        E::TSSatisfiesExpression(x) => (addr_of(x), NodeKind::TSSatisfiesExpression),
        E::TSInstantiationExpression(x) => (addr_of(x), NodeKind::TSInstantiationExpression),
        E::TSNonNullExpression(x) => (addr_of(x), NodeKind::TSNonNullExpression),
        E::TSParameterProperty(x) => (addr_of(x), NodeKind::TSParameterProperty),
        E::ImportExpression(x) => (addr_of(x), NodeKind::ImportExpression),
        E::MetaProperty(x) => (addr_of(x), NodeKind::MetaProperty),
        E::ParenthesizedExpression(x) => (addr_of(x), NodeKind::ParenthesizedExpression),
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
        let id = bound
            .address_map
            .get(&(addr, NodeKind::VariableDeclaration))
            .copied()
            .expect("mapped");
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
    fn require_node_id_resolves_a_known_node() {
        let arena = Bump::new();
        let program = tsv_ts::parse("let a = 1;", &arena).expect("parse");
        let bound = bind_file(&program, "let a = 1;", FileId::ROOT);
        let addr = std::ptr::from_ref(&program.body[0]) as usize;
        let id = bound.require_node_id(addr, NodeKind::VariableDeclaration);
        assert_eq!(bound.kinds[id.index()], NodeKind::VariableDeclaration);
    }

    #[test]
    #[should_panic(expected = "not covered by the SoA walk")]
    fn require_node_id_hard_fails_on_a_miss() {
        // Address 0 is never a real arena node — the strict resolver must abort
        // rather than return a corrupting `NodeId::FIRST` sentinel.
        let bound = bind("const x = 1;");
        let _ = bound.require_node_id(0, NodeKind::Program);
    }

    #[test]
    fn method_and_value_resolve_distinctly() {
        // The compound key disambiguates the one offset-0 pair: a
        // `MethodDefinition` and its inline `value: FunctionExpression` share an
        // address (value at struct offset 0), yet each resolves to its own id
        // through the `(address, NodeKind)` key. Both look-ups hit the same
        // address; only the kind tells them apart.
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
        // The two share one address (the collision the compound key resolves).
        let method_addr = std::ptr::from_ref(method) as usize;
        let value_addr = std::ptr::from_ref(&method.value) as usize;
        assert_eq!(method_addr, value_addr);
        // The method resolves to its own id …
        let method_id = bound.require_node_id(method_addr, NodeKind::MethodDefinition);
        assert_eq!(bound.kinds[method_id.index()], NodeKind::MethodDefinition);
        // … and the value resolves to a distinct id.
        let value_id = bound.require_node_id(value_addr, NodeKind::FunctionExpression);
        assert_eq!(bound.kinds[value_id.index()], NodeKind::FunctionExpression);
        assert_ne!(method_id, value_id);
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
            &crate::options::CheckOptions::default(),
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

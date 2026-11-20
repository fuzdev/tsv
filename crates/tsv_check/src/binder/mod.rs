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
//!   descends its `: T` before the binding), which the interval test does not rely
//!   on. The one deliberate carve-out is the pure list-wrapper nodes (`ClassBody` /
//!   `TSInterfaceBody` / a function/try/catch/finally/static body `BlockStatement` /
//!   the `Program.body` slice / the transparent `TSTypeAnnotation` `: T` wrapper):
//!   their members/inner stay flat children of the owning node, matching today's
//!   shape (documented at the sites).
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
//! declaration's [`NodeId`] through the SoA walk's address map. The statement payloads
//! it keys on (`TSExportAssignment`, `ExportDefaultDeclaration`, a statement-position
//! `TSModuleDeclaration` — every one but a dotted `A.B` continuation segment) are
//! their own store rows, while the SoA walk keyed the enclosing `Statement` ROW, so
//! those `node_id_of` lookups fall back to the root id — the id is not consumed by the
//! family cascade, which keys on name spans, so the fallback is inert. The **strict**
//! resolver flow consumers use is [`BoundFile::require_node_id`], which hard-fails on
//! a miss (a flow graph must never silently drop an attachment).
//!
//! **Keyed in place (load-bearing).** The address map keys on a row's address, a
//! [`NodeAddr`] (`addr_of`, a safe reference-to-integer cast — the crate keeps
//! `unsafe_code = "forbid"`; the `keyed` module owns every way to make one): a row of
//! the parse's stores, or a field inside one, reached through the [`AstView`] and
//! borrowed for the walk. Store addresses are stable while the parse lives. Every AST
//! row is `Copy`, so a `let s = *view.x(id)` or a by-value iteration of a value run
//! puts a copy on the stack, and keying it (or a field of it) keys a slot another node
//! reuses. The visitors take `&'a` rows, which a stack copy cannot satisfy, and every
//! keyed address is checked against the stores in debug builds
//! ([`debug_assert_stored_key`]) — the `Program` root, held beside the stores, is the
//! one key excepted.
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
pub(crate) mod flow;
mod keyed;
mod lower;
mod sym;
pub(crate) mod symbols;

pub(crate) use keyed::NodeAddr;
pub(crate) use keyed::{Keyed, addr_of, debug_assert_stored_key};

use crate::diag::Diagnostic;
use crate::ids::{FileId, NodeId};
use crate::merge::FileMerge;
use tsv_lang::{FxHashMap, Span};
use tsv_ts::ast::{
    AstView, ExprId, Expression, ExpressionKind, ParsedRef, Program, Run, Statement, StatementKind,
    StmtId, TSModuleReference,
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
///
/// `Hash` is derived so a `(NodeAddr, NodeKind)` pair can key the address map — the
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
    WithStatement,
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
    CommentBoundPair,
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
    /// Node `(address, kind)` -> id (the random-access escape hatch), the address
    /// a row of the parse's stores (the `Program` root's excepted). The kind is part
    /// of the key so the one offset-0 collision pair (`MethodDefinition` and its
    /// inline `value: FunctionExpression`) stays distinctly resolvable (see
    /// [`BoundFile::require_node_id`]).
    ///
    /// These keys assume a node is read in place: every AST row is `Copy`, so a plain
    /// `*deref` copy re-addresses it with no clone syntax to flag. The store-range
    /// guard ([`debug_assert_stored_key`]) catches a copy keyed in a debug build.
    pub(crate) address_map: FxHashMap<(NodeAddr, NodeKind), NodeId>,
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
    /// `address` is `addr_of(node)` for the same node the walk keyed on — read in
    /// place through the same stores — and `kind` is the [`NodeKind`] the walk
    /// assigned it.
    ///
    /// **The offset-0 collision, resolved by compound keying.** Pointer-identity
    /// alone cannot distinguish a node from an offset-0 inline struct-typed child
    /// at the same address — `MethodDefinition` and its inline
    /// `value: FunctionExpression`. The kind component of the key disambiguates
    /// such a pair, and the insert-time same-kind assert (in `SoaWalk::insert`) is
    /// what guards a shared address, so `(address, kind)` is a total key.
    /// `require_node_id(addr_of(method), NodeKind::MethodDefinition)` and
    /// `require_node_id(addr_of(&method.value), NodeKind::FunctionExpression)`
    /// resolve to their respective distinct ids (pinned by
    /// `method_and_value_resolve_distinctly` below).
    #[must_use]
    #[track_caller]
    pub(crate) fn require_node_id(&self, address: NodeAddr, kind: NodeKind) -> NodeId {
        match self.address_map.get(&(address, kind)) {
            Some(&id) => id,
            None => node_id_miss(address, kind),
        }
    }
}

/// The `require_node_id` miss path, isolated so its deliberate panic carries the
/// one `#[expect(clippy::panic)]` the crate's restriction-lint posture requires
/// (panic points need an explicit allow + justification). A miss means the SoA
/// walk did not id a node a flow consumer reached — an internal invariant break
/// that must abort, not a recoverable data error.
#[cold]
#[inline(never)]
#[track_caller]
#[expect(clippy::panic)]
fn node_id_miss(address: NodeAddr, kind: NodeKind) -> ! {
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
/// The module indicators are all structural — no name resolution (and so no
/// `source`) is needed here.
#[must_use]
pub fn module_ness(parsed: ParsedRef<'_>) -> ModuleNess {
    let view = parsed.view;
    let body = view.stmt_list(parsed.program.body);
    for &stmt in body {
        if is_external_module_indicator(view, view.stmt(stmt)) {
            return ModuleNess::Module;
        }
    }
    if body
        .iter()
        .any(|&stmt| stmt_contains_import_meta(view, view.stmt(stmt)))
    {
        return ModuleNess::Module;
    }
    ModuleNess::Script
}

/// tsgo's `isAnExternalModuleIndicatorNode` over one top-level statement.
fn is_external_module_indicator(view: AstView<'_>, stmt: &Statement) -> bool {
    match &stmt.kind {
        // `import ...` / `export ... from` / `export {}` / `export *`.
        StatementKind::ImportDeclaration(_)
        | StatementKind::ExportNamedDeclaration(_)
        | StatementKind::ExportAllDeclaration(_)
        // `export = x` and `export default ...` are both `ExportAssignment` in
        // tsgo, both indicators.
        | StatementKind::TSExportAssignment(_)
        | StatementKind::ExportDefaultDeclaration(_) => true,
        // `import x = require('y')` counts only with an external-module reference;
        // `import x = A.B` (an entity name) does not.
        &StatementKind::TSImportEqualsDeclaration(decl) => matches!(
            view.import_equals(decl).module_reference,
            TSModuleReference::ExternalModuleReference(_)
        ),
        _ => false,
    }
}

/// Whether a statement subtree contains an `import.meta` meta-property (tsgo's
/// `getImportMetaIfNecessary`). A bounded structural walk over the statement and
/// its nested expressions/blocks — `import.meta` is inert for the bind cascade,
/// so this only refines the recorded [`ModuleNess`] fact.
fn stmt_contains_import_meta(view: AstView<'_>, stmt: &Statement) -> bool {
    use StatementKind as S;
    let expr = |id: ExprId| expr_contains_import_meta(view, view.expr(id));
    let opt_expr = |id: Option<ExprId>| id.is_some_and(expr);
    let stmt_at = |id: StmtId| stmt_contains_import_meta(view, view.stmt(id));
    let list = |run: Run<StmtId>| view.stmt_list(run).iter().copied().any(stmt_at);
    match &stmt.kind {
        S::ExpressionStatement(s) => expr(s.expression),
        S::VariableDeclaration(d) => view
            .declarators(d.declarations)
            .iter()
            .any(|decl| opt_expr(decl.init)),
        S::ReturnStatement(s) => opt_expr(s.argument),
        S::ThrowStatement(s) => expr(s.argument),
        S::BlockStatement(b) => list(b.body),
        S::IfStatement(s) => {
            expr(s.test) || stmt_at(s.consequent) || s.alternate.is_some_and(stmt_at)
        }
        S::ForStatement(s) => opt_expr(s.test) || stmt_at(s.body),
        S::ForInStatement(s) => expr(s.right) || stmt_at(s.body),
        S::ForOfStatement(s) => expr(s.right) || stmt_at(s.body),
        S::WhileStatement(s) => expr(s.test) || stmt_at(s.body),
        S::DoWhileStatement(s) => expr(s.test) || stmt_at(s.body),
        S::WithStatement(s) => expr(s.object) || stmt_at(s.body),
        S::SwitchStatement(s) => {
            expr(s.discriminant)
                || view
                    .cases(s.cases)
                    .iter()
                    .any(|c| opt_expr(c.test) || list(c.consequent))
        }
        &S::TryStatement(t) => {
            let t = view.try_stmt(t);
            list(t.block.body)
                || view.opt_catch(t.handler).is_some_and(|h| list(h.body.body))
                || t.finalizer.as_ref().is_some_and(|f| list(f.body))
        }
        &S::LabeledStatement(s) => stmt_at(view.labeled_stmt(s).body),
        _ => false,
    }
}

/// Whether an expression subtree contains an `import.meta` meta-property. Covers
/// the common expression positions; deliberately not exhaustive over every type
/// node (types never carry `import.meta`).
fn expr_contains_import_meta(view: AstView<'_>, expr: &Expression) -> bool {
    use ExpressionKind as E;
    let at = |id: ExprId| expr_contains_import_meta(view, view.expr(id));
    let list = |run: Run<ExprId>| view.expr_list(run).iter().copied().any(at);
    match &expr.kind {
        // `import.meta` vs `new.target`: the only two meta-properties, told apart
        // by the meta keyword's name length (`import` = 6, `new` = 3).
        &E::MetaProperty(m) => view.meta_prop(m).meta.name_len == 6,
        E::ParenthesizedExpression(p) => at(p.expression),
        E::UnaryExpression(u) => at(u.argument),
        E::UpdateExpression(u) => at(u.argument),
        E::AwaitExpression(a) => at(a.argument),
        E::YieldExpression(y) => y.argument.is_some_and(at),
        E::BinaryExpression(b) => at(b.left) || at(b.right),
        E::AssignmentExpression(a) => at(a.left) || at(a.right),
        E::ConditionalExpression(c) => at(c.test) || at(c.consequent) || at(c.alternate),
        E::SequenceExpression(s) => list(s.expressions),
        E::CallExpression(c) => at(c.callee) || list(c.arguments),
        E::NewExpression(n) => at(n.callee) || list(n.arguments),
        E::MemberExpression(m) => at(m.object) || at(m.property),
        E::TSNonNullExpression(t) => at(t.expression),
        E::TSAsExpression(t) => at(t.expression),
        E::TSSatisfiesExpression(t) => at(t.expression),
        E::ArrayExpression(a) => view
            .expr_opt_list(a.elements)
            .iter()
            .flatten()
            .copied()
            .any(at),
        _ => false,
    }
}

/// Bind one file: run the SoA walk and the symbol bind, returning the [`BoundFile`].
///
/// `parsed` is the tree and the view over its stores; every key the walk records is
/// the address of a row in those stores (or of the `Program` root beside them), so the
/// flow walk ([`flow::build_flow`]) must be handed the same `parsed` — the same
/// `Program` place and the same stores — for its lookups to hit. `source` is the host
/// document the AST spans index into (the binder resolves declared names by slicing
/// it, matching the parser's span-identity names).
#[must_use]
pub fn bind_file(parsed: ParsedRef<'_>, source: &str, file: FileId) -> BoundFile {
    let ParsedRef { program, view } = parsed;
    // Pass 1: the SoA node-identity walk (source pre-order).
    let mut walk = SoaWalk::new(view);
    let root = walk.add_root(program);
    // The `Program.body` slice is a pure list-wrapper: its statements stay flat
    // children of the root (no separate node), matching today's shape.
    walk.visit_statements(program.body, root);
    walk.close(root);

    let facts = FileFacts {
        module_ness: module_ness(parsed),
    };

    // Pass 2: the symbol bind (functions-first, container-threaded).
    let (diagnostics, merge) = {
        let mut binder = sym::SymbolBinder::new(source, view, &walk.address_map, file, facts);
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

/// The mutable SoA columns being filled by pass 1, and the view the walk reads the
/// tree through.
struct SoaWalk<'a> {
    view: AstView<'a>,
    parents: Vec<Option<NodeId>>,
    kinds: Vec<NodeKind>,
    spans: Vec<Span>,
    subtree_end: Vec<NodeId>,
    address_map: FxHashMap<(NodeAddr, NodeKind), NodeId>,
}

impl<'a> SoaWalk<'a> {
    fn new(view: AstView<'a>) -> Self {
        SoaWalk {
            view,
            parents: Vec::new(),
            kinds: Vec::new(),
            spans: Vec::new(),
            subtree_end: Vec::new(),
            address_map: FxHashMap::default(),
        }
    }

    /// Assign the root id to the `Program`: the one node keyed outside the stores (the
    /// tree's root is held beside them), so the one key the store-range guard excepts.
    fn add_root(&mut self, program: &Program) -> NodeId {
        self.insert(NodeKind::Program, program.span, None, addr_of(program))
    }

    /// Assign the next pre-order id to a node under `parent`, recording its columns and
    /// its address — a row of the stores, which the store-range guard asserts.
    #[track_caller]
    fn add(&mut self, kind: NodeKind, span: Span, parent: NodeId, address: NodeAddr) -> NodeId {
        debug_assert_stored_key(self.view, address, kind);
        self.insert(kind, span, Some(parent), address)
    }

    #[track_caller]
    fn insert(
        &mut self,
        kind: NodeKind,
        span: Span,
        parent: Option<NodeId>,
        address: NodeAddr,
    ) -> NodeId {
        let id = NodeId::from_index(self.kinds.len());
        self.parents.push(parent);
        self.kinds.push(kind);
        self.spans.push(span);
        self.subtree_end.push(id); // provisional (a leaf owns only itself)
        // The insert must run in ALL profiles — the flow walk's strict
        // `require_node_id` reads this map at runtime. Each node is added exactly
        // once by the pre-order walk, so a prior entry for this `(address, kind)`
        // key is a genuine same-kind offset-0 collision — the assert the total-key
        // paragraph of `require_node_id` rests on, made a corpus-checked invariant
        // (`tsc_conformance run` is a debug build). Only that
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
    #[track_caller]
    fn leaf(&mut self, kind: NodeKind, span: Span, address: NodeAddr, parent: NodeId) {
        let id = self.add(kind, span, parent, address);
        self.close(id);
    }
}

/// The [`NodeKind`] for a statement variant. Shared with the flow walk and the
/// unreachable-code shim, which resolve statements through the compound-keyed
/// address map (`require_node_id` / a lenient `address_map` lookup) — a statement is
/// keyed on its own `Statement` row, whichever variant it is.
pub(crate) fn statement_kind(stmt: &Statement) -> NodeKind {
    match &stmt.kind {
        StatementKind::ExpressionStatement(_) => NodeKind::ExpressionStatement,
        StatementKind::VariableDeclaration(_) => NodeKind::VariableDeclaration,
        StatementKind::TSTypeAliasDeclaration(_) => NodeKind::TSTypeAliasDeclaration,
        StatementKind::TSInterfaceDeclaration(_) => NodeKind::TSInterfaceDeclaration,
        StatementKind::TSDeclareFunction(_) => NodeKind::TSDeclareFunction,
        StatementKind::TSEnumDeclaration(_) => NodeKind::TSEnumDeclaration,
        StatementKind::TSModuleDeclaration(_) => NodeKind::TSModuleDeclaration,
        StatementKind::ReturnStatement(_) => NodeKind::ReturnStatement,
        StatementKind::BlockStatement(_) => NodeKind::BlockStatement,
        StatementKind::FunctionDeclaration(_) => NodeKind::FunctionDeclaration,
        StatementKind::ClassDeclaration(_) => NodeKind::ClassDeclaration,
        StatementKind::ExportNamedDeclaration(_) => NodeKind::ExportNamedDeclaration,
        StatementKind::ExportDefaultDeclaration(_) => NodeKind::ExportDefaultDeclaration,
        StatementKind::ExportAllDeclaration(_) => NodeKind::ExportAllDeclaration,
        StatementKind::TSExportAssignment(_) => NodeKind::TSExportAssignment,
        StatementKind::TSNamespaceExportDeclaration(_) => NodeKind::TSNamespaceExportDeclaration,
        StatementKind::ImportDeclaration(_) => NodeKind::ImportDeclaration,
        StatementKind::TSImportEqualsDeclaration(_) => NodeKind::TSImportEqualsDeclaration,
        StatementKind::IfStatement(_) => NodeKind::IfStatement,
        StatementKind::ForStatement(_) => NodeKind::ForStatement,
        StatementKind::ForInStatement(_) => NodeKind::ForInStatement,
        StatementKind::ForOfStatement(_) => NodeKind::ForOfStatement,
        StatementKind::WhileStatement(_) => NodeKind::WhileStatement,
        StatementKind::DoWhileStatement(_) => NodeKind::DoWhileStatement,
        StatementKind::WithStatement(_) => NodeKind::WithStatement,
        StatementKind::SwitchStatement(_) => NodeKind::SwitchStatement,
        StatementKind::TryStatement(_) => NodeKind::TryStatement,
        StatementKind::ThrowStatement(_) => NodeKind::ThrowStatement,
        StatementKind::BreakStatement(_) => NodeKind::BreakStatement,
        StatementKind::ContinueStatement(_) => NodeKind::ContinueStatement,
        StatementKind::LabeledStatement(_) => NodeKind::LabeledStatement,
        StatementKind::EmptyStatement(_) => NodeKind::EmptyStatement,
        StatementKind::DebuggerStatement(_) => NodeKind::DebuggerStatement,
    }
}

/// The `(address, NodeKind)` compound key for an expression — the key
/// `SoaWalk::visit_expression` registers it under in the address map. `e` must be a
/// row read in place through `view` (a store row, or an `Expression` held inside one),
/// never a stack copy: an inline variant's key is the address of its payload inside
/// `e`. Shared with the flow walk's `expr_id`, so the two mappings cannot drift: a
/// `debug_assert` at the end of `visit_expression` pins the agreement on every
/// lowered expression (corpus-exercised — the conformance gate runs debug
/// builds), and the flow walk's strict resolver hard-fails on any residual
/// mismatch. A `CommentBoundPair` resolves to its **inner** expression's key: the flow
/// walk treats the cast wrapper as transparent (matching tsgo, where the
/// reparsed cast is not a flow subject), and the SoA walk lowers both the
/// wrapper and the inner, so the inner's key is always present.
pub(crate) fn expression_addr_kind(view: AstView<'_>, e: &Expression) -> (NodeAddr, NodeKind) {
    use ExpressionKind as E;
    // The stored variants key their payload's own row in its store (`view.arrow_fn(x)`
    // and siblings), the row `visit_expression` hands its visitor; every other variant
    // keys its payload where it sits, inside `e`.
    match &e.kind {
        &E::CommentBoundPair(c) => {
            expression_addr_kind(view, view.expr(view.comment_pair(c).inner))
        }
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
        &E::ArrowFunctionExpression(x) => {
            (addr_of(view.arrow_fn(x)), NodeKind::ArrowFunctionExpression)
        }
        &E::FunctionExpression(x) => (addr_of(view.fn_expr(x)), NodeKind::FunctionExpression),
        &E::ClassExpression(x) => (addr_of(view.class_expr(x)), NodeKind::ClassExpression),
        E::SpreadElement(x) => (addr_of(x), NodeKind::SpreadElement),
        E::TemplateLiteral(x) => (addr_of(x), NodeKind::TemplateLiteral),
        &E::TaggedTemplateExpression(x) => (
            addr_of(view.tagged_tpl(x)),
            NodeKind::TaggedTemplateExpression,
        ),
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
        &E::MetaProperty(x) => (addr_of(view.meta_prop(x)), NodeKind::MetaProperty),
        E::ParenthesizedExpression(x) => (addr_of(x), NodeKind::ParenthesizedExpression),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tsv_ts::ast::{ClassMember, Parsed};

    fn parse(source: &str) -> Parsed {
        tsv_ts::__private::parse(source).expect("parse")
    }

    fn bind(source: &str) -> BoundFile {
        bind_file((&parse(source)).into(), source, FileId::ROOT)
    }

    /// The `index`th top-level statement of `parsed`, read in place.
    fn top_statement(parsed: &Parsed, index: usize) -> &Statement {
        let view = parsed.view();
        view.stmt(view.stmt_list(parsed.program.body)[index])
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
        let src = "let a = 1; let b = 2;";
        let parsed = parse(src);
        let bound = bind_file((&parsed).into(), src, FileId::ROOT);
        let addr = addr_of(top_statement(&parsed, 1));
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
        let src = "let a = 1;";
        let parsed = parse(src);
        let bound = bind_file((&parsed).into(), src, FileId::ROOT);
        let addr = addr_of(top_statement(&parsed, 0));
        let id = bound.require_node_id(addr, NodeKind::VariableDeclaration);
        assert_eq!(bound.kinds[id.index()], NodeKind::VariableDeclaration);
    }

    #[test]
    #[should_panic(expected = "not covered by the SoA walk")]
    fn require_node_id_hard_fails_on_a_miss() {
        // A row of another live parse was never keyed by this walk — the strict
        // resolver must abort rather than return a corrupting `NodeId::FIRST`
        // sentinel. Both parses stay alive, so their rows cannot share an address.
        let src = "const x = 1;";
        let parsed = parse(src);
        let bound = bind_file((&parsed).into(), src, FileId::ROOT);
        let other = parse("const y = 2;");
        let _ = bound.require_node_id(
            addr_of(top_statement(&other, 0)),
            NodeKind::VariableDeclaration,
        );
    }

    #[test]
    fn method_and_value_resolve_distinctly() {
        // The compound key disambiguates the one offset-0 pair: a
        // `MethodDefinition` and its inline `value: FunctionExpression` share an
        // address (value at struct offset 0), yet each resolves to its own id
        // through the `(address, NodeKind)` key. Both look-ups hit the same
        // address; only the kind tells them apart.
        let src = "class C { m() {} }";
        let parsed = parse(src);
        let view = parsed.view();
        let bound = bind_file((&parsed).into(), src, FileId::ROOT);
        let &StatementKind::ClassDeclaration(class) = &top_statement(&parsed, 0).kind else {
            panic!("expected a class declaration");
        };
        let class = view.class_decl(class);
        let ClassMember::MethodDefinition(method) = &view.class_members(class.body.body)[0] else {
            panic!("expected a method definition");
        };
        // The two share one address (the collision the compound key resolves).
        let method_addr = addr_of(method);
        let value_addr = addr_of(&method.value);
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
        let bound = bind("class C<T> {}");
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

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "a copied row was keyed")]
    fn stored_key_guard_refuses_a_stack_copy() {
        // A row read in place passes the store-range guard; a `Copy` of the same row on
        // the stack does not, and keying it is the bug the guard names.
        let parsed = parse("let a = 1;");
        let row = top_statement(&parsed, 0);
        debug_assert_stored_key(parsed.view(), addr_of(row), NodeKind::VariableDeclaration);
        let copy = *row;
        debug_assert_stored_key(parsed.view(), addr_of(&copy), NodeKind::VariableDeclaration);
    }

    /// The sorted family diagnostic codes a source produces — via the full
    /// program pipeline, so the canonical sort + dedup applies (a conflict emits
    /// one diagnostic per position after collapsing the repeated prior-decl ones).
    fn diag_codes(source: &str) -> Vec<u32> {
        let result = crate::program::check_program(
            &[crate::program::SourceUnit::new("t.ts", source)],
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

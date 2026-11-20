//! The address keys of the SoA walk's map, typed end to end.
//!
//! A key is a [`NodeAddr`]: the address of an AST row the walk ids, read in place
//! through the parse's stores (or of the `Program` root held beside them). Its field
//! is private to this module, so the only ways to make one are [`addr_of`], bounded by
//! the sealed [`Keyed`] trait, and the one named constructor for the deliberate
//! MetaProperty id-slot key ([`NodeAddr::baseline_meta_property_slot`]). No raw
//! integer becomes a key, and nothing outside this module can implement [`Keyed`]:
//! its seal lives here, so the `keyed_rows!` list below is the whole impl set. The
//! runtime half of the keying rule, the store-range guard
//! ([`debug_assert_stored_key`]), also lives here, the one reader of the raw address.

use std::fmt;

use tsv_ts::__private::holds_address;
use tsv_ts::ast::{AstView, MetaPropId};

use super::NodeKind;

/// The address of a keyed AST row: one half of the address map's `(NodeAddr,
/// NodeKind)` key. Stable while the parse's stores live; meaningful only against the
/// stores (and `Program` root) of the parse it was taken from.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct NodeAddr(usize);

impl NodeAddr {
    /// The address of a `MetaPropId` slot inside its `Expression` row — NOT the
    /// `meta_prop` row the lowering keys, so a lookup on it misses. The flow walk's
    /// `MetaProperty` arm keys this on purpose: it reproduces the baseline's
    /// MetaProperty walk miss (the strict resolver panics on it), which identity with
    /// that baseline requires. The store-range guard still passes (the slot lies in a
    /// store row); only [`Keyed`] would refuse the id, which is why this is its own
    /// constructor rather than an `addr_of` call.
    ///
    /// TODO: key `addr_of(view.meta_prop(*m))` (the lowering's key, and
    /// `expression_addr_kind`'s) once reproducing the baseline's walk miss is no longer
    /// required, and delete this constructor.
    #[inline]
    pub(crate) fn baseline_meta_property_slot(slot: &MetaPropId) -> Self {
        NodeAddr(std::ptr::from_ref(slot).addr())
    }
}

/// Hex, as the miss and collision messages print an address.
impl fmt::LowerHex for NodeAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::LowerHex::fmt(&self.0, f)
    }
}

/// The seal of [`Keyed`]: a trait nothing outside this module can name.
mod sealed {
    pub(crate) trait Sealed {}
}

/// A type whose address the SoA walk keys: an AST row (or an inline payload inside
/// one) of a kind [`NodeKind`] names. Sealed, and implemented by `keyed_rows!` for
/// exactly the types some `addr_of` call site keys, so [`addr_of`] refuses, at
/// compile time, every other referent:
///
/// - **an id slot** (`addr_of(&id)` for any `*Id`, `Id<T>`, `Run<T>`) — the address
///   of the id field inside its row, not of the row the id names; the store-range
///   guard cannot catch it, because that slot also lies inside a store row;
/// - **a reference** (`addr_of(&&row)`, the binder a `match` on a `&&Row` hands
///   out) — the address of the pointer slot, which the next binding reuses, so
///   unrelated nodes share a key and a lookup resolves the wrong one or none;
/// - **any other value** (a span, a flag, a `Copy` scalar) the walk never keys.
///
/// What the bound cannot see — a stack copy of a keyed row (`let s = *view.x(id)`)
/// — is the store-range guard's ([`debug_assert_stored_key`]).
pub(crate) trait Keyed: sealed::Sealed {}

/// Seal and mark each listed AST type as [`Keyed`].
macro_rules! keyed_rows {
    ($($row:ident),* $(,)?) => {
        $(
            impl sealed::Sealed for tsv_ts::ast::$row {}
            impl Keyed for tsv_ts::ast::$row {}
        )*
    };
}

keyed_rows!(
    ArrayExpression,
    ArrayPattern,
    ArrowFunctionExpression,
    AssignmentExpression,
    AssignmentPattern,
    AwaitExpression,
    BinaryExpression,
    CallExpression,
    CatchClause,
    ClassDeclaration,
    ClassExpression,
    CommentBoundPair,
    ConditionalExpression,
    Decorator,
    ExportDefaultDeclaration,
    ExportSpecifier,
    FunctionDeclaration,
    FunctionExpression,
    Identifier,
    ImportAttribute,
    ImportDefaultSpecifier,
    ImportExpression,
    ImportNamedSpecifier,
    ImportNamespaceSpecifier,
    Literal,
    MemberExpression,
    MetaProperty,
    MethodDefinition,
    NewExpression,
    ObjectExpression,
    ObjectPattern,
    ParenthesizedExpression,
    PrivateIdentifier,
    Program,
    PropertyDefinition,
    Property,
    RegexLiteral,
    RestElement,
    SequenceExpression,
    SpreadElement,
    Statement,
    StaticBlock,
    Super,
    SwitchCase,
    TaggedTemplateExpression,
    TemplateElement,
    TemplateLiteral,
    TemplateLiteralType,
    ThisExpression,
    TSArrayType,
    TSAsExpression,
    TSCallSignatureDeclaration,
    TSConditionalType,
    TSConstructorType,
    TSConstructSignatureDeclaration,
    TSDeclareFunction,
    TSEnumMember,
    TSExportAssignment,
    TSExternalModuleReference,
    TSFunctionType,
    TSImportType,
    TSIndexedAccessType,
    TSIndexSignature,
    TSInferType,
    TSInstantiationExpression,
    TSInterfaceDeclaration,
    TSInterfaceHeritage,
    TSIntersectionType,
    TSKeywordType,
    TSMappedType,
    TSMappedTypeParameter,
    TSMethodSignature,
    TSModuleBlock,
    TSModuleDeclaration,
    TSNamedTupleMember,
    TSNonNullExpression,
    TSOptionalType,
    TSParameterProperty,
    TSParenthesizedType,
    TSPropertySignature,
    TSQualifiedName,
    TSRestType,
    TSSatisfiesExpression,
    TSThisType,
    TSTupleType,
    TSTypeAssertion,
    TSTypeLiteral,
    TSTypeOperator,
    TSTypeParameterDeclaration,
    TSTypeParameterInstantiation,
    TSTypeParameter,
    TSTypePredicate,
    TSTypeQuery,
    TSTypeReference,
    TSUnionType,
    UnaryExpression,
    UpdateExpression,
    VariableDeclaration,
    VariableDeclarator,
    YieldExpression,
);

/// The address key of a node: a reference-to-integer cast (safe — no dereference,
/// so `unsafe_code = "forbid"` holds), stable while the parse's stores live.
///
/// `T: Keyed` is the type-level half of the keying rule: only a keyed row or payload
/// type compiles, so an id slot, a `&&Row` (the address of the REFERENCE rather than
/// the row) and any unkeyed value are refused at compile time ([`Keyed`]); the
/// store-range guard ([`debug_assert_stored_key`]) is the runtime half.
#[inline]
pub(crate) fn addr_of<T: Keyed>(node: &T) -> NodeAddr {
    NodeAddr(std::ptr::from_ref(node).addr())
}

/// The store-range guard (debug builds): `address`, keyed or looked up as `kind`, must
/// lie in a row of the stores `view` reads. A row read in place through the view, or a
/// field inside one, always does; a copy of a `Copy` row on the stack never does, and
/// keying one keys a slot the next copy reuses — a silent mis-attribution, since no
/// type error is available. The one shared check of every site that keys or resolves
/// an address: `SoaWalk::add`, the flow builder's strict `require`, and the lenient
/// lookups of the symbol bind and the unreachable-candidate walk. The `Program` root,
/// held beside the stores rather than in one, is excepted by never reaching here
/// (`SoaWalk::add_root` and the flow builder's root lookup).
#[inline]
#[track_caller]
pub(crate) fn debug_assert_stored_key(view: AstView<'_>, address: NodeAddr, kind: NodeKind) {
    debug_assert!(
        holds_address(view, address.0),
        "({address:#x}, {kind:?}) lies in no store row: a copied row was keyed or looked up"
    );
}

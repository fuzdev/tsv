//! The `size_of` board: every public AST type's width, in one command.
//!
//! **Why this is a command and not a test.** A type's width is a design
//! decision here, not an implementation detail: the AST enums are the ELEMENT
//! WIDTH of the containers that hold them, so a variant that widens one
//! multiplies through every slice in the tree, and a container's width is what
//! its list moves on every `push` and every regrow. Every density lever the
//! perf arc has landed was priced off a `size_of` table — and every session
//! that needed one hand-rolled a throwaway `#[ignore]` test and deleted it, so
//! the same table has been re-derived from scratch again and again while
//! nothing watched the numbers between sessions.
//!
//! The in-crate `const _: () = assert!(size_of::<T>() == N)` guards are the
//! complement, not a substitute: they pin the handful of types whose width a
//! change must not move, and deliberately leave the rest unbounded (a tight
//! bound on every AST type would fail on every honest edit). This board makes
//! the unbounded ones *visible* — `Statement` growing 104 → 200 shows up as a
//! row that moved, without anyone having to have predicted it.
//!
//! **`Result<T>` is boarded beside `T`** because a `Result` wider than its
//! payload costs a word on every fallible return: `tsv_lang::ParseError` is
//! pointer-sized, so `Result<T, ParseError>` is free exactly when `T` has a
//! niche to hide it in — which every AST node enum does, and a struct whose
//! fields are all fully occupied does not. The star marks a payload of at least
//! [`NICHE_MIN`] bytes that pays it. Narrower payloads are excluded rather than
//! flagged: a pointer-sized error cannot fit in a one-byte enum, so those rows
//! would all star and say nothing.
//!
//! **The list is the three language crates' public AST types plus the
//! foundation types**, written out rather than derived — nothing in the tree
//! enumerates a module's types at compile time. It is mechanical to refresh:
//!
//! ```sh
//! grep -h "^pub enum \|^pub struct " crates/tsv_ts/src/ast/internal/*.rs \
//!   | sed "s/^pub \(enum\|struct\) \([A-Za-z0-9_]*\)\(<'[a-z]*>\)\?.*/\2\3/"
//! ```
//!
//! A type missing from the board is a gap, never a wrong number — every row
//! shown is measured by the compiler that built this binary.

use argh::FromArgs;
use std::collections::BTreeMap;

use tsv_lang::Result as TsvResult;

use crate::cli::CliError;

/// The `size_of` / `align_of` board for the public AST and foundation types.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "type_sizes")]
pub struct TypeSizesCommand {
    /// emit JSON
    #[argh(switch)]
    json: bool,

    /// only rows at least this many bytes wide (default 0 — the whole board)
    #[argh(option, default = "0")]
    min: usize,

    /// only the N widest rows overall (applied after --min and --group)
    #[argh(option)]
    top: Option<usize>,

    /// only rows in groups whose name contains this substring (e.g. `ts/`, `svelte`)
    #[argh(option)]
    group: Option<String>,
}

/// The payload width from which an unfilled error niche is worth flagging. A
/// `Result<T, ParseError>` needs a pointer-sized hole in `T`, so anything
/// narrower than the pointer it must hide is widened by arithmetic rather than
/// by a layout choice anyone could make differently.
const NICHE_MIN: usize = 16;

/// One measured type.
pub(crate) struct TypeRow {
    pub(crate) group: &'static str,
    /// The type's name with any lifetime argument stripped — `Expression`, not
    /// `Expression < 'static >`, which is what `stringify!` on a type produces.
    pub(crate) name: &'static str,
    pub(crate) size: usize,
    pub(crate) align: usize,
    /// `size_of::<tsv_lang::Result<T>>()` — equal to `size` when the niche is filled.
    pub(crate) result_size: usize,
}

impl TypeRow {
    /// True when a `Result<T, ParseError>` costs a word over the bare payload
    /// and the payload is wide enough for that to have been avoidable — see
    /// [`NICHE_MIN`].
    pub(crate) const fn pays_for_the_error(&self) -> bool {
        self.result_size > self.size && self.size >= NICHE_MIN
    }
}

/// Board one type. `stringify!` on a generic type token tree spaces the
/// lifetime out (`Expression < 'static >`), so the name is cut at the `<`.
macro_rules! row {
    ($group:expr, $t:ty) => {
        TypeRow {
            group: $group,
            name: match stringify!($t).split_once('<') {
                Some((base, _)) => base.trim_ascii_end(),
                None => stringify!($t),
            },
            size: size_of::<$t>(),
            align: align_of::<$t>(),
            result_size: size_of::<TsvResult<$t>>(),
        }
    };
}

/// Board a group's types. One `extend` per group rather than a `push` per row:
/// the rows are a fixed list, so the group is an array literal.
macro_rules! rows {
    ($out:expr, $group:expr, [$($t:ty),* $(,)?]) => {
        $out.extend([$(row!($group, $t)),*]);
    };
}

impl TypeSizesCommand {
    pub(crate) fn run(self) -> Result<(), CliError> {
        let mut rows = board();

        if let Some(needle) = &self.group {
            rows.retain(|r| r.group.contains(needle.as_str()));
        }
        rows.retain(|r| r.size >= self.min);

        // Widest first, then by name so equal-width rows read in a stable order.
        rows.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.name.cmp(b.name)));
        if let Some(n) = self.top {
            rows.truncate(n);
        }

        if rows.is_empty() {
            eprintln!("No types matched.");
            return Err(CliError::Failed);
        }

        if self.json {
            print_json(&rows);
        } else {
            print_table(&rows);
        }
        Ok(())
    }
}

fn print_table(rows: &[TypeRow]) {
    let name_w = rows.iter().map(|r| r.name.len()).max().unwrap_or(4).max(4);
    let group_w = rows.iter().map(|r| r.group.len()).max().unwrap_or(5).max(5);

    println!(
        "{:<name_w$}  {:>5}  {:>5}  {:>9}  {:<group_w$}",
        "type", "size", "align", "Result<T>", "group"
    );
    println!(
        "{}  {}  {}  {}  {}",
        "-".repeat(name_w),
        "-".repeat(5),
        "-".repeat(5),
        "-".repeat(9),
        "-".repeat(group_w)
    );
    for r in rows {
        // A `Result` wider than its payload means the error niche is unfilled —
        // flagged inline so it reads as a finding rather than as a column to
        // scan by eye. See `NICHE_MIN` for why narrow rows are excluded.
        let widened = if r.pays_for_the_error() { " *" } else { "" };
        println!(
            "{:<name_w$}  {:>5}  {:>5}  {:>9}  {:<group_w$}{}",
            r.name, r.size, r.align, r.result_size, r.group, widened
        );
    }

    let widened = rows.iter().filter(|r| r.pays_for_the_error()).count();
    println!();
    println!("{} types boarded", rows.len());
    if widened > 0 {
        println!(
            "* {widened} of >= {NICHE_MIN} B pay a word for Result<T> (no niche for the error)"
        );
    }

    // Per-group widest — the ladder's entry point: the density question is
    // always "what sets this family's width?".
    let mut widest: BTreeMap<&str, &TypeRow> = BTreeMap::new();
    for r in rows {
        widest
            .entry(r.group)
            .and_modify(|w| {
                if r.size > w.size {
                    *w = r;
                }
            })
            .or_insert(r);
    }
    println!();
    println!("widest per group:");
    for (group, r) in widest {
        println!("  {:<16} {} ({} B)", group, r.name, r.size);
    }
}

fn print_json(rows: &[TypeRow]) {
    let items: Vec<_> = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "group": r.group,
                "name": r.name,
                "size": r.size,
                "align": r.align,
                "result_size": r.result_size,
                "pays_for_the_error": r.pays_for_the_error(),
            })
        })
        .collect();
    let out = serde_json::json!({ "types": items, "count": rows.len() });
    println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
}

/// Every boarded type, grouped by the module it is defined in — so a group is
/// a family whose members compete to set each other's width, which is the
/// question the ladder asks.
#[allow(clippy::too_many_lines)]
pub(crate) fn board() -> Vec<TypeRow> {
    use tsv_css::ast::internal as css;
    use tsv_svelte::ast::internal as svelte;
    use tsv_ts::ast::internal as ts;

    let mut out = Vec::new();

    rows!(
        out,
        "ts/expression",
        [
            ts::ArrayExpression<'static>,
            ts::ArrowFunctionBody<'static>,
            ts::ArrowFunctionExpression<'static>,
            ts::AssignmentExpression<'static>,
            ts::AssignmentOperator,
            ts::AwaitExpression<'static>,
            ts::BinaryExpression<'static>,
            ts::BinaryOperator,
            ts::CallExpression<'static>,
            ts::ConditionalExpression<'static>,
            ts::Expression<'static>,
            ts::FunctionExpression<'static>,
            ts::ImportExpression<'static>,
            ts::JsdocCast<'static>,
            ts::MemberExpression<'static>,
            ts::MetaProperty<'static>,
            ts::NewExpression<'static>,
            ts::ObjectExpression<'static>,
            ts::ObjectProperty<'static>,
            ts::ParenthesizedExpression<'static>,
            ts::Property<'static>,
            ts::PropertyKind,
            ts::RegexLiteral,
            ts::SequenceExpression<'static>,
            ts::SpreadElement<'static>,
            ts::Super,
            ts::TSAsExpression<'static>,
            ts::TSInstantiationExpression<'static>,
            ts::TSNonNullExpression<'static>,
            ts::TSSatisfiesExpression<'static>,
            ts::TSTypeAssertion<'static>,
            ts::TaggedTemplateExpression<'static>,
            ts::TemplateCooked<'static>,
            ts::TemplateElement<'static>,
            ts::TemplateLiteral<'static>,
            ts::ThisExpression,
            ts::UnaryExpression<'static>,
            ts::UnaryOperator,
            ts::UpdateExpression<'static>,
            ts::UpdateOperator,
            ts::YieldExpression<'static>,
        ]
    );

    rows!(
        out,
        "ts/statement",
        [
            ts::BlockStatement<'static>,
            ts::BreakStatement<'static>,
            ts::CatchClause<'static>,
            ts::ContinueStatement<'static>,
            ts::DebuggerStatement,
            ts::DoWhileStatement<'static>,
            ts::EmptyStatement,
            ts::ExpressionStatement<'static>,
            ts::ForInOfLeft<'static>,
            ts::ForInStatement<'static>,
            ts::ForInit<'static>,
            ts::ForOfStatement<'static>,
            ts::ForStatement<'static>,
            ts::FunctionDeclaration<'static>,
            ts::IfStatement<'static>,
            ts::LabeledStatement<'static>,
            ts::ReturnStatement<'static>,
            ts::Statement<'static>,
            ts::SwitchCase<'static>,
            ts::SwitchStatement<'static>,
            ts::ThrowStatement<'static>,
            ts::TryStatement<'static>,
            ts::VariableDeclaration<'static>,
            ts::VariableDeclarationKind,
            ts::VariableDeclarator<'static>,
            ts::WhileStatement<'static>,
        ]
    );

    rows!(
        out,
        "ts/type",
        [
            ts::TSArrayType<'static>,
            ts::TSCallSignatureDeclaration<'static>,
            ts::TSConditionalType<'static>,
            ts::TSConstructSignatureDeclaration<'static>,
            ts::TSConstructorType<'static>,
            ts::TSEntityName<'static>,
            ts::TSFunctionType<'static>,
            ts::TSImportType<'static>,
            ts::TSIndexSignature<'static>,
            ts::TSIndexedAccessType<'static>,
            ts::TSInferType<'static>,
            ts::TSIntersectionType<'static>,
            ts::TSKeywordKind,
            ts::TSKeywordType,
            ts::TSLiteralType<'static>,
            ts::TSMappedType<'static>,
            ts::TSMappedTypeModifier,
            ts::TSMappedTypeParameter<'static>,
            ts::TSMethodSignature<'static>,
            ts::TSNamedTupleMember<'static>,
            ts::TSOptionalType<'static>,
            ts::TSParenthesizedType<'static>,
            ts::TSPropertySignature<'static>,
            ts::TSQualifiedName<'static>,
            ts::TSRestType<'static>,
            ts::TSThisType,
            ts::TSTupleType<'static>,
            ts::TSType<'static>,
            ts::TSTypeAliasDeclaration<'static>,
            ts::TSTypeAnnotation<'static>,
            ts::TSTypeElement<'static>,
            ts::TSTypeLiteral<'static>,
            ts::TSTypeOperator<'static>,
            ts::TSTypeOperatorKind,
            ts::TSTypeParameter<'static>,
            ts::TSTypeParameterDeclaration<'static>,
            ts::TSTypeParameterInstantiation<'static>,
            ts::TSTypeParameterModifier,
            ts::TSTypeParameterModifiers,
            ts::TSTypePredicate<'static>,
            ts::TSTypeQuery<'static>,
            ts::TSTypeQueryExprName<'static>,
            ts::TSTypeReference<'static>,
            ts::TSUnionType<'static>,
            ts::TemplateLiteralType<'static>,
        ]
    );

    rows!(
        out,
        "ts/class",
        [
            ts::Accessibility,
            ts::ClassBody<'static>,
            ts::ClassDeclaration<'static>,
            ts::ClassExpression<'static>,
            ts::ClassMember<'static>,
            ts::MethodDefinition<'static>,
            ts::MethodKind,
            ts::PropertyDefinition<'static>,
            ts::PropertyModifier,
            ts::StaticBlock<'static>,
            ts::TSParameterProperty<'static>,
        ]
    );

    rows!(
        out,
        "ts/declaration",
        [
            ts::TSDeclareFunction<'static>,
            ts::TSEnumDeclaration<'static>,
            ts::TSEnumMember<'static>,
            ts::TSEnumMemberId<'static>,
            ts::TSInterfaceBody<'static>,
            ts::TSInterfaceDeclaration<'static>,
            ts::TSInterfaceHeritage<'static>,
            ts::TSModuleBlock<'static>,
            ts::TSModuleDeclaration<'static>,
            ts::TSModuleDeclarationBody<'static>,
            ts::TSModuleDeclarationKind,
            ts::TSModuleName<'static>,
        ]
    );

    rows!(
        out,
        "ts/module",
        [
            ts::ExportAllDeclaration<'static>,
            ts::ExportDefaultDeclaration<'static>,
            ts::ExportDefaultValue<'static>,
            ts::ExportFunctionDeclaration<'static>,
            ts::ExportKind,
            ts::ExportNamedDeclaration<'static>,
            ts::ExportSpecifier<'static>,
            ts::ImportAttribute<'static>,
            ts::ImportAttributeKey<'static>,
            ts::ImportDeclaration<'static>,
            ts::ImportDefaultSpecifier<'static>,
            ts::ImportKind,
            ts::ImportNamedSpecifier<'static>,
            ts::ImportNamespaceSpecifier<'static>,
            ts::ImportPhase,
            ts::ImportSpecifier<'static>,
            ts::ModuleExportName<'static>,
            ts::TSExportAssignment<'static>,
            ts::TSExternalModuleReference<'static>,
            ts::TSImportEqualsDeclaration<'static>,
            ts::TSModuleReference<'static>,
            ts::TSNamespaceExportDeclaration<'static>,
        ]
    );

    rows!(
        out,
        "ts/pattern",
        [
            ts::ArrayPattern<'static>,
            ts::AssignmentPattern<'static>,
            ts::ObjectPattern<'static>,
            ts::ObjectPatternProperty<'static>,
            ts::RestElement<'static>,
        ]
    );

    rows!(
        out,
        "ts/common",
        [
            ts::Decorator<'static>,
            ts::IdentName<'static>,
            ts::Identifier<'static>,
            ts::IdentifierParamExtra<'static>,
            ts::Literal<'static>,
            ts::LiteralValue<'static>,
            ts::PrivateIdentifier<'static>,
            ts::Program<'static>,
            ts::StringCooked<'static>,
        ]
    );

    rows!(
        out,
        "svelte",
        [
            svelte::AcornPrefixes<'static>,
            svelte::AcornRegion,
            svelte::AnimateDirective<'static>,
            svelte::AttachTag<'static>,
            svelte::Attribute<'static>,
            svelte::AttributeNode<'static>,
            svelte::AttributeValue<'static>,
            svelte::AwaitBlock<'static>,
            svelte::BindDirective<'static>,
            svelte::ClassDirective<'static>,
            svelte::ConstTag<'static>,
            svelte::DebugTag<'static>,
            svelte::DeclarationTag<'static>,
            svelte::EachBlock<'static>,
            svelte::EachKey<'static>,
            svelte::Element<'static>,
            svelte::ElementKind,
            svelte::EmbeddedLang,
            svelte::ExpressionTag<'static>,
            svelte::Fragment<'static>,
            svelte::FragmentNode<'static>,
            svelte::HtmlComment,
            svelte::HtmlTag<'static>,
            svelte::IfBlock<'static>,
            svelte::KeyBlock<'static>,
            svelte::LetDirective<'static>,
            svelte::OnDirective<'static>,
            svelte::RenderTag<'static>,
            svelte::Root<'static>,
            svelte::Script<'static>,
            svelte::ScriptContext,
            svelte::SnippetBlock<'static>,
            svelte::SpecialElement<'static>,
            svelte::SpecialElementKind<'static>,
            svelte::SpecialElementTag,
            svelte::SpecialThis<'static>,
            svelte::SpreadAttribute<'static>,
            svelte::Style<'static>,
            svelte::StyleDirective<'static>,
            svelte::StyleDirectiveValue<'static>,
            svelte::SvelteOptions<'static>,
            svelte::Text,
            svelte::TextDecoding,
            svelte::TransitionDirection,
            svelte::TransitionDirective<'static>,
            svelte::UseDirective<'static>,
        ]
    );

    rows!(
        out,
        "css",
        [
            css::AngleUnit,
            css::AttributeMatcher,
            css::Color,
            css::ColorChannel,
            css::Combinator,
            css::ComplexSelector<'static>,
            css::ConditionConnector,
            css::ConditionPart<'static>,
            css::ConditionQuery<'static>,
            css::ConditionSegment<'static>,
            css::CssAtrule<'static>,
            css::CssAtruleBlock<'static>,
            css::CssBlockChild<'static>,
            css::CssDeclaration<'static>,
            css::CssNode<'static>,
            css::CssRule<'static>,
            css::CssStyleSheet<'static>,
            css::CssValue<'static>,
            css::PreludeValue<'static>,
            css::PseudoClassArgs<'static>,
            css::RelativeSelector<'static>,
            css::ScopeClause<'static>,
            css::ScopeLimit<'static>,
            css::SelectorList<'static>,
            css::SimpleSelector<'static>,
            css::StringCooked<'static>,
        ]
    );

    rows!(
        out,
        "lang",
        [
            tsv_lang::Comment,
            tsv_lang::ClassifiedComments<'static>,
            tsv_lang::CommentPosition,
            tsv_lang::EmbedContext,
            tsv_lang::LayoutMode,
            tsv_lang::ParseError,
            tsv_lang::Position,
            tsv_lang::Span,
            tsv_lang::doc::DocContext,
            tsv_lang::doc::DocText,
            tsv_lang::doc::LineKind,
            tsv_lang::doc::Mode,
            tsv_lang::doc::PoolSpan,
            tsv_lang::doc::arena::DocId,
        ]
    );

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// The board is a hand-written 250-row table, so the realistic failure is a
    /// copy-paste: one type boarded twice, or boarded into two groups. Either
    /// double-counts a family in the "widest per group" footer.
    #[test]
    fn every_type_is_boarded_once() {
        let mut seen: HashSet<&str> = HashSet::new();
        for row in board() {
            assert!(seen.insert(row.name), "{} is boarded twice", row.name);
        }
    }

    /// `stringify!` on a generic type spaces the lifetime out
    /// (`Expression < 'static >`); the name column must show the bare type.
    #[test]
    fn the_name_column_strips_the_lifetime() {
        let board = board();
        let named = |n: &str| board.iter().any(|r| r.name == n);
        assert!(named("ts::Expression"), "lifetime not stripped");
        assert!(named("ts::Statement"));
        // A type with no lifetime parameter takes the other arm of the split.
        assert!(named("tsv_lang::Span"));
    }

    /// A `Result<T, ParseError>` can never be narrower than `T`, so a row where
    /// it is means the board measured two unrelated types.
    #[test]
    fn a_result_is_never_narrower_than_its_payload() {
        for row in board() {
            assert!(
                row.result_size >= row.size,
                "{}: Result {} < payload {}",
                row.name,
                row.result_size,
                row.size
            );
        }
    }

    /// The niche flag is a claim about types wide enough to have had a choice —
    /// a one-byte enum widens by arithmetic and must not star.
    #[test]
    fn the_niche_flag_excludes_payloads_narrower_than_the_error() {
        let narrow = TypeRow {
            group: "test",
            name: "narrow",
            size: 1,
            align: 1,
            result_size: 16,
        };
        assert!(!narrow.pays_for_the_error());
        let wide = TypeRow {
            group: "test",
            name: "wide",
            size: 32,
            align: 8,
            result_size: 40,
        };
        assert!(wide.pays_for_the_error());
    }
}

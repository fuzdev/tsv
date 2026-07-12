//! Synthetic `tsv_ts` AST construction over the **hybrid appendix buffer**.
//!
//! The compiler generates JS by constructing `tsv_ts` internal-AST nodes and
//! printing them through the existing printer (`tsv_ts::format_canonical`).
//! That printer recovers most leaf text by slicing spans out of its `source`
//! argument, so the generator maintains a *buffer* = the host `.svelte` source
//! followed by an **appendix** of minted lexemes:
//!
//! - **Borrowed user subtrees** (script statements, template expressions) keep
//!   their real host spans — they print verbatim through the normal machinery.
//! - **Minted literals / template quasis** get spans pointing into the
//!   appendix, which contains their exact text (appended monotonically, so
//!   every span is in-bounds and on char boundaries).
//! - **Synthetic identifiers** ride the interned-name channel
//!   (`IdentName { escaped: Some(symbol), raw_len: 0 }`) — resolved through the
//!   shared interner, source-free. Their spans are still backed by minted text
//!   for debuggability (the appendix reads as the generated skeleton).
//! - Keywords/punctuation are printer statics and need no buffer text; the
//!   skeleton around them is minted anyway so node spans cover plausible text.
//!
//! Construction discipline: child collections are built as arena slices
//! (`bumpalo::collections::Vec` → `into_bump_slice`), single children via
//! `arena.alloc`. Borrowed nodes are never deep-copied; where a *wrapper* node
//! must be rebuilt with one field changed (the `$props()` init rewrite), the
//! by-value fields are shallow-cloned — children remain shared `&'arena` refs
//! into the parsed AST, and the original wrapper never enters the printed tree
//! (no duplicate spans in what the printer walks).

use bumpalo::Bump;
use tsv_lang::{SharedInterner, Span};
use tsv_ts::ast::internal::{
    CallExpression, Expression, IdentName, Identifier, ImportDeclaration, ImportKind,
    ImportNamespaceSpecifier, ImportPhase, ImportSpecifier, Literal, LiteralValue,
    MemberExpression, StringCooked, TemplateCooked, TemplateElement, TemplateLiteral,
};

/// The appendix-buffer bookkeeping plus interner access — everything node
/// constructors need. Owns the growing buffer; the arena and interner are
/// shared with the parsed host AST so borrowed subtrees and synthetic nodes
/// coexist in one graph.
pub(crate) struct Builder<'arena> {
    pub arena: &'arena Bump,
    /// Host source + appendix of minted lexemes. Passed to `format_canonical`
    /// as the source every span in the synthetic program indexes into.
    pub buffer: String,
    /// The parse's interner — synthetic identifier names are interned here so
    /// the printer's symbol resolver (built from `Program.interner`) finds them.
    pub interner: SharedInterner,
}

impl<'arena> Builder<'arena> {
    pub fn new(arena: &'arena Bump, host_source: &str, interner: SharedInterner) -> Self {
        Self {
            arena,
            buffer: host_source.to_string(),
            interner,
        }
    }

    /// Append minted text to the appendix, returning its span.
    pub fn mint(&mut self, text: &str) -> Span {
        let start = self.buffer.len() as u32;
        self.buffer.push_str(text);
        Span::new(start, self.buffer.len() as u32)
    }

    /// A synthetic identifier: interned name (source-free resolution) with its
    /// text minted into the appendix so the span is backed.
    pub fn ident(&mut self, name: &str) -> Identifier<'arena> {
        let span = self.mint(name);
        let ident_name = IdentName {
            escaped: Some(self.interner.borrow_mut().get_or_intern(name)),
            raw_len: 0,
        };
        Identifier::simple(ident_name, span)
    }

    /// A synthetic identifier as an arena-allocated expression.
    pub fn ident_expr(&mut self, name: &str) -> &'arena Expression<'arena> {
        let ident = self.ident(name);
        self.arena.alloc(Expression::Identifier(ident))
    }

    /// A single-quoted string literal minted into the appendix. `content` must
    /// not itself require escaping (module specifiers do not).
    pub fn string_literal(&mut self, content: &str) -> Literal<'arena> {
        debug_assert!(
            !content.contains(['\'', '\\', '\n']),
            "string_literal content must not need escaping: {content:?}"
        );
        let span = self.mint(&format!("'{content}'"));
        Literal {
            value: LiteralValue::String(StringCooked::Verbatim),
            span,
        }
    }

    /// `import * as <local> from '<specifier>';`
    pub fn import_namespace(&mut self, local: &str, specifier: &str) -> ImportDeclaration<'arena> {
        let start = self.mint("import * as ").start;
        let local = self.ident(local);
        let local_span = local.span;
        self.mint(" from ");
        let source = self.string_literal(specifier);
        let end = self.mint(";").end;
        let mut specifiers = bumpalo::collections::Vec::new_in(self.arena);
        specifiers.push(ImportSpecifier::Namespace(ImportNamespaceSpecifier {
            local,
            span: local_span,
        }));
        ImportDeclaration {
            specifiers: specifiers.into_bump_slice(),
            source,
            attributes: None,
            import_kind: ImportKind::Value,
            phase: ImportPhase::None,
            span: Span::new(start, end),
        }
    }

    /// `<object>.<property>(<arguments>)` — a call on a synthetic member chain.
    /// The arguments slice may hold borrowed expressions (host spans).
    pub fn member_call(
        &mut self,
        object: &str,
        property: &str,
        arguments: &'arena [Expression<'arena>],
    ) -> Expression<'arena> {
        let obj = self.ident_expr(object);
        self.mint(".");
        let prop = self.ident_expr(property);
        let member_span = Span::new(obj.span().start, prop.span().end);
        self.mint("(");
        let end = self.mint(")").end;
        let callee = self
            .arena
            .alloc(Expression::MemberExpression(MemberExpression {
                object: obj,
                property: prop,
                computed: false,
                optional: false,
                span: member_span,
            }));
        Expression::CallExpression(CallExpression {
            callee,
            type_arguments: None,
            arguments,
            optional: false,
            span: Span::new(member_span.start, end),
        })
    }

    /// A template literal from alternating static parts and expressions
    /// (`texts.len() == expressions.len() + 1`). Static text is minted into the
    /// appendix (already template-escaped by the caller); expressions may be
    /// borrowed user subtrees. The `${`/`}` delimiters are minted as
    /// placeholders so the appendix stays a readable mirror of the output — the
    /// printer emits them statically and never reads those bytes.
    pub fn template_literal(
        &mut self,
        texts: &[String],
        expressions: &'arena [Expression<'arena>],
    ) -> Expression<'arena> {
        debug_assert_eq!(texts.len(), expressions.len() + 1);
        let start = self.mint("`").start;
        let mut quasis = bumpalo::collections::Vec::new_in(self.arena);
        let last = texts.len() - 1;
        for (i, text) in texts.iter().enumerate() {
            let raw_span = self.mint(text);
            let tail = i == last;
            quasis.push(TemplateElement {
                raw_span,
                cooked: TemplateCooked::Verbatim,
                has_newline: text.contains('\n'),
                tail,
                span: raw_span,
            });
            if !tail {
                self.mint("${}");
            }
        }
        let end = self.mint("`").end;
        Expression::TemplateLiteral(TemplateLiteral {
            quasis: quasis.into_bump_slice(),
            expressions,
            span: Span::new(start, end),
        })
    }
}

/// Escape static text for inclusion in a template-literal quasi: backslash,
/// backtick, and `${` (the `$` is escaped only when a `{` follows).
pub(crate) fn escape_template_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => out.push_str("\\\\"),
            '`' => out.push_str("\\`"),
            '$' if chars.peek() == Some(&'{') => out.push_str("\\$"),
            _ => out.push(c),
        }
    }
    out
}

// Type-to-string conversion utilities for TypeScript
//
// Provides string serialization for all TypeScript type syntax. Used by:
// - expression_stringifier.rs (for type assertions, satisfies expressions, etc.)
// - Template literal type analysis (checking if types fit on a line)
//
// This is a "best-effort" serializer that reconstructs minimal string
// representations without comments or complex formatting.

use super::Printer;
use super::expressions::normalize_number_literal;
use crate::ast::internal::{
    self, TSLiteralType, TSMappedTypeModifier, TSType, TemplateLiteralType,
};
use tsv_lang::SymbolResolver;
use tsv_lang::printing::{StringFormatOptions, format_string_literal};

impl<'a> Printer<'a> {
    /// Convert a type annotation to a string (for inline building)
    pub(super) fn type_annotation_to_string(
        &self,
        annotation: &internal::TSTypeAnnotation,
    ) -> String {
        format!(": {}", self.type_to_string(&annotation.type_annotation))
    }

    /// Convert a type to a string (for inline building)
    pub(super) fn type_to_string(&self, ts_type: &TSType) -> String {
        match ts_type {
            TSType::Keyword(kw) => kw.kind.as_str().to_string(),
            TSType::Literal(lit) => self.literal_type_to_string(lit),
            TSType::Array(arr) => format!("{}[]", self.type_to_string(&arr.element_type)),
            TSType::Union(u) => u
                .types
                .iter()
                .map(|t| self.type_to_string(t))
                .collect::<Vec<_>>()
                .join(" | "),
            TSType::Intersection(i) => i
                .types
                .iter()
                .map(|t| self.type_to_string(t))
                .collect::<Vec<_>>()
                .join(" & "),
            TSType::TypeReference(r) => {
                let name = self.entity_name_to_string(&r.type_name);
                if let Some(type_args) = &r.type_arguments {
                    let args: Vec<_> = type_args
                        .params
                        .iter()
                        .map(|t| self.type_to_string(t))
                        .collect();
                    format!("{}<{}>", name, args.join(", "))
                } else {
                    name
                }
            }
            TSType::TypeLiteral(t) => t.span.extract(self.source).to_string(),
            TSType::Function(f) => f.span.extract(self.source).to_string(),
            TSType::Constructor(c) => c.span.extract(self.source).to_string(),
            TSType::Tuple(t) => {
                let elems: Vec<_> = t
                    .element_types
                    .iter()
                    .map(|e| self.type_to_string(e))
                    .collect();
                format!("[{}]", elems.join(", "))
            }
            TSType::Parenthesized(p) => {
                format!("({})", self.type_to_string(&p.type_annotation))
            }
            TSType::TypePredicate(p) => {
                let mut result = String::new();
                if p.asserts {
                    result.push_str("asserts ");
                }
                result.push_str(&self.resolve_symbol(p.parameter_name.name));
                if let Some(type_ann) = &p.type_annotation {
                    result.push_str(" is ");
                    result.push_str(&self.type_to_string(type_ann));
                }
                result
            }
            TSType::Conditional(c) => {
                format!(
                    "{} extends {} ? {} : {}",
                    self.type_to_string(&c.check_type),
                    self.type_to_string(&c.extends_type),
                    self.type_to_string(&c.true_type),
                    self.type_to_string(&c.false_type)
                )
            }
            TSType::Mapped(m) => self.mapped_type_to_string(m),
            TSType::TypeOperator(o) => {
                format!(
                    "{} {}",
                    o.operator.as_str(),
                    self.type_to_string(&o.type_annotation)
                )
            }
            TSType::Import(i) => {
                let mut result = format!("import({}", self.literal_to_string(&i.argument));
                // Import type options
                if let Some(options) = &i.options {
                    result.push_str(", ");
                    result.push_str(&self.expression_to_string(options));
                }
                result.push(')');
                if let Some(qualifier) = &i.qualifier {
                    result.push('.');
                    result.push_str(&self.entity_name_to_string(qualifier));
                }
                if let Some(type_args) = &i.type_arguments {
                    let args: Vec<_> = type_args
                        .params
                        .iter()
                        .map(|t| self.type_to_string(t))
                        .collect();
                    result.push('<');
                    result.push_str(&args.join(", "));
                    result.push('>');
                }
                result
            }
            TSType::TypeQuery(q) => {
                let mut result = String::from("typeof ");
                result.push_str(&self.type_query_expr_name_to_string(&q.expr_name));
                if let Some(type_args) = &q.type_arguments {
                    let args: Vec<_> = type_args
                        .params
                        .iter()
                        .map(|t| self.type_to_string(t))
                        .collect();
                    result.push('<');
                    result.push_str(&args.join(", "));
                    result.push('>');
                }
                result
            }
            TSType::IndexedAccess(i) => {
                format!(
                    "{}[{}]",
                    self.type_to_string(&i.object_type),
                    self.type_to_string(&i.index_type)
                )
            }
            TSType::Rest(r) => format!("...{}", self.type_to_string(&r.type_annotation)),
            TSType::Optional(o) => format!("{}?", self.type_to_string(&o.type_annotation)),
            TSType::NamedTupleMember(n) => {
                let label = self.resolve_symbol(n.label.name);
                let opt = if n.optional { "?" } else { "" };
                format!("{}{}: {}", label, opt, self.type_to_string(&n.element_type))
            }
            TSType::Infer(i) => {
                format!("infer {}", self.resolve_symbol(i.type_parameter.name.name))
            }
        }
    }

    /// Convert type query expression name to string
    fn type_query_expr_name_to_string(&self, expr_name: &internal::TSTypeQueryExprName) -> String {
        match expr_name {
            internal::TSTypeQueryExprName::EntityName(entity) => self.entity_name_to_string(entity),
            internal::TSTypeQueryExprName::Import(i) => {
                let mut result = format!("import({})", self.literal_to_string(&i.argument));
                if let Some(qualifier) = &i.qualifier {
                    result.push('.');
                    result.push_str(&self.entity_name_to_string(qualifier));
                }
                if let Some(type_args) = &i.type_arguments {
                    let args: Vec<_> = type_args
                        .params
                        .iter()
                        .map(|t| self.type_to_string(t))
                        .collect();
                    result.push('<');
                    result.push_str(&args.join(", "));
                    result.push('>');
                }
                result
            }
        }
    }

    /// Convert mapped type to string
    fn mapped_type_to_string(&self, m: &internal::TSMappedType) -> String {
        let mut result = String::from("{");

        // readonly modifier
        if let Some(readonly) = m.readonly {
            result.push_str(match readonly {
                TSMappedTypeModifier::True => "readonly ",
                TSMappedTypeModifier::Plus => "+readonly ",
                TSMappedTypeModifier::Minus => "-readonly ",
            });
        }

        // [K in constraint]
        result.push('[');
        result.push_str(&m.type_parameter.name);
        result.push_str(" in ");
        result.push_str(&self.type_to_string(&m.type_parameter.constraint));

        // as clause
        if let Some(name_type) = &m.name_type {
            result.push_str(" as ");
            result.push_str(&self.type_to_string(name_type));
        }

        result.push(']');

        // optional modifier
        if let Some(optional) = m.optional {
            result.push_str(match optional {
                TSMappedTypeModifier::True => "?",
                TSMappedTypeModifier::Plus => "+?",
                TSMappedTypeModifier::Minus => "-?",
            });
        }

        result.push_str(": ");

        // value type
        if let Some(type_ann) = &m.type_annotation {
            result.push_str(&self.type_to_string(type_ann));
        }

        result.push('}');
        result
    }

    /// Convert entity name to string
    pub(super) fn entity_name_to_string(&self, name: &internal::TSEntityName) -> String {
        match name {
            internal::TSEntityName::Identifier(id) => self.resolve_symbol(id.name),
            internal::TSEntityName::QualifiedName(qn) => {
                format!(
                    "{}.{}",
                    self.entity_name_to_string(&qn.left),
                    self.resolve_symbol(qn.right.name)
                )
            }
        }
    }

    /// Convert a literal to a string
    pub(super) fn literal_to_string(&self, literal: &internal::Literal) -> String {
        match &literal.value {
            internal::LiteralValue::Number(_) => {
                normalize_number_literal(literal.span.extract(self.source))
            }
            internal::LiteralValue::String { content: _, quote } => {
                let raw_literal = literal.span.extract(self.source);
                let raw_content = &raw_literal[1..raw_literal.len() - 1];
                format_string_literal(raw_content, *quote, StringFormatOptions::default())
            }
            internal::LiteralValue::BigInt(_) => {
                normalize_number_literal(literal.span.extract(self.source))
            }
            internal::LiteralValue::Boolean(b) => (if *b { "true" } else { "false" }).to_string(),
            internal::LiteralValue::Null => "null".to_string(),
            internal::LiteralValue::Undefined => "undefined".to_string(),
        }
    }

    /// Convert a literal type to a string
    fn literal_type_to_string(&self, lit: &TSLiteralType) -> String {
        match lit {
            TSLiteralType::TemplateLiteral(template) => {
                self.template_literal_type_to_string(template)
            }
            TSLiteralType::String(literal) => self.literal_to_string(literal),
            TSLiteralType::Number(literal) => self.literal_to_string(literal),
            TSLiteralType::BigInt(literal) => self.literal_to_string(literal),
            TSLiteralType::UnaryExpression(unary) => {
                format!(
                    "{}{}",
                    unary.operator.as_str(),
                    self.expression_to_string(&unary.argument)
                )
            }
        }
    }

    /// Convert a template literal type to a string
    fn template_literal_type_to_string(&self, template: &TemplateLiteralType) -> String {
        let mut result = String::from("`");
        for (i, quasi) in template.quasis.iter().enumerate() {
            result.push_str(&quasi.raw);
            if i < template.types.len() {
                result.push_str("${");
                result.push_str(&self.type_to_string(&template.types[i]));
                result.push('}');
            }
        }
        result.push('`');
        result
    }
}

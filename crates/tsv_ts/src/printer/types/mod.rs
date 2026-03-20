// Type annotation printing for TypeScript
//
// Handles printing of TypeScript-specific type syntax:
// - Type annotations (: Type)
// - Type keywords (number, string, boolean, etc.)
// - Complex types (unions, intersections, generics, etc.)
//
// This module coordinates type printing and delegates to specialized submodules:
// - helpers.rs: Standalone helper functions (parenthesization, unwrapping)
// - type_params.rs: Type parameter declarations and instantiation
// - type_annotation.rs: Type annotations (`: Type`)
// - type_members.rs: Type literal members (PropertySignature, MethodSignature, etc.)
// - type_literal.rs: Type literals (`{ a: T }`) and object alignment
// - function_types.rs: Function types, constructor types, signature params
// - union_intersection.rs: Union and intersection types
// - composite.rs: Conditional, mapped, tuple, array types
// - literal_types.rs: Literal types (string, number, template literal)

mod composite;
pub(in crate::printer) mod function_types;
pub(crate) mod helpers;
mod literal_types;
mod type_annotation;
mod type_literal;
mod type_members;
mod type_params;
mod union_intersection;

// Re-export public items from helpers
pub use helpers::{
    intersection_has_expanding_first_type, intersection_has_huggable_last_type,
    should_hug_union_type, unwrap_parenthesized,
};

// Re-export for submodules to use `super::X` instead of `super::super::X`
pub(super) use super::{CommentFilter, CommentSpacing, Printer};

use crate::ast::internal::TSType;
use helpers::type_needs_parens_for_indexed_access_object;
use helpers::type_needs_parens_for_prefix_operator;
use tsv_lang::SymbolToU32;
use tsv_lang::doc::arena::DocId;

impl<'a> Printer<'a> {
    //
    // Main Type Doc Builders
    //

    /// Build a Doc for a TypeScript type expression
    pub(in crate::printer) fn build_type_doc(&self, ts_type: &TSType) -> DocId {
        self.build_type_doc_inner(ts_type, false)
    }

    /// Build a Doc for a TypeScript type expression with wrapping type arguments.
    ///
    /// Used in type alias RHS where TypeReference type arguments should break
    /// internally (e.g., `Promise<LongType | null>` breaks inside `<>`).
    pub(in crate::printer) fn build_type_doc_with_wrapping_type_args(
        &self,
        ts_type: &TSType,
    ) -> DocId {
        self.build_type_doc_inner(ts_type, true)
    }

    /// Inner implementation for type doc building.
    /// When `wrap_type_args` is true, TypeReference uses wrapping type arguments.
    pub(super) fn build_type_doc_inner(&self, ts_type: &TSType, wrap_type_args: bool) -> DocId {
        let d = self.d();
        match ts_type {
            TSType::Keyword(kw) => d.text_owned(kw.kind.as_str().to_string()),
            TSType::Literal(lit) => self.build_literal_type_doc(lit),
            TSType::Array(arr) => self.build_array_type_doc(arr),
            TSType::Union(u) => self.build_union_type_doc(u, true),
            TSType::Intersection(i) => self.build_intersection_type_doc(i, true),
            TSType::TypeReference(r) => {
                let mut parts = vec![self.build_type_entity_name_doc(&r.type_name)];
                if let Some(type_args) = &r.type_arguments {
                    if wrap_type_args {
                        parts.push(self.build_type_arguments_doc_wrapping(type_args));
                    } else {
                        parts.push(self.build_type_arguments_doc(type_args));
                    }
                }
                d.concat(&parts)
            }
            TSType::TypeLiteral(t) => self.build_type_literal_doc(t),
            TSType::Function(f) => self.build_function_type_doc(f),
            TSType::Constructor(c) => self.build_constructor_type_doc(c),
            TSType::Tuple(t) => self.build_tuple_type_doc(t),
            // Parenthesized types: just unwrap. Parent contexts (IndexedAccess, Array,
            // TypeOperator) add parens when needed based on the inner type.
            TSType::Parenthesized(p) => self.build_type_doc(&p.type_annotation),
            TSType::TypePredicate(p) => {
                let mut parts = vec![];
                if p.asserts {
                    parts.push(d.text("asserts "));
                }
                parts.push(d.symbol(p.parameter_name.name.to_u32()));
                if let Some(type_ann) = &p.type_annotation {
                    parts.push(d.text(" is "));
                    parts.push(self.build_type_doc(type_ann));
                }
                d.concat(&parts)
            }
            TSType::Conditional(c) => {
                // Conditional types use width-aware wrapping:
                // When broken, ternary arms are indented:
                //   check extends extends_type
                //     ? true_type
                //     : false_type
                //
                // The outer-most conditional is wrapped in a group. Nested conditionals
                // (in true_type or false_type) are NOT wrapped in their own group - they
                // inherit breaking from the parent. This matches prettier's behavior.
                d.group(self.build_conditional_type_doc_inner(c))
            }
            TSType::Mapped(m) => self.build_mapped_type_doc(m),
            TSType::TypeOperator(o) => {
                let needs_parens = type_needs_parens_for_prefix_operator(&o.type_annotation);
                let operand_doc = self.build_type_doc(&o.type_annotation);
                if needs_parens {
                    d.concat(&[
                        d.text(o.operator.as_str()),
                        d.text(" ("),
                        operand_doc,
                        d.text(")"),
                    ])
                } else {
                    d.concat(&[d.text(o.operator.as_str()), d.text(" "), operand_doc])
                }
            }
            TSType::Import(i) => {
                let mut parts = vec![d.text("import(")];
                parts.push(self.build_literal_doc(&i.argument));
                // Import type options
                if let Some(options) = &i.options {
                    parts.push(d.text(", "));
                    parts.push(self.build_expression_doc(options));
                }
                parts.push(d.text(")"));
                if let Some(qualifier) = &i.qualifier {
                    parts.push(d.text("."));
                    parts.push(self.build_type_entity_name_doc(qualifier));
                }
                if let Some(type_args) = &i.type_arguments {
                    parts.push(self.build_type_arguments_doc(type_args));
                }
                d.concat(&parts)
            }
            TSType::TypeQuery(q) => {
                let mut parts = vec![d.text("typeof ")];
                parts.push(self.build_type_query_expr_name_doc(&q.expr_name));
                if let Some(type_args) = &q.type_arguments {
                    parts.push(self.build_type_arguments_doc(type_args));
                }
                d.concat(&parts)
            }
            TSType::IndexedAccess(i) => {
                let object_doc = self.build_type_doc(&i.object_type);
                let needs_parens = type_needs_parens_for_indexed_access_object(&i.object_type);
                if needs_parens {
                    d.concat(&[
                        d.text("("),
                        object_doc,
                        d.text(")["),
                        self.build_type_doc(&i.index_type),
                        d.text("]"),
                    ])
                } else {
                    d.concat(&[
                        object_doc,
                        d.text("["),
                        self.build_type_doc(&i.index_type),
                        d.text("]"),
                    ])
                }
            }
            TSType::Rest(r) => d.concat(&[d.text("..."), self.build_type_doc(&r.type_annotation)]),
            TSType::Optional(o) => {
                d.concat(&[self.build_type_doc(&o.type_annotation), d.text("?")])
            }
            TSType::NamedTupleMember(n) => {
                let mut parts = vec![d.symbol(n.label.name.to_u32())];
                if n.optional {
                    parts.push(d.text("?"));
                }
                parts.push(d.text(": "));
                parts.push(self.build_type_doc(&n.element_type));
                d.concat(&parts)
            }
            TSType::Infer(i) => d.concat(&[
                d.text("infer "),
                d.symbol(i.type_parameter.name.name.to_u32()),
            ]),
        }
    }
}

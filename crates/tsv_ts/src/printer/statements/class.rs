// Class declaration printing for TypeScript

use super::super::Printer;
use crate::ast::internal;
use tsv_lang::{SymbolResolver, doc};

impl<'a> Printer<'a> {
    /// Print a class declaration or anonymous class: `class Foo {}` or `class {}`
    pub(super) fn print_class_declaration(&mut self, decl: &internal::ClassDeclaration) {
        self.write("class");
        if let Some(id) = &decl.id {
            self.write(" ");
            self.print_identifier(id);
        }

        // Handle extends clause
        if let Some(super_class) = &decl.super_class {
            self.write(" extends ");
            self.print_expression(super_class);
        }

        self.write(" ");
        self.print_class_body(&decl.body);
    }

    /// Print a class body with blank line preservation
    pub(super) fn print_class_body(&mut self, body: &internal::ClassBody) {
        if body.body.is_empty() {
            self.write("{}");
            return;
        }

        self.write("{\n");
        self.indent_level += 1;

        let mut prev_end = 0u32;

        for (i, member) in body.body.iter().enumerate() {
            // Preserve blank lines between class members
            if i > 0
                && tsv_lang::printing::has_blank_line_between(
                    self.source,
                    prev_end,
                    member.span().start,
                )
            {
                self.write("\n");
            }

            self.write_indent();
            self.print_class_member(member);
            self.write("\n");

            prev_end = member.span().end;
        }

        self.indent_level -= 1;
        self.write_indent();
        self.write("}");
    }

    /// Print a class member (method or property)
    fn print_class_member(&mut self, member: &internal::ClassMember) {
        match member {
            internal::ClassMember::MethodDefinition(method) => {
                self.print_method_definition(method);
            }
            internal::ClassMember::PropertyDefinition(prop) => {
                self.print_property_definition(prop);
            }
        }
    }

    /// Print a property definition
    fn print_property_definition(&mut self, prop: &internal::PropertyDefinition) {
        // Print static modifier if applicable
        if prop.is_static {
            self.write("static ");
        }

        // Print key
        if prop.computed {
            self.write("[");
            self.print_expression(&prop.key);
            self.write("]");
        } else {
            self.print_expression(&prop.key);
        }

        // Print type annotation if present
        if let Some(type_annotation) = &prop.type_annotation {
            self.print_type_annotation(type_annotation);
        }

        // Print value if present
        if let Some(value) = &prop.value {
            self.write(" = ");
            self.print_expression(value);
        }

        self.write(";");
    }

    /// Print a method definition
    fn print_method_definition(&mut self, method: &internal::MethodDefinition) {
        // Print static modifier if applicable
        if method.is_static {
            self.write("static ");
        }

        // Print get/set for accessors
        match method.kind {
            internal::MethodKind::Get => self.write("get "),
            internal::MethodKind::Set => self.write("set "),
            _ => {}
        }

        // Print key
        if method.computed {
            self.write("[");
            self.print_expression(&method.key);
            self.write("]");
        } else {
            self.print_expression(&method.key);
        }

        // Print parameters (can be Identifier, ArrayPattern, ObjectPattern, AssignmentPattern)
        self.write("(");
        for (i, param) in method.value.params.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            self.print_expression(param);
        }
        self.write(")");

        // Print return type annotation if present
        if let Some(return_type) = &method.value.return_type {
            self.print_type_annotation(return_type);
        }

        self.write(" ");

        // Print body
        self.print_block_statement(&method.value.body);
    }

    /// Build a Doc for a class declaration
    pub(super) fn build_class_declaration_doc(
        &self,
        decl: &internal::ClassDeclaration,
    ) -> doc::Doc {
        let mut parts = vec![doc::text("class")];
        if let Some(id) = &decl.id {
            let id_str = self.resolve_symbol(id.name);
            parts.push(doc::text(" "));
            parts.push(doc::text(id_str));
        }

        // Handle extends clause
        if let Some(super_class) = &decl.super_class {
            parts.push(doc::text(" extends "));
            parts.push(self.build_expression_doc(super_class));
        }

        parts.push(doc::text(" "));
        parts.push(self.build_class_body_doc(&decl.body));

        doc::concat(parts)
    }

    /// Build a Doc for a class body
    fn build_class_body_doc(&self, body: &internal::ClassBody) -> doc::Doc {
        if body.body.is_empty() {
            return doc::text("{}");
        }

        let mut parts = vec![doc::text("{"), doc::hardline()];

        for (i, member) in body.body.iter().enumerate() {
            if i > 0 {
                parts.push(doc::hardline());
            }
            parts.push(doc::indent(self.build_class_member_doc(member)));
        }

        parts.push(doc::hardline());
        parts.push(doc::text("}"));

        doc::concat(parts)
    }

    /// Build a Doc for a class member
    fn build_class_member_doc(&self, member: &internal::ClassMember) -> doc::Doc {
        match member {
            internal::ClassMember::MethodDefinition(method) => {
                self.build_method_definition_doc(method)
            }
            internal::ClassMember::PropertyDefinition(prop) => {
                self.build_property_definition_doc(prop)
            }
        }
    }

    /// Build a Doc for a property definition
    fn build_property_definition_doc(&self, prop: &internal::PropertyDefinition) -> doc::Doc {
        let mut parts = vec![];

        // Static modifier
        if prop.is_static {
            parts.push(doc::text("static "));
        }

        // Key
        if prop.computed {
            parts.push(doc::text("["));
            parts.push(self.build_expression_doc(&prop.key));
            parts.push(doc::text("]"));
        } else {
            parts.push(self.build_expression_doc(&prop.key));
        }

        // Value if present
        if let Some(value) = &prop.value {
            parts.push(doc::text(" = "));
            parts.push(self.build_expression_doc(value));
        }

        parts.push(doc::text(";"));

        doc::concat(parts)
    }

    /// Build a Doc for a method definition
    fn build_method_definition_doc(&self, method: &internal::MethodDefinition) -> doc::Doc {
        let mut parts = vec![];

        // Static modifier
        if method.is_static {
            parts.push(doc::text("static "));
        }

        // Get/set for accessors
        match method.kind {
            internal::MethodKind::Get => parts.push(doc::text("get ")),
            internal::MethodKind::Set => parts.push(doc::text("set ")),
            _ => {}
        }

        // Key
        if method.computed {
            parts.push(doc::text("["));
            parts.push(self.build_expression_doc(&method.key));
            parts.push(doc::text("]"));
        } else {
            parts.push(self.build_expression_doc(&method.key));
        }

        // Parameters (can be Identifier, ArrayPattern, ObjectPattern, AssignmentPattern)
        parts.push(doc::text("("));
        for (i, param) in method.value.params.iter().enumerate() {
            if i > 0 {
                parts.push(doc::text(", "));
            }
            parts.push(self.build_expression_doc(param));
        }
        parts.push(doc::text(") "));

        // Body
        parts.push(self.build_block_statement_doc(&method.value.body));

        doc::concat(parts)
    }
}

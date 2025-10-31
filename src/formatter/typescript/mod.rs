// TypeScript formatter - converts internal AST back to formatted source code
//
// ## Architecture
//
// This module is organized by concern to support future expansion:
//
// - **mod.rs** (this file): Orchestration - coordinates formatting of programs
// - **statements.rs**: Statement formatting (declarations, control flow, etc.)
// - **expressions.rs**: Expression formatting (literals, identifiers, binary ops, etc.)
// - **types.rs**: Type annotation formatting (TypeScript-specific type syntax)
//
// ## Design Principles
//
// 1. **Match Prettier**: Format output matches prettier for compatibility
// 2. **Preserve Semantics**: Never change TypeScript semantics
// 3. **Modularity**: Each module has single responsibility for future maintainability

mod expressions;
mod statements;
mod types;

use crate::ast::internal;
use crate::formatter::Formatter;

impl Formatter {
    /// Format a TypeScript program
    pub fn format_program(&mut self, program: &internal::Program) {
        for (i, statement) in program.body.iter().enumerate() {
            if i > 0 {
                self.write("\n");
            }
            self.format_statement(statement);
        }
        // Add trailing newline (matches prettier)
        self.write("\n");
    }
}

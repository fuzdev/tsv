// Conversion from internal AST to public AST

use super::internal;
use super::public;
use tsv_lang::{LocationTracker, Span};

// Submodules
mod control_flow;
mod declarations;
mod expressions;
mod functions;
mod modules;
mod patterns;
mod statements;
mod types;

// Re-export conversion functions (pub(in crate::ast) for internal use)
pub(in crate::ast) use control_flow::*;
pub(in crate::ast) use declarations::*;
pub(in crate::ast) use functions::*;
pub(in crate::ast) use modules::*;
pub(in crate::ast) use patterns::*;
pub(in crate::ast) use statements::*;
pub(in crate::ast) use types::*;

// Public API exports
pub use expressions::convert_expression;

/// Convert tsv_lang::SourceLocation to public::SourceLocation
///
/// Converts from the generic location type to the TypeScript-specific public type
/// with serde derives.
#[inline]
pub(super) fn to_public_location(loc: tsv_lang::SourceLocation) -> public::SourceLocation {
    public::SourceLocation {
        start: public::Position {
            line: loc.start.line,
            column: loc.start.column,
        },
        end: public::Position {
            line: loc.end.line,
            column: loc.end.column,
        },
    }
}

/// Create source location, automatically handling offset if needed
///
/// Unified helper that eliminates repetitive if/else checks throughout conversion.
/// When offset is 0, uses fast path directly. When offset is non-zero, adjusts span accordingly.
#[inline]
pub(super) fn create_location(
    span: Span,
    tracker: &LocationTracker,
    offset: usize,
) -> public::SourceLocation {
    let loc = if offset == 0 {
        tracker.span_to_location(span)
    } else {
        tracker.span_to_location_with_offset(span, offset)
    };
    to_public_location(loc)
}

pub fn convert_program(
    program: &internal::Program,
    source: &str,
    loc: &LocationTracker,
) -> public::Program {
    convert_program_with_offset(program, source, loc, 0)
}

// Convert Program with position offset for embedded content
pub fn convert_program_with_offset(
    program: &internal::Program,
    source: &str,
    loc: &LocationTracker,
    offset: usize,
) -> public::Program {
    let interner = program.interner.borrow();

    public::Program {
        node_type: "Program".to_string(),
        start: program.span.start,
        end: program.span.end,
        loc: create_location(program.span, loc, offset),
        body: program
            .body
            .iter()
            .map(|s| convert_statement(s, source, loc, &interner, offset))
            .collect(),
        source_type: "module".to_string(),
    }
}

// Language-level semantics and shared utilities
//
// This module contains language-specific information and utilities that are
// needed by multiple tools (formatter, linter, type-checker, language server, etc.)
// and are independent of any specific implementation.
//
// Examples:
// - HTML element classification (inline, block, void, whitespace-preserving)
// - HTML nesting rules and constraints
// - Language-specific metadata and properties

pub mod html;

//! Per-phase validation functions (P* parser, F* formatter, N* normalization).
//!
//! Each phase appends errors/successes to the shared `FixtureValidation`;
//! `validate_fixture` in mod.rs orchestrates the sequence.

mod formatter;
mod normalization;
mod parser;

pub(super) use parser::{
    validate_invalid_syntax, validate_parser_external, validate_parser_ours,
    validate_parser_ours_matches_expected,
};

pub(super) use formatter::{
    validate_formatter_idempotent, validate_formatter_prettier, validate_prettier_nonconvergent,
    validate_prettier_rejects,
};

pub(super) use normalization::{validate_normalization_ours, validate_normalization_prettier};

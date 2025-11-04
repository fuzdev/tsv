pub mod commands;

use commands::{
    ast_diff::AstDiffCommand, canonical_parse::CanonicalParseCommand, compare::CompareCommand,
    fixtures_update_expected::FixturesUpdateExpectedCommand,
    fixtures_update_formatted::FixturesUpdateFormattedCommand,
    fixtures_validate::FixturesValidateCommand, format_prettier::FormatPrettierCommand,
};
use tsv_cli::cli::commands::CommandRegistry;

/// Build and return the command registry with debug commands
pub fn build_registry() -> CommandRegistry {
    let mut registry = CommandRegistry::new();

    // Register debug commands
    registry.register(Box::new(CompareCommand));
    registry.register(Box::new(AstDiffCommand));

    // Register parser commands
    registry.register(Box::new(CanonicalParseCommand));
    registry.register(Box::new(FormatPrettierCommand));

    // Register fixture management commands
    registry.register(Box::new(FixturesUpdateExpectedCommand));
    registry.register(Box::new(FixturesUpdateFormattedCommand));
    registry.register(Box::new(FixturesValidateCommand));

    registry
}

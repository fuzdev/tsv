pub mod commands;

use commands::{
    ast_diff::AstDiffCommand, canonical_parse::CanonicalParseCommand, compare::CompareCommand,
    fixtures_update::FixturesUpdateCommand,
    fixtures_update_formatted::FixturesUpdateFormattedCommand,
    fixtures_update_parsed::FixturesUpdateParsedCommand,
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
    registry.register(Box::new(FixturesUpdateCommand));
    registry.register(Box::new(FixturesUpdateParsedCommand));
    registry.register(Box::new(FixturesUpdateFormattedCommand));
    registry.register(Box::new(FixturesValidateCommand));

    registry
}

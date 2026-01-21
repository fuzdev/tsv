pub mod commands;
pub mod input_parser;

use commands::{
    ast_diff::AstDiffCommand, canonical_parse::CanonicalParseCommand, check::CheckCommand,
    compare::CompareCommand, fixtures_update::FixturesUpdateCommand,
    fixtures_update_formatted::FixturesUpdateFormattedCommand,
    fixtures_update_parsed::FixturesUpdateParsedCommand,
    fixtures_validate::FixturesValidateCommand, format_prettier::FormatPrettierCommand,
    line_width::LineWidthCommand, test262::Test262Command,
};
use tsv_cli::cli::commands::CommandRegistry;

/// Build and return the command registry with debug commands
pub fn build_registry() -> CommandRegistry {
    let mut registry = CommandRegistry::new();

    // Register utility commands
    registry.register(Box::new(CheckCommand));

    // Register debug commands
    registry.register(Box::new(CompareCommand));
    registry.register(Box::new(AstDiffCommand));
    registry.register(Box::new(LineWidthCommand));

    // Register parser commands
    registry.register(Box::new(CanonicalParseCommand));
    registry.register(Box::new(FormatPrettierCommand));

    // Register fixture management commands
    registry.register(Box::new(FixturesUpdateCommand));
    registry.register(Box::new(FixturesUpdateParsedCommand));
    registry.register(Box::new(FixturesUpdateFormattedCommand));
    registry.register(Box::new(FixturesValidateCommand));

    // Register test262 command
    registry.register(Box::new(Test262Command));

    registry
}

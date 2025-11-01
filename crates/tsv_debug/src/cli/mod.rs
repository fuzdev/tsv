pub mod commands;

use commands::{
    ast_diff::AstDiffCommand, compare::CompareCommand,
    fixtures_check_formatted::FixturesCheckFormattedCommand,
    fixtures_update_expected::FixturesUpdateExpectedCommand,
    fixtures_update_formatted::FixturesUpdateFormattedCommand,
    format_prettier::FormatPrettierCommand, parse_svelte::ParseSvelteCommand,
    parse_typescript::ParseTypeScriptCommand,
};
use tsv_cli::cli::commands::CommandRegistry;

/// Build and return the command registry with debug commands
pub fn build_registry() -> CommandRegistry {
    let mut registry = CommandRegistry::new();

    // Register debug commands
    registry.register(Box::new(CompareCommand));
    registry.register(Box::new(AstDiffCommand));

    // Register parser commands
    registry.register(Box::new(ParseSvelteCommand));
    registry.register(Box::new(ParseTypeScriptCommand));
    registry.register(Box::new(FormatPrettierCommand));

    // Register fixture management commands
    registry.register(Box::new(FixturesUpdateExpectedCommand));
    registry.register(Box::new(FixturesUpdateFormattedCommand));
    registry.register(Box::new(FixturesCheckFormattedCommand));

    registry
}

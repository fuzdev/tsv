pub mod commands;

use commands::{
    compare::CompareCommand,
    fixtures_check_formatted::FixturesCheckFormattedCommand,
    fixtures_update_expected::FixturesUpdateExpectedCommand,
    fixtures_update_formatted::FixturesUpdateFormattedCommand,
};
use tsv_cli::cli::commands::CommandRegistry;

/// Build and return the command registry with debug commands
pub fn build_registry() -> CommandRegistry {
    let mut registry = CommandRegistry::new();

    // Register debug commands
    registry.register(Box::new(CompareCommand));

    // Register fixture management commands
    registry.register(Box::new(FixturesUpdateExpectedCommand));
    registry.register(Box::new(FixturesUpdateFormattedCommand));
    registry.register(Box::new(FixturesCheckFormattedCommand));

    registry
}

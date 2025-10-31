pub mod args;
pub mod commands;
pub mod input;

use commands::{CommandRegistry, format::FormatCommand, parse::ParseCommand};

/// Build and return the command registry with all available commands
pub fn build_registry() -> CommandRegistry {
    let mut registry = CommandRegistry::new();

    // Register commands
    registry.register(Box::new(ParseCommand));
    registry.register(Box::new(FormatCommand));

    // TODO: Future commands:
    // registry.register(Box::new(TokensCommand));  // Show lexer token stream (debugging)
    // registry.register(Box::new(CheckCommand));   // Validate syntax only (fast check)
    // registry.register(Box::new(BenchCommand));   // Parse N times, show timing

    registry
}

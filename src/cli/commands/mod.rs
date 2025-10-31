use super::args::Args;
use std::process;

/// Trait for CLI commands
pub trait Command {
    /// Command name (e.g., "parse", "format")
    fn name(&self) -> &str;

    /// Parse arguments and return executable instance
    fn parse_args(&self, args: &mut Args) -> Result<Box<dyn Executable>, String>;

    /// Usage documentation for this command
    fn usage(&self) -> Vec<String>;
}

/// Trait for executable command instances
pub trait Executable {
    /// Execute the command
    fn execute(&self);
}

/// Registry of available commands
pub struct CommandRegistry {
    commands: Vec<Box<dyn Command>>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }

    /// Register a command
    pub fn register(&mut self, command: Box<dyn Command>) {
        self.commands.push(command);
    }

    /// Find command by name
    pub fn find(&self, name: &str) -> Option<&dyn Command> {
        self.commands
            .iter()
            .find(|cmd| cmd.name() == name)
            .map(|b| &**b)
    }

    /// Parse args and execute command
    pub fn run(&self, args: Vec<String>) {
        if args.len() < 2 {
            self.print_usage(&args[0]);
            process::exit(1);
        }

        let command_name = &args[1];
        let command = match self.find(command_name) {
            Some(cmd) => cmd,
            None => {
                eprintln!("Unknown command: '{}'", command_name);
                eprintln!();
                self.print_usage(&args[0]);
                process::exit(1);
            }
        };

        // Build Args from args[2..] (skip program name and command name)
        let mut parsed_args = Args::new(args[2..].to_vec());

        match command.parse_args(&mut parsed_args) {
            Ok(executable) => executable.execute(),
            Err(err) => {
                eprintln!("Error: {}", err);
                eprintln!();
                self.print_command_usage(&args[0], command);
                process::exit(1);
            }
        }
    }

    /// Print usage for all commands
    fn print_usage(&self, program: &str) {
        eprintln!("Usage: {} <command> [options]", program);
        eprintln!();
        eprintln!("Commands:");
        for cmd in &self.commands {
            for line in cmd.usage() {
                eprintln!("  {}", line);
            }
        }
        eprintln!();
        eprintln!("Use '{} <command> --help' for more information", program);
    }

    /// Print usage for specific command
    fn print_command_usage(&self, program: &str, command: &dyn Command) {
        eprintln!("Usage: {} {}", program, command.name());
        for line in command.usage() {
            eprintln!("  {}", line);
        }
    }
}

pub mod format;
pub mod parse;

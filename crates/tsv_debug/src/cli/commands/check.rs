//! check command - verify Deno sidecar is available

use tsv_cli::cli::args::Args;
use tsv_cli::cli::commands::{Command, Executable};

/// check command - verify Deno sidecar is available and show version info
pub struct CheckCommand;

impl Command for CheckCommand {
    fn name(&self) -> &str {
        "check"
    }

    fn parse_args(&self, _args: &mut Args) -> Result<Box<dyn Executable>, String> {
        Ok(Box::new(CheckExecutable))
    }

    fn usage(&self) -> Vec<String> {
        vec![
            "check                                              Verify Deno sidecar is available"
                .to_string(),
        ]
    }
}

struct CheckExecutable;

impl Executable for CheckExecutable {
    fn execute(&self) {
        let rt = super::create_runtime();
        match rt.block_on(crate::deno::check()) {
            Ok(info) => {
                println!("Deno sidecar: ok");
                println!();
                println!("Runtime:");
                println!("  deno:       {}", info.deno);
                println!("  typescript: {}", info.typescript);
                println!();
                println!("Dependencies:");
                println!("  prettier:              {}", info.prettier);
                println!("  prettier-plugin-svelte: {}", info.prettier_plugin_svelte);
                println!("  svelte:                {}", info.svelte);
                println!("  acorn:                 {}", info.acorn);
                println!("  acorn-typescript:      {}", info.acorn_typescript);
            }
            Err(e) => {
                eprintln!("Deno sidecar: error");
                eprintln!();
                eprintln!("Error: {e}");
                let hint = e.hint();
                if !hint.is_empty() {
                    eprintln!("Hint: {hint}");
                }
                std::process::exit(1);
            }
        }
    }
}

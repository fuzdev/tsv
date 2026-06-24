//! check command - verify Deno sidecar is available

use crate::cli::CliError;
use argh::FromArgs;

/// Verify Deno sidecar is available and show version info.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "check")]
pub struct CheckCommand {}

impl CheckCommand {
    pub(crate) fn run(self) -> Result<(), CliError> {
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
                Ok(())
            }
            Err(e) => {
                eprintln!("Deno sidecar: error");
                eprintln!();
                eprintln!("Error: {e}");
                let hint = e.hint();
                if !hint.is_empty() {
                    eprintln!("Hint: {hint}");
                }
                Err(CliError::Failed)
            }
        }
    }
}

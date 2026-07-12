use crate::cli::CliError;
use crate::compile_fixtures::{
    EXPECTED_CSS, EXPECTED_SERVER_JS, INPUT_FILE, with_trailing_newline,
};
use crate::deno::{self, SvelteGenerate};
use crate::fixtures::{self, InputType};
use argh::FromArgs;
use std::path::Path;
use tsv_svelte_compile::canonicalize_js;

/// Create or reinitialize a compile fixture (prettier-formats the component,
/// compiles it with the canonical Svelte compiler, writes the canonicalized
/// server JS and the CSS).
///
/// Content sources (in priority order): `--content`, `--stdin`, existing
/// `input.svelte`. Expected files are ALWAYS oracle-generated, never
/// hand-written. The input must be a runes component — the oracle rejects
/// legacy syntax.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "compile_fixture_init")]
pub struct CompileFixtureInitCommand {
    /// overwrite existing input file
    #[argh(switch)]
    force: bool,

    /// read content from stdin (for heredocs and pipes)
    #[argh(switch)]
    stdin: bool,

    /// content string
    #[argh(option)]
    content: Option<String>,

    /// fixture directory path
    #[argh(positional)]
    dir: String,
}

impl CompileFixtureInitCommand {
    pub(crate) fn run(self) -> Result<(), CliError> {
        let rt = super::create_runtime();
        rt.block_on(self.run_async())
    }

    async fn run_async(self) -> Result<(), CliError> {
        let dir = Path::new(&self.dir);

        let raw_content =
            match resolve_content(self.content.as_deref(), self.stdin, self.force, dir) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Error: {e}");
                    return Err(CliError::Failed);
                }
            };

        if let Err(e) = std::fs::create_dir_all(dir) {
            eprintln!("Error creating directory {dir:?}: {e}");
            return Err(CliError::Failed);
        }

        // Prettier-format the component so the committed input is canonical.
        let formatted =
            match deno::run_prettier(&raw_content, InputType::Svelte.prettier_parser()).await {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("Error: prettier formatting failed: {e}");
                    eprintln!("Hint: {}", e.hint());
                    return Err(CliError::Failed);
                }
            };

        let input_path = dir.join(INPUT_FILE);
        if let Err(e) = fixtures::write_file(&input_path, &formatted) {
            eprintln!("Error writing {INPUT_FILE}: {e}");
            return Err(CliError::Failed);
        }
        println!("✓ {INPUT_FILE} (prettier-formatted)");

        // Compile with the deterministic oracle (server, non-dev — the parity
        // configuration `compile_compare` and validation use).
        let compiled = match deno::svelte_compile(&formatted, SvelteGenerate::Server, false).await {
            Ok(o) => o,
            Err(e) => {
                eprintln!("Error: oracle compile failed: {e}");
                eprintln!("Hint: {}", e.hint());
                eprintln!("({INPUT_FILE} was written; fix the component and re-run)");
                return Err(CliError::Failed);
            }
        };

        // Canonicalize the oracle JS — the committed expectation is the
        // intent-erased reprint, the same form the parity comparison uses.
        let canonical = match canonicalize_js(&compiled.js) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Error: could not canonicalize oracle output: {e}");
                return Err(CliError::Failed);
            }
        };
        let js_path = dir.join(EXPECTED_SERVER_JS);
        if let Err(e) = fixtures::write_file(&js_path, &canonical) {
            eprintln!("Error writing {EXPECTED_SERVER_JS}: {e}");
            return Err(CliError::Failed);
        }
        println!("✓ {EXPECTED_SERVER_JS} (canonicalized oracle output)");

        // CSS: written only when the oracle produced any; a stale expected.css
        // from a previous styled revision is removed.
        let css_path = dir.join(EXPECTED_CSS);
        match &compiled.css {
            Some(css) => {
                if let Err(e) = fixtures::write_file(&css_path, &with_trailing_newline(css)) {
                    eprintln!("Error writing {EXPECTED_CSS}: {e}");
                    return Err(CliError::Failed);
                }
                println!("✓ {EXPECTED_CSS} (raw oracle css)");
            }
            None => {
                if css_path.exists() {
                    if let Err(e) = std::fs::remove_file(&css_path) {
                        eprintln!("Error removing stale {EXPECTED_CSS}: {e}");
                        return Err(CliError::Failed);
                    }
                    println!("✓ removed stale {EXPECTED_CSS} (component is unstyled)");
                }
            }
        }

        println!("\nCompile fixture initialized: {}", self.dir);
        Ok(())
    }
}

/// Resolve content from --content, --stdin, or the existing input file.
fn resolve_content(
    content_flag: Option<&str>,
    use_stdin: bool,
    force: bool,
    dir: &Path,
) -> Result<String, String> {
    let input_path = dir.join(INPUT_FILE);
    if let Some(content) = content_flag {
        if !force && input_path.exists() {
            return Err("Input file already exists. Use --force to overwrite.".to_string());
        }
        return Ok(content.to_string());
    }
    if use_stdin {
        if !force && input_path.exists() {
            return Err("Input file already exists. Use --force to overwrite.".to_string());
        }
        let mut buffer = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut buffer)
            .map_err(|e| format!("Failed to read stdin: {e}"))?;
        if buffer.is_empty() {
            return Err("No content received from stdin.".to_string());
        }
        return Ok(buffer);
    }
    if input_path.exists() {
        return fixtures::read_file(&input_path);
    }
    Err(
        "No content source. Provide --content, --stdin (heredoc), or ensure input.svelte exists."
            .to_string(),
    )
}

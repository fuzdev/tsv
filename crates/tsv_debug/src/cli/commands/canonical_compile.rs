use crate::cli::CliError;
use crate::cli::commands::{print_block, print_json_tabs};
use crate::deno::{self, SvelteCompileOutput, SvelteGenerate};
use argh::FromArgs;
use tsv_cli::cli::input::{InputArgs, ParserType};

/// Compile Svelte with the canonical Svelte compiler (the deterministic oracle).
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "canonical_compile")]
#[allow(clippy::struct_excessive_bools)] // independent CLI flags
pub struct CanonicalCompileCommand {
    /// compile target: server | client (default: server)
    #[argh(option, default = "SvelteGenerate::Server")]
    target: SvelteGenerate,

    /// also print the compiled CSS
    #[argh(switch)]
    css: bool,

    /// development-mode output
    #[argh(switch)]
    dev: bool,

    /// emit { js, css, warnings } as JSON
    #[argh(switch)]
    json: bool,

    /// content to compile
    #[argh(option)]
    content: Option<String>,

    /// read from stdin
    #[argh(switch)]
    stdin: bool,

    /// file path
    #[argh(positional)]
    file: Option<String>,
}

impl CanonicalCompileCommand {
    pub(crate) fn run(self) -> Result<(), CliError> {
        // Compile is Svelte-only, so force the parser: --content/--stdin don't
        // require an explicit --parser, and a file argument's extension is ignored.
        let (input, _parser) = super::resolve_input_or_fail(
            InputArgs {
                content: self.content,
                stdin: self.stdin,
                parser: Some(ParserType::Svelte),
                file: self.file,
            },
            CliError::Failed,
        )?;

        let rt = super::create_runtime();
        match rt.block_on(deno::svelte_compile(input.content(), self.target, self.dev)) {
            Ok(output) => print_output(&output, self.css, self.json),
            Err(err) => {
                eprintln!("Error compiling: {err}");
                Err(CliError::Failed)
            }
        }
    }
}

/// Print the compile output: JSON when `as_json`, else the JS (and the CSS when
/// `show_css`, under a delimiting comment).
fn print_output(
    output: &SvelteCompileOutput,
    show_css: bool,
    as_json: bool,
) -> Result<(), CliError> {
    if as_json {
        print_json_tabs(output, CliError::Failed)
    } else {
        print_block(&output.js);
        if show_css {
            println!("/* --- css --- */");
            match &output.css {
                Some(css) => print_block(css),
                None => println!("/* (none) */"),
            }
        }
        Ok(())
    }
}

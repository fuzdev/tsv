/// Helpers for running Deno scripts with npm imports
use std::io::Write;
use std::process::{Command, Stdio};

/// NPM package versions used by tsv_debug
/// These are the single source of truth for version pinning
pub const PRETTIER_VERSION: &str = "^3.6.2";
pub const PRETTIER_SVELTE_VERSION: &str = "^3.4.0";
pub const SVELTE_VERSION: &str = "^5.43.0";
pub const ACORN_VERSION: &str = "^8.14.0";
pub const ACORN_TYPESCRIPT_VERSION: &str = "^1.0.1";

/// Run a Deno script via stdin with the given permissions and arguments
///
/// # Arguments
/// * `script` - The JavaScript/TypeScript code to execute
/// * `permissions` - Deno permissions (e.g., `["--allow-read", "--allow-env"]`)
/// * `args` - Arguments passed to the script via `Deno.args`
///
/// # Returns
/// * `Ok(stdout)` on success
/// * `Err(stderr)` on failure
pub fn run_script(script: &str, permissions: &[&str], args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new("deno");
    cmd.arg("run");

    // Add permissions
    for perm in permissions {
        cmd.arg(perm);
    }

    // Script from stdin
    cmd.arg("-");

    // Add arguments
    for arg in args {
        cmd.arg(arg);
    }

    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn deno: {}", e))?;

    // Write script to stdin
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(script.as_bytes())
            .map_err(|e| format!("Failed to write script to stdin: {}", e))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("Failed to wait for deno: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

/// Standard permissions for prettier operations
pub const PRETTIER_PERMISSIONS: &[&str] = &["--allow-read", "--allow-env", "--allow-sys"];

/// Standard permissions for Svelte/acorn parser operations
pub const PARSER_PERMISSIONS: &[&str] = &["--allow-read", "--allow-env"];

/// Build a Deno script that formats code with prettier
///
/// # Arguments
/// * `filepath` - Virtual filepath for prettier to infer parser (e.g., "temp.svelte")
///
/// # Script arguments (via Deno.args)
/// * `args[0]` - filepath
/// * `args[1]` - input content to format
///
/// # Returns
/// The JavaScript code as a string
pub fn prettier_format_script() -> String {
    format!(
        r#"
import {{ format }} from 'npm:prettier@{}';
import prettierPluginSvelte from 'npm:prettier-plugin-svelte@{}';

const filepath = Deno.args[0];
const input = Deno.args[1];

try {{
    // Match the config in package.json
    const config = {{
        plugins: [prettierPluginSvelte],
        useTabs: true,
        printWidth: 100,
        singleQuote: true,
        bracketSpacing: false,
        filepath: filepath,
    }};
    const output = await format(input, config);
    Deno.stdout.writeSync(new TextEncoder().encode(output));
}} catch (err) {{
    Deno.stderr.writeSync(new TextEncoder().encode(err.message + '\n'));
    Deno.exit(1);
}}
"#,
        PRETTIER_VERSION, PRETTIER_SVELTE_VERSION
    )
}

/// Run prettier on input content via Deno
///
/// # Arguments
/// * `content` - The code to format
/// * `filepath` - Virtual filepath for prettier parser detection (e.g., "temp.svelte", "temp.ts")
pub fn run_prettier(content: &str, filepath: &str) -> Result<String, String> {
    let script = prettier_format_script();
    run_script(&script, PRETTIER_PERMISSIONS, &[filepath, content])
}

/// Build a Deno script that parses Svelte code using the official parser
///
/// # Script arguments (via Deno.args)
/// * `args[0]` - input source code
///
/// # Returns
/// JSON AST on success
pub fn svelte_parse_script() -> String {
    format!(
        r#"
import {{ parse }} from 'npm:svelte@{}/compiler';

const source = Deno.args[0];

try {{
    const ast = parse(source, {{ modern: true }});
    const json = JSON.stringify(ast, null, 2);
    Deno.stdout.writeSync(new TextEncoder().encode(json));
}} catch (err) {{
    Deno.stderr.writeSync(new TextEncoder().encode(err.message + '\n'));
    Deno.exit(1);
}}
"#,
        SVELTE_VERSION
    )
}

/// Parse Svelte source code using the official Svelte compiler
///
/// # Arguments
/// * `source` - The Svelte source code
///
/// # Returns
/// JSON AST as a string
pub fn parse_svelte(source: &str) -> Result<String, String> {
    let script = svelte_parse_script();
    run_script(&script, PARSER_PERMISSIONS, &[source])
}

/// Build a Deno script that parses TypeScript using acorn + acorn-typescript
///
/// # Script arguments (via Deno.args)
/// * `args[0]` - input source code
///
/// # Returns
/// JSON AST on success
pub fn acorn_parse_script() -> String {
    format!(
        r#"
import * as acorn from 'npm:acorn@{}';
import {{ tsPlugin }} from 'npm:@sveltejs/acorn-typescript@{}';

const source = Deno.args[0];

// Create parser with TypeScript support, matching Svelte's configuration
const ParserWithTS = acorn.Parser.extend(tsPlugin());

try {{
    const ast = ParserWithTS.parse(source, {{
        sourceType: 'module',
        ecmaVersion: 16,
        locations: true,
    }});
    const json = JSON.stringify(ast, null, 2);
    Deno.stdout.writeSync(new TextEncoder().encode(json));
}} catch (err) {{
    Deno.stderr.writeSync(new TextEncoder().encode(err.message + '\n'));
    Deno.exit(1);
}}
"#,
        ACORN_VERSION, ACORN_TYPESCRIPT_VERSION
    )
}

/// Parse TypeScript source code using acorn with TypeScript plugin
///
/// # Arguments
/// * `source` - The TypeScript source code
///
/// # Returns
/// JSON AST as a string
pub fn parse_typescript(source: &str) -> Result<String, String> {
    let script = acorn_parse_script();
    run_script(&script, PARSER_PERMISSIONS, &[source])
}

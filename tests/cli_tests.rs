/// Integration tests for CLI commands
/// Tests the full command execution flow without spawning processes
use std::process::Command;

#[test]
fn test_parse_command_with_content() {
    let output = Command::new("cargo")
        .args(&[
            "run",
            "-p",
            "tsv_cli",
            "-q",
            "parse",
            "--content",
            "const x = 42;",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success(), "Parse command should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(r#""type":"Program"#),
        "Should output AST JSON"
    );
    assert!(stdout.contains(r#""type":"VariableDeclaration"#));
}

#[test]
fn test_parse_command_with_pretty() {
    let output = Command::new("cargo")
        .args(&[
            "run",
            "-p",
            "tsv_cli",
            "-q",
            "parse",
            "--content",
            "const x = 42;",
            "--pretty",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success(), "Parse command should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Pretty output should have newlines
    assert!(stdout.contains('\n'), "Pretty output should be formatted");
}

#[test]
fn test_format_command_typescript() {
    let output = Command::new("cargo")
        .args(&[
            "run",
            "-p",
            "tsv_cli",
            "-q",
            "format",
            "--content",
            "const    x    =    42;",
            "--parser",
            "typescript",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success(), "Format command should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should normalize whitespace
    assert!(
        stdout.contains("const x = 42;"),
        "Should format TypeScript code"
    );
}

#[test]
fn test_format_command_svelte() {
    let output = Command::new("cargo")
        .args(&[
            "run",
            "-p",
            "tsv_cli",
            "-q",
            "format",
            "--content",
            "<div>test</div>",
            "--parser",
            "svelte",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success(), "Format command should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("<div>test</div>"),
        "Should format Svelte code"
    );
}

#[test]
fn test_format_command_css() {
    let output = Command::new("cargo")
        .args(&[
            "run",
            "-p",
            "tsv_cli",
            "-q",
            "format",
            "--content",
            "body{color:red;}",
            "--parser",
            "css",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success(), "Format command should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should format CSS with proper spacing
    assert!(stdout.contains("color: red;"), "Should format CSS code");
}

#[test]
fn test_unknown_command() {
    let output = Command::new("cargo")
        .args(&["run", "-p", "tsv_cli", "-q", "unknown-command"])
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success(), "Unknown command should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Unknown command"),
        "Should report unknown command"
    );
}

#[test]
fn test_parse_invalid_syntax() {
    let output = Command::new("cargo")
        .args(&[
            "run",
            "-p",
            "tsv_cli",
            "-q",
            "parse",
            "--content",
            "const x = ",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success(), "Invalid syntax should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Parse error") || stderr.contains("error"),
        "Should report parse error"
    );
}

#[test]
fn test_format_missing_parser() {
    let output = Command::new("cargo")
        .args(&[
            "run",
            "-p",
            "tsv_cli",
            "-q",
            "format",
            "--content",
            "<div>test</div>",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(
        !output.status.success(),
        "Format without --parser should fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--parser") || stderr.contains("Error"),
        "Should report missing parser option"
    );
}

#[test]
fn test_no_command() {
    let output = Command::new("cargo")
        .args(&["run", "-p", "tsv_cli", "-q"])
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success(), "No command should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Usage"), "Should show usage message");
}

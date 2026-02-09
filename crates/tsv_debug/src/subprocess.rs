//! Subprocess utilities for running external commands

use crate::error::{DebugError, Result};
use std::process::Command;

/// Run tsv_cli format command with content and parser type
pub fn run_tsv_format(content: &str, parser: &str) -> Result<String> {
    let child = Command::new("cargo")
        .args([
            "run",
            "-p",
            "tsv_cli",
            "--quiet",
            "--",
            "format",
            "--content",
        ])
        .arg(content)
        .args(["--parser", parser])
        .stderr(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::piped())
        .spawn()?;

    let output = child.wait_with_output()?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(DebugError::Command(
            "Command failed (see stderr above)".to_string(),
        ))
    }
}

/// Run a generic cargo command and return stdout
#[allow(dead_code)] // Reserved for future use
pub fn run_cargo_command(package: &str, subcommand: &[&str], args: &[&str]) -> Result<String> {
    let mut cmd = Command::new("cargo");
    cmd.args(["run", "-p", package, "--quiet", "--"]);
    cmd.args(subcommand);
    cmd.args(args);
    cmd.stderr(std::process::Stdio::inherit());
    cmd.stdout(std::process::Stdio::piped());

    let child = cmd.spawn()?;
    let output = child.wait_with_output()?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(DebugError::Command(
            "Command failed (see stderr above)".to_string(),
        ))
    }
}

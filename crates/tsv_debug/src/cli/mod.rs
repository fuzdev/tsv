pub mod commands;

use argh::FromArgs;
#[cfg(feature = "swallow_check")]
use commands::swallow_audit::SwallowAuditCommand;
use commands::{
    arena_stats::ArenaStatsCommand, ast_diff::AstDiffCommand,
    authoring_audit::AuthoringAuditCommand, buffer_sizes::BufferSizesCommand,
    build_fanout_audit::BuildFanoutAuditCommand, canonical_parse::CanonicalParseCommand,
    check::CheckCommand, compare::CompareCommand, conformance_audit::ConformanceAuditCommand,
    fixture_init::FixtureInitCommand, fixtures_audit::FixturesAuditCommand,
    fixtures_update::FixturesUpdateCommand,
    fixtures_update_formatted::FixturesUpdateFormattedCommand,
    fixtures_update_parsed::FixturesUpdateParsedCommand,
    fixtures_validate::FixturesValidateCommand, format_prettier::FormatPrettierCommand,
    fuzz::FuzzCommand, json_profile::JsonProfileCommand, lex_diff::LexDiffCommand,
    line_width::LineWidthCommand, metrics::MetricsCommand, profile::ProfileCommand,
    roundtrip_audit::RoundtripAuditCommand, scan_audit::ScanAuditCommand, test262::Test262Command,
    ts_fixture_audit::TsFixtureAuditCommand,
};

/// A command failure, carrying the process exit code up to the single exit
/// point in `main`.
///
/// Commands print their own diagnostics where the failure happens; this only
/// carries the resulting code, so exit policy lives in one place instead of the
/// scattered `std::process::exit` calls it replaces. The codes are a stable
/// contract: success is `Ok(())` (exit `0`), `Failed` is exit `1` (a reported
/// error), and `TaskPanic` is exit `2` (a spawned task panicked while joining —
/// a distinct failure class).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliError {
    /// A failure the command already reported — exit code 1.
    Failed,
    /// A spawned task panicked while being joined — exit code 2.
    TaskPanic,
}

impl CliError {
    /// The process exit code for this failure.
    #[must_use]
    pub fn exit_code(self) -> u8 {
        match self {
            Self::Failed => 1,
            Self::TaskPanic => 2,
        }
    }
}

/// tsv_debug — internal debugging tools (fixtures, comparisons, conformance).
#[derive(FromArgs, Debug)]
pub struct TopLevel {
    #[argh(subcommand)]
    pub nested: Subcommand,
}

#[derive(FromArgs, Debug)]
#[argh(subcommand)]
pub enum Subcommand {
    Check(CheckCommand),
    ArenaStats(ArenaStatsCommand),
    AuthoringAudit(AuthoringAuditCommand),
    BufferSizes(BufferSizesCommand),
    BuildFanoutAudit(BuildFanoutAuditCommand),
    Compare(CompareCommand),
    ConformanceAudit(ConformanceAuditCommand),
    AstDiff(AstDiffCommand),
    LineWidth(LineWidthCommand),
    CanonicalParse(CanonicalParseCommand),
    FormatPrettier(FormatPrettierCommand),
    Fuzz(FuzzCommand),
    FixtureInit(FixtureInitCommand),
    FixturesUpdate(FixturesUpdateCommand),
    FixturesUpdateParsed(FixturesUpdateParsedCommand),
    FixturesUpdateFormatted(FixturesUpdateFormattedCommand),
    FixturesValidate(FixturesValidateCommand),
    FixturesAudit(FixturesAuditCommand),
    Profile(ProfileCommand),
    JsonProfile(JsonProfileCommand),
    LexDiff(LexDiffCommand),
    Metrics(MetricsCommand),
    RoundtripAudit(RoundtripAuditCommand),
    ScanAudit(ScanAuditCommand),
    // Requires the `swallow_check` feature so default builds keep the
    // render-time swallow instrumentation compiled out (production-shaped
    // profiles); the `swallow:audit` deno task passes it.
    #[cfg(feature = "swallow_check")]
    SwallowAudit(SwallowAuditCommand),
    Test262(Test262Command),
    TsFixtureAudit(TsFixtureAuditCommand),
}

impl TopLevel {
    /// Dispatch the selected subcommand, threading its exit decision up to `main`.
    ///
    /// # Errors
    ///
    /// Returns the subcommand's [`CliError`] when it fails; `main` maps it to the
    /// process exit code.
    pub fn run(self) -> Result<(), CliError> {
        match self.nested {
            Subcommand::Check(c) => c.run(),
            Subcommand::ArenaStats(c) => c.run(),
            Subcommand::AuthoringAudit(c) => c.run(),
            Subcommand::BufferSizes(c) => c.run(),
            Subcommand::BuildFanoutAudit(c) => c.run(),
            Subcommand::Compare(c) => c.run(),
            Subcommand::ConformanceAudit(c) => c.run(),
            Subcommand::AstDiff(c) => c.run(),
            Subcommand::LineWidth(c) => c.run(),
            Subcommand::CanonicalParse(c) => c.run(),
            Subcommand::FormatPrettier(c) => c.run(),
            Subcommand::Fuzz(c) => c.run(),
            Subcommand::FixtureInit(c) => c.run(),
            Subcommand::FixturesUpdate(c) => c.run(),
            Subcommand::FixturesUpdateParsed(c) => c.run(),
            Subcommand::FixturesUpdateFormatted(c) => c.run(),
            Subcommand::FixturesValidate(c) => c.run(),
            Subcommand::FixturesAudit(c) => c.run(),
            Subcommand::Profile(c) => c.run(),
            Subcommand::JsonProfile(c) => c.run(),
            Subcommand::LexDiff(c) => c.run(),
            Subcommand::Metrics(c) => c.run(),
            Subcommand::RoundtripAudit(c) => c.run(),
            Subcommand::ScanAudit(c) => c.run(),
            #[cfg(feature = "swallow_check")]
            Subcommand::SwallowAudit(c) => c.run(),
            Subcommand::Test262(c) => c.run(),
            Subcommand::TsFixtureAudit(c) => c.run(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CliError;

    #[test]
    fn exit_codes_are_stable() {
        // The exit-code contract `main` maps to a process code: 1 for a reported
        // failure, 2 for a spawned-task panic. Pinned so the refactor can't drift it.
        assert_eq!(CliError::Failed.exit_code(), 1);
        assert_eq!(CliError::TaskPanic.exit_code(), 2);
    }
}

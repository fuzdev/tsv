pub mod commands;

use argh::FromArgs;
#[cfg(feature = "comment_check")]
use commands::blank_audit::BlankAuditCommand;
#[cfg(feature = "comment_check")]
use commands::comment_audit::CommentAuditCommand;
#[cfg(feature = "comment_check")]
use commands::gap_audit::GapAuditCommand;
#[cfg(feature = "comment_check")]
use commands::ignore_audit::IgnoreAuditCommand;
#[cfg(feature = "swallow_check")]
use commands::swallow_audit::SwallowAuditCommand;
use commands::{
    arena_stats::ArenaStatsCommand, ast_diff::AstDiffCommand,
    authoring_audit::AuthoringAuditCommand, binding_audit::BindingAuditCommand,
    buffer_sizes::BufferSizesCommand, build_fanout_audit::BuildFanoutAuditCommand,
    canonical_compile::CanonicalCompileCommand, canonical_parse::CanonicalParseCommand,
    canonicalize_audit::CanonicalizeAuditCommand, check::CheckCommand, compare::CompareCommand,
    compile_compare::CompileCompareCommand,
    compile_conformance_audit::CompileConformanceAuditCommand,
    compile_corpus_compare::CompileCorpusCompareCommand,
    compile_fixture_init::CompileFixtureInitCommand,
    compile_fixtures_validate::CompileFixturesValidateCommand, compile_fuzz::CompileFuzzCommand,
    compile_profile::CompileProfileCommand, conformance_audit::ConformanceAuditCommand,
    erase_comment_census::EraseCommentCensusCommand, fixture_init::FixtureInitCommand,
    fixtures_audit::FixturesAuditCommand, fixtures_update::FixturesUpdateCommand,
    fixtures_update_formatted::FixturesUpdateFormattedCommand,
    fixtures_update_parsed::FixturesUpdateParsedCommand,
    fixtures_validate::FixturesValidateCommand, format_prettier::FormatPrettierCommand,
    fuzz::FuzzCommand, json_profile::JsonProfileCommand, lex_diff::LexDiffCommand,
    line_width::LineWidthCommand, metrics::MetricsCommand,
    neutrality_audit::NeutralityAuditCommand, profile::ProfileCommand,
    render_audit::RenderAuditCommand, roundtrip_audit::RoundtripAuditCommand,
    scan_audit::ScanAuditCommand, test262::Test262Command, ts_fixture_audit::TsFixtureAuditCommand,
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
    /// A failure the command already reported — exit code 1. Also carries the
    /// `compile_compare` "real difference found" outcome (a reported non-match).
    Failed,
    /// A spawned task panicked while being joined — exit code 2.
    TaskPanic,
    /// A hard error distinct from a reported difference — exit code 2. Used by
    /// `compile_compare` for compile/canonicalize failures and the still-unimplemented
    /// tsv side, so its `0` parity / `1` diff / `2` error contract stays intact.
    Errored,
}

impl CliError {
    /// The process exit code for this failure.
    #[must_use]
    pub fn exit_code(self) -> u8 {
        match self {
            Self::Failed => 1,
            Self::TaskPanic | Self::Errored => 2,
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
    BindingAudit(BindingAuditCommand),
    BufferSizes(BufferSizesCommand),
    BuildFanoutAudit(BuildFanoutAuditCommand),
    // Requires the `comment_check` feature so default builds keep the comment ledger's
    // registration + record hooks compiled out (production-shaped profiles); the
    // `comments:audit` deno task passes it.
    #[cfg(feature = "comment_check")]
    CommentAudit(CommentAuditCommand),
    // Same `comment_check` gate as `CommentAudit` — it drives the same ledger; the
    // `gaps:audit` deno task passes the feature.
    #[cfg(feature = "comment_check")]
    GapAudit(GapAuditCommand),
    // Same `comment_check` gate — the blank-injection audit drives the ledger for its
    // ledger-clean invariant; the `blanks:audit` deno task passes the feature.
    #[cfg(feature = "comment_check")]
    BlankAudit(BlankAuditCommand),
    // Same `comment_check` gate — the ignore-directive audit drives the ledger via
    // `pristine_format`; the `ignore:audit` deno task passes the feature.
    #[cfg(feature = "comment_check")]
    IgnoreAudit(IgnoreAuditCommand),
    Compare(CompareCommand),
    ConformanceAudit(ConformanceAuditCommand),
    AstDiff(AstDiffCommand),
    LineWidth(LineWidthCommand),
    CanonicalParse(CanonicalParseCommand),
    CanonicalCompile(CanonicalCompileCommand),
    CanonicalizeAudit(CanonicalizeAuditCommand),
    CompileCompare(CompileCompareCommand),
    CompileConformanceAudit(CompileConformanceAuditCommand),
    CompileCorpusCompare(CompileCorpusCompareCommand),
    CompileFixtureInit(CompileFixtureInitCommand),
    CompileFuzz(CompileFuzzCommand),
    CompileFixturesValidate(CompileFixturesValidateCommand),
    CompileProfile(CompileProfileCommand),
    EraseCommentCensus(EraseCommentCensusCommand),
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
    NeutralityAudit(NeutralityAuditCommand),
    RenderAudit(RenderAuditCommand),
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
            Subcommand::BindingAudit(c) => c.run(),
            Subcommand::BufferSizes(c) => c.run(),
            Subcommand::BuildFanoutAudit(c) => c.run(),
            #[cfg(feature = "comment_check")]
            Subcommand::CommentAudit(c) => c.run(),
            #[cfg(feature = "comment_check")]
            Subcommand::GapAudit(c) => c.run(),
            #[cfg(feature = "comment_check")]
            Subcommand::BlankAudit(c) => c.run(),
            #[cfg(feature = "comment_check")]
            Subcommand::IgnoreAudit(c) => c.run(),
            Subcommand::Compare(c) => c.run(),
            Subcommand::ConformanceAudit(c) => c.run(),
            Subcommand::AstDiff(c) => c.run(),
            Subcommand::LineWidth(c) => c.run(),
            Subcommand::CanonicalParse(c) => c.run(),
            Subcommand::CanonicalCompile(c) => c.run(),
            Subcommand::CanonicalizeAudit(c) => c.run(),
            Subcommand::CompileCompare(c) => c.run(),
            Subcommand::CompileConformanceAudit(c) => c.run(),
            Subcommand::CompileCorpusCompare(c) => c.run(),
            Subcommand::CompileFixtureInit(c) => c.run(),
            Subcommand::CompileFuzz(c) => c.run(),
            Subcommand::CompileFixturesValidate(c) => c.run(),
            Subcommand::CompileProfile(c) => c.run(),
            Subcommand::EraseCommentCensus(c) => c.run(),
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
            Subcommand::NeutralityAudit(c) => c.run(),
            Subcommand::RenderAudit(c) => c.run(),
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
        // failure/difference, 2 for a spawned-task panic or a hard error. Pinned so
        // the refactor can't drift it.
        assert_eq!(CliError::Failed.exit_code(), 1);
        assert_eq!(CliError::TaskPanic.exit_code(), 2);
        assert_eq!(CliError::Errored.exit_code(), 2);
    }
}

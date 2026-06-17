pub mod commands;

use argh::FromArgs;
use commands::{
    ast_diff::AstDiffCommand, canonical_parse::CanonicalParseCommand, check::CheckCommand,
    compare::CompareCommand, conformance_audit::ConformanceAuditCommand,
    fixture_init::FixtureInitCommand, fixtures_audit::FixturesAuditCommand,
    fixtures_update::FixturesUpdateCommand,
    fixtures_update_formatted::FixturesUpdateFormattedCommand,
    fixtures_update_parsed::FixturesUpdateParsedCommand,
    fixtures_validate::FixturesValidateCommand, format_prettier::FormatPrettierCommand,
    json_profile::JsonProfileCommand, line_width::LineWidthCommand, metrics::MetricsCommand,
    profile::ProfileCommand, swallow_audit::SwallowAuditCommand, test262::Test262Command,
    ts_fixture_audit::TsFixtureAuditCommand,
};

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
    Compare(CompareCommand),
    ConformanceAudit(ConformanceAuditCommand),
    AstDiff(AstDiffCommand),
    LineWidth(LineWidthCommand),
    CanonicalParse(CanonicalParseCommand),
    FormatPrettier(FormatPrettierCommand),
    FixtureInit(FixtureInitCommand),
    FixturesUpdate(FixturesUpdateCommand),
    FixturesUpdateParsed(FixturesUpdateParsedCommand),
    FixturesUpdateFormatted(FixturesUpdateFormattedCommand),
    FixturesValidate(FixturesValidateCommand),
    FixturesAudit(FixturesAuditCommand),
    Profile(ProfileCommand),
    JsonProfile(JsonProfileCommand),
    Metrics(MetricsCommand),
    SwallowAudit(SwallowAuditCommand),
    Test262(Test262Command),
    TsFixtureAudit(TsFixtureAuditCommand),
}

impl TopLevel {
    pub fn run(self) {
        match self.nested {
            Subcommand::Check(c) => c.run(),
            Subcommand::Compare(c) => c.run(),
            Subcommand::ConformanceAudit(c) => c.run(),
            Subcommand::AstDiff(c) => c.run(),
            Subcommand::LineWidth(c) => c.run(),
            Subcommand::CanonicalParse(c) => c.run(),
            Subcommand::FormatPrettier(c) => c.run(),
            Subcommand::FixtureInit(c) => c.run(),
            Subcommand::FixturesUpdate(c) => c.run(),
            Subcommand::FixturesUpdateParsed(c) => c.run(),
            Subcommand::FixturesUpdateFormatted(c) => c.run(),
            Subcommand::FixturesValidate(c) => c.run(),
            Subcommand::FixturesAudit(c) => c.run(),
            Subcommand::Profile(c) => c.run(),
            Subcommand::JsonProfile(c) => c.run(),
            Subcommand::Metrics(c) => c.run(),
            Subcommand::SwallowAudit(c) => c.run(),
            Subcommand::Test262(c) => c.run(),
            Subcommand::TsFixtureAudit(c) => c.run(),
        }
    }
}

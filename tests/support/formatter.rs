//! `tsv format <path>`'s format entry point, keyed by parser type: the language's
//! `Formatter`, which a CLI worker keeps warm across files.

use tsv_cli::cli::input::ParserType;

/// Format `source` through its language's `Formatter` twice — cold, then on the arenas the
/// first call left behind — assert the two agree, and return the result. The second call is
/// the shape every file but a worker's first takes in a directory format.
#[expect(
    clippy::panic,
    reason = "a helper's assertion, not reached by clippy's in-test allowance"
)]
pub fn format_via_formatter(source: &str, parser: ParserType) -> Result<String, tsv_lang::Error> {
    let twice = |format: &mut dyn FnMut(&str) -> Result<String, tsv_lang::Error>| {
        let cold = format(source);
        let warm = format(source);
        match (&cold, &warm) {
            (Ok(a), Ok(b)) if a == b => {}
            (Err(a), Err(b)) if format!("{a:#}") == format!("{b:#}") => {}
            _ => panic!("a warm Formatter disagrees with a cold one: {cold:?} vs {warm:?}"),
        }
        cold
    };
    match parser {
        ParserType::TypeScript => {
            let mut formatter = tsv_ts::Formatter::new();
            twice(&mut |s| formatter.format(s))
        }
        ParserType::Css => {
            let mut formatter = tsv_css::Formatter::new();
            twice(&mut |s| formatter.format(s))
        }
        ParserType::Svelte => {
            let mut formatter = tsv_svelte::Formatter::new();
            twice(&mut |s| formatter.format(s))
        }
    }
}

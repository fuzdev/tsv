//! Pairwise render-equivalence triage: do two Svelte sources render the same page?
//!
//! The render-equivalence oracles already run at two scopes — the fixture R rules
//! over `tests/fixtures` and `render_audit` over a corpus, both comparing a source
//! against `tsv format(source)`. What neither answers is the **triage** question
//! that comes up whenever a whitespace rule is being designed or a divergence
//! investigated: *are these two specific authorings the same document?* Until now
//! that meant running `canonical_compile` on each form and eyeballing the baked
//! output; this command is that comparison, graded.
//!
//! ## Tiers
//!
//! - `identical` — the compiled server JS is byte-equal: the compiler itself erases
//!   the difference (`clean_nodes` trimmed it away).
//! - `cosmetic` — the compiled bytes differ but the browser-visible render key
//!   ([`deno::svelte_render_key`]) is equal: the whitespace reaches the baked
//!   template, and CSS `white-space: normal` collapses it. Same page.
//! - `visible` — the render keys differ: the two sources render differently.
//!
//! The key is derived statically from the baked template (no SSR execution), so
//! sources with unresolvable imports — e.g. component-hosted cases — still grade.
//!
//! Exit codes: 0 same render (`identical` / `cosmetic`), 1 `visible`, 2 error.

use argh::FromArgs;

use crate::cli::CliError;
use crate::deno::{self, SvelteGenerate};

/// Compare the browser-visible render of two Svelte sources.
#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "render_compare")]
pub struct RenderCompareCommand {
    /// inline source (repeatable; two inputs total across --content and files)
    #[argh(option)]
    content: Vec<String>,

    /// machine-readable JSON report
    #[argh(switch)]
    json: bool,

    /// file paths (two inputs total across --content and files)
    #[argh(positional)]
    files: Vec<String>,
}

impl RenderCompareCommand {
    pub(crate) fn run(self) -> Result<(), CliError> {
        let mut sources: Vec<(String, String)> = Vec::new();
        for content in self.content {
            sources.push((format!("content[{}]", sources.len()), content));
        }
        for path in &self.files {
            match std::fs::read_to_string(path) {
                Ok(text) => sources.push((path.clone(), text)),
                Err(err) => {
                    eprintln!("Error reading {path}: {err}");
                    return Err(CliError::Errored);
                }
            }
        }
        let [(label_a, a), (label_b, b)] =
            <[(String, String); 2]>::try_from(sources).map_err(|got| {
                eprintln!(
                    "Error: exactly two inputs required (via --content and/or file paths), got {}",
                    got.len()
                );
                CliError::Errored
            })?;

        let rt = super::create_runtime();
        let graded = rt.block_on(async {
            let compiled_a = deno::svelte_compile(&a, SvelteGenerate::Server, false).await?;
            let compiled_b = deno::svelte_compile(&b, SvelteGenerate::Server, false).await?;
            if compiled_a.js == compiled_b.js {
                return Ok::<_, deno::DenoError>((Tier::Identical, None));
            }
            let key_a = deno::svelte_render_key(&a).await?;
            let key_b = deno::svelte_render_key(&b).await?;
            let tier = if key_a == key_b {
                Tier::Cosmetic
            } else {
                Tier::Visible
            };
            Ok((tier, Some((key_a, key_b))))
        });

        let (tier, keys) = match graded {
            Ok(graded) => graded,
            Err(err) => {
                eprintln!("Error compiling: {err}");
                return Err(CliError::Errored);
            }
        };

        if self.json {
            let (key_a, key_b) = keys.map_or((None, None), |(a, b)| (Some(a), Some(b)));
            let report = serde_json::json!({
                "tier": tier.name(),
                "a": label_a,
                "b": label_b,
                "key_a": key_a,
                "key_b": key_b,
            });
            println!("{report}");
        } else {
            println!("{}", tier.describe());
            if let (Tier::Visible, Some((key_a, key_b))) = (tier, &keys) {
                println!("  a ({label_a}): {key_a}");
                println!("  b ({label_b}): {key_b}");
            }
        }

        match tier {
            Tier::Identical | Tier::Cosmetic => Ok(()),
            Tier::Visible => Err(CliError::Failed),
        }
    }
}

/// How far apart the two sources are.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tier {
    Identical,
    Cosmetic,
    Visible,
}

impl Tier {
    fn name(self) -> &'static str {
        match self {
            Tier::Identical => "identical",
            Tier::Cosmetic => "cosmetic",
            Tier::Visible => "visible",
        }
    }

    fn describe(self) -> &'static str {
        match self {
            Tier::Identical => "identical — compiled server JS is byte-equal",
            Tier::Cosmetic => "cosmetic — compiled bytes differ, render key equal (same page)",
            Tier::Visible => "VISIBLE — the render keys differ (different page)",
        }
    }
}

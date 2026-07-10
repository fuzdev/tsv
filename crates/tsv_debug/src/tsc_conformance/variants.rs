//! Fan a test's settings out into its variant configurations and synthesize each
//! variant's baseline filename, faithful to tsgo's harness.
//!
//! Only **varyBy** options fan out; a comma list in a list-typed option (`@lib:
//! es5, dom`) is one value, not a set of variants. `splitOptionValues` handles the
//! `*` (all values), `-x`/`!x` (exclusion), and typed-dedup semantics; the variant
//! product is capped at 25 (a hard harness failure above). The baseline name is
//! `name(k1=v1,k2=v2).errors.txt` with sorted keys and lowercased values — the
//! exact on-disk form.
//
// tsgo: internal/testutil/harnessutil/harnessutil.go GetFileBasedTestConfigurations
// tsgo: internal/testutil/harnessutil/harnessutil.go splitOptionValues (comma/`*`/exclusion/dedup)
// tsgo: internal/testutil/harnessutil/harnessutil.go getFileBasedTestConfigurationDescription
// tsgo: internal/testrunner/compiler_runner.go newCompilerTest (configuredName)

use crate::tsc_conformance::options_meta::{all_values, is_vary_by, normalize_value, NormValue};
use std::collections::BTreeMap;

/// The variant cap: a product above this is a hard harness failure.
const VARIATION_CAP: usize = 25;

/// One expanded variant of a test.
#[derive(Debug, Clone)]
pub struct Variant {
    /// The variant description (`k1=v1,k2=v2`, sorted keys, lowercased values), or
    /// empty for the single unvaried configuration.
    pub description: String,
    /// The merged resolved config (varying values plus non-varying options),
    /// lowercased keys.
    pub config: BTreeMap<String, String>,
}

/// The result of expanding a test's settings.
#[derive(Debug, Clone)]
pub struct Expansion {
    /// The variants (at least one; a settingless test yields one empty variant).
    pub variants: Vec<Variant>,
    /// Whether the variant product exceeded the cap (tsgo `t.Fatal`). Never true
    /// on the valid corpus.
    pub cap_exceeded: bool,
    /// Include values with no normalized identity, summed across this test's varyBy
    /// options. tsgo hard-fails on such an include (`getValueOfOptionString`'s
    /// `t.Fatalf`); this harness keeps a graceful `Other` identity so the value
    /// still expands, and counts it here so the corpus index can pin the total (a
    /// nonzero count means a corpus pull introduced values the ported option
    /// universe doesn't know, i.e. phantom variants).
    pub unknown_includes: usize,
}

/// The outcome of splitting a varyBy option's directive value: its distinct
/// writable values plus the count of include values with no normalized identity.
#[derive(Debug, Clone, Default)]
pub struct SplitOutcome {
    /// The original include strings, deduped by normalized identity (first wins).
    pub values: Vec<String>,
    /// Include values `normalize_value` did not recognize (the graceful-`Other`
    /// fallbacks); tsgo hard-fails on each of these instead.
    pub unknown_includes: usize,
}

/// Split a varyBy option's directive value into its distinct writable values,
/// handling `*` (all values), `-x`/`!x` (exclusion), and typed dedup — the
/// original include strings, deduped by normalized identity (first wins).
///
/// An include the option universe doesn't recognize keeps its own lowercased
/// identity (so the variant count is never silently reduced) and is tallied in
/// [`SplitOutcome::unknown_includes`]; unrecognized *excludes* are skipped
/// silently, matching tsgo (`tryGetValueOfOptionString` returns "not ok" and the
/// exclude is a no-op).
#[must_use]
pub fn split_option_values(value: &str, option_lower: &str) -> SplitOutcome {
    if value.is_empty() {
        return SplitOutcome::default();
    }
    let mut star = false;
    let mut includes: Vec<&str> = Vec::new();
    let mut excludes: Vec<&str> = Vec::new();
    for part in value.split(',') {
        let s = part.trim();
        if s.is_empty() {
            continue;
        }
        if s == "*" {
            star = true;
        } else if let Some(rest) = s.strip_prefix('-').or_else(|| s.strip_prefix('!')) {
            excludes.push(rest);
        } else {
            includes.push(s);
        }
    }
    if includes.is_empty() && !star && excludes.is_empty() {
        return SplitOutcome::default();
    }

    // Insertion-ordered dedup by normalized identity; unrecognized includes keep
    // their own identity so the variant count is never silently reduced.
    let mut order: Vec<(NormValue, String)> = Vec::new();
    let mut insert = |canon: NormValue, original: &str| {
        if !order.iter().any(|(c, _)| *c == canon) {
            order.push((canon, original.to_string()));
        }
    };
    let mut unknown_includes = 0usize;
    for inc in &includes {
        let canon = match normalize_value(option_lower, inc) {
            Some(canon) => canon,
            None => {
                unknown_includes += 1;
                NormValue::Other(inc.to_lowercase())
            }
        };
        insert(canon, inc);
    }
    if star {
        for key in all_values(option_lower) {
            if let Some(canon) = normalize_value(option_lower, key) {
                insert(canon, key);
            }
        }
    }
    for exc in &excludes {
        if let Some(canon) = normalize_value(option_lower, exc) {
            order.retain(|(c, _)| *c != canon);
        }
    }
    SplitOutcome {
        values: order.into_iter().map(|(_, original)| original).collect(),
        unknown_includes,
    }
}

/// Expand a settings map into its variant configurations
/// (`GetFileBasedTestConfigurations`).
#[must_use]
pub fn expand(settings: &BTreeMap<String, String>) -> Expansion {
    let mut option_entries: Vec<(String, Vec<String>)> = Vec::new();
    let mut variation_count = 1usize;
    let mut non_varying: BTreeMap<String, String> = BTreeMap::new();
    let mut unknown_includes = 0usize;

    for (opt, value) in settings {
        if is_vary_by(opt) {
            let outcome = split_option_values(value, opt);
            unknown_includes += outcome.unknown_includes;
            let entries = outcome.values;
            if entries.len() > 1 {
                variation_count = variation_count.saturating_mul(entries.len());
                if variation_count > VARIATION_CAP {
                    return Expansion {
                        variants: Vec::new(),
                        cap_exceeded: true,
                        unknown_includes,
                    };
                }
                option_entries.push((opt.clone(), entries));
            } else if entries.len() == 1 {
                non_varying.insert(opt.clone(), entries[0].clone());
            }
            // len 0: the option is dropped entirely.
        } else {
            non_varying.insert(opt.clone(), value.clone());
        }
    }

    let variants = if option_entries.is_empty() {
        if non_varying.is_empty() {
            vec![Variant {
                description: String::new(),
                config: BTreeMap::new(),
            }]
        } else {
            vec![Variant {
                description: String::new(),
                config: non_varying,
            }]
        }
    } else {
        // Cartesian product over the varying options.
        let mut combos: Vec<Vec<(String, String)>> = vec![Vec::new()];
        for (key, values) in &option_entries {
            let mut next = Vec::with_capacity(combos.len() * values.len());
            for combo in &combos {
                for v in values {
                    let mut c = combo.clone();
                    c.push((key.clone(), v.clone()));
                    next.push(c);
                }
            }
            combos = next;
        }
        combos
            .into_iter()
            .map(|combo| {
                let varying: BTreeMap<String, String> = combo.into_iter().collect();
                let description = varying
                    .iter()
                    .map(|(k, v)| format!("{k}={}", v.to_lowercase()))
                    .collect::<Vec<_>>()
                    .join(",");
                let mut config = varying;
                for (k, v) in &non_varying {
                    config.entry(k.clone()).or_insert_with(|| v.clone());
                }
                Variant { description, config }
            })
            .collect()
    };

    Expansion {
        variants,
        cap_exceeded: false,
        unknown_includes,
    }
}

/// Synthesize a variant's baseline filename (`configuredName` then the
/// `.tsx?`→`.errors.txt` replacement). An empty description yields the plain
/// `basename.errors.txt`. A non-`.ts`/`.tsx` basename keeps its extension (so its
/// synthesized name never joins an `.errors.txt` baseline).
#[must_use]
pub fn config_name(basename: &str, description: &str) -> String {
    if description.is_empty() {
        return errors_name(basename);
    }
    let configured = if let Some(stem) = basename.strip_suffix(".tsx") {
        format!("{stem}({description}).tsx")
    } else if let Some(stem) = basename.strip_suffix(".ts") {
        format!("{stem}({description}).ts")
    } else {
        format!("{basename}({description})")
    };
    errors_name(&configured)
}

/// Replace a trailing `.ts`/`.tsx` with `.errors.txt` (tsgo's `tsExtension`
/// regex); other extensions are left unchanged.
fn errors_name(name: &str) -> String {
    if let Some(stem) = name.strip_suffix(".tsx") {
        format!("{stem}.errors.txt")
    } else if let Some(stem) = name.strip_suffix(".ts") {
        format!("{stem}.errors.txt")
    } else {
        name.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    #[test]
    fn split_star_and_exclusion() {
        // `*` expands to all bool values, `-` exclusion removes one.
        let mut v = split_option_values("*, -true", "strict").values;
        v.sort();
        assert_eq!(v, vec!["false"]);
    }

    #[test]
    fn split_bang_exclusion() {
        // `!x` is the exclusion form alongside `-x` (both strip one value).
        let out = split_option_values("*, !true", "strict");
        assert_eq!(out.values, vec!["false"]);
        assert_eq!(out.unknown_includes, 0);
    }

    #[test]
    fn split_empty_set_after_exclusion() {
        // `*` minus every value: tsgo panics ("resulted in an empty set"); this
        // harness returns an empty value list gracefully (the option is then
        // dropped by `expand`, not a variant).
        let out = split_option_values("*, -true, -false", "strict");
        assert!(out.values.is_empty());
        assert_eq!(out.unknown_includes, 0);
    }

    #[test]
    fn split_unknown_include_fallback_is_counted() {
        // An include the option universe doesn't recognize still expands (graceful
        // `Other` identity) but is tallied — tsgo hard-fails on it instead.
        let out = split_option_values("es5, notarealtarget", "target");
        assert_eq!(out.values, vec!["es5", "notarealtarget"]);
        assert_eq!(out.unknown_includes, 1);
    }

    #[test]
    fn split_dedup_aliases() {
        // es6 and es2015 alias; first (es6) wins, so one value.
        assert_eq!(split_option_values("es6, es2015", "target").values, vec!["es6"]);
    }

    #[test]
    fn split_list_typed_is_single() {
        // A list option is never a varyBy option, so this helper is not called for
        // it; but a plain multi-value on a scalar enum splits.
        assert_eq!(split_option_values("es5, es2015", "target").values.len(), 2);
    }

    #[test]
    fn expand_single_target_pair() {
        let e = expand(&settings(&[("target", "es5, es2015")]));
        assert!(!e.cap_exceeded);
        assert_eq!(e.variants.len(), 2);
        let descs: Vec<_> = e.variants.iter().map(|v| v.description.clone()).collect();
        assert!(descs.contains(&"target=es5".to_string()));
        assert!(descs.contains(&"target=es2015".to_string()));
    }

    #[test]
    fn expand_product_sorted_description() {
        let e = expand(&settings(&[("strict", "true, false"), ("module", "commonjs, esnext")]));
        assert_eq!(e.variants.len(), 4);
        // Keys are sorted in the description.
        assert!(e
            .variants
            .iter()
            .all(|v| v.description.starts_with("module=") && v.description.contains(",strict=")));
    }

    #[test]
    fn expand_non_varying_kept() {
        // A single-value varyBy option is non-varying; a non-varyBy option too.
        let e = expand(&settings(&[("target", "es2015"), ("jsxfactory", "h")]));
        assert_eq!(e.variants.len(), 1);
        assert_eq!(e.variants[0].description, "");
        assert_eq!(e.variants[0].config.get("target").map(String::as_str), Some("es2015"));
        assert_eq!(e.variants[0].config.get("jsxfactory").map(String::as_str), Some("h"));
    }

    #[test]
    fn expand_cap_exceeded() {
        // target `*` (13 distinct) × strict `true,false` (2) = 26 > the cap of 25:
        // a hard harness failure (tsgo `t.Fatal`), here surfaced as `cap_exceeded`
        // with no variants.
        let e = expand(&settings(&[("target", "*"), ("strict", "true, false")]));
        assert!(e.cap_exceeded);
        assert!(e.variants.is_empty());
    }

    #[test]
    fn expand_surfaces_unknown_includes() {
        // The unknown-include tally propagates from `split_option_values` up to the
        // expansion (the F1 phantom-variant guard).
        let e = expand(&settings(&[("target", "es2015, notarealtarget")]));
        assert!(!e.cap_exceeded);
        assert_eq!(e.unknown_includes, 1);
        assert_eq!(e.variants.len(), 2);
    }

    #[test]
    fn expand_empty_after_exclusion_drops_option() {
        // A varyBy option that reduces to an empty set is dropped entirely, leaving
        // the single unvaried configuration.
        let e = expand(&settings(&[("strict", "*, -true, -false")]));
        assert!(!e.cap_exceeded);
        assert_eq!(e.variants.len(), 1);
        assert_eq!(e.variants[0].description, "");
        assert!(e.variants[0].config.is_empty());
    }

    #[test]
    fn config_name_synthesis() {
        assert_eq!(config_name("foo.ts", ""), "foo.errors.txt");
        assert_eq!(config_name("foo.ts", "target=es2015"), "foo(target=es2015).errors.txt");
        assert_eq!(config_name("foo.tsx", "jsx=react"), "foo(jsx=react).errors.txt");
        assert_eq!(config_name("foo.d.ts", "target=es5"), "foo.d(target=es5).errors.txt");
        // A .js basename keeps its extension (never joins an .errors.txt).
        assert_eq!(config_name("foo.js", ""), "foo.js");
    }
}

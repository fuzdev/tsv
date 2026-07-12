use super::*;

/// Which graded sub-family the `--family` filter isolates.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FamilyFilter {
    /// One [`FAMILIES`] row, by index (`dup` = 0, `flow` = 1).
    One(usize),
    /// The whole graded family ([`FAMILY_CODES`]) — isolates the family-graded slice.
    All,
}

impl FamilyFilter {
    /// Parse a `--family` token: a [`FAMILIES`] key, or `all`.
    #[must_use]
    pub fn parse(arg: &str) -> Option<FamilyFilter> {
        if arg == "all" {
            return Some(FamilyFilter::All);
        }
        FAMILIES
            .iter()
            .position(|f| f.key == arg)
            .map(FamilyFilter::One)
    }

    /// The valid `--family` tokens, for error messages (`dup / flow / all`).
    #[must_use]
    pub fn tokens() -> String {
        let mut tokens: Vec<&str> = FAMILIES.iter().map(|f| f.key).collect();
        tokens.push("all");
        tokens.join(" / ")
    }
}

/// The code set a [`FamilyFilter`] keeps a variant for (its baseline must carry at
/// least one).
fn family_filter_codes(f: FamilyFilter) -> &'static [u32] {
    match f {
        FamilyFilter::One(i) => FAMILIES[i].codes,
        FamilyFilter::All => &FAMILY_CODES,
    }
}

/// Filters for a scoped `run` sweep. Any active filter SKIPS the exact pins (the
/// `roundtrip`/`query` convention), so a filtered run is a triage view — the
/// invariant gates (clean grading, no panics, `family_extra == 0`) still hold.
#[derive(Default, Clone)]
pub struct RunFilter {
    /// Keep only tests whose relative path contains this substring.
    pub test: Option<String>,
    /// Keep only variants whose joined baseline carries this TS code.
    pub code: Option<u32>,
    /// Keep only variants whose config has this `key=value` (key lowercased).
    pub variant: Option<(String, String)>,
    /// Keep only variants whose baseline carries a code in this sub-family
    /// (`dup` / `flow` / `all`).
    pub family: Option<FamilyFilter>,
}

impl RunFilter {
    /// Whether any filter is active (drives pin skipping).
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.test.is_some()
            || self.code.is_some()
            || self.variant.is_some()
            || self.family.is_some()
    }

    /// Whether a test passes the `--test` substring filter (absent filter ⇒ keep).
    pub(super) fn keeps_test(&self, relative_path: &str) -> bool {
        self.test
            .as_deref()
            .is_none_or(|sub| relative_path.contains(sub))
    }

    /// Whether a variant passes the `--variant key=value` filter (absent ⇒ keep).
    /// The key is already lowercased (the config maps store lowercased keys).
    pub(super) fn keeps_variant(&self, config: &BTreeMap<String, String>) -> bool {
        self.variant
            .as_ref()
            .is_none_or(|(k, v)| config.get(k).map(String::as_str) == Some(v.as_str()))
    }

    /// Whether a variant passes the `--code` filter. `baseline_carries` reports
    /// whether the variant's baseline carries a given code; it is consulted only
    /// when the filter is active, so a run without `--code` never reads a baseline
    /// on its behalf. Absent filter ⇒ keep.
    pub(super) fn keeps_code(&self, baseline_carries: impl FnOnce(u32) -> bool) -> bool {
        self.code.is_none_or(baseline_carries)
    }

    /// Whether a variant passes the `--family` filter: its baseline must carry at
    /// least one code in the selected sub-family (an expect-clean variant carries
    /// none, so it is filtered out). `baseline_carries` is consulted only when the
    /// filter is active. Absent filter ⇒ keep.
    pub(super) fn keeps_family(&self, baseline_carries: impl Fn(u32) -> bool) -> bool {
        self.family.is_none_or(|f| {
            family_filter_codes(f)
                .iter()
                .any(|&code| baseline_carries(code))
        })
    }
}

/// Options for the skeleton sweep.
#[derive(Default, Clone)]
pub struct RunOptions {
    /// The triage filter (empty = full pinned run).
    pub filter: RunFilter,
    /// Collect the per-variant verdict rows (for `--emit-manifest`).
    pub collect_manifest: bool,
}

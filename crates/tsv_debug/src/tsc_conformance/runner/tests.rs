use super::*;

/// A variant config from `key=value` pairs (the maps store lowercased keys).
fn config(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

#[test]
fn keeps_test_substring() {
    // No `--test` filter keeps every path; an active one keeps only substrings.
    let none = RunFilter::default();
    assert!(none.keeps_test("compiler/anything.ts"));

    let f = RunFilter {
        test: Some("duplicate".to_string()),
        ..RunFilter::default()
    };
    assert!(f.keeps_test("compiler/duplicateVar.ts"));
    assert!(!f.keeps_test("compiler/asyncAwait.ts"));
}

#[test]
fn keeps_variant_key_value() {
    // No `--variant` filter keeps everything.
    let none = RunFilter::default();
    assert!(none.keeps_variant(&config(&[("target", "es5")])));

    let f = RunFilter {
        variant: Some(("target".to_string(), "es2015".to_string())),
        ..RunFilter::default()
    };
    // Exact key=value match keeps.
    assert!(f.keeps_variant(&config(&[("target", "es2015")])));
    // Wrong value excludes.
    assert!(!f.keeps_variant(&config(&[("target", "es5")])));
    // Absent key excludes (the variant doesn't set it).
    assert!(!f.keeps_variant(&config(&[("strict", "true")])));
}

#[test]
fn keeps_code_consults_baseline_only_when_active() {
    // No `--code` filter keeps without ever consulting the baseline resolver
    // (the closure must not run — it would panic if it did).
    let none = RunFilter::default();
    assert!(none.keeps_code(|_| panic!("resolver consulted with no --code filter")));

    let f = RunFilter {
        code: Some(2300),
        ..RunFilter::default()
    };
    // Active filter keeps iff the baseline carries the code.
    let carried = [2300u32, 2451];
    assert!(f.keeps_code(|code| carried.contains(&code)));
    let other = [2451u32];
    assert!(!f.keeps_code(|code| other.contains(&code)));
    // A variant with no baseline (resolver reports false) is excluded.
    assert!(!f.keeps_code(|_| false));
}

#[test]
fn keeps_family_selects_sub_family() {
    // No `--family` filter keeps without consulting the baseline resolver.
    let none = RunFilter::default();
    assert!(none.keeps_family(|_| panic!("resolver consulted with no --family filter")));

    // `flow` keeps iff the baseline carries a FLOW_CODES member; a dup-only
    // baseline is excluded, a flow baseline is kept. (Parsed through the
    // `FAMILIES`-table tokens — the same path the CLI takes.)
    let flow = RunFilter {
        family: FamilyFilter::parse("flow"),
        ..RunFilter::default()
    };
    assert!(flow.keeps_family(|c| c == 7027));
    assert!(!flow.keeps_family(|c| c == 2300));

    // `dup` is the complementary partition.
    let dup = RunFilter {
        family: FamilyFilter::parse("dup"),
        ..RunFilter::default()
    };
    assert!(dup.keeps_family(|c| c == 2300));
    assert!(!dup.keeps_family(|c| c == 7027));

    // `all` keeps any family code (either partition); an unknown token
    // refuses to parse.
    assert!(FamilyFilter::parse("nope").is_none());
    let all = RunFilter {
        family: FamilyFilter::parse("all"),
        ..RunFilter::default()
    };
    assert!(all.keeps_family(|c| c == 7028));
    assert!(all.keeps_family(|c| c == 2451));
    // A non-family code (or no baseline) is excluded.
    assert!(!all.keeps_family(|c| c == 9999));
}

#[test]
fn filters_compose_as_and() {
    // The call site ANDs the three predicates; all must keep for a variant to be
    // graded, and any one failing excludes it.
    let f = RunFilter {
        test: Some("dup".to_string()),
        code: Some(2300),
        variant: Some(("target".to_string(), "es5".to_string())),
        family: None,
    };
    let cfg = config(&[("target", "es5")]);
    let carried = [2300u32];
    let keeps = |path: &str, cfg: &BTreeMap<String, String>, codes: &[u32]| {
        f.keeps_test(path) && f.keeps_variant(cfg) && f.keeps_code(|c| codes.contains(&c))
    };
    // All three match.
    assert!(keeps("compiler/dupVar.ts", &cfg, &carried));
    // Test substring misses.
    assert!(!keeps("compiler/other.ts", &cfg, &carried));
    // Variant value misses.
    assert!(!keeps(
        "compiler/dupVar.ts",
        &config(&[("target", "es2015")]),
        &carried
    ));
    // Code missing from the baseline.
    assert!(!keeps("compiler/dupVar.ts", &cfg, &[2451]));
}

//! Lib (`.d.ts`) resolution + a per-run [`tsv_check::LibBase`] cache — the S5 seam
//! that lets the checker leg conflict a test's globals against the standard library.
//!
//! Ported from tsgo's file loader: a variant's resolved lib set is its default lib
//! (from `target`, `internal/tsoptions/enummaps.go targetToLibMap` /
//! `GetDefaultLibFileName`) or its explicit `@lib` list (`GetLibFileName`),
//! transitively expanded over each lib file's `/// <reference lib="…" />` directives
//! and **priority-ordered** (`internal/compiler/fileloader.go getDefaultLibFilePriority`
//! — `lib.d.ts`/`lib.es6.d.ts` first, then the `LibMap`-key index). Priority order is
//! the fold order, which fixes each global symbol's declaration order and therefore
//! the TS6203/6204 related-info attribution.
//!
//! The default `target` when unset is `ScriptTargetLatestStandard` (ES2025 →
//! `lib.es2025.full.d.ts`), matching `core.CompilerOptions.GetEmitScriptTarget`.
//! `@noLib` resolves to no libs; `@libFiles` is absent from the in-scope corpus and
//! unsupported (a `@libFiles` variant would silently get the default set — the index
//! gate pins the corpus so a new one would surface).
//!
//! The libs are read at runtime from `<checkout>/internal/bundled/libs/` (no
//! vendoring). Each lib file is parsed + bound **once per run** (file-keyed owned
//! product), and each distinct resolved set folds into a [`tsv_check::LibBase`]
//! **once per run**, shared across every variant that resolves to it.
//
// tsgo: internal/tsoptions/enummaps.go (LibMap, targetToLibMap, GetDefaultLibFileName,
//       GetLibFileName), internal/compiler/fileloader.go (sortLibs,
//       getDefaultLibFilePriority), internal/core/compileroptions.go
//       (GetEmitScriptTarget default = ScriptTargetLatestStandard/ES2025)

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use tsv_check::{LibBase, LibFile, bind_lib};

/// The ported `LibMap` (`enummaps.go`), in insertion order — the tuple order is the
/// `Libs` slice the priority index reads. Maps a lib **name** (`@lib` /
/// `/// <reference lib>` value) to its `.d.ts` file.
const LIB_MAP: &[(&str, &str)] = &[
    ("es5", "lib.es5.d.ts"),
    ("es6", "lib.es2015.d.ts"),
    ("es2015", "lib.es2015.d.ts"),
    ("es7", "lib.es2016.d.ts"),
    ("es2016", "lib.es2016.d.ts"),
    ("es2017", "lib.es2017.d.ts"),
    ("es2018", "lib.es2018.d.ts"),
    ("es2019", "lib.es2019.d.ts"),
    ("es2020", "lib.es2020.d.ts"),
    ("es2021", "lib.es2021.d.ts"),
    ("es2022", "lib.es2022.d.ts"),
    ("es2023", "lib.es2023.d.ts"),
    ("es2024", "lib.es2024.d.ts"),
    ("es2025", "lib.es2025.d.ts"),
    ("esnext", "lib.esnext.d.ts"),
    ("dom", "lib.dom.d.ts"),
    ("dom.iterable", "lib.dom.iterable.d.ts"),
    ("dom.asynciterable", "lib.dom.asynciterable.d.ts"),
    ("webworker", "lib.webworker.d.ts"),
    (
        "webworker.importscripts",
        "lib.webworker.importscripts.d.ts",
    ),
    ("webworker.iterable", "lib.webworker.iterable.d.ts"),
    (
        "webworker.asynciterable",
        "lib.webworker.asynciterable.d.ts",
    ),
    ("scripthost", "lib.scripthost.d.ts"),
    ("es2015.core", "lib.es2015.core.d.ts"),
    ("es2015.collection", "lib.es2015.collection.d.ts"),
    ("es2015.generator", "lib.es2015.generator.d.ts"),
    ("es2015.iterable", "lib.es2015.iterable.d.ts"),
    ("es2015.promise", "lib.es2015.promise.d.ts"),
    ("es2015.proxy", "lib.es2015.proxy.d.ts"),
    ("es2015.reflect", "lib.es2015.reflect.d.ts"),
    ("es2015.symbol", "lib.es2015.symbol.d.ts"),
    (
        "es2015.symbol.wellknown",
        "lib.es2015.symbol.wellknown.d.ts",
    ),
    ("es2016.array.include", "lib.es2016.array.include.d.ts"),
    ("es2016.intl", "lib.es2016.intl.d.ts"),
    ("es2017.arraybuffer", "lib.es2017.arraybuffer.d.ts"),
    ("es2017.date", "lib.es2017.date.d.ts"),
    ("es2017.object", "lib.es2017.object.d.ts"),
    ("es2017.sharedmemory", "lib.es2017.sharedmemory.d.ts"),
    ("es2017.string", "lib.es2017.string.d.ts"),
    ("es2017.intl", "lib.es2017.intl.d.ts"),
    ("es2017.typedarrays", "lib.es2017.typedarrays.d.ts"),
    ("es2018.asyncgenerator", "lib.es2018.asyncgenerator.d.ts"),
    ("es2018.asynciterable", "lib.es2018.asynciterable.d.ts"),
    ("es2018.intl", "lib.es2018.intl.d.ts"),
    ("es2018.promise", "lib.es2018.promise.d.ts"),
    ("es2018.regexp", "lib.es2018.regexp.d.ts"),
    ("es2019.array", "lib.es2019.array.d.ts"),
    ("es2019.object", "lib.es2019.object.d.ts"),
    ("es2019.string", "lib.es2019.string.d.ts"),
    ("es2019.symbol", "lib.es2019.symbol.d.ts"),
    ("es2019.intl", "lib.es2019.intl.d.ts"),
    ("es2020.bigint", "lib.es2020.bigint.d.ts"),
    ("es2020.date", "lib.es2020.date.d.ts"),
    ("es2020.promise", "lib.es2020.promise.d.ts"),
    ("es2020.sharedmemory", "lib.es2020.sharedmemory.d.ts"),
    ("es2020.string", "lib.es2020.string.d.ts"),
    (
        "es2020.symbol.wellknown",
        "lib.es2020.symbol.wellknown.d.ts",
    ),
    ("es2020.intl", "lib.es2020.intl.d.ts"),
    ("es2020.number", "lib.es2020.number.d.ts"),
    ("es2021.promise", "lib.es2021.promise.d.ts"),
    ("es2021.string", "lib.es2021.string.d.ts"),
    ("es2021.weakref", "lib.es2021.weakref.d.ts"),
    ("es2021.intl", "lib.es2021.intl.d.ts"),
    ("es2022.array", "lib.es2022.array.d.ts"),
    ("es2022.error", "lib.es2022.error.d.ts"),
    ("es2022.intl", "lib.es2022.intl.d.ts"),
    ("es2022.object", "lib.es2022.object.d.ts"),
    ("es2022.string", "lib.es2022.string.d.ts"),
    ("es2022.regexp", "lib.es2022.regexp.d.ts"),
    ("es2023.array", "lib.es2023.array.d.ts"),
    ("es2023.collection", "lib.es2023.collection.d.ts"),
    ("es2023.intl", "lib.es2023.intl.d.ts"),
    ("es2024.arraybuffer", "lib.es2024.arraybuffer.d.ts"),
    ("es2024.collection", "lib.es2024.collection.d.ts"),
    ("es2024.object", "lib.es2024.object.d.ts"),
    ("es2024.promise", "lib.es2024.promise.d.ts"),
    ("es2024.regexp", "lib.es2024.regexp.d.ts"),
    ("es2024.sharedmemory", "lib.es2024.sharedmemory.d.ts"),
    ("es2024.string", "lib.es2024.string.d.ts"),
    ("es2025.collection", "lib.es2025.collection.d.ts"),
    ("es2025.float16", "lib.es2025.float16.d.ts"),
    ("es2025.intl", "lib.es2025.intl.d.ts"),
    ("es2025.iterator", "lib.es2025.iterator.d.ts"),
    ("es2025.promise", "lib.es2025.promise.d.ts"),
    ("es2025.regexp", "lib.es2025.regexp.d.ts"),
    ("esnext.asynciterable", "lib.es2018.asynciterable.d.ts"),
    ("esnext.symbol", "lib.es2019.symbol.d.ts"),
    ("esnext.bigint", "lib.es2020.bigint.d.ts"),
    ("esnext.weakref", "lib.es2021.weakref.d.ts"),
    ("esnext.object", "lib.es2024.object.d.ts"),
    ("esnext.regexp", "lib.es2024.regexp.d.ts"),
    ("esnext.string", "lib.es2024.string.d.ts"),
    ("esnext.float16", "lib.es2025.float16.d.ts"),
    ("esnext.iterator", "lib.es2025.iterator.d.ts"),
    ("esnext.promise", "lib.es2025.promise.d.ts"),
    ("esnext.array", "lib.esnext.array.d.ts"),
    ("esnext.collection", "lib.esnext.collection.d.ts"),
    ("esnext.date", "lib.esnext.date.d.ts"),
    ("esnext.decorators", "lib.esnext.decorators.d.ts"),
    ("esnext.disposable", "lib.esnext.disposable.d.ts"),
    ("esnext.error", "lib.esnext.error.d.ts"),
    ("esnext.intl", "lib.esnext.intl.d.ts"),
    ("esnext.sharedmemory", "lib.esnext.sharedmemory.d.ts"),
    ("esnext.temporal", "lib.esnext.temporal.d.ts"),
    ("esnext.typedarrays", "lib.esnext.typedarrays.d.ts"),
    ("decorators", "lib.decorators.d.ts"),
    ("decorators.legacy", "lib.decorators.legacy.d.ts"),
];

/// tsgo `GetDefaultLibFileName` composed with `GetEmitScriptTarget`: the default
/// lib file for a (possibly unset) `target`. Unset → ES2025 (LatestStandard) →
/// `lib.es2025.full.d.ts`; a target below ES2015 (or otherwise absent from
/// `targetToLibMap`) → `lib.d.ts`.
fn default_lib_for_target(target: Option<&str>) -> &'static str {
    match target.map(str::to_ascii_lowercase).as_deref() {
        None => "lib.es2025.full.d.ts", // GetEmitScriptTarget default = ScriptTargetLatestStandard
        Some("es6" | "es2015") => "lib.es6.d.ts",
        Some("es2016") => "lib.es2016.full.d.ts",
        Some("es2017") => "lib.es2017.full.d.ts",
        Some("es2018") => "lib.es2018.full.d.ts",
        Some("es2019") => "lib.es2019.full.d.ts",
        Some("es2020") => "lib.es2020.full.d.ts",
        Some("es2021") => "lib.es2021.full.d.ts",
        Some("es2022") => "lib.es2022.full.d.ts",
        Some("es2023") => "lib.es2023.full.d.ts",
        Some("es2024") => "lib.es2024.full.d.ts",
        Some("es2025") => "lib.es2025.full.d.ts",
        Some("esnext") => "lib.esnext.full.d.ts",
        // es3 / es5 / json / anything not in targetToLibMap.
        Some(_) => "lib.d.ts",
    }
}

/// tsgo `GetLibFileName` — a lib **name** (or an already-resolved file name) to its
/// `.d.ts` file, or `None` when unrecognized.
fn get_lib_file_name(name: &str) -> Option<&'static str> {
    let lower = name.to_ascii_lowercase();
    if let Some((_, file)) = LIB_MAP.iter().find(|(_, f)| *f == lower) {
        return Some(file); // already a file name
    }
    LIB_MAP.iter().find(|(k, _)| *k == lower).map(|(_, f)| *f)
}

/// tsgo `getDefaultLibFilePriority` (scoped to file basenames): `lib.d.ts` /
/// `lib.es6.d.ts` sort first, then a lib file's `LibMap`-key index; an unrecognized
/// name (e.g. an aggregator like `lib.es2025.full.d.ts`, which carries no
/// declarations) sorts last.
fn lib_priority(file: &str) -> i32 {
    if file == "lib.d.ts" || file == "lib.es6.d.ts" {
        return 0;
    }
    let name = file
        .strip_prefix("lib.")
        .and_then(|s| s.strip_suffix(".d.ts"))
        .unwrap_or(file);
    match LIB_MAP.iter().position(|(k, _)| *k == name) {
        Some(i) => i32::try_from(i).unwrap_or(i32::MAX - 2) + 1,
        None => i32::try_from(LIB_MAP.len()).unwrap_or(i32::MAX - 2) + 2,
    }
}

/// Extract the `/// <reference lib="X" />` names from a lib source (the bundled libs
/// use a single space and appear only in the header directive block).
fn extract_lib_references(source: &str) -> Vec<String> {
    const NEEDLE: &str = "reference lib=\"";
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(pos) = source[from..].find(NEEDLE) {
        let start = from + pos + NEEDLE.len();
        match source[start..].find('"') {
            Some(end) => {
                out.push(source[start..start + end].to_string());
                from = start + end + 1;
            }
            None => break,
        }
    }
    out
}

/// Whether `@noLib` is set truthily.
fn is_no_lib(config: &BTreeMap<String, String>) -> bool {
    config
        .get("nolib")
        .is_some_and(|v| v.eq_ignore_ascii_case("true"))
}

/// Whether a bound lib contributed its globals through **nothing**: it bound as an
/// external module (so its top-level members reach module exports, not global scope,
/// leaving `source_locals` empty) yet carries no `declare global {}` block to route
/// globals back in. Such a lib silently folds to zero globals — a no-op the resolver's
/// other census counters (files bound / sets folded) can't see. `bind_lib` guards the
/// same invariant with a `debug_assert!` for fast dev feedback; this predicate backs
/// the harness gate that holds on every build.
fn binds_external_without_globals(lib: &LibFile) -> bool {
    lib.merge.is_external && lib.merge.global_augmentations.is_empty()
}

/// Per-run lib resolver + cache: parses + binds each lib file once, and folds each
/// distinct resolved set into a [`LibBase`] once, sharing both across variants.
pub struct LibResolver {
    libs_dir: PathBuf,
    /// Lib file name -> its `/// <reference lib>` names (cached read).
    ref_cache: HashMap<String, Vec<String>>,
    /// Lib file name -> its bound product (`None` = parse-rejected / missing).
    file_cache: HashMap<String, Option<Rc<LibFile>>>,
    /// Set key (joined priority-ordered file names) -> its folded base.
    base_cache: HashMap<String, Rc<LibBase>>,
    /// Lib files that failed to parse: `(file, error)` — expected empty.
    parse_errors: Vec<(String, String)>,
    /// Referenced lib files not found on disk — expected empty.
    missing_files: Vec<String>,
    /// `@lib` / reference names `GetLibFileName` did not recognize — expected empty.
    unknown_libs: Vec<String>,
    /// Distinct resolved sets folded into a base (informational).
    sets_built: usize,
}

impl LibResolver {
    /// Build a resolver rooted at `<checkout>/internal/bundled/libs`.
    #[must_use]
    pub fn new(checkout: &Path) -> LibResolver {
        LibResolver {
            libs_dir: checkout.join("internal").join("bundled").join("libs"),
            ref_cache: HashMap::new(),
            file_cache: HashMap::new(),
            base_cache: HashMap::new(),
            parse_errors: Vec::new(),
            missing_files: Vec::new(),
            unknown_libs: Vec::new(),
            sets_built: 0,
        }
    }

    /// The resolved, priority-ordered lib files for a variant config (empty for
    /// `@noLib`). Unrecognized `@lib`/reference names are recorded and skipped.
    fn resolve_set(&mut self, config: &BTreeMap<String, String>) -> Vec<String> {
        if is_no_lib(config) {
            return Vec::new();
        }
        let mut roots: Vec<String> = Vec::new();
        if let Some(lib) = config.get("lib") {
            for part in lib.split(',') {
                let p = part.trim();
                if p.is_empty() {
                    continue;
                }
                match get_lib_file_name(p) {
                    Some(f) => roots.push(f.to_string()),
                    None => self.unknown_libs.push(p.to_string()),
                }
            }
            // An all-unrecognized @lib leaves no roots — nothing to resolve.
        } else {
            roots
                .push(default_lib_for_target(config.get("target").map(String::as_str)).to_string());
        }

        // Transitive `/// <reference lib>` closure.
        let mut closure: Vec<String> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        let mut queue: VecDeque<String> = roots.into_iter().collect();
        while let Some(file) = queue.pop_front() {
            if !seen.insert(file.clone()) {
                continue;
            }
            closure.push(file.clone());
            for name in self.references_of(&file).to_vec() {
                match get_lib_file_name(&name) {
                    Some(rf) if !seen.contains(rf) => queue.push_back(rf.to_string()),
                    Some(_) => {}
                    None => self.unknown_libs.push(name),
                }
            }
        }
        // Priority order = fold order (fixes the TS6203/6204 attribution).
        closure.sort_by_key(|f| lib_priority(f));
        closure
    }

    /// Read + bind a lib file exactly once, caching both its bound product and its
    /// `/// <reference lib>` names. A single disk read feeds both the reference scan
    /// (`references_of`, during set resolution) and the bind (`bound_file`, during the
    /// base fold) — which target the same files — so each lib file hits the disk once
    /// per run. A read failure records the file as missing (once) and caches an empty
    /// reference list + a `None` product; a bind failure records the parse error and
    /// caches `None`.
    fn ensure_loaded(&mut self, file: &str) {
        if self.file_cache.contains_key(file) {
            return;
        }
        let (bound, refs) = match std::fs::read_to_string(self.libs_dir.join(file)) {
            Ok(src) => {
                let refs = extract_lib_references(&src);
                let bound = match bind_lib(file, &src) {
                    Ok(lf) => Some(Rc::new(lf)),
                    Err(e) => {
                        self.parse_errors.push((file.to_string(), e));
                        None
                    }
                };
                (bound, refs)
            }
            Err(_) => {
                self.missing_files.push(file.to_string());
                (None, Vec::new())
            }
        };
        self.ref_cache.insert(file.to_string(), refs);
        self.file_cache.insert(file.to_string(), bound);
    }

    /// The cached `/// <reference lib>` names of a lib file (read + bound once).
    fn references_of(&mut self, file: &str) -> &[String] {
        self.ensure_loaded(file);
        &self.ref_cache[file]
    }

    /// The cached bound product of a lib file (read + bound once).
    fn bound_file(&mut self, file: &str) -> Option<Rc<LibFile>> {
        self.ensure_loaded(file);
        self.file_cache[file].clone()
    }

    /// The [`LibBase`] for a variant config, built once per distinct resolved set.
    /// `None` for `@noLib` (or an empty resolution): the phase-1 `globalThis` check
    /// still fires, but no lib globals participate.
    #[must_use]
    pub fn base_for(&mut self, config: &BTreeMap<String, String>) -> Option<Rc<LibBase>> {
        let set = self.resolve_set(config);
        if set.is_empty() {
            return None;
        }
        let key = set.join(",");
        if let Some(base) = self.base_cache.get(&key) {
            return Some(Rc::clone(base));
        }
        let files: Vec<Rc<LibFile>> = set.iter().filter_map(|f| self.bound_file(f)).collect();
        let refs: Vec<&LibFile> = files.iter().map(AsRef::as_ref).collect();
        let base = Rc::new(LibBase::build(&refs));
        self.sets_built += 1;
        self.base_cache.insert(key, Rc::clone(&base));
        Some(base)
    }

    /// The lib files that failed to parse (`(file, error)`), expected empty.
    #[must_use]
    pub fn parse_errors(&self) -> &[(String, String)] {
        &self.parse_errors
    }

    /// Referenced lib files not found on disk, expected empty.
    #[must_use]
    pub fn missing_files(&self) -> &[String] {
        &self.missing_files
    }

    /// Unrecognized `@lib` / reference names, expected empty.
    #[must_use]
    pub fn unknown_libs(&self) -> &[String] {
        &self.unknown_libs
    }

    /// Lib files that bound as an external module with no `declare global {}` block —
    /// their globals silently fold to nothing (see [`binds_external_without_globals`]).
    /// Expected empty; sorted for a deterministic gate.
    #[must_use]
    pub fn external_no_globals(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .file_cache
            .values()
            .filter_map(|slot| slot.as_deref())
            .filter(|lib| binds_external_without_globals(lib))
            .map(|lib| lib.name.clone())
            .collect();
        names.sort_unstable();
        names
    }

    /// Distinct lib files parsed + bound this run.
    #[must_use]
    pub fn files_bound(&self) -> usize {
        self.file_cache.values().filter(|v| v.is_some()).count()
    }

    /// Distinct resolved lib sets folded into a base this run.
    #[must_use]
    pub fn sets_built(&self) -> usize {
        self.sets_built
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    #[test]
    fn default_target_is_es2025_full() {
        assert_eq!(default_lib_for_target(None), "lib.es2025.full.d.ts");
        assert_eq!(default_lib_for_target(Some("es2015")), "lib.es6.d.ts");
        assert_eq!(default_lib_for_target(Some("ES2015")), "lib.es6.d.ts");
        assert_eq!(default_lib_for_target(Some("es5")), "lib.d.ts");
        assert_eq!(
            default_lib_for_target(Some("esnext")),
            "lib.esnext.full.d.ts"
        );
    }

    #[test]
    fn get_lib_file_name_names_and_files() {
        assert_eq!(get_lib_file_name("es5"), Some("lib.es5.d.ts"));
        assert_eq!(get_lib_file_name("ES2015"), Some("lib.es2015.d.ts"));
        assert_eq!(get_lib_file_name("dom"), Some("lib.dom.d.ts"));
        assert_eq!(get_lib_file_name("lib.dom.d.ts"), Some("lib.dom.d.ts"));
        assert_eq!(get_lib_file_name("notareallib"), None);
    }

    #[test]
    fn priority_orders_symbol_and_promise_related_chains() {
        // es5 leads (priority 1); the es2015 features follow by LibMap-key index.
        assert!(lib_priority("lib.es5.d.ts") < lib_priority("lib.es2015.symbol.d.ts"));
        assert!(
            lib_priority("lib.es2015.symbol.d.ts")
                < lib_priority("lib.es2015.symbol.wellknown.d.ts")
        );
        // Promise chain: es5 < es2015.iterable < es2015.promise < es2015.symbol.wellknown.
        assert!(lib_priority("lib.es5.d.ts") < lib_priority("lib.es2015.iterable.d.ts"));
        assert!(lib_priority("lib.es2015.iterable.d.ts") < lib_priority("lib.es2015.promise.d.ts"));
        assert!(
            lib_priority("lib.es2015.promise.d.ts")
                < lib_priority("lib.es2015.symbol.wellknown.d.ts")
        );
        // The aggregator roots (no declarations) sort last; lib.es6.d.ts is special.
        assert_eq!(lib_priority("lib.es6.d.ts"), 0);
        assert!(lib_priority("lib.es2025.full.d.ts") > lib_priority("lib.dom.d.ts"));
    }

    #[test]
    fn reference_extraction() {
        let src = "/// <reference lib=\"es2015\" />\n/// <reference lib=\"dom\" />\ninterface X {}";
        assert_eq!(
            extract_lib_references(src),
            vec!["es2015".to_string(), "dom".to_string()]
        );
    }

    #[test]
    fn no_lib_resolves_empty() {
        // A bare resolver needs no libs_dir for the noLib short-circuit.
        let mut r = LibResolver::new(Path::new("/nonexistent"));
        assert!(r.resolve_set(&cfg(&[("nolib", "true")])).is_empty());
    }

    /// A hand-built [`LibFile`] with a chosen module-ness and `declare global`
    /// presence — enough to exercise [`binds_external_without_globals`] without
    /// touching the filesystem or the binder.
    fn lib_file(name: &str, is_external: bool, has_global_block: bool) -> LibFile {
        use tsv_check::FileId;
        use tsv_check::merge::FileMerge;
        LibFile {
            name: name.to_string(),
            merge: FileMerge {
                file: FileId::ROOT,
                is_external,
                source_locals: Vec::new(),
                // A present-but-empty `declare global {}` block still counts (the
                // guard is presence, matching `bind_lib`'s `debug_assert!`).
                global_augmentations: if has_global_block {
                    vec![Vec::new()]
                } else {
                    Vec::new()
                },
                module_augmentations: Vec::new(),
            },
        }
    }

    #[test]
    fn external_without_globals_is_flagged() {
        // External module, no `declare global` block: globals fold to nothing — flagged.
        assert!(binds_external_without_globals(&lib_file(
            "lib.bad.d.ts",
            true,
            false
        )));
    }

    #[test]
    fn external_with_global_block_is_clean() {
        // External module WITH a `declare global {}` block (the lib.es2025.iterator
        // shape) routes globals back in — not flagged.
        assert!(!binds_external_without_globals(&lib_file(
            "lib.es2025.iterator.d.ts",
            true,
            true
        )));
    }

    #[test]
    fn ambient_script_lib_is_clean() {
        // A plain ambient script (globals in source_locals, not external) — not flagged,
        // whether or not it also carries a `declare global` block.
        assert!(!binds_external_without_globals(&lib_file(
            "lib.es5.d.ts",
            false,
            false
        )));
        assert!(!binds_external_without_globals(&lib_file(
            "lib.es5.d.ts",
            false,
            true
        )));
    }
}

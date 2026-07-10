//! Compiler-option metadata ported from tsgo, the substrate the corpus-input side
//! is built on. Two consumers: the **varyBy** derivation (which options a test may
//! fan out over) and the **known-directive universe** (which `// @key` directives
//! the harness recognizes). Both are read directly off the ported table so they
//! can't drift apart.
//!
//! The table is a faithful port of `tsoptions.OptionsDeclarations`
//! (`internal/tsoptions/declscompiler.go`: `commonOptionsWithBuild` +
//! `optionsForCompiler`) at the pinned tsgo commit — name, value kind, the
//! command-line-only flag, whether any of the eight `Affects*` flags is set
//! (collapsed to one bool, since varyBy only asks "any"), and the `strictFlag`.
//! The enum maps are ported from `internal/tsoptions/enummaps.go`; their canonical
//! value is the tsgo `core.*Kind` constant name, so aliases (`es6`/`es2015`) share
//! one identity for dedup.
//
// tsgo: internal/tsoptions/declscompiler.go OptionsDeclarations
// tsgo: internal/tsoptions/enummaps.go
// tsgo: internal/testutil/harnessutil/harnessutil.go compilerOptions/harnessCommandLineOptions

use std::collections::BTreeMap;

/// The value kind of a compiler option — the subset of tsgo's
/// `CommandLineOptionKind` the harness distinguishes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OptionKind {
    /// `boolean`.
    Boolean,
    /// `number`.
    Number,
    /// `string`.
    Str,
    /// `object` (e.g. `paths`).
    Object,
    /// `list`.
    List,
    /// `enum` (map-backed).
    Enum,
}

/// One ported compiler-option declaration (the fields the harness reads).
#[derive(Clone, Copy, Debug)]
pub struct OptionMeta {
    /// The option's canonical (camelCase) name; directive lookup is
    /// case-insensitive.
    pub name: &'static str,
    /// The value kind.
    pub kind: OptionKind,
    /// `IsCommandLineOnly` — command-line-only options never vary.
    pub command_line_only: bool,
    /// OR of the eight `Affects*` flags (varyBy only asks whether any is set).
    pub affects: bool,
    /// `strictFlag` — a member of the `strict` family (inherits `strict` when
    /// unset).
    pub strict_flag: bool,
}

const fn om(
    name: &'static str,
    kind: OptionKind,
    command_line_only: bool,
    affects: bool,
    strict_flag: bool,
) -> OptionMeta {
    OptionMeta {
        name,
        kind,
        command_line_only,
        affects,
        strict_flag,
    }
}

/// The ported `OptionsDeclarations` table (duplicate names collapsed to the
/// first, matching `getCommandLineOption`'s first-match lookup).
pub const OPTIONS: &[OptionMeta] = &[
    om("help", OptionKind::Boolean, true, false, false),
    om("watch", OptionKind::Boolean, true, false, false),
    om("preserveWatchOutput", OptionKind::Boolean, false, false, false),
    om("listFiles", OptionKind::Boolean, false, false, false),
    om("explainFiles", OptionKind::Boolean, false, false, false),
    om("listEmittedFiles", OptionKind::Boolean, false, false, false),
    om("pretty", OptionKind::Boolean, false, false, false),
    om("traceResolution", OptionKind::Boolean, false, false, false),
    om("diagnostics", OptionKind::Boolean, false, false, false),
    om("extendedDiagnostics", OptionKind::Boolean, false, false, false),
    om("generateCpuProfile", OptionKind::Str, false, false, false),
    om("generateTrace", OptionKind::Str, false, false, false),
    om("incremental", OptionKind::Boolean, false, false, false),
    om("declaration", OptionKind::Boolean, false, true, false),
    om("declarationMap", OptionKind::Boolean, false, true, false),
    om("emitDeclarationOnly", OptionKind::Boolean, false, true, false),
    om("sourceMap", OptionKind::Boolean, false, true, false),
    om("inlineSourceMap", OptionKind::Boolean, false, true, false),
    om("noCheck", OptionKind::Boolean, false, false, false),
    om("deduplicatePackages", OptionKind::Boolean, false, true, false),
    om("noEmit", OptionKind::Boolean, false, false, false),
    om("assumeChangesOnlyAffectDirectDependencies", OptionKind::Boolean, false, true, false),
    om("locale", OptionKind::Str, true, false, false),
    om("quiet", OptionKind::Boolean, false, false, false),
    om("singleThreaded", OptionKind::Boolean, false, false, false),
    om("pprofDir", OptionKind::Str, false, false, false),
    om("checkers", OptionKind::Number, false, false, false),
    om("all", OptionKind::Boolean, false, false, false),
    om("version", OptionKind::Boolean, false, false, false),
    om("init", OptionKind::Boolean, false, false, false),
    om("project", OptionKind::Str, false, false, false),
    om("showConfig", OptionKind::Boolean, true, false, false),
    om("listFilesOnly", OptionKind::Boolean, true, false, false),
    om("ignoreConfig", OptionKind::Boolean, true, false, false),
    om("target", OptionKind::Enum, false, true, false),
    om("module", OptionKind::Enum, false, true, false),
    om("lib", OptionKind::List, false, true, false),
    om("allowJs", OptionKind::Boolean, false, true, false),
    om("checkJs", OptionKind::Boolean, false, true, false),
    om("jsx", OptionKind::Enum, false, true, false),
    om("outFile", OptionKind::Str, false, true, false),
    om("outDir", OptionKind::Str, false, true, false),
    om("rootDir", OptionKind::Str, false, true, false),
    om("composite", OptionKind::Boolean, false, true, false),
    om("tsBuildInfoFile", OptionKind::Str, false, true, false),
    om("removeComments", OptionKind::Boolean, false, true, false),
    om("importHelpers", OptionKind::Boolean, false, true, false),
    om("downlevelIteration", OptionKind::Boolean, false, true, false),
    om("isolatedModules", OptionKind::Boolean, false, false, false),
    om("verbatimModuleSyntax", OptionKind::Boolean, false, true, false),
    om("isolatedDeclarations", OptionKind::Boolean, false, true, false),
    om("erasableSyntaxOnly", OptionKind::Boolean, false, true, false),
    om("libReplacement", OptionKind::Boolean, false, true, false),
    om("strict", OptionKind::Boolean, false, true, false),
    om("noImplicitAny", OptionKind::Boolean, false, true, true),
    om("strictNullChecks", OptionKind::Boolean, false, true, true),
    om("strictFunctionTypes", OptionKind::Boolean, false, true, true),
    om("strictBindCallApply", OptionKind::Boolean, false, true, true),
    om("strictPropertyInitialization", OptionKind::Boolean, false, true, true),
    om("strictBuiltinIteratorReturn", OptionKind::Boolean, false, true, true),
    om("noImplicitThis", OptionKind::Boolean, false, true, true),
    om("useUnknownInCatchVariables", OptionKind::Boolean, false, true, true),
    om("alwaysStrict", OptionKind::Boolean, false, true, false),
    om("stableTypeOrdering", OptionKind::Boolean, false, true, false),
    om("noUnusedLocals", OptionKind::Boolean, false, true, false),
    om("noUnusedParameters", OptionKind::Boolean, false, true, false),
    om("exactOptionalPropertyTypes", OptionKind::Boolean, false, true, false),
    om("noImplicitReturns", OptionKind::Boolean, false, true, false),
    om("noFallthroughCasesInSwitch", OptionKind::Boolean, false, true, false),
    om("noUncheckedIndexedAccess", OptionKind::Boolean, false, true, false),
    om("noImplicitOverride", OptionKind::Boolean, false, true, false),
    om("noPropertyAccessFromIndexSignature", OptionKind::Boolean, false, true, false),
    om("moduleResolution", OptionKind::Enum, false, true, false),
    om("baseUrl", OptionKind::Str, false, true, false),
    om("paths", OptionKind::Object, false, true, false),
    om("rootDirs", OptionKind::List, false, true, false),
    om("typeRoots", OptionKind::List, false, true, false),
    om("types", OptionKind::List, false, true, false),
    om("allowSyntheticDefaultImports", OptionKind::Boolean, false, true, false),
    om("esModuleInterop", OptionKind::Boolean, false, true, false),
    om("preserveSymlinks", OptionKind::Boolean, false, false, false),
    om("allowUmdGlobalAccess", OptionKind::Boolean, false, true, false),
    om("moduleSuffixes", OptionKind::List, false, true, false),
    om("allowImportingTsExtensions", OptionKind::Boolean, false, true, false),
    om("rewriteRelativeImportExtensions", OptionKind::Boolean, false, true, false),
    om("resolvePackageJsonExports", OptionKind::Boolean, false, true, false),
    om("resolvePackageJsonImports", OptionKind::Boolean, false, true, false),
    om("customConditions", OptionKind::List, false, true, false),
    om("noUncheckedSideEffectImports", OptionKind::Boolean, false, true, false),
    om("sourceRoot", OptionKind::Str, false, true, false),
    om("mapRoot", OptionKind::Str, false, true, false),
    om("inlineSources", OptionKind::Boolean, false, true, false),
    om("experimentalDecorators", OptionKind::Boolean, false, true, false),
    om("emitDecoratorMetadata", OptionKind::Boolean, false, true, false),
    om("jsxFactory", OptionKind::Str, false, false, false),
    om("jsxFragmentFactory", OptionKind::Str, false, false, false),
    om("jsxImportSource", OptionKind::Str, false, true, false),
    om("resolveJsonModule", OptionKind::Boolean, false, true, false),
    om("allowArbitraryExtensions", OptionKind::Boolean, false, true, false),
    om("reactNamespace", OptionKind::Str, false, true, false),
    om("skipDefaultLibCheck", OptionKind::Boolean, false, true, false),
    om("emitBOM", OptionKind::Boolean, false, true, false),
    om("newLine", OptionKind::Enum, false, true, false),
    om("noErrorTruncation", OptionKind::Boolean, false, true, false),
    om("noLib", OptionKind::Boolean, false, true, false),
    om("noResolve", OptionKind::Boolean, false, true, false),
    om("stripInternal", OptionKind::Boolean, false, true, false),
    om("disableSizeLimit", OptionKind::Boolean, false, true, false),
    om("disableSourceOfProjectReferenceRedirect", OptionKind::Boolean, false, false, false),
    om("disableSolutionSearching", OptionKind::Boolean, false, false, false),
    om("disableReferencedProjectLoad", OptionKind::Boolean, false, false, false),
    om("noEmitHelpers", OptionKind::Boolean, false, true, false),
    om("noEmitOnError", OptionKind::Boolean, false, true, false),
    om("preserveConstEnums", OptionKind::Boolean, false, true, false),
    om("declarationDir", OptionKind::Str, false, true, false),
    om("skipLibCheck", OptionKind::Boolean, false, true, false),
    om("allowUnusedLabels", OptionKind::Boolean, false, true, false),
    om("allowUnreachableCode", OptionKind::Boolean, false, true, false),
    om("forceConsistentCasingInFileNames", OptionKind::Boolean, false, true, false),
    om("maxNodeModuleJsDepth", OptionKind::Number, false, true, false),
    om("useDefineForClassFields", OptionKind::Boolean, false, true, false),
    om("plugins", OptionKind::List, false, false, false),
    om("moduleDetection", OptionKind::Enum, false, true, false),
    om("ignoreDeprecations", OptionKind::Str, false, false, false),
];

/// One enum-option value mapping: the directive key and its canonical identity
/// (the tsgo `core.*Kind` constant name), so aliases collapse for dedup.
#[derive(Clone, Copy, Debug)]
pub struct EnumEntry {
    /// The owning option's name.
    pub option: &'static str,
    /// The directive-writable value key (already lowercase).
    pub key: &'static str,
    /// The canonical identity shared by aliases.
    pub canonical: &'static str,
}

const fn ee(option: &'static str, key: &'static str, canonical: &'static str) -> EnumEntry {
    EnumEntry {
        option,
        key,
        canonical,
    }
}

/// The ported enum maps for every varyBy-eligible enum option. `lib` is omitted:
/// it is a list, never a variant.
pub const ENUM_ENTRIES: &[EnumEntry] = &[
    ee("target", "es5", "core.ScriptTargetES5"),
    ee("target", "es6", "core.ScriptTargetES2015"),
    ee("target", "es2015", "core.ScriptTargetES2015"),
    ee("target", "es2016", "core.ScriptTargetES2016"),
    ee("target", "es2017", "core.ScriptTargetES2017"),
    ee("target", "es2018", "core.ScriptTargetES2018"),
    ee("target", "es2019", "core.ScriptTargetES2019"),
    ee("target", "es2020", "core.ScriptTargetES2020"),
    ee("target", "es2021", "core.ScriptTargetES2021"),
    ee("target", "es2022", "core.ScriptTargetES2022"),
    ee("target", "es2023", "core.ScriptTargetES2023"),
    ee("target", "es2024", "core.ScriptTargetES2024"),
    ee("target", "es2025", "core.ScriptTargetES2025"),
    ee("target", "esnext", "core.ScriptTargetESNext"),
    ee("module", "commonjs", "core.ModuleKindCommonJS"),
    ee("module", "amd", "core.ModuleKindAMD"),
    ee("module", "system", "core.ModuleKindSystem"),
    ee("module", "umd", "core.ModuleKindUMD"),
    ee("module", "es6", "core.ModuleKindES2015"),
    ee("module", "es2015", "core.ModuleKindES2015"),
    ee("module", "es2020", "core.ModuleKindES2020"),
    ee("module", "es2022", "core.ModuleKindES2022"),
    ee("module", "esnext", "core.ModuleKindESNext"),
    ee("module", "node16", "core.ModuleKindNode16"),
    ee("module", "node18", "core.ModuleKindNode18"),
    ee("module", "node20", "core.ModuleKindNode20"),
    ee("module", "nodenext", "core.ModuleKindNodeNext"),
    ee("module", "preserve", "core.ModuleKindPreserve"),
    ee("moduleResolution", "node16", "core.ModuleResolutionKindNode16"),
    ee("moduleResolution", "nodenext", "core.ModuleResolutionKindNodeNext"),
    ee("moduleResolution", "bundler", "core.ModuleResolutionKindBundler"),
    ee("moduleResolution", "classic", "core.ModuleResolutionKindClassic"),
    ee("moduleResolution", "node", "core.ModuleResolutionKindNode10"),
    ee("moduleResolution", "node10", "core.ModuleResolutionKindNode10"),
    ee("jsx", "preserve", "core.JsxEmitPreserve"),
    ee("jsx", "react-native", "core.JsxEmitReactNative"),
    ee("jsx", "react-jsx", "core.JsxEmitReactJSX"),
    ee("jsx", "react-jsxdev", "core.JsxEmitReactJSXDev"),
    ee("jsx", "react", "core.JsxEmitReact"),
    ee("moduleDetection", "auto", "core.ModuleDetectionKindAuto"),
    ee("moduleDetection", "legacy", "core.ModuleDetectionKindLegacy"),
    ee("moduleDetection", "force", "core.ModuleDetectionKindForce"),
    ee("newLine", "crlf", "core.NewLineKindCRLF"),
    ee("newLine", "lf", "core.NewLineKindLF"),
];

/// The four synthetic compiler options the harness appends to `OptionsDeclarations`
/// (`harnessutil.go`'s `compilerOptions`) — known directives, but not real
/// compiler flags. `noErrorTruncation` / `noCheck` also exist in the real table;
/// the duplicates are harmless for a membership set.
pub const SYNTHETIC_OPTIONS: &[&str] = &[
    "allowNonTsExtensions",
    "noErrorTruncation",
    "suppressOutputPathCheck",
    "noCheck",
];

/// The thirteen harness-only command-line options (`harnessCommandLineOptions`).
/// These are recognized directives that configure the harness rather than the
/// compiler.
pub const HARNESS_OPTIONS: &[&str] = &[
    "useCaseSensitiveFileNames",
    "baselineFile",
    "includeBuiltFile",
    "fileName",
    "libFiles",
    "noImplicitReferences",
    "currentDirectory",
    "symlink",
    "link",
    "noTypesAndSymbols",
    "fullEmitPaths",
    "reportDiagnostics",
    "captureSuggestions",
];

/// The static test-level skip list (`compiler_runner.go` `skippedTests`): 45 tests
/// skipped by basename before directives are parsed (built-API dependence or
/// completely-removed options that fail to parse). They produce no baseline.
pub const SKIPPED_TESTS: &[&str] = &[
    "APILibCheck.ts",
    "APISample_Watch.ts",
    "APISample_WatchWithDefaults.ts",
    "APISample_WatchWithOwnWatchHost.ts",
    "APISample_compile.ts",
    "APISample_jsdoc.ts",
    "APISample_linter.ts",
    "APISample_parseConfig.ts",
    "APISample_transform.ts",
    "APISample_watcher.ts",
    "preserveUnusedImports.ts",
    "noCrashWithVerbatimModuleSyntaxAndImportsNotUsedAsValues.ts",
    "verbatimModuleSyntaxCompat.ts",
    "verbatimModuleSyntaxCompat2.ts",
    "verbatimModuleSyntaxCompat3.ts",
    "verbatimModuleSyntaxCompat4.ts",
    "preserveValueImports.ts",
    "preserveValueImports_importsNotUsedAsValues.ts",
    "preserveValueImports_errors.ts",
    "preserveValueImports_mixedImports.ts",
    "preserveValueImports_module.ts",
    "importsNotUsedAsValues_error.ts",
    "alwaysStrictNoImplicitUseStrict.ts",
    "nonPrimitiveIndexingWithForInSupressError.ts",
    "parameterInitializerBeforeDestructuringEmit.ts",
    "mappedTypeUnionConstraintInferences.ts",
    "lateBoundConstraintTypeChecksCorrectly.ts",
    "keyofDoesntContainSymbols.ts",
    "isolatedModulesOut.ts",
    "noStrictGenericChecks.ts",
    "noImplicitUseStrict_umd.ts",
    "noImplicitUseStrict_system.ts",
    "noImplicitUseStrict_es6.ts",
    "noImplicitUseStrict_commonjs.ts",
    "noImplicitUseStrict_amd.ts",
    "noImplicitAnyIndexingSuppressed.ts",
    "excessPropertyErrorsSuppressed.ts",
    "moduleNoneDynamicImport.ts",
    "moduleNoneErrors.ts",
    "moduleNoneOutFile.ts",
    "noErrorUsingImportExportModuleAugmentationInDeclarationFile1.ts",
    "noErrorUsingImportExportModuleAugmentationInDeclarationFile2.ts",
    "noErrorUsingImportExportModuleAugmentationInDeclarationFile3.ts",
    "requireOfJsonFileWithModuleEmitNone.ts",
    "requireOfJsonFileWithModuleNodeResolutionEmitNone.ts",
];

/// The extra directive the harness swallows without validation
/// (`SetOptionsFromTestConfig` special-cases `typescriptversion`).
pub const SWALLOWED_DIRECTIVE: &str = "typescriptversion";

/// tsgo's three-state boolean (`core.Tristate`): unset inherits, only an explicit
/// value is `False`/`True`. Modeled because the harness's skip check reads
/// `IsFalse()` (explicit false), not the inherited value.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tristate {
    /// No value provided — inherits (e.g. from `strict`).
    Unset,
    /// Explicit `false`.
    False,
    /// Explicit `true`.
    True,
}

/// The harness-forced compiler defaults (`CompileFiles`), kept distinct from the
/// real compiler defaults: `newLine`→CRLF and `skipDefaultLibCheck`→true when
/// unset, `noErrorTruncation`→true unconditionally. None of these are skip
/// triggers, so they don't affect variant selection — recorded as substrate.
pub const HARNESS_FORCED_DEFAULTS: &[(&str, &str)] = &[
    ("newLine", "crlf (when unset)"),
    ("skipDefaultLibCheck", "true (when unset)"),
    ("noErrorTruncation", "true (always)"),
];

/// The harness default for case-sensitive file names (`HarnessOptions{
/// UseCaseSensitiveFileNames: true }`).
pub const DEFAULT_USE_CASE_SENSITIVE_FILE_NAMES: bool = true;

/// Whether a unit's file name is a tsconfig/jsconfig (`GetConfigNameFromFileName`):
/// its basename, lowercased, is `tsconfig.json` or `jsconfig.json`.
#[must_use]
pub fn is_config_file_name(file_name: &str) -> bool {
    let base = file_name.rsplit(['/', '\\']).next().unwrap_or(file_name);
    let lower = base.to_ascii_lowercase();
    lower == "tsconfig.json" || lower == "jsconfig.json"
}

/// Find an option by name, case-insensitively (mirrors `getCommandLineOption`'s
/// `EqualFold` first-match).
#[must_use]
pub fn lookup(name: &str) -> Option<&'static OptionMeta> {
    OPTIONS.iter().find(|o| o.name.eq_ignore_ascii_case(name))
}

/// Whether an option (by lowercased directive key) is a varyBy option:
/// `!IsCommandLineOnly && (boolean|enum) && any Affects*`, plus the two hardcoded
/// additions `noEmit` and `isolatedModules` (`getCompilerVaryByMap`).
#[must_use]
pub fn is_vary_by(name_lower: &str) -> bool {
    if name_lower == "noemit" || name_lower == "isolatedmodules" {
        return true;
    }
    lookup(name_lower).is_some_and(|o| {
        !o.command_line_only
            && matches!(o.kind, OptionKind::Boolean | OptionKind::Enum)
            && o.affects
    })
}

/// Whether a `// @key` directive (lowercased key) is recognized by the harness:
/// a compiler option, a synthetic option, a harness option, or the swallowed
/// `typescriptversion`. An unrecognized directive is a hard harness failure.
#[must_use]
pub fn is_known_directive(name_lower: &str) -> bool {
    if name_lower == SWALLOWED_DIRECTIVE {
        return true;
    }
    lookup(name_lower).is_some()
        || SYNTHETIC_OPTIONS.iter().any(|s| s.eq_ignore_ascii_case(name_lower))
        || HARNESS_OPTIONS.iter().any(|s| s.eq_ignore_ascii_case(name_lower))
}

/// A normalized option value — the identity used for variant dedup within one
/// option (never compared across options).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum NormValue {
    /// A canonical enum identity (shared by aliases).
    Enum(&'static str),
    /// A boolean.
    Bool(bool),
    /// Any other kind's raw string.
    Other(String),
}

/// Normalize a value for a varyBy option, or `None` if the value is unrecognized
/// (an unknown enum key / non-boolean). Mirrors `tryGetValueOfOptionString`.
#[must_use]
pub fn normalize_value(option_lower: &str, value: &str) -> Option<NormValue> {
    let vlower = value.to_ascii_lowercase();
    if ENUM_ENTRIES.iter().any(|e| e.option.eq_ignore_ascii_case(option_lower)) {
        return ENUM_ENTRIES
            .iter()
            .find(|e| e.option.eq_ignore_ascii_case(option_lower) && e.key == vlower)
            .map(|e| NormValue::Enum(e.canonical));
    }
    match lookup(option_lower).map(|o| o.kind) {
        Some(OptionKind::Boolean) => match vlower.as_str() {
            "true" => Some(NormValue::Bool(true)),
            "false" => Some(NormValue::Bool(false)),
            _ => None,
        },
        _ => Some(NormValue::Other(value.to_string())),
    }
}

/// Every writable value for a varyBy option, in declaration order (enum keys, or
/// `true`/`false`). Empty for non-enum/non-boolean options. Mirrors
/// `getAllValuesForOption` (the `*` expansion source).
#[must_use]
pub fn all_values(option_lower: &str) -> Vec<&'static str> {
    if ENUM_ENTRIES.iter().any(|e| e.option.eq_ignore_ascii_case(option_lower)) {
        return ENUM_ENTRIES
            .iter()
            .filter(|e| e.option.eq_ignore_ascii_case(option_lower))
            .map(|e| e.key)
            .collect();
    }
    match lookup(option_lower).map(|o| o.kind) {
        Some(OptionKind::Boolean) => vec!["true", "false"],
        _ => Vec::new(),
    }
}

/// The `strict`-family member names (their `strictFlag`): each inherits `strict`
/// when unset (and unset `strict` counts as `true`). Substrate for the options
/// model; variant selection does not use inheritance (the skip check reads the
/// explicit tri-state).
#[must_use]
pub fn strict_members() -> Vec<&'static str> {
    OPTIONS.iter().filter(|o| o.strict_flag).map(|o| o.name).collect()
}

/// Resolve a boolean option's explicit tri-state from a variant config: `True` /
/// `False` for an explicit value, `Unset` when absent or unparseable. This is the
/// `IsFalse()` distinction the skip check needs — an unset boolean inherits and is
/// not treated as `false`.
#[must_use]
pub fn resolve_bool(config: &BTreeMap<String, String>, key_lower: &str) -> Tristate {
    match config.get(key_lower).map(|v| normalize_value(key_lower, v)) {
        Some(Some(NormValue::Bool(true))) => Tristate::True,
        Some(Some(NormValue::Bool(false))) => Tristate::False,
        _ => Tristate::Unset,
    }
}

/// Whether a resolved variant config is skipped by `SkipUnsupportedCompilerOptions`
/// (the Removed-in-TS7 option classes): unsupported `module` / `moduleResolution`
/// / `target`, an explicitly-false `esModuleInterop` / `allowSyntheticDefaultImports`
/// / `alwaysStrict`, or a set `baseUrl` / `outFile`.
///
/// Resolved from directive-provided values only (tsconfig-provided options are out
/// of scope). This is exact for directive-driven tests and can only *under*-skip a
/// tsconfig-driven one — safe for the join, since a skipped variant has no baseline
/// either way.
///
/// `config` maps lowercased directive keys to their raw string values.
#[must_use]
pub fn variant_is_unsupported(config: &BTreeMap<String, String>) -> bool {
    // Unsupported module kinds.
    if let Some(v) = config.get("module")
        && let Some(NormValue::Enum(c)) = normalize_value("module", v)
        && matches!(
            c,
            "core.ModuleKindAMD" | "core.ModuleKindUMD" | "core.ModuleKindSystem"
        )
    {
        return true;
    }
    // Unsupported module-resolution kinds.
    if let Some(v) = config.get("moduleresolution")
        && let Some(NormValue::Enum(c)) = normalize_value("moduleResolution", v)
        && matches!(
            c,
            "core.ModuleResolutionKindNode10" | "core.ModuleResolutionKindClassic"
        )
    {
        return true;
    }
    // Unsupported target ES5.
    if let Some(v) = config.get("target")
        && normalize_value("target", v) == Some(NormValue::Enum("core.ScriptTargetES5"))
    {
        return true;
    }
    // Explicitly-false booleans (`IsFalse()`: an unset value inherits, so only an
    // explicit `False` triggers the skip).
    for key in ["esmoduleinterop", "allowsyntheticdefaultimports", "alwaysstrict"] {
        if resolve_bool(config, key) == Tristate::False {
            return true;
        }
    }
    // Set string paths.
    if config.get("baseurl").is_some_and(|v| !v.trim().is_empty()) {
        return true;
    }
    if config.get("outfile").is_some_and(|v| !v.trim().is_empty()) {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_shape_pins() {
        // Ported from tsgo at the pinned commit; a move is a deliberate re-port.
        assert_eq!(OPTIONS.len(), 124);
        assert_eq!(ENUM_ENTRIES.len(), 44);
        assert_eq!(SKIPPED_TESTS.len(), 45);
        assert_eq!(HARNESS_OPTIONS.len(), 13);
        assert_eq!(SYNTHETIC_OPTIONS.len(), 4);
        // getCompilerVaryByMap derives 70 affects-based options plus the two
        // hardcoded additions (noEmit, isolatedModules) = 72 at the pin.
        let vary = OPTIONS
            .iter()
            .filter(|o| is_vary_by(&o.name.to_ascii_lowercase()))
            .count();
        assert_eq!(vary, 72);
    }

    #[test]
    fn vary_by_rules() {
        assert!(is_vary_by("strict")); // boolean + affects
        assert!(is_vary_by("target")); // enum + affects
        assert!(is_vary_by("noemit")); // hardcoded
        assert!(is_vary_by("isolatedmodules")); // hardcoded
        assert!(!is_vary_by("lib")); // list (comma is not a variant)
        assert!(!is_vary_by("help")); // command-line-only
        assert!(!is_vary_by("jsxfactory")); // string, no affects
        assert!(!is_vary_by("filename")); // harness option, not a compiler option
    }

    #[test]
    fn known_directive_universe() {
        assert!(is_known_directive("strict"));
        assert!(is_known_directive("filename")); // harness
        assert!(is_known_directive("allownontsextensions")); // synthetic
        assert!(is_known_directive("typescriptversion")); // swallowed
        assert!(!is_known_directive("definitelynotanoption"));
    }

    #[test]
    fn value_normalization_and_dedup() {
        // es6 and es2015 alias to one identity.
        assert_eq!(
            normalize_value("target", "es6"),
            normalize_value("target", "ES2015")
        );
        assert_eq!(normalize_value("strict", "TRUE"), Some(NormValue::Bool(true)));
        assert_eq!(normalize_value("target", "nope"), None);
        assert_eq!(all_values("moduledetection"), vec!["auto", "legacy", "force"]);
        assert_eq!(all_values("strict"), vec!["true", "false"]);
    }

    #[test]
    fn skip_resolution() {
        let mk = |k: &str, v: &str| {
            let mut m = BTreeMap::new();
            m.insert(k.to_string(), v.to_string());
            m
        };
        assert!(variant_is_unsupported(&mk("target", "es5")));
        assert!(variant_is_unsupported(&mk("module", "amd")));
        assert!(variant_is_unsupported(&mk("moduleresolution", "classic")));
        assert!(variant_is_unsupported(&mk("esmoduleinterop", "false")));
        assert!(variant_is_unsupported(&mk("outfile", "out.js")));
        assert!(!variant_is_unsupported(&mk("target", "es2015")));
        assert!(!variant_is_unsupported(&mk("module", "esnext")));
        assert!(!variant_is_unsupported(&mk("esmoduleinterop", "true")));
        assert!(!variant_is_unsupported(&BTreeMap::new()));
    }
}

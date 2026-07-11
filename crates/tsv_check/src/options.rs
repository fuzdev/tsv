//! Checker options — tsv_check's first options surface.
//!
//! The checker's observable behavior is mostly option-independent (the
//! bind/merge duplicate-conflict family, the syntactic check pass), but the
//! reachability shims read a small set of compiler options: `allowUnreachableCode`
//! (TS7027), `allowUnusedLabels` (TS7028), and `preserveConstEnums` (which of an
//! unreachable module/enum member's declarations count as executable). This is the
//! whole of tsv_check's options model — deliberately minimal, ported only where a
//! diagnostic's category or existence actually depends on it.
//
// tsgo: internal/core/tristate.go Tristate; internal/core/compileroptions.go
//       (AllowUnreachableCode / AllowUnusedLabels / PreserveConstEnums +
//        ShouldPreserveConstEnums).

/// A three-state boolean mirroring tsgo's `core.Tristate`: an **unset** option
/// (`Unknown`, the default) inherits rather than reading as `false`, so the
/// suggestion-vs-error routing needs the explicit-`False` distinction.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Tristate {
    /// No explicit value — inherits (routes reachability diagnostics as
    /// *suggestions*, tsgo's default when the flag is unset).
    #[default]
    Unknown,
    /// Explicit `false` — the reachability diagnostic is an **error**.
    False,
    /// Explicit `true` — the reachability probe is **skipped** entirely.
    True,
}

/// The checker options tsv_check reads. `Default` is the all-unset state
/// (`Unknown` / `Unknown` / `false`), which every non-harness caller passes.
#[derive(Clone, Copy, Debug, Default)]
pub struct CheckOptions {
    /// `allowUnreachableCode` — gates TS7027 (`Unreachable code detected.`):
    /// `False` → error, `Unknown` → suggestion, `True` → no report.
    pub allow_unreachable_code: Tristate,
    /// `allowUnusedLabels` — gates TS7028 (`Unused label.`): same routing.
    pub allow_unused_labels: Tristate,
    /// `ShouldPreserveConstEnums()` — whether an unreachable `const enum` (and a
    /// const-enum-only namespace) counts as executable and so is reportable.
    pub preserve_const_enums: bool,
}

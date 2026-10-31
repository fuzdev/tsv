//! tsv's file-discovery policy (the build-output heuristic, safety-net pruning and the
//! warnings) over [`tsv_ignore`] — part of [tsv](https://github.com/fuzdev/tsv).
//!
//! **No public API.** The crate exists for tsv's own tools — the `tsv` CLI and its WASM and
//! N-API bindings, which share its discovery decisions — and every item in it is hidden from
//! these docs: outside semver, free to change in any release. A tool that wants gitignore
//! matching alone should depend on [`tsv_ignore`].

// Maintainer notes (not rendered — the crate has no public API to document):
//
// The per-directory and per-file decisions `tsv format` makes while walking a tree, as pure
// functions over a `tsv_ignore::IgnoreStack` (the matcher) plus the entry name and its
// format-root-relative path. This is the single home of the build-output heuristic, the
// always-pruned safety nets, the formattable-extension check (both as a discovery filter and
// as the unsupported-extension error for a named file argument), the heuristic-shadow warning
// text, the `.prettierignore`-shadowed warning, the `.prettierignore`-outside-a-repo warning,
// and the gate on a path an argument names (bounded by the ignore files alone, with its
// warning). The three discovery surfaces — the native CLI, the JS CLI in tsv's npm packages,
// and the VS Code extension — call it instead of reimplementing the decision, so they agree
// by construction rather than by hand-mirrored constants and templates.
//
// Everything here is pure: no filesystem access. Locating directories, reading ignore files,
// resolving the format root, and walking the tree stay in each caller; only the verdict is
// shared. The matcher this builds on (`tsv_ignore`) stays a pure gitignore(5) matcher and
// deliberately does not absorb this policy (the `dist`/`build`/`target` list, the hidden-dir
// rule, the safety nets, the warning).
//
// Every path a verdict or a warning takes (`child_rel`, `rel`, `dir`) is relative to the
// format root and `/`-separated, read as `tsv_ignore` reads one (its §Paths); an entry's name
// is its last `/` segment.
//
// tsv is non-configurable for style; file scope (which files get reformatted) is the one
// sanctioned carve-out, and this crate is the policy half of it. It is not a language
// abstraction — no `Language` trait, registry, or dispatch.

#![warn(missing_docs)]

mod policy;
mod quote;
mod warnings;

#[doc(hidden)]
pub use policy::{
    BuildOutputHeuristic, DirVerdict, FORMATTABLE_EXTENSIONS, HEURISTIC_DIRS, SAFETY_NET_DIRS,
    classify_dir, formattable_extension_list, is_formattable, is_path_pruned, is_safety_net,
    path_shadow_warning, should_format_file, unstrippable_extension_error,
    unsupported_extension_error,
};
#[doc(hidden)]
pub use quote::{quote_path, quote_path_bytes, quote_path_owned};
#[doc(hidden)]
pub use warnings::{
    DirIgnoreFiles, excluded_argument_warning, gitignore_symlink_warning,
    prettierignore_outside_repo_warning, prettierignore_shadowed_warning, shadow_warning,
    unresolvable_root_error,
};

/// The stacks the modules' tests build — one override layer at the root, or any number of
/// `.gitignore` and `.formatignore` layers at their anchors.
#[cfg(test)]
pub(crate) mod test_support {
    use tsv_ignore::IgnoreStack;

    /// One `.formatignore` layer at the root.
    pub(crate) fn override_stack(content: &str) -> IgnoreStack {
        let mut stack = IgnoreStack::new();
        stack.push_formatignore("", content);
        stack
    }

    /// Assemble a stack the way a per-file consumer (the VS Code extension) does:
    /// every `.gitignore` layer shallow→deep, then every override layer shallow→deep.
    pub(crate) fn stack_from(
        gitignores: &[(&str, &str)],
        overrides: &[(&str, &str)],
    ) -> IgnoreStack {
        let mut stack = IgnoreStack::new();
        for (anchor, content) in gitignores {
            stack.push_gitignore(anchor, content);
        }
        for (anchor, content) in overrides {
            stack.push_formatignore(anchor, content);
        }
        stack
    }
}

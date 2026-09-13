//! tsv's file-discovery **policy** — the per-directory and per-file decisions
//! `tsv format` makes while walking a tree, as pure functions over a
//! [`tsv_ignore::IgnoreStack`] (the matcher) plus the entry name and its
//! format-root-relative path.
//!
//! This is the single home of the build-output heuristic, the always-pruned
//! safety nets, the formattable-extension check (both as a discovery filter and
//! as the unsupported-extension error for a named file argument), the
//! heuristic-shadow warning text, the `.prettierignore`-shadowed warning, the
//! `.prettierignore`-outside-a-repo warning, and the gate on a path an argument names
//! (bounded by the ignore files alone, with its warning). The three discovery
//! surfaces — the native CLI (`tsv_cli`), the
//! WASM CLI (`tsv_wasm`'s `npm/cli.js`), and the VS Code extension — call it
//! instead of reimplementing the decision, so they agree **by construction**
//! rather than by hand-mirrored constants and templates.
//!
//! Everything here is **pure**: no filesystem access. Locating directories,
//! reading ignore files, resolving the format root, and walking the tree stay in
//! each caller; only the *verdict* is shared. The matcher this builds on
//! ([`tsv_ignore`]) stays a pure gitignore(5) matcher and deliberately does
//! **not** absorb this policy (the `dist`/`build`/`target` list, the hidden-dir
//! rule, the safety nets, the warning).
//!
//! tsv is non-configurable for *style*; file *scope* (which files get
//! reformatted) is the one sanctioned carve-out. This crate is the policy half
//! of that carve-out. It is **not** a language abstraction — no `Language`
//! trait, registry, or dispatch — so it doesn't touch tsv's "Closed Scope, Open
//! Convention" stance.
//!
//! ```
//! use tsv_discover::{DirVerdict, classify_dir, should_format_file};
//! use tsv_ignore::IgnoreStack;
//!
//! let mut stack = IgnoreStack::new();
//! stack.push_gitignore("", "dist/\n"); // a .gitignore present → heuristic off
//!
//! // gitignored `dist` → pruned; clean `src` → descend
//! assert_eq!(classify_dir("dist", "dist", false, &stack), DirVerdict::Prune);
//! assert_eq!(classify_dir("src", "src", false, &stack), DirVerdict::Descend);
//! // a non-ignored `.ts` file → format it
//! assert!(should_format_file("app.ts", "src/app.ts", &stack));
//! ```

mod policy;
mod quote;
mod warnings;

pub use policy::{
    DirVerdict, FORMATTABLE_EXTENSIONS, HEURISTIC_DIRS, SAFETY_NET_DIRS, classify_dir,
    formattable_extension_list, is_formattable, is_path_pruned, is_safety_net, path_shadow_warning,
    should_format_file, unsupported_extension_error,
};
pub use quote::{quote_path, quote_path_bytes, quote_path_owned};
pub use warnings::{
    excluded_argument_warning, gitignore_symlink_warning, prettierignore_outside_repo_warning,
    prettierignore_shadowed_warning, shadow_warning, unresolvable_root_error,
};

/// The stacks the modules' tests build — one tsv layer at the root, or any number of
/// `.gitignore` and `.formatignore` layers at their anchors.
#[cfg(test)]
pub(crate) mod test_support {
    use tsv_ignore::IgnoreStack;

    /// One `.formatignore` layer at the root.
    pub(crate) fn tsv_stack(content: &str) -> IgnoreStack {
        let mut stack = IgnoreStack::new();
        stack.push_formatignore("", content);
        stack
    }

    /// Assemble a stack the way a per-file consumer (the VS Code extension) does:
    /// every `.gitignore` layer shallow→deep, then every tsv layer shallow→deep.
    pub(crate) fn stack_from(gitignores: &[(&str, &str)], tsvs: &[(&str, &str)]) -> IgnoreStack {
        let mut stack = IgnoreStack::new();
        for (anchor, content) in gitignores {
            stack.push_gitignore(anchor, content);
        }
        for (anchor, content) in tsvs {
            stack.push_formatignore(anchor, content);
        }
        stack
    }
}

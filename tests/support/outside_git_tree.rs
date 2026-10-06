//! The precondition every loose-file-regime test stands on: the temp tree it builds sits
//! outside any git tree. The format root is found by walking up to the nearest `.git`
//! entry with no ceiling, so one above the system temp dir — a sandbox's leftover mount
//! target, a `git init` run in the wrong directory — turns every tree built there into a
//! repo subdirectory, and the regime's tests then fail on its symptoms (a `.prettierignore`
//! honored, a warning missing) with nothing naming the cause. Included by `#[path]` from
//! each harness that builds such a tree (`cli_tests`, `discovery_parity.rs`), so the check
//! has one definition; `scripts/discovery_parity_suite.ts` states it for the JS CLIs.

use std::fs;
use std::path::Path;

/// Fail, naming the enclosing git tree, when an ancestor of `dir` (inclusive) holds a
/// `.git` entry. Reads `dir` resolved, as the walk does — a symlinked temp dir has two
/// ancestor chains, and the regime follows the real one.
pub fn assert_outside_git_tree(dir: &Path) {
    let resolved = fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    let enclosing = resolved
        .ancestors()
        .find(|ancestor| ancestor.join(".git").exists());
    assert!(
        enclosing.is_none(),
        "{} is inside a git tree: {} exists, so tsv reads this tree as part of a repo and \
         the outside-a-repo regime under test is unreachable. Remove it (git itself may not \
         count it a repo — an empty `.git` directory is enough), or point TMPDIR at a \
         directory no git tree encloses.",
        dir.display(),
        enclosing.unwrap_or(&resolved).join(".git").display()
    );
}
